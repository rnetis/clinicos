use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use sqlx::{Column, Row, SqlitePool};

use crate::auth::{hash_password, AuthUser, ClientIp};
use crate::db::{self, audit};
use crate::error::{ApiResult, AppError};
use crate::rbac;
use crate::state::AppState;
use crate::util;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/meta", get(meta))
        .route("/api/settings", get(settings_get).patch(settings_patch))
        .route("/api/users", get(list_users).post(create_user))
        .route("/api/users/{id}", patch(update_user))
        .route("/api/users/{id}/password", post(reset_password))
        .route("/api/doctors", get(list_doctors).post(create_doctor))
        .route("/api/doctors/{id}", patch(update_doctor))
        .route("/api/services", get(list_services).post(create_service))
        .route("/api/services/{id}", patch(update_service))
        .route("/api/insurers", get(list_insurers).post(create_insurer))
        .route("/api/insurers/{id}", patch(update_insurer))
        .route("/api/departments", get(list_departments).post(create_department))
        .route("/api/admin/backup", get(backup))
}

const SETTING_DEFAULTS: &[(&str, &str)] = &[
    ("clinic_name_fa", "درمانگاه تخصصی ایران زمین"),
    ("clinic_name_en", "Iran Zamin Specialty Clinic"),
    ("clinic_phone", "021-88776655"),
    ("clinic_address_fa", "تهران، خیابان ولیعصر، پلاک ۱۲۳۴"),
    ("clinic_address_en", "No. 1234, Valiasr St, Tehran"),
    ("calendar", "jalali"),
    ("slot_minutes", "30"),
    ("work_start", "08:00"),
    ("work_end", "20:00"),
    ("discount_limit_pct", "10"),
];

async fn bootstrap_settings(pool: &SqlitePool) {
    for (k, v) in SETTING_DEFAULTS {
        let _ = db::set_setting(pool, k, v).await;
    }
}

async fn settings_map(pool: &SqlitePool) -> ApiResult<Map<String, Value>> {
    let rows = sqlx::query("SELECT key, value FROM settings ORDER BY key")
        .fetch_all(pool)
        .await?;
    let mut map = Map::new();
    for r in rows {
        let key: String = r.get("key");
        let value: String = r.get("value");
        map.insert(key, json!(value));
    }
    Ok(map)
}

