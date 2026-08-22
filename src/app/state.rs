use std::sync::Arc;

/// Sanctioned surface for infra types consumed by `interfaces`; do not
/// import from `crate::infra` outside `src/app` and `main.rs`.
pub use crate::infra::metrics::AppMetrics;

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
