use std::sync::Arc;

use chrono_tz::Tz;
use sqlx::SqlitePool;

use crate::auth::LoginGuard;
use crate::config::Config;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub cfg: Arc<Config>,
    pub login_guard: Arc<LoginGuard>,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

impl AppState {
    pub fn tz(&self) -> Tz {
        self.cfg.tz
    }
}