pub async fn meta(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    bootstrap_settings(&state.pool).await;
    let settings = settings_map(&state.pool).await?;

    let departments = sqlx::query("SELECT id, name_fa, name_en, active FROM departments WHERE active = 1 ORDER BY name_fa")
        .fetch_all(&state.pool)
        .await?;
    let insurers = sqlx::query("SELECT id, name_fa, name_en, cover, active FROM insurers WHERE active = 1 ORDER BY name_fa")
        .fetch_all(&state.pool)
        .await?;
    let doctors = sqlx::query(
        "SELECT d.*, dept.name_fa AS dept_fa, dept.name_en AS dept_en
         FROM doctors d LEFT JOIN departments dept ON dept.id = d.dept_id
         ORDER BY d.active DESC, d.name_fa",
    )
    .fetch_all(&state.pool)
    .await?;
    let services = sqlx::query("SELECT * FROM services ORDER BY category, name_fa")
        .fetch_all(&state.pool)
        .await?;
    let lab_tests = sqlx::query("SELECT * FROM lab_tests ORDER BY cat, code")
        .fetch_all(&state.pool)
        .await?;
    let meds = sqlx::query("SELECT * FROM meds ORDER BY name_fa")
        .fetch_all(&state.pool)
        .await?;
    let icd = sqlx::query("SELECT code, title_fa, title_en FROM icd10 ORDER BY code")
        .fetch_all(&state.pool)
        .await?;

    let users = if rbac::can(&user.role, rbac::ADMIN_USERS) {
        let rows = sqlx::query(
            "SELECT id, username, role, name_fa, name_en, doctor_id, active, created_at, last_login_at
             FROM users ORDER BY active DESC, name_fa",
        )
        .fetch_all(&state.pool)
        .await?;
        json!(rows.iter().map(|r| json!({
            "id": r.get::<String, _>("id"),
            "username": r.get::<String, _>("username"),
            "role": r.get::<String, _>("role"),
            "nameFa": r.get::<String, _>("name_fa"),
            "nameEn": r.get::<String, _>("name_en"),
            "doctorId": r.try_get::<Option<String>, _>("doctor_id").ok().flatten(),
            "active": r.get::<i64, _>("active") == 1,
            "createdAt": r.get::<String, _>("created_at"),
            "lastLoginAt": r.try_get::<Option<String>, _>("last_login_at").ok().flatten(),
        })).collect::<Vec<_>>())
    } else {
        json!([])
    };

    Ok(Json(json!({
        "settings": settings,
        "clinic": {
            "nameFa": settings.get("clinic_name_fa").cloned().unwrap_or(json!("")),
            "nameEn": settings.get("clinic_name_en").cloned().unwrap_or(json!("")),
            "phone": settings.get("clinic_phone").cloned().unwrap_or(json!("")),
            "addressFa": settings.get("clinic_address_fa").cloned().unwrap_or(json!("")),
            "addressEn": settings.get("clinic_address_en").cloned().unwrap_or(json!("")),
        },
        "departments": departments.iter().map(|r| json!({
            "id": r.get::<String, _>("id"),
            "nameFa": r.get::<String, _>("name_fa"),
            "nameEn": r.get::<String, _>("name_en"),
        })).collect::<Vec<_>>(),
        "insurers": insurers.iter().map(|r| json!({
            "id": r.get::<String, _>("id"),
            "nameFa": r.get::<String, _>("name_fa"),
            "nameEn": r.get::<String, _>("name_en"),
            "cover": r.get::<i64, _>("cover"),
        })).collect::<Vec<_>>(),
        "doctors": doctors.iter().map(|r| json!({
            "id": r.get::<String, _>("id"),
            "nameFa": r.get::<String, _>("name_fa"),
            "nameEn": r.get::<String, _>("name_en"),
            "specialtyFa": r.get::<String, _>("specialty_fa"),
            "specialtyEn": r.get::<String, _>("specialty_en"),
            "license": r.get::<String, _>("license"),
            "deptId": r.try_get::<Option<String>, _>("dept_id").ok().flatten(),
            "deptNameFa": r.try_get::<Option<String>, _>("dept_fa").ok().flatten(),
            "deptNameEn": r.try_get::<Option<String>, _>("dept_en").ok().flatten(),
            "workStart": r.get::<String, _>("work_start"),
            "workEnd": r.get::<String, _>("work_end"),
            "slotMin": r.get::<i64, _>("slot_min"),
            "fee": r.get::<i64, _>("fee"),
            "daysOff": serde_json::from_str::<Value>(&r.get::<String, _>("days_off")).unwrap_or(json!([])),
            "color": r.get::<String, _>("color"),
            "active": r.get::<i64, _>("active") == 1,
            "userId": r.try_get::<Option<String>, _>("user_id").ok().flatten(),
        })).collect::<Vec<_>>(),
        "services": services.iter().map(|r| json!({
            "id": r.get::<String, _>("id"),
            "code": r.get::<String, _>("code"),
            "nameFa": r.get::<String, _>("name_fa"),
            "nameEn": r.get::<String, _>("name_en"),
            "category": r.get::<String, _>("category"),
            "price": r.get::<i64, _>("price"),
            "duration": r.get::<i64, _>("duration"),
            "deptId": r.try_get::<Option<String>, _>("dept_id").ok().flatten(),
            "active": r.get::<i64, _>("active") == 1,
        })).collect::<Vec<_>>(),
        "labTests": lab_tests.iter().map(|r| json!({
            "id": r.get::<String, _>("id"),
            "code": r.get::<String, _>("code"),
            "nameFa": r.get::<String, _>("name_fa"),
            "nameEn": r.get::<String, _>("name_en"),
            "unit": r.get::<String, _>("unit"),
            "refM": r.get::<String, _>("ref_m"),
            "refF": r.get::<String, _>("ref_f"),
            "price": r.get::<i64, _>("price"),
            "cat": r.get::<String, _>("cat"),
        })).collect::<Vec<_>>(),
        "meds": meds.iter().map(|r| json!({
            "id": r.get::<String, _>("id"),
            "nameFa": r.get::<String, _>("name_fa"),
            "nameEn": r.get::<String, _>("name_en"),
            "genericFa": r.get::<String, _>("generic_fa"),
            "genericEn": r.get::<String, _>("generic_en"),
            "forms": serde_json::from_str::<Value>(&r.get::<String, _>("forms")).unwrap_or(json!(["tablet"])),
            "route": r.get::<String, _>("route"),
            "freq": r.get::<String, _>("freq"),
            "allergenTag": r.try_get::<Option<String>, _>("allergen_tag").ok().flatten(),
        })).collect::<Vec<_>>(),
        "icd10": icd.iter().map(|r| json!({
            "code": r.get::<String, _>("code"),
            "titleFa": r.get::<String, _>("title_fa"),
            "titleEn": r.get::<String, _>("title_en"),
        })).collect::<Vec<_>>(),
        "users": users,
        "serverDate": util::today(&state.cfg.tz),
        "timezone": state.cfg.tz.name(),
        "demo": state.cfg.demo_seed,
        "permissions": rbac::perms(&user.role),
    })))
}

