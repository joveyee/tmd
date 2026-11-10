use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Network request error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("HTML parsing error: {0}")]
    HtmlParse(String),

    #[error("Token counter error: {0}")]
    TokenCount(String),

    #[error("File watcher error: {0}")]
    Watcher(#[from] notify::Error),

    #[error("Processing error: {0}")]
    Processing(String),
}

// Map AppError directly into Axum HTTP Responses
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, err_msg) = match self {
            AppError::Io(e) => (StatusCode::NOT_FOUND, e.to_string()),
            AppError::Network(e) => (StatusCode::BAD_GATEWAY, e.to_string()),
            AppError::HtmlParse(e) | AppError::Processing(e) => {
                (StatusCode::UNPROCESSABLE_ENTITY, e)
            }
            AppError::TokenCount(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
            AppError::Watcher(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        };

        let body = Json(json!({
            "success": false,
            "error": err_msg
        }));

        (status, body).into_response()
    }
}
