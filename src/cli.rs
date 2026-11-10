use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "tmd",
    about = "Ultra-fast tokens.md engine for LLM token optimization",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Strips boilerplate/metadata from text, code, or HTML and emits token-dense Markdown
    Convert(ConvertArgs),

    /// Collapses a codebase directory into a single, token-optimized manifest document
    Repo(RepoArgs),

    /// Fetches a web page, strips scripts/ads/sidebars, and extracts article markdown
    Fetch(FetchArgs),

    /// Monitors a directory asynchronously and auto-optimizes incoming documents
    Watch(WatchArgs),

    /// Launches the lightweight local web GUI
    Ui(UiArgs),
}

#[derive(Args, Debug)]
pub struct ConvertArgs {
    /// Path to input file or directory
    #[arg(short, long)]
    pub input: PathBuf,

    /// Optional output file path (defaults to stdout if omitted)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Tiktoken BPE encoding model name (e.g. gpt-4o, cl100k_base)
    #[arg(short, long, default_value = "gpt-4o")]
    pub model: String,
}

#[derive(Args, Debug)]
pub struct RepoArgs {
    /// Root path of the codebase to collapse
    #[arg(default_value = ".")]
    pub dir: PathBuf,

    /// Destination markdown file
    #[arg(short, long, default_value = "repo_manifest.tokens.md")]
    pub output: PathBuf,

    /// Maximum file size threshold in KiB to prevent ingesting large binaries
    #[arg(long, default_value_t = 512)]
    pub max_size_kb: u64,
}

#[derive(Args, Debug)]
pub struct FetchArgs {
    /// Target URL to scrape and convert
    pub url: String,

    /// Optional output path
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

#[derive(Args, Debug)]
pub struct WatchArgs {
    /// Directory path to monitor for incoming files
    #[arg(default_value = "./hot-folder")]
    pub dir: PathBuf,

    /// Directory where converted .md files will be written
    #[arg(short, long, default_value = "./optimized-output")]
    pub output_dir: PathBuf,
}

#[derive(Args, Debug)]
pub struct UiArgs {
    /// Host interface to bind
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,

    /// Port to listen on
    #[arg(short, long, default_value_t = 3000)]
    pub port: u16,
}
