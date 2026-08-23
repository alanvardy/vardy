use std::net::SocketAddr;

use axum::{Router, extract::ConnectInfo};
use tower_governor::{
    GovernorError, GovernorLayer, governor::GovernorConfigBuilder, key_extractor::KeyExtractor,
};

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

    router.layer(GovernorLayer::new(governor_cfg))
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
}