pub async fn settings_get(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_SEARCH)?;
    bootstrap_settings(&state.pool).await;
    Ok(Json(json!(settings_map(&state.pool).await?)))
}

#[derive(Debug, Deserialize)]
pub struct SettingsPatch {
    #[serde(flatten)]
    pub values: Map<String, Value>,
}

pub async fn settings_patch(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Json(body): Json<SettingsPatch>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::ADMIN_CONFIG)?;
    let allowed = [
        "clinic_name_fa",
        "clinic_name_en",
        "clinic_phone",
        "clinic_address_fa",
        "clinic_address_en",
        "calendar",
        "slot_minutes",
        "work_start",
        "work_end",
        "discount_limit_pct",
    ];
    for (key, value) in &body.values {
        if !allowed.contains(&key.as_str()) {
            return Err(AppError::bad(format!("unknown setting {key}")));
        }
        let raw = match value {
            Value::String(s) => s.trim().to_string(),
            Value::Number(n) => n.to_string(),
            _ => return Err(AppError::bad(format!("invalid value for {key}"))),
        };
        match key.as_str() {
            "calendar" => {
                util::one_of(&raw, &["jalali", "gregorian"], "calendar")?;
            }
            "slot_minutes" => {
                let n: i64 = raw.parse().map_err(|_| AppError::bad("slot_minutes must be a number"))?;
                if !(5..=120).contains(&n) {
                    return Err(AppError::bad("slot_minutes must be between 5 and 120"));
                }
            }
            "work_start" | "work_end" => util::validate_time(&raw)?,
            "discount_limit_pct" => {
                let n: i64 = raw.parse().map_err(|_| AppError::bad("discount_limit_pct must be a number"))?;
                if !(0..=100).contains(&n) {
                    return Err(AppError::bad("discount_limit_pct must be between 0 and 100"));
                }
            }
            _ => {
                util::clean(&raw, 400)?;
            }
        }
        db::set_setting(&state.pool, key, &raw).await?;
    }
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "settings_update", "settings", "", &body.values.keys().cloned().collect::<Vec<_>>().join(","), "success", &ip.0).await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn list_users(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::ADMIN_USERS)?;
    let rows = sqlx::query(
        "SELECT u.id, u.username, u.role, u.name_fa, u.name_en, u.doctor_id, u.active,
                u.created_at, u.last_login_at, d.name_fa AS doctor_fa, d.name_en AS doctor_en
         FROM users u LEFT JOIN doctors d ON d.id = u.doctor_id ORDER BY u.active DESC, u.name_fa",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!(rows.iter().map(|r| json!({
        "id": r.get::<String, _>("id"),
        "username": r.get::<String, _>("username"),
        "role": r.get::<String, _>("role"),
        "nameFa": r.get::<String, _>("name_fa"),
        "nameEn": r.get::<String, _>("name_en"),
        "doctorId": r.try_get::<Option<String>, _>("doctor_id").ok().flatten(),
        "doctorNameFa": r.try_get::<Option<String>, _>("doctor_fa").ok().flatten(),
        "doctorNameEn": r.try_get::<Option<String>, _>("doctor_en").ok().flatten(),
        "active": r.get::<i64, _>("active") == 1,
        "createdAt": r.get::<String, _>("created_at"),
        "lastLoginAt": r.try_get::<Option<String>, _>("last_login_at").ok().flatten(),
    })).collect::<Vec<_>>())))
}

