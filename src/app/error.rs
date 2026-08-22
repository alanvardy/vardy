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
}

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
    fn sqlx_error_converts_via_from() {
        let err: WebError = sqlx::Error::RowNotFound.into();
        assert!(matches!(err, WebError::Database(_)));
    }
}
