mod app;
mod interfaces;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:data/vardy.db".to_string());
    let state = app::state::AppState {
        templates: app::templates::init(),
        db: app::db::init(&database_url).await,
    };
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    println!("Hosting on http://localhost:3000");
    axum::serve(
        listener,
        interfaces::routes::routes()
            .with_state(state)
            .into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod test;