fn validate_password(password: &str) -> ApiResult<()> {
    if password.chars().count() < 10 {
        return Err(AppError::bad("password must be at least 10 characters"));
    }
    let has_letter = password.chars().any(|c| c.is_alphabetic());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    if !has_letter || !has_digit {
        return Err(AppError::bad("password must contain letters and digits"));
    }
    if password.chars().count() > 200 {
        return Err(AppError::bad("password is too long"));
    }
    Ok(())
}

fn validate_username(username: &str) -> ApiResult<String> {
    let u = username.trim().to_lowercase();
    let ok = (3..=32).contains(&u.len())
        && u.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        && u.chars().next().map(|c| c.is_ascii_alphanumeric()).unwrap_or(false);
    if ok {
        Ok(u)
    } else {
        Err(AppError::bad(
            "username must be 3–32 characters: letters, digits, dot, underscore or dash",
        ))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInput {
    pub username: String,
    pub password: String,
    pub role: String,
    pub name_fa: String,
    #[serde(default)]
    pub name_en: String,
    #[serde(default)]
    pub doctor_id: Option<String>,
}

pub async fn create_user(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Json(body): Json<UserInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    rbac::require(&user, rbac::ADMIN_USERS)?;
    let username = validate_username(&body.username)?;
    validate_password(&body.password)?;
    let role = util::one_of(&body.role, &rbac::ROLES, "role")?;
    let name_fa = util::require_nonempty(&body.name_fa, "name")?;
    if let Some(doc) = body.doctor_id.as_deref().filter(|s| !s.is_empty()) {
        let exists: Option<String> = sqlx::query_scalar("SELECT id FROM doctors WHERE id = ?")
            .bind(doc)
            .fetch_optional(&state.pool)
            .await?;
        if exists.is_none() {
            return Err(AppError::bad("unknown doctor"));
        }
    }
    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE username = ?")
        .bind(&username)
        .fetch_optional(&state.pool)
        .await?;
    if exists.is_some() {
        return Err(AppError::conflict("username is already taken"));
    }

    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO users (id, username, password_hash, role, name_fa, name_en, doctor_id, active, created_at)
         VALUES (?,?,?,?,?,?,?,1,?)",
    )
    .bind(&id)
    .bind(&username)
    .bind(hash_password(&body.password)?)
    .bind(&role)
    .bind(&name_fa)
    .bind(util::clean(&body.name_en, 120)?)
    .bind(body.doctor_id.clone().filter(|s| !s.is_empty()))
    .bind(util::now())
    .execute(&state.pool)
    .await?;
    if let Some(doc) = body.doctor_id.as_deref().filter(|s| !s.is_empty()) {
        let _ = sqlx::query("UPDATE doctors SET user_id = ? WHERE id = ?")
            .bind(&id)
            .bind(doc)
            .execute(&state.pool)
            .await;
    }
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "user_create", "user", &id, &format!("{username} {role}"), "success", &ip.0).await;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPatch {
    #[serde(default)]
    pub name_fa: Option<String>,
    #[serde(default)]
    pub name_en: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub active: Option<bool>,
    #[serde(default)]
    pub doctor_id: Option<String>,
}

async fn active_admin_count(pool: &SqlitePool) -> ApiResult<i64> {
    Ok(sqlx::query_scalar(
        "SELECT COUNT(*) FROM users WHERE role = 'admin' AND active = 1",
    )
    .fetch_one(pool)
    .await?)
}

pub async fn update_user(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<UserPatch>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::ADMIN_USERS)?;
    let row = sqlx::query("SELECT role, active FROM users WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| AppError::not_found("user"))?;
    let old_role: String = row.get("role");
    let old_active = row.get::<i64, _>("active") == 1;

    if id == user.id && body.active == Some(false) {
        return Err(AppError::conflict("you cannot deactivate your own account"));
    }
    let losing_admin = old_role == "admin"
        && old_active
        && (body.active == Some(false) || body.role.as_deref().map(|r| r != "admin").unwrap_or(false));
    if losing_admin && active_admin_count(&state.pool).await? <= 1 {
        return Err(AppError::conflict("at least one active administrator is required"));
    }
    if let Some(role) = body.role.as_deref() {
        util::one_of(role, &rbac::ROLES, "role")?;
    }

    sqlx::query(
        "UPDATE users SET name_fa = COALESCE(?, name_fa), name_en = COALESCE(?, name_en),
            role = COALESCE(?, role), active = COALESCE(?, active), doctor_id = ?
         WHERE id = ?",
    )
    .bind(body.name_fa.as_deref().map(|s| s.trim().to_string()))
    .bind(body.name_en.as_deref().map(|s| s.trim().to_string()))
    .bind(body.role.clone())
    .bind(body.active.map(|a| a as i32))
    .bind(
        body.doctor_id
            .as_ref()
            .map(|d| if d.is_empty() { None } else { Some(d.clone()) }),
    )
    .bind(&id)
    .execute(&state.pool)
    .await?;

    if body.active == Some(false) {
        let _ = sqlx::query("DELETE FROM sessions WHERE user_id = ?")
            .bind(&id)
            .execute(&state.pool)
            .await;
    }
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "user_update", "user", &id, "", "success", &ip.0).await;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
pub struct PasswordReset {
    pub password: String,
}

pub async fn reset_password(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<PasswordReset>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::ADMIN_USERS)?;
    validate_password(&body.password)?;
    let exists: Option<String> = sqlx::query_scalar("SELECT id FROM users WHERE id = ?")
        .bind(&id)
        .fetch_optional(&state.pool)
        .await?;
    if exists.is_none() {
        return Err(AppError::not_found("user"));
    }
    sqlx::query("UPDATE users SET password_hash = ?, must_change_password = 1, failed_attempts = 0, locked_until = NULL WHERE id = ?")
        .bind(hash_password(&body.password)?)
        .bind(&id)
        .execute(&state.pool)
        .await?;
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(&id)
        .execute(&state.pool)
        .await?;
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "password_reset", "user", &id, "", "success", &ip.0).await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn list_doctors(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_SEARCH)?;
    let rows = sqlx::query(
        "SELECT d.*, dept.name_fa AS dept_fa, dept.name_en AS dept_en
         FROM doctors d LEFT JOIN departments dept ON dept.id = d.dept_id
         ORDER BY d.active DESC, d.name_fa",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!(rows.iter().map(|r| json!({
        "id": r.get::<String, _>("id"),
        "nameFa": r.get::<String, _>("name_fa"),
        "nameEn": r.get::<String, _>("name_en"),
        "specialtyFa": r.get::<String, _>("specialty_fa"),
        "specialtyEn": r.get::<String, _>("specialty_en"),
        "license": r.get::<String, _>("license"),
        "deptId": r.try_get::<Option<String>, _>("dept_id").ok().flatten(),
        "deptNameFa": r.try_get::<Option<String>, _>("dept_fa").ok().flatten(),
        "workStart": r.get::<String, _>("work_start"),
        "workEnd": r.get::<String, _>("work_end"),
        "slotMin": r.get::<i64, _>("slot_min"),
        "fee": r.get::<i64, _>("fee"),
        "daysOff": serde_json::from_str::<Value>(&r.get::<String, _>("days_off")).unwrap_or(json!([])),
        "color": r.get::<String, _>("color"),
        "active": r.get::<i64, _>("active") == 1,
        "userId": r.try_get::<Option<String>, _>("user_id").ok().flatten(),
    })).collect::<Vec<_>>())))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorInput {
    pub name_fa: String,
    #[serde(default)]
    pub name_en: String,
    #[serde(default)]
    pub specialty_fa: String,
    #[serde(default)]
    pub specialty_en: String,
    #[serde(default)]
    pub license: String,
    #[serde(default)]
    pub dept_id: Option<String>,
    #[serde(default)]
    pub work_start: Option<String>,
    #[serde(default)]
    pub work_end: Option<String>,
    #[serde(default)]
    pub slot_min: Option<i64>,
    #[serde(default)]
    pub fee: Option<i64>,
    #[serde(default)]
    pub days_off: Option<Vec<i64>>,
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub active: Option<bool>,
}

