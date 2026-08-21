#[derive(Clone)]
pub struct AppState {
    pub templates: minijinja::Environment<'static>,
    /// Unused until the first handler query lands; mirrors the
    /// `#[allow(dead_code)]` precedent on `WebError::NotFound`.
    #[allow(dead_code)]
    pub db: sqlx::SqlitePool,
}
