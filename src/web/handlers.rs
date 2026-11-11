use crate::core::converter::ContentOptimizer;
use crate::core::parsers::html::extract_article_markdown;
use crate::core::parsers::parse_file;
use crate::core::token_estimator::{TokenEstimator, TokenMetrics};
use crate::error::AppError;
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;

pub struct AppState {
    pub estimator: TokenEstimator,
}

#[derive(Deserialize)]
pub struct ConvertPayload {
    pub content: String,
    pub filename: Option<String>,
}

#[derive(Serialize)]
pub struct ConvertResponse {
    pub markdown: String,
    pub metrics: TokenMetrics,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

pub async fn convert_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ConvertPayload>,
) -> Result<Json<ConvertResponse>, AppError> {
    // Leverage the filename extension to select the proper parser (e.g. HTML vs text)
    let parsed_text = if let Some(ref name) = payload.filename {
        parse_file(Path::new(name), &payload.content)?
    } else {
        payload.content.clone()
    };

    let mut optimizer = ContentOptimizer::new();
    let optimized = optimizer.optimize(&parsed_text);
    let metrics = state.estimator.compute_metrics(&payload.content, &optimized);

    Ok(Json(ConvertResponse {
        markdown: optimized,
        metrics,
        source: None,
    }))
}

#[derive(Deserialize)]
pub struct FetchPayload {
    pub url: String,
}

pub async fn fetch_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<FetchPayload>,
) -> Result<Json<ConvertResponse>, AppError> {
    let client = reqwest::Client::new();
    let html = client.get(&payload.url).send().await?.text().await?;

    let parsed = extract_article_markdown(&html)?;
    let mut optimizer = ContentOptimizer::new();
    let optimized = optimizer.optimize(&parsed);
    let metrics = state.estimator.compute_metrics(&html, &optimized);

    Ok(Json(ConvertResponse {
        markdown: optimized,
        metrics,
        source: Some(html),
    }))
}
