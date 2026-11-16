use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{ConnectInfo, FromRequestParts, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{Duration as ChronoDuration, Utc};
use rand::RngCore;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};

use crate::db;
use crate::error::{ApiResult, AppError};
use crate::state::AppState;

pub const COOKIE_NAME: &str = "clinicos_session";
pub const CSRF_HEADER: &str = "x-clinicos-csrf";

#[derive(Clone, Debug)]
pub struct AuthUser {
    pub id: String,
    pub username: String,
    pub role: String,
    pub name_fa: String,
    pub name_en: String,
    pub doctor_id: Option<String>,
}

impl AuthUser {
    pub fn display_name(&self) -> String {
        if self.name_en.is_empty() {
            self.name_fa.clone()
        } else {
            format!("{} ({})", self.name_fa, self.name_en)
        }
    }
}

/// Extracts the session cookie, validated against the sessions table on every request
/// so revocation (logout, deactivation, password reset) takes effect immediately.
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = cookie_value(&parts.headers, COOKIE_NAME).ok_or(AppError::Unauthorized)?;
        let hash = token_hash(&token);
        let row = sqlx::query(
            "SELECT u.id, u.username, u.role, u.name_fa, u.name_en, u.doctor_id
             FROM sessions s JOIN users u ON u.id = s.user_id
             WHERE s.token_hash = ? AND s.expires_at > ? AND u.active = 1",
        )
        .bind(hash)
        .bind(crate::util::now())
        .fetch_optional(&state.pool)
        .await?;

        let row = row.ok_or(AppError::Unauthorized)?;
        Ok(AuthUser {
            id: row.try_get("id")?,
            username: row.try_get("username")?,
            role: row.try_get("role")?,
            name_fa: row.try_get("name_fa")?,
            name_en: row.try_get("name_en")?,
            doctor_id: row.try_get("doctor_id")?,
        })
    }
}

pub fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    for part in raw.split(';') {
        let mut kv = part.trim().splitn(2, '=');
        if kv.next() == Some(name) {
            return kv.next().map(|v| v.trim().to_string());
        }
    }
    None
}

pub fn token_hash(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

fn new_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::bad(format!("password hashing failed: {e}")))
}

