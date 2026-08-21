#[derive(Clone)]
pub struct AppState {
    pub templates: minijinja::Environment<'static>,
    pub db: sqlx::SqlitePool,
    pub metrics: std::sync::Arc<crate::infra::metrics::AppMetrics>,
}
