#[derive(Clone)]
pub struct AppState {
    pub templates: minijinja::Environment<'static>,
}
