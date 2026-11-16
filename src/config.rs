use std::env;
use std::path::PathBuf;

use chrono_tz::Tz;

#[derive(Clone, Debug)]
pub struct Config {
    pub bind: String,
    pub database_url: String,
    pub static_dir: PathBuf,
    pub session_ttl_hours: i64,
    pub cookie_secure: bool,
    pub trust_proxy: bool,
    pub admin_username: String,
    pub admin_password: Option<String>,
    pub demo_seed: bool,
    pub demo_password: String,
    pub tz: Tz,
    pub clinic_name_fa: String,
    pub clinic_name_en: String,
    pub log_json: bool,
    pub max_body_bytes: usize,
}

fn var(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn flag(key: &str, default: bool) -> bool {
    match env::var(key) {
        Ok(v) => matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes" | "on"),
        Err(_) => default,
    }
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let tz = var("CLINICOS_TZ", "Asia/Tehran")
            .parse::<Tz>()
            .map_err(|_| "CLINICOS_TZ is not a valid IANA timezone".to_string())?;
        let session_ttl_hours = var("CLINICOS_SESSION_TTL_HOURS", "12")
            .parse::<i64>()
            .map_err(|_| "CLINICOS_SESSION_TTL_HOURS must be a number".to_string())?;
        if !(1..=720).contains(&session_ttl_hours) {
            return Err("CLINICOS_SESSION_TTL_HOURS must be between 1 and 720".into());
        }
        let max_body_bytes = var("CLINICOS_MAX_BODY_BYTES", "1048576")
            .parse::<usize>()
            .map_err(|_| "CLINICOS_MAX_BODY_BYTES must be a number".to_string())?;

        Ok(Self {
            bind: var("CLINICOS_BIND", "127.0.0.1:8080"),
            database_url: var("DATABASE_URL", "sqlite://clinicos.db"),
            static_dir: PathBuf::from(var("CLINICOS_STATIC_DIR", "web")),
            session_ttl_hours,
            cookie_secure: flag("CLINICOS_COOKIE_SECURE", false),
            trust_proxy: flag("CLINICOS_TRUST_PROXY", false),
            admin_username: var("CLINICOS_ADMIN_USERNAME", "admin"),
            admin_password: env::var("CLINICOS_ADMIN_PASSWORD").ok().filter(|p| !p.trim().is_empty()),
            demo_seed: flag("CLINICOS_DEMO", false),
            demo_password: var("CLINICOS_DEMO_PASSWORD", "DemoClinic!2026"),
            tz,
            clinic_name_fa: var("CLINICOS_NAME_FA", "درمانگاه تخصصی ایران زمین"),
            clinic_name_en: var("CLINICOS_NAME_EN", "Iran Zamin Specialty Clinic"),
            log_json: flag("CLINICOS_LOG_JSON", false),
            max_body_bytes,
        })
    }

    pub fn is_production(&self) -> bool {
        self.cookie_secure
    }
}
