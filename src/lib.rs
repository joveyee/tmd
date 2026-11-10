pub mod cli;
pub mod core;
pub mod error;
pub mod web;

use clap::Parser;
use cli::{Cli, Commands};
use core::{
    converter::ContentOptimizer,
    parsers::{html::extract_article_markdown, parse_file},
    repo::flatten_repository,
    token_estimator::TokenEstimator,
    watcher::watch_directory,
};
pub use error::AppError;
use std::fs;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

pub async fn run_app() -> Result<(), AppError> {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    let cli = Cli::parse();

    match cli.command {
        Commands::Convert(args) => {
            let raw_content = fs::read_to_string(&args.input)?;
            let parsed = parse_file(&args.input, &raw_content)?;

            let mut optimizer = ContentOptimizer::new();
            let optimized = optimizer.optimize(&parsed);

            let estimator = TokenEstimator::new()?;
            let metrics = estimator.compute_metrics(&raw_content, &optimized);

            info!(
                "Token Optimization: {} -> {} tokens ({:.2}% reduction)",
                metrics.raw_tokens, metrics.optimized_tokens, metrics.savings_percentage
            );

            if let Some(out_path) = args.output {
                fs::write(out_path, optimized)?;
            } else {
                println!("{}", optimized);
            }
        }

        Commands::Repo(args) => {
            info!("Flattening repository at: {}", args.dir.display());
            let manifest = flatten_repository(&args.dir, args.max_size_kb)?;
            fs::write(&args.output, manifest)?;
            info!("Manifest successfully written to: {}", args.output.display());
        }

        Commands::Fetch(args) => {
            info!("Fetching remote target: {}", args.url);
            let client = reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
                .build()
                .map_err(|e| AppError::Processing(e.to_string()))?;
            let resp = client.get(&args.url).send().await?.text().await?;
            let markdown = extract_article_markdown(&resp)?;

            let mut optimizer = ContentOptimizer::new();
            let optimized = optimizer.optimize(&markdown);

            if let Some(out) = args.output {
                fs::write(out, optimized)?;
            } else {
                println!("{}", optimized);
            }
        }

        Commands::Watch(args) => {
            info!("Starting Hot-Folder Watcher mode");
            watch_directory(args.dir, args.output_dir).await?;
        }

        Commands::Ui(args) => {
            web::server::run_server(&args.host, args.port).await?;
        }
    }

    Ok(())
}
