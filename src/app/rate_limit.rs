use std::net::SocketAddr;

use axum::{Router, extract::ConnectInfo, response::IntoResponse};
use tower_governor::{
    GovernorError, GovernorLayer, governor::GovernorConfigBuilder, key_extractor::KeyExtractor,
};

use crate::app::error::WebError;
use crate::app::state::AppState;

/// Reads `Fly-Client-IP` set by the Fly Proxy, which cannot be spoofed.
/// Falls back to the TCP peer address for local development.
///
/// `X-Forwarded-For` is deliberately ignored because Fly Proxy appends to it,
/// making it trivially spoofable by clients.
#[derive(Clone)]
pub struct FlyClientIpKeyExtractor;

impl KeyExtractor for FlyClientIpKeyExtractor {
    type Key = std::net::IpAddr;

    fn extract<T>(&self, req: &axum::http::Request<T>) -> Result<Self::Key, GovernorError> {
        if let Some(ip) = req
            .headers()
            .get("fly-client-ip")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
        {
            return Ok(ip);
        }

        req.extensions()
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ci| ci.0.ip())
            .ok_or(GovernorError::UnableToExtractKey)
    }
}

/// Map governor errors through the `WebError` chokepoint so middleware and
/// handlers share one 429 format.
fn rate_limit_error_response(err: GovernorError) -> axum::response::Response {
    match err {
        GovernorError::TooManyRequests { wait_time, headers } => {
            let mut response = WebError::TooManyRequests {
                retry_after_secs: wait_time,
            }
            .into_response();
            if let Some(headers) = headers {
                for (name, value) in headers.into_iter() {
                    if let Some(name) = name {
                        response.headers_mut().insert(name, value);
                    }
                }
            }
            response
        }
        // Unreachable with our extractor (header or ConnectInfo always present),
        // but keep it total and logged.
        other => {
            tracing::error!(?other, "rate limiter failed to extract key");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal server error",
            )
                .into_response()
        }
    }
}

/// Apply a global per-IP rate limiter to the router.
pub fn with_global_limit(router: Router<AppState>, per_ms: u64, burst: u32) -> Router<AppState> {
    let governor_cfg = std::sync::Arc::new(
        GovernorConfigBuilder::default()
            .key_extractor(FlyClientIpKeyExtractor)
            .per_millisecond(per_ms)
            .burst_size(burst)
            .use_headers()
            .finish()
            .expect("rate-limit config must be valid"),
    );

    router.layer(GovernorLayer::new(governor_cfg).error_handler(rate_limit_error_response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::Request;

    #[test]
    fn extracts_fly_client_ip() {
        let req = Request::builder()
            .header("fly-client-ip", "1.2.3.4")
            .body(())
            .unwrap();

        let ip = FlyClientIpKeyExtractor.extract(&req).unwrap();
        assert_eq!(ip, std::net::IpAddr::from([1, 2, 3, 4]));
    }

    #[test]
    fn ignores_x_forwarded_for() {
        let req = Request::builder()
            .header("x-forwarded-for", "10.0.0.1")
            .body(())
            .unwrap();

        assert!(FlyClientIpKeyExtractor.extract(&req).is_err());
    }

    #[test]
    fn prefers_fly_client_ip_over_xff() {
        let req = Request::builder()
            .header("x-forwarded-for", "10.0.0.1")
            .header("fly-client-ip", "9.9.9.9")
            .body(())
            .unwrap();

        let ip = FlyClientIpKeyExtractor.extract(&req).unwrap();
        assert_eq!(ip, std::net::IpAddr::from([9, 9, 9, 9]));
    }

    #[test]
    fn falls_back_to_connect_info() {
        let req = Request::builder()
            .extension(ConnectInfo("127.0.0.1:8080".parse::<SocketAddr>().unwrap()))
            .body(())
            .unwrap();

        let ip = FlyClientIpKeyExtractor.extract(&req).unwrap();
        assert_eq!(ip, std::net::IpAddr::from([127, 0, 0, 1]));
    }

    #[test]
    fn errors_when_no_key_available() {
        let req = Request::builder().body(()).unwrap();

        match FlyClientIpKeyExtractor.extract(&req) {
            Err(GovernorError::UnableToExtractKey) => {}
            other => panic!("expected UnableToExtractKey, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn too_many_requests_error_maps_to_web_error_shape() {
        let res = rate_limit_error_response(GovernorError::TooManyRequests {
            wait_time: 5,
            headers: None,
        });
        assert_eq!(res.status(), 429);
        assert_eq!(
            res.headers().get("retry-after"),
            Some(&"5".parse().unwrap())
        );
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(bytes.as_ref(), b"too many requests");
    }
}
