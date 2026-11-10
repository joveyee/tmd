use crate::core::converter::ContentOptimizer;
use crate::error::AppError;
use ignore::WalkBuilder;
use std::fs;
use std::path::Path;

/// Concurrently flattens a codebase directory into a single token-optimized markdown document
pub fn flatten_repository(root_dir: &Path, max_size_kb: u64) -> Result<String, AppError> {
    let mut manifest = String::with_capacity(64 * 1024);
    manifest.push_str(&format!("# Repository Manifest: {}\n\n", root_dir.display()));
    manifest.push_str("## Project Tree\n```text\n");

    // Pass 1: Build directory tree map
    let walker = WalkBuilder::new(root_dir)
        .hidden(true)
        .git_ignore(true)
        .build();

    for result in walker {
        let entry = result.map_err(|e| AppError::Processing(e.to_string()))?;
        let depth = entry.depth();
        if depth == 0 {
            continue;
        }
        let indent = "  ".repeat(depth.saturating_sub(1));
        let name = entry.file_name().to_string_lossy();
        if entry.file_type().map_or(false, |ft| ft.is_dir()) {
            manifest.push_str(&format!("{}{}/\n", indent, name));
        } else {
            manifest.push_str(&format!("{}{}\n", indent, name));
        }
    }
    manifest.push_str("```\n\n## Source Code Contents\n\n");

    // Pass 2: Ingest and optimize source contents
    let walker = WalkBuilder::new(root_dir)
        .hidden(true)
        .git_ignore(true)
        .build();

    let mut optimizer = ContentOptimizer::new();
    let max_bytes = max_size_kb * 1024;

    for result in walker {
        let entry = result.map_err(|e| AppError::Processing(e.to_string()))?;
        let path = entry.path();

        if path.is_file() {
            let metadata = fs::metadata(path)?;
            if metadata.len() > max_bytes {
                continue;
            }

            if let Ok(raw_content) = fs::read_to_string(path) {
                let optimized = optimizer.optimize(&raw_content);
                let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("txt");

                manifest.push_str(&format!("### File: `{}`\n", path.display()));
                manifest.push_str(&format!("```{ext}\n"));
                manifest.push_str(&optimized);
                manifest.push_str("\n```\n\n");
            }
        }
    }

    Ok(manifest)
}