fn doctor_fields(body: &DoctorInput) -> ApiResult<(String, String, i64, i64, String, String)> {
    let start = body.work_start.clone().unwrap_or_else(|| "09:00".into());
    let end = body.work_end.clone().unwrap_or_else(|| "17:00".into());
    util::validate_time(&start)?;
    util::validate_time(&end)?;
    if util::time_minutes(&start)? >= util::time_minutes(&end)? {
        return Err(AppError::bad("work end must be after work start"));
    }
    let slot = body.slot_min.unwrap_or(30);
    if !(5..=120).contains(&slot) {
        return Err(AppError::bad("slot must be between 5 and 120 minutes"));
    }
    let fee = util::money(body.fee.unwrap_or(0))?;
    let days = body.days_off.clone().unwrap_or_default();
    if days.iter().any(|d| !(0..=6).contains(d)) {
        return Err(AppError::bad("days off must be weekday numbers 0–6"));
    }
    Ok((start, end, slot, fee, serde_json::to_string(&days).unwrap_or_else(|_| "[]".into()), body.color.clone().unwrap_or_else(|| "#0e7490".into())))
}

pub async fn create_doctor(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Json(body): Json<DoctorInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    rbac::require(&user, rbac::ADMIN_CONFIG)?;
    let name = util::require_nonempty(&body.name_fa, "name")?;
    let (start, end, slot, fee, days, color) = doctor_fields(&body)?;
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO doctors (id, name_fa, name_en, specialty_fa, specialty_en, license, dept_id,
            work_start, work_end, slot_min, fee, days_off, color, active)
         VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,1)",
    )
    .bind(&id)
    .bind(&name)
    .bind(util::clean(&body.name_en, 120)?)
    .bind(util::clean(&body.specialty_fa, 120)?)
    .bind(util::clean(&body.specialty_en, 120)?)
    .bind(util::clean(&body.license, 40)?)
    .bind(body.dept_id.clone().filter(|s| !s.is_empty()))
    .bind(&start)
    .bind(&end)
    .bind(slot)
    .bind(fee)
    .bind(&days)
    .bind(&color)
    .execute(&state.pool)
    .await?;
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "doctor_create", "doctor", &id, "", "success", &ip.0).await;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

