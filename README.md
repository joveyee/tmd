# tmd (`tokens-md-rs`)

> ⚠️ **Notice: Early Development / Testing Stage**
> This project is currently in active development and testing. It is not ready for production use, and breaking changes may occur without notice.

High-performance Rust drop-in engine for LLM token optimization. `tmd` strips boilerplate, navigation elements, duplicates, and redundant metadata from HTML, Markdown, and source code to output token-dense, context-window-friendly Markdown.

---

## Features

- **Document Stripping & Deduplication**: Removes repetitive headers, footers, pagination marks, and duplicate paragraphs via FNV-1a hashing.
- **Accurate Token Estimation**: Real-time BPE token counting with OpenAI tokenizer models (e.g. `gpt-4o`, `cl100k_base`).
- **Repository Flattening**: Flattens code repositories into a single token-optimized manifest document.
- **Article Extraction**: Fetches web pages and extracts clean article Markdown with scripts, ads, and sidebars stripped.
- **Hot-Folder Watcher**: Asynchronously monitors a folder and optimizes incoming files automatically.
- **Local Web UI**: Embedded web interface for drag-and-drop optimization and comparison metrics.
- **Modular Build Flavors**: Lightweight default build, with opt-in PDF and DOCX extraction via Cargo feature flags.

---

## Installation & Build

### Lightweight Version (Default)
Optimized for HTML, Markdown, and plain text. Fast compile time, zero native document engine overhead.

```bash
cargo build --release
```
The binary will be available at `target/release/tmd`.

### Full Version (PDF & DOCX Support)
Enables binary document extraction (`pdf-extract` for PDF, `docx-lite` for Microsoft Word):

```bash
cargo build --release --features full
```

*(You can also selectively enable formats with `--features pdf` or `--features docx`).*

---

## CLI Usage

```bash
# Convert a document or web page
tmd convert -i document.html -o optimized.md

# Convert a PDF or DOCX (requires build with --features full)
tmd convert -i document.pdf -o optimized.md

# Flatten an entire codebase into a token-optimized prompt file
tmd repo --dir ./my-project --output ./manifest.md --max-size-kb 1024

# Scrape and convert a remote article
tmd fetch https://example.com/article -o article.md

# Watch a directory for incoming files
tmd watch --dir ./incoming --output-dir ./optimized

# Launch the local web GUI
tmd ui --host 127.0.0.1 --port 3000
```

---

## Running Tests

```bash
# Run tests for default features
cargo test

# Run tests with all features enabled
cargo test --features full
```

---

## License

GPL-3.0 (GNU General Public License v3.0)

