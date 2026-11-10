use tokens_md_rs::error::AppError;

#[tokio::main]
async fn main() -> Result<(), AppError> {
    tokens_md_rs::run_app().await
}