pub async fn update_doctor(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<DoctorInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::ADMIN_CONFIG)?;
    let (start, end, slot, fee, days, color) = doctor_fields(&body)?;
    let res = sqlx::query(
        "UPDATE doctors SET name_fa = ?, name_en = ?, specialty_fa = ?, specialty_en = ?,
            license = ?, dept_id = ?, work_start = ?, work_end = ?, slot_min = ?, fee = ?,
            days_off = ?, color = ?, active = COALESCE(?, active)
         WHERE id = ?",
    )
    .bind(util::require_nonempty(&body.name_fa, "name")?)
    .bind(util::clean(&body.name_en, 120)?)
    .bind(util::clean(&body.specialty_fa, 120)?)
    .bind(util::clean(&body.specialty_en, 120)?)
    .bind(util::clean(&body.license, 40)?)
    .bind(body.dept_id.clone().filter(|s| !s.is_empty()))
    .bind(&start)
    .bind(&end)
    .bind(slot)
    .bind(fee)
    .bind(&days)
    .bind(&color)
    .bind(body.active.map(|a| a as i32))
    .bind(&id)
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::not_found("doctor"));
    }
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "doctor_update", "doctor", &id, "", "success", &ip.0).await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn list_services(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_SEARCH)?;
    let rows = sqlx::query("SELECT * FROM services ORDER BY category, name_fa")
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(json!(rows.iter().map(|r| json!({
        "id": r.get::<String, _>("id"),
        "code": r.get::<String, _>("code"),
        "nameFa": r.get::<String, _>("name_fa"),
        "nameEn": r.get::<String, _>("name_en"),
        "category": r.get::<String, _>("category"),
        "price": r.get::<i64, _>("price"),
        "duration": r.get::<i64, _>("duration"),
        "deptId": r.try_get::<Option<String>, _>("dept_id").ok().flatten(),
        "active": r.get::<i64, _>("active") == 1,
    })).collect::<Vec<_>>())))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceInput {
    pub code: String,
    pub name_fa: String,
    #[serde(default)]
    pub name_en: String,
    #[serde(default)]
    pub category: String,
    pub price: i64,
    #[serde(default)]
    pub duration: Option<i64>,
    #[serde(default)]
    pub dept_id: Option<String>,
    #[serde(default)]
    pub active: Option<bool>,
}

