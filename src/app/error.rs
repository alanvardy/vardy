use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

/// The `NotFound` arm is only constructed from unit tests (coverage
/// hardening); keep it alive for non-test builds.
#[allow(dead_code)]
pub enum WebError {
    Template(minijinja::Error),
    NotFound,
}

impl From<minijinja::Error> for WebError {
    fn from(err: minijinja::Error) -> Self {
        WebError::Template(err)
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        match self {
            WebError::NotFound => (StatusCode::NOT_FOUND, "not found").into_response(),
            WebError::Template(err) => {
                eprintln!("template render error: {err}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
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
}
