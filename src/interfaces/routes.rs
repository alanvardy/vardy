use axum::{Router, routing::get};

use crate::app::state::AppState;
use crate::interfaces::handlers;

pub fn routes() -> Router<AppState> {
    Router::new().route("/", get(handlers::home::web::index))
}