pub async fn create_service(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Json(body): Json<ServiceInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    rbac::require(&user, rbac::ADMIN_CONFIG)?;
    let id = uuid::Uuid::new_v4().to_string();
    let category = util::one_of(
        if body.category.is_empty() { "consult" } else { &body.category },
        &["consult", "procedure", "diagnostic", "lab", "other"],
        "category",
    )?;
    sqlx::query(
        "INSERT INTO services (id, code, name_fa, name_en, category, price, duration, dept_id, active)
         VALUES (?,?,?,?,?,?,?,?,1)",
    )
    .bind(&id)
    .bind(util::require_nonempty(&body.code, "code")?)
    .bind(util::require_nonempty(&body.name_fa, "name")?)
    .bind(util::clean(&body.name_en, 160)?)
    .bind(category)
    .bind(util::money(body.price)?)
    .bind(body.duration.unwrap_or(30).clamp(1, 480))
    .bind(body.dept_id.clone().filter(|s| !s.is_empty()))
    .execute(&state.pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db) if db.is_unique_violation() => AppError::conflict("service code already exists"),
        other => AppError::Internal(other),
    })?;
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "service_create", "service", &id, &body.code, "success", &ip.0).await;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

pub async fn update_service(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<ServiceInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::ADMIN_CONFIG)?;
    let category = util::one_of(
        if body.category.is_empty() { "consult" } else { &body.category },
        &["consult", "procedure", "diagnostic", "lab", "other"],
        "category",
    )?;
    let res = sqlx::query(
        "UPDATE services SET code = ?, name_fa = ?, name_en = ?, category = ?, price = ?,
            duration = ?, dept_id = ?, active = COALESCE(?, active) WHERE id = ?",
    )
    .bind(util::require_nonempty(&body.code, "code")?)
    .bind(util::require_nonempty(&body.name_fa, "name")?)
    .bind(util::clean(&body.name_en, 160)?)
    .bind(category)
    .bind(util::money(body.price)?)
    .bind(body.duration.unwrap_or(30).clamp(1, 480))
    .bind(body.dept_id.clone().filter(|s| !s.is_empty()))
    .bind(body.active.map(|a| a as i32))
    .bind(&id)
    .execute(&state.pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db) if db.is_unique_violation() => AppError::conflict("service code already exists"),
        other => AppError::Internal(other),
    })?;
    if res.rows_affected() == 0 {
        return Err(AppError::not_found("service"));
    }
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "service_update", "service", &id, "", "success", &ip.0).await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn list_insurers(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_SEARCH)?;
    let rows = sqlx::query("SELECT * FROM insurers ORDER BY name_fa")
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(json!(rows.iter().map(|r| json!({
        "id": r.get::<String, _>("id"),
        "nameFa": r.get::<String, _>("name_fa"),
        "nameEn": r.get::<String, _>("name_en"),
        "cover": r.get::<i64, _>("cover"),
        "active": r.get::<i64, _>("active") == 1,
    })).collect::<Vec<_>>())))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsurerInput {
    pub name_fa: String,
    #[serde(default)]
    pub name_en: String,
    pub cover: i64,
    #[serde(default)]
    pub active: Option<bool>,
}

pub async fn create_insurer(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Json(body): Json<InsurerInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    rbac::require(&user, rbac::ADMIN_CONFIG)?;
    if !(0..=100).contains(&body.cover) {
        return Err(AppError::bad("cover must be between 0 and 100"));
    }
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO insurers (id, name_fa, name_en, cover, active) VALUES (?,?,?,?,1)")
        .bind(&id)
        .bind(util::require_nonempty(&body.name_fa, "name")?)
        .bind(util::clean(&body.name_en, 120)?)
        .bind(body.cover)
        .execute(&state.pool)
        .await?;
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "insurer_create", "insurer", &id, "", "success", &ip.0).await;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

pub async fn update_insurer(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Path(id): Path<String>,
    Json(body): Json<InsurerInput>,
) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::ADMIN_CONFIG)?;
    if !(0..=100).contains(&body.cover) {
        return Err(AppError::bad("cover must be between 0 and 100"));
    }
    let res = sqlx::query(
        "UPDATE insurers SET name_fa = ?, name_en = ?, cover = ?, active = COALESCE(?, active) WHERE id = ?",
    )
    .bind(util::require_nonempty(&body.name_fa, "name")?)
    .bind(util::clean(&body.name_en, 120)?)
    .bind(body.cover)
    .bind(body.active.map(|a| a as i32))
    .bind(&id)
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::not_found("insurer"));
    }
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "insurer_update", "insurer", &id, "", "success", &ip.0).await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn list_departments(State(state): State<AppState>, user: AuthUser) -> ApiResult<Json<Value>> {
    rbac::require(&user, rbac::PATIENT_SEARCH)?;
    let rows = sqlx::query("SELECT * FROM departments ORDER BY name_fa")
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(json!(rows.iter().map(|r| json!({
        "id": r.get::<String, _>("id"),
        "nameFa": r.get::<String, _>("name_fa"),
        "nameEn": r.get::<String, _>("name_en"),
        "active": r.get::<i64, _>("active") == 1,
    })).collect::<Vec<_>>())))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepartmentInput {
    pub name_fa: String,
    #[serde(default)]
    pub name_en: String,
}

