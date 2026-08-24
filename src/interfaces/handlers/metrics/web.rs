use axum::{extract::State, http::header::CONTENT_TYPE, response::IntoResponse};
use std::sync::Arc;

use crate::{app::state::AppMetrics, infra::metrics};

/// GET /metrics — returns Prometheus text exposition format.
pub async fn metrics_handler(State(metrics): State<Arc<AppMetrics>>) -> impl IntoResponse {
    // let content_type = TextEncoder::new().format_type().to_owned();
    let content_type = metrics::content_type();
    let body = metrics.render();
    ([(CONTENT_TYPE, content_type)], body)
}
