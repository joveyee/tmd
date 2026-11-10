use crate::core::token_estimator::TokenEstimator;
use crate::error::AppError;
use crate::web::assets::static_asset_handler;
use crate::web::handlers::{convert_handler, fetch_handler, AppState};
use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;

pub async fn run_server(host: &str, port: u16) -> Result<(), AppError> {
    let shared_state = Arc::new(AppState {
        estimator: TokenEstimator::new()?,
    });

    let app = Router::new()
        .route("/api/convert", post(convert_handler))
        .route("/api/fetch", post(fetch_handler))
        .fallback(get(static_asset_handler))
        .layer(CorsLayer::permissive())
        .with_state(shared_state);

    let addr: SocketAddr = format!("{}:{}", host, port)
        .parse()
        .map_err(|e: std::net::AddrParseError| AppError::Processing(e.to_string()))?;

    info!("Starting tmd Web Server on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await.map_err(AppError::Io)?;

    Ok(())
}
