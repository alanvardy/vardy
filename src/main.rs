mod app;
mod interfaces;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    axum::serve(listener, interfaces::routes::routes().into_make_service()).await?;
    Ok(())
}

#[cfg(test)]
mod test;
