mod app;
mod interfaces;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = app::state::AppState {
        templates: app::templates::init(),
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
