use std::sync::Arc;

mod app;
mod domain;
mod infra;
mod interfaces;

use tracing::info;

const UNSPLASH_BASE_URL: &str = "https://api.unsplash.com";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    app::log::init();

    let env = app::env::Env::init();
    let metrics = Arc::new(infra::metrics::AppMetrics::new()?);
    let state = app::state::AppState {
        templates: app::templates::init(),
        db: app::db::init(&env.database_url).await,
        metrics: metrics.clone(),
        env: Arc::new(env),
        unsplash_base_url: UNSPLASH_BASE_URL.into(),
    };
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    info!("Hosting on http://localhost:3000");
    let metrics_listener = tokio::net::TcpListener::bind("0.0.0.0:9090").await?;
    info!("Metrics listening on http://localhost:9090");
    tokio::try_join!(
        axum::serve(
            listener,
            interfaces::routes::routes()
                .with_state(state)
                .layer(app::log::trace_layer())
                .into_make_service_with_connect_info::<std::net::SocketAddr>(),
        ),
        axum::serve(
            metrics_listener,
            interfaces::routes::metrics_router(metrics).into_make_service(),
        ),
    )?;
    Ok(())
}

#[cfg(test)]
mod test;
