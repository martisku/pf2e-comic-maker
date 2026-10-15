use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};

#[derive(Debug)]
pub struct ApiError(pub StatusCode, pub &'static str);

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, self.1).into_response()
    }
}
impl From<sqlx::Error> for ApiError {
    fn from(_: sqlx::Error) -> Self {
        unavailable()
    }
}
pub fn invalid(message: &'static str) -> ApiError {
    ApiError(StatusCode::BAD_REQUEST, message)
}
pub fn conflict() -> ApiError {
    ApiError(
        StatusCode::CONFLICT,
        "This workspace changed in another session. Reload before editing; unsaved text remains on screen.",
    )
}
pub fn unavailable() -> ApiError {
    ApiError(
        StatusCode::SERVICE_UNAVAILABLE,
        "Storage is unavailable. Your operation was not acknowledged; retry or reload saved work before starting another upload.",
    )
}