pub async fn create_department(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Json(body): Json<DepartmentInput>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    rbac::require(&user, rbac::ADMIN_CONFIG)?;
    let id = uuid::Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO departments (id, name_fa, name_en, active) VALUES (?,?,?,1)")
        .bind(&id)
        .bind(util::require_nonempty(&body.name_fa, "name")?)
        .bind(util::clean(&body.name_en, 120)?)
        .execute(&state.pool)
        .await?;
    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "department_create", "department", &id, "", "success", &ip.0).await;
    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

#[derive(Debug, Deserialize)]
pub struct BackupQuery {
    pub format: Option<String>,
}

pub async fn backup(
    State(state): State<AppState>,
    ip: ClientIp,
    user: AuthUser,
    Query(_q): Query<BackupQuery>,
) -> ApiResult<Response> {
    rbac::require(&user, rbac::ADMIN_CONFIG)?;
    let tables = [
        "settings", "departments", "insurers", "doctors", "patients", "patient_alerts",
        "allergies", "conditions", "patient_meds", "services", "lab_tests", "meds", "icd10",
        "appointments", "queue", "encounters", "encounter_versions", "lab_orders",
        "lab_order_tests", "lab_results", "invoices", "invoice_items", "payments",
        "follow_ups", "vitals", "nurse_notes", "procedures", "notifications",
    ];
    let mut dump = Map::new();
    for table in tables {
        let rows = sqlx::query(&format!("SELECT * FROM {table}"))
            .fetch_all(&state.pool)
            .await?;
        let mut list = Vec::new();
        for row in rows {
            let mut obj = Map::new();
            for (i, col) in row.columns().iter().enumerate() {
                let name = col.name();
                let value: Value = row
                    .try_get::<Option<i64>, _>(i)
                    .map(|v| v.map(|n| json!(n)).unwrap_or(Value::Null))
                    .or_else(|_| row.try_get::<Option<f64>, _>(i).map(|v| v.map(|n| json!(n)).unwrap_or(Value::Null)))
                    .or_else(|_| row.try_get::<Option<String>, _>(i).map(|v| v.map(|s| json!(s)).unwrap_or(Value::Null)))
                    .unwrap_or(Value::Null);
                obj.insert(name.to_string(), value);
            }
            list.push(Value::Object(obj));
        }
        dump.insert(table.to_string(), json!(list));
    }
    // Accounts are exported without password hashes: a backup must never be a credential dump.
    let users = sqlx::query(
        "SELECT id, username, role, name_fa, name_en, doctor_id, active, created_at, last_login_at FROM users",
    )
    .fetch_all(&state.pool)
    .await?;
    dump.insert(
        "users".into(),
        json!(users.iter().map(|r| json!({
            "id": r.get::<String, _>("id"),
            "username": r.get::<String, _>("username"),
            "role": r.get::<String, _>("role"),
            "name_fa": r.get::<String, _>("name_fa"),
            "name_en": r.get::<String, _>("name_en"),
            "doctor_id": r.try_get::<Option<String>, _>("doctor_id").ok().flatten(),
            "active": r.get::<i64, _>("active"),
            "created_at": r.get::<String, _>("created_at"),
            "last_login_at": r.try_get::<Option<String>, _>("last_login_at").ok().flatten(),
        })).collect::<Vec<_>>()),
    );
    dump.insert("exported_at".into(), json!(util::now()));
    dump.insert("format_version".into(), json!(1));

    audit(&state.pool, Some(&user.id), &user.display_name(), &user.role, "backup", "system", "", "", "success", &ip.0).await;

    let body = serde_json::to_vec_pretty(&Value::Object(dump)).map_err(|e| AppError::bad(e.to_string()))?;
    let filename = format!("clinicos-backup-{}.json", util::today(&state.cfg.tz));
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\""))
            .map_err(|_| AppError::bad("invalid filename"))?,
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok((StatusCode::OK, headers, body).into_response())
}
