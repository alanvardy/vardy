use std::sync::Arc;

use crate::app::env::Env;

#[derive(Clone)]
pub struct AppState {
    pub templates: minijinja::Environment<'static>,
    pub db: sqlx::SqlitePool,
    pub env: Arc<Env>,
    pub metrics: std::sync::Arc<crate::infra::metrics::AppMetrics>,
    /// Overridable so tests can point at a local stub server.
    pub unsplash_base_url: Arc<str>,
}