pub fn verify_password(password: &str, hash: &str) -> bool {
    match PasswordHash::new(hash) {
        Ok(parsed) => Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

pub fn session_cookie(token: &str, ttl_hours: i64, secure: bool) -> String {
    let mut cookie = format!(
        "{COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
        ttl_hours * 3600
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

pub fn cleared_cookie(secure: bool) -> String {
    let mut cookie = format!("{COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0");
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

#[derive(Default)]
pub struct LoginGuard {
    hits: Mutex<HashMap<String, Vec<Instant>>>,
}

impl LoginGuard {
    const WINDOW: Duration = Duration::from_secs(600);
    const MAX: usize = 8;

    pub fn allowed(&self, key: &str) -> bool {
        let mut map = match self.hits.lock() {
            Ok(g) => g,
            Err(poison) => poison.into_inner(),
        };
        let now = Instant::now();
        let entry = map.entry(key.to_string()).or_default();
        entry.retain(|t| now.duration_since(*t) < Self::WINDOW);
        entry.len() < Self::MAX
    }

    pub fn record_failure(&self, key: &str) {
        let mut map = match self.hits.lock() {
            Ok(g) => g,
            Err(poison) => poison.into_inner(),
        };
        map.entry(key.to_string()).or_default().push(Instant::now());
    }

    pub fn clear(&self, key: &str) {
        let mut map = match self.hits.lock() {
            Ok(g) => g,
            Err(poison) => poison.into_inner(),
        };
        map.remove(key);
    }
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

pub async fn login(
    State(state): State<AppState>,
    ip: ClientIp,
    Json(body): Json<LoginRequest>,
) -> ApiResult<Response> {
    let username = body.username.trim().to_lowercase();
    let guard_key = format!("{}|{}", ip.0, username);
    if !state.login_guard.allowed(&guard_key) {
        db::audit(
            &state.pool,
            None,
            &body.username,
            "",
            "login_blocked",
            "session",
            "",
            "rate limited",
            "failure",
            &ip.0,
        )
        .await;
        return Err(AppError::RateLimited);
    }

    let row = sqlx::query(
        "SELECT id, username, password_hash, role, name_fa, name_en, doctor_id, active,
                failed_attempts, locked_until
         FROM users WHERE username = ?",
    )
    .bind(&username)
    .fetch_optional(&state.pool)
    .await?;

    let Some(row) = row else {
        state.login_guard.record_failure(&guard_key);
        db::audit(
            &state.pool,
            None,
            &body.username,
            "",
            "login_fail",
            "session",
            "",
            "unknown user",
            "failure",
            &ip.0,
        )
        .await;
        return Err(AppError::Unauthorized);
    };

    let active: i64 = row.try_get("active")?;
    let locked_until: Option<String> = row.try_get("locked_until")?;
    let locked = locked_until
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.with_timezone(&Utc) > Utc::now())
        .unwrap_or(false);

    let pw_hash: String = row.try_get("password_hash")?;
    if active != 1 || locked || !verify_password(&body.password, &pw_hash) {
        let failed: i64 = row.try_get("failed_attempts").unwrap_or(0) + 1;
        let lock = if failed >= 5 {
            Some((Utc::now() + ChronoDuration::minutes(10)).to_rfc3339())
        } else {
            None
        };
        sqlx::query("UPDATE users SET failed_attempts = ?, locked_until = ? WHERE id = ?")
            .bind(failed)
            .bind(lock)
            .bind(row.try_get::<String, _>("id")?)
            .execute(&state.pool)
            .await?;
        state.login_guard.record_failure(&guard_key);
        db::audit(
            &state.pool,
            None,
            &body.username,
            "",
            "login_fail",
            "session",
            row.try_get::<String, _>("id")?.as_str(),
            "invalid credentials",
            "failure",
            &ip.0,
        )
        .await;
        return Err(AppError::Unauthorized);
    }

    let user = AuthUser {
        id: row.try_get("id")?,
        username: row.try_get("username")?,
        role: row.try_get("role")?,
        name_fa: row.try_get("name_fa")?,
        name_en: row.try_get("name_en")?,
        doctor_id: row.try_get("doctor_id")?,
    };

    let token = new_token();
    let expires = Utc::now() + ChronoDuration::hours(state.cfg.session_ttl_hours);
    sqlx::query(
        "INSERT INTO sessions (token_hash, user_id, created_at, expires_at, ip, user_agent)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(token_hash(&token))
    .bind(&user.id)
    .bind(crate::util::now())
    .bind(expires.to_rfc3339())
    .bind(&ip.0)
    .bind("")
    .execute(&state.pool)
    .await?;

    sqlx::query(
        "UPDATE users SET failed_attempts = 0, locked_until = NULL, last_login_at = ? WHERE id = ?",
    )
    .bind(crate::util::now())
    .bind(&user.id)
    .execute(&state.pool)
    .await?;

    // Housekeeping: drop expired sessions.
    let expired = crate::util::now();
    let _ = sqlx::query("DELETE FROM sessions WHERE expires_at < ?")
        .bind(expired)
        .execute(&state.pool)
        .await;

    state.login_guard.clear(&guard_key);
    db::audit(
        &state.pool,
        Some(&user.id),
        &user.display_name(),
        &user.role,
        "login",
        "session",
        &user.id,
        &format!("role: {}", user.role),
        "success",
        &ip.0,
    )
    .await;

    let mut response = Json(user_payload(&user, &state)).into_response();
    let cookie = session_cookie(&token, state.cfg.session_ttl_hours, state.cfg.cookie_secure);
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(|_| AppError::Internal(sqlx::Error::RowNotFound))?,
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store"),
    );
    Ok(response)
}

/// Unauthenticated branding/demo hints for the sign-in screen only.
pub async fn public_config(State(state): State<AppState>) -> Json<serde_json::Value> {
    let clinic_fa = db::get_setting(&state.pool, "clinic_name_fa")
        .await
        .unwrap_or_default();
    let clinic_en = db::get_setting(&state.pool, "clinic_name_en")
        .await
        .unwrap_or_default();
    // Demo mode deliberately publishes throw-away demo accounts so the seeded clinic can be
    // explored. Real deployments run with CLINICOS_DEMO unset and never expose this.
    let demo_users = if state.cfg.demo_seed {
        let rows = sqlx::query("SELECT username, role, name_fa, name_en FROM users WHERE active = 1 ORDER BY role")
            .fetch_all(&state.pool)
            .await
            .unwrap_or_default();
        rows.iter()
            .map(|r| {
                json!({
                    "username": r.try_get::<String, _>("username").unwrap_or_default(),
                    "role": r.try_get::<String, _>("role").unwrap_or_default(),
                    "nameFa": r.try_get::<String, _>("name_fa").unwrap_or_default(),
                    "nameEn": r.try_get::<String, _>("name_en").unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    Json(json!({
        "demo": state.cfg.demo_seed,
        "demoUsers": demo_users,
        "demoPassword": if state.cfg.demo_seed { Some(state.cfg.demo_password.clone()) } else { None },
        "clinicFa": clinic_fa,
        "clinicEn": clinic_en,
        "defaultCalendar": db::get_setting(&state.pool, "calendar").await.unwrap_or_else(|| "jalali".into()),
    }))
}

pub async fn logout(State(state): State<AppState>, ip: ClientIp, headers: HeaderMap) -> Response {
    if let Some(token) = cookie_value(&headers, COOKIE_NAME) {
        let _ = sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(token_hash(&token))
            .execute(&state.pool)
            .await;
        db::audit(
            &state.pool,
            None,
            "",
            "",
            "logout",
            "session",
            "",
            "",
            "success",
            &ip.0,
        )
        .await;
    }
    let mut response = Json(json!({ "ok": true })).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cleared_cookie(state.cfg.cookie_secure))
            .unwrap_or_else(|_| HeaderValue::from_static("")),
    );
    response
}

pub async fn me(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(user_payload(&user, &state)))
}

pub fn user_payload(user: &AuthUser, state: &AppState) -> serde_json::Value {
    json!({
        "id": user.id,
        "username": user.username,
        "role": user.role,
        "roleLabel": crate::rbac::label(&user.role),
        "nameFa": user.name_fa,
        "nameEn": user.name_en,
        "doctorId": user.doctor_id,
        "permissions": crate::rbac::perms(&user.role),
        "demo": state.cfg.demo_seed,
        "sessionTtlHours": state.cfg.session_ttl_hours,
    })
}

#[derive(Clone, Debug)]
pub struct ClientIp(pub String);

/// Captures the peer address (or the first X-Forwarded-For hop when a trusted proxy is configured).
pub async fn client_ip_mw(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let ip = if state.cfg.trust_proxy {
        req.headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(|v| v.trim().to_string())
            .unwrap_or_else(|| addr.ip().to_string())
    } else {
        addr.ip().to_string()
    };
    req.extensions_mut().insert(ClientIp(ip));
    next.run(req).await
}

impl<S: Send + Sync> axum::extract::FromRequestParts<S> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        Ok(parts
            .extensions
            .get::<ClientIp>()
            .cloned()
            .unwrap_or(ClientIp("unknown".into())))
    }
}

/// CSRF defence for state-changing requests: requires the custom header (which a
/// cross-site form cannot set) and rejects foreign origins outright.
pub async fn csrf_mw(State(_state): State<AppState>, req: Request, next: Next) -> Response {
    let method = req.method();
    if !matches!(
        *method,
        axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
    ) {
        let header_ok = req
            .headers()
            .get(CSRF_HEADER)
            .map(|v| v == "1")
            .unwrap_or(false);
        if !header_ok {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({ "error": "csrf", "message": "missing CSRF header" })),
            )
                .into_response();
        }
        if let Some(origin) = req.headers().get(header::ORIGIN).and_then(|v| v.to_str().ok()) {
            let host = req
                .headers()
                .get(header::HOST)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            let origin_host = origin
                .split("://")
                .nth(1)
                .unwrap_or(origin)
                .split('/')
                .next()
                .unwrap_or("");
            if !host.is_empty() && origin_host != host {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({ "error": "csrf", "message": "cross-origin request rejected" })),
                )
                    .into_response();
            }
        }
    }
    next.run(req).await
}

pub async fn health(State(state): State<AppState>) -> impl IntoResponse {
    let db_ok = sqlx::query("SELECT 1")
        .fetch_one(&state.pool)
        .await
        .is_ok();
    let uptime = (Utc::now() - state.started_at).num_seconds();
    let status = if db_ok { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE };
    (
        status,
        Json(json!({
            "status": if db_ok { "ok" } else { "degraded" },
            "db": db_ok,
            "uptimeSeconds": uptime,
            "version": env!("CARGO_PKG_VERSION"),
        })),
    )
}

/// Convenience for handlers that need the caller's doctor record.
pub async fn doctor_id_for(pool: &SqlitePool, user: &AuthUser) -> ApiResult<Option<String>> {
    if user.role != "doctor" {
        return Ok(user.doctor_id.clone());
    }
    if let Some(id) = &user.doctor_id {
        return Ok(Some(id.clone()));
    }
    let row = sqlx::query("SELECT id FROM doctors WHERE user_id = ?")
        .bind(&user.id)
        .fetch_optional(pool)
        .await?;
    Ok(row.and_then(|r| r.try_get::<String, _>("id").ok()))
}
