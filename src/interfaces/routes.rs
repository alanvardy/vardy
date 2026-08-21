use axum::{Router, routing::get};

use crate::interfaces::handlers;

pub fn routes() -> Router {
    Router::new().route("/", get(handlers::home::web::index))
}
