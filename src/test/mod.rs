use axum::Router;
use std::net::SocketAddr;

/// Bind a random port, spawn the app, return the bound address.
pub async fn start_app() -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let router: Router = crate::interfaces::routes::routes();
    tokio::spawn(async move {
        axum::serve(listener, router.into_make_service())
            .await
            .expect("server");
    });
    addr
}

pub fn test_client() -> reqwest::Client {
    reqwest::Client::new()
}
