use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub templates: minijinja::Environment<'static>,
    pub db: sqlx::SqlitePool,
    pub metrics: std::sync::Arc<crate::infra::metrics::AppMetrics>,
    /// Unused until the Phase 3 fetch lands; mirrors the
    /// `#[allow(dead_code)]` precedent on `WebError::NotFound`.
    #[allow(dead_code)]
    pub unsplash_api_key: Arc<str>,
    /// Overridable so tests can point at a local stub server.
    /// Unused until Phase 3.
    #[allow(dead_code)]
    pub unsplash_base_url: Arc<str>,
}
