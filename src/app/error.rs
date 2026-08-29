use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

/// The `NotFound` arm is only constructed from unit tests (coverage
/// hardening); keep it alive for non-test builds.
#[allow(dead_code)]
#[derive(Debug)]
pub enum WebError {
    Template(minijinja::Error),
    Database(sqlx::Error),
    NotFound,
    External(String),
    TooManyRequests { retry_after_secs: u64 },
}

/// Newtype wrapper so `WebError::External(String)` can be passed to
/// `sentry::capture_error` the same way the `Database` and `Template`
/// arms pass their inner error types.
#[derive(Debug)]
struct ExternalError(String);

impl std::fmt::Display for ExternalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for ExternalError {}

impl From<minijinja::Error> for WebError {
    fn from(err: minijinja::Error) -> Self {
        WebError::Template(err)
    }
}

impl From<sqlx::Error> for WebError {
    fn from(err: sqlx::Error) -> Self {
        WebError::Database(err)
    }
}

impl From<crate::infra::unsplash::UnsplashError> for WebError {
    fn from(err: crate::infra::unsplash::UnsplashError) -> Self {
        WebError::External(err.0)
    }
}

impl From<crate::infra::resend::ResendError> for WebError {
    fn from(err: crate::infra::resend::ResendError) -> Self {
        WebError::External(err.0)
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        match self {
            WebError::NotFound => (StatusCode::NOT_FOUND, "not found").into_response(),
            WebError::Database(err) => {
                tracing::error!(error = ?err, "database error");
                sentry::capture_error(&err);
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
            }
            WebError::Template(err) => {
                tracing::error!(error = ?err, "template render error");
                sentry::capture_error(&err);
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
            }
            WebError::External(message) => {
                tracing::error!(error = %message, "external error");
                (StatusCode::BAD_GATEWAY, "bad gateway").into_response()
            }
            // Client fault, like `External`: log nothing to Sentry.
            WebError::TooManyRequests { retry_after_secs } => (
                StatusCode::TOO_MANY_REQUESTS,
                [("retry-after", retry_after_secs.to_string())],
                "too many requests",
            )
                .into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn not_found_is_404() {
        let res = WebError::NotFound.into_response();
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn template_error_is_500() {
        let err = minijinja::Error::new(minijinja::ErrorKind::TemplateNotFound, "nope.html");
        let res = WebError::from(err).into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn database_error_is_500() {
        let res = WebError::from(sqlx::Error::RowNotFound).into_response();
        assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn external_error_is_502() {
        let res = WebError::External("boom".into()).into_response();
        assert_eq!(res.status(), StatusCode::BAD_GATEWAY);
    }

    #[test]
    fn resend_error_is_502() {
        let res = WebError::from(crate::infra::resend::ResendError("boom".into())).into_response();
        assert_eq!(res.status(), StatusCode::BAD_GATEWAY);
    }

    #[tokio::test]
    async fn too_many_requests_is_429_with_body_and_retry_after() {
        let res = WebError::TooManyRequests {
            retry_after_secs: 7,
        }
        .into_response();
        assert_eq!(res.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            res.headers().get("retry-after"),
            Some(&"7".parse().unwrap())
        );
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(bytes.as_ref(), b"too many requests");
    }

    #[test]
    fn sqlx_error_converts_via_from() {
        let err: WebError = sqlx::Error::RowNotFound.into();
        assert!(matches!(err, WebError::Database(_)));
    }

    #[test]
    fn external_error_implements_error() {
        let err = ExternalError("boom".into());
        assert_eq!(err.to_string(), "boom");

        // Bound-check: &ExternalError satisfies `impl Error`.
        fn assert_error(_: &dyn std::error::Error) {}
        assert_error(&err);
    }

    #[test]
    fn external_error_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ExternalError>();
    }
}
