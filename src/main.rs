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
    let _guard = env
        .enable_sentry
        .then(|| infra::sentry::init(&env.sentry_dsn));
    let metrics = Arc::new(infra::metrics::AppMetrics::new()?);
    let http = reqwest::Client::new();
    let db = app::db::init(&env.database_url).await;
    app::db::migrate(&db).await?;
    info!("Database migrated");
    let rate_limit_per_ms = env.rate_limit_per_ms;
    let rate_limit_burst = env.rate_limit_burst;
    let state = app::state::AppState {
        templates: app::templates::init(),
        db,
        metrics: metrics.clone(),
        http,
        env: Arc::new(env),
        unsplash_base_url: UNSPLASH_BASE_URL.into(),
    };
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    info!("Hosting on http://localhost:3000");
    let metrics_listener = tokio::net::TcpListener::bind("0.0.0.0:9090").await?;
    info!("Metrics listening on http://localhost:9090");
    let router = interfaces::routes::routes().layer(app::log::trace_layer());
    let router = app::rate_limit::with_global_limit(router, rate_limit_per_ms, rate_limit_burst);
    tokio::try_join!(
        axum::serve(
            listener,
            router
                .with_state(state)
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
