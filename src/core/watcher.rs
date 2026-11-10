use crate::core::converter::ContentOptimizer;
use crate::core::parsers::parse_file;
use crate::error::AppError;
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tracing::{error, info};

pub async fn watch_directory(watch_dir: PathBuf, output_dir: PathBuf) -> Result<(), AppError> {
    fs::create_dir_all(&watch_dir)?;
    fs::create_dir_all(&output_dir)?;

    let (tx, mut rx) = mpsc::channel::<notify::Result<Event>>(100);

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            let _ = tx.blocking_send(res);
        },
        Config::default(),
    )?;

    watcher.watch(&watch_dir, RecursiveMode::Recursive)?;
    info!("Watching {} for changes...", watch_dir.display());

    let mut optimizer = ContentOptimizer::new();

    while let Some(res) = rx.recv().await {
        match res {
            Ok(event) => {
                if matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_)) {
                    for path in event.paths {
                        if path.is_file() {
                            if let Err(e) = process_watched_file(&path, &output_dir, &mut optimizer) {
                                error!("Failed to process {}: {}", path.display(), e);
                            }
                        }
                    }
                }
            }
            Err(e) => error!("Watch error: {}", e),
        }
    }

    Ok(())
}

fn process_watched_file(
    file_path: &Path,
    out_dir: &Path,
    optimizer: &mut ContentOptimizer,
) -> Result<(), AppError> {
    let raw = fs::read_to_string(file_path)?;
    let parsed = parse_file(file_path, &raw)?;
    let optimized = optimizer.optimize(&parsed);

    let stem = file_path.file_stem().unwrap_or_default().to_string_lossy();
    let out_path = out_dir.join(format!("{}.tokens.md", stem));

    fs::write(&out_path, optimized)?;
    info!("Successfully optimized: -> {}", out_path.display());
    Ok(())
}
