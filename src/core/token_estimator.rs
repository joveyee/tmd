use crate::error::AppError;
use tiktoken_rs::{cl100k_base, CoreBPE};

#[derive(Debug, Clone, serde::Serialize)]
pub struct TokenMetrics {
    pub raw_tokens: usize,
    pub optimized_tokens: usize,
    pub savings_percentage: f64,
}

pub struct TokenEstimator {
    bpe: CoreBPE,
}

impl TokenEstimator {
    pub fn new() -> Result<Self, AppError> {
        let bpe = cl100k_base().map_err(|e| AppError::TokenCount(e.to_string()))?;
        Ok(Self { bpe })
    }

    pub fn count_tokens(&self, text: &str) -> usize {
        self.bpe.encode_ordinary(text).len()
    }

    pub fn compute_metrics(&self, original: &str, optimized: &str) -> TokenMetrics {
        let raw = self.count_tokens(original);
        let opt = self.count_tokens(optimized);
        let savings = if raw == 0 {
            0.0
        } else {
            ((raw.saturating_sub(opt)) as f64 / raw as f64) * 100.0
        };

        TokenMetrics {
            raw_tokens: raw,
            optimized_tokens: opt,
            savings_percentage: (savings * 100.0).round() / 100.0,
        }
    }
}
