#![recursion_limit = "512"]

mod api;
mod auth;
mod config;
mod db;
mod error;
mod rbac;
mod seed;
mod state;
mod util;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::http::{header, HeaderName, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{any, get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde_json::json;
use tower_http::compression::CompressionLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

use crate::auth::LoginGuard;
use crate::config::Config;
use crate::state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = dotenvy::dotenv();
    let cfg = match Config::from_env() {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("configuration error: {err}");
            std::process::exit(2);
        }
    };
    init_tracing(cfg.log_json);

    tracing::info!(bind = %cfg.bind, db = %cfg.database_url, "starting ClinicOS");

    let pool = db::connect(&cfg.database_url).await?;
    let cfg = Arc::new(cfg);
    seed::bootstrap(&pool, &cfg).await?;

    let state = AppState {
        pool,
        cfg: cfg.clone(),
        login_guard: Arc::new(LoginGuard::default()),
        started_at: Utc::now(),
    };

    let api = Router::new()
        .route("/api/health", get(auth::health))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/auth/me", get(auth::me))
        .route("/api/auth/config", get(auth::public_config))
        .merge(api::patients::routes())
        .merge(api::patients::routes_audit())
        .merge(api::appointments::routes())
        .merge(api::queue::routes())
        .merge(api::encounters::routes())
        .merge(api::labs::routes())
        .merge(api::billing::routes())
        .merge(api::dashboard::routes())
        .merge(api::reports::routes())
        .merge(api::notifications::routes())
        .merge(api::admin::routes())
        .route("/api/{*rest}", any(api_not_found))
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth::csrf_mw))
        .layer(DefaultBodyLimit::max(cfg.max_body_bytes));

    let index = cfg.static_dir.join("index.html");
    if !cfg.static_dir.exists() {
        tracing::warn!(dir = %cfg.static_dir.display(), "static directory is missing; API only");
    }
    let static_files = ServeDir::new(&cfg.static_dir)
        .append_index_html_on_directories(true)
        .not_found_service(ServeFile::new(index));

    let app = api
        .fallback_service(static_files)
        .layer(axum::middleware::from_fn_with_state(state.clone(), auth::client_ip_mw))
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            header::REFERRER_POLICY,
            HeaderValue::from_static("same-origin"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("permissions-policy"),
            HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("content-security-policy"),
            HeaderValue::from_static(
                "default-src 'self'; \
                 script-src 'self'; \
                 style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; \
                 font-src 'self' https://fonts.gstatic.com data:; \
                 img-src 'self' data:; \
                 connect-src 'self'; \
                 frame-ancestors 'none'; \
                 base-uri 'none'; \
                 form-action 'self'",
            ),
        ))
        .layer(SetResponseHeaderLayer::if_not_present(
            HeaderName::from_static("cross-origin-opener-policy"),
            HeaderValue::from_static("same-origin"),
        ))
        .with_state(state);

    let app = if cfg.cookie_secure {
        app.layer(SetResponseHeaderLayer::if_not_present(
            header::STRICT_TRANSPORT_SECURITY,
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        ))
    } else {
        app
    };

    let addr: SocketAddr = cfg.bind.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "listening");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown_signal())
    .await?;
    Ok(())
}

fn init_tracing(json: bool) {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "clinicos=info,tower_http=warn".into());
    if json {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .json()
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    }
}

async fn api_not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "error": "not_found", "message": "Unknown API endpoint" })),
    )
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown requested");
}
