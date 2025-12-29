//! triglyphd - D-Bus daemon for triglyph trigram search
//!
//! Provides immediate hookability for Nautilus and other GNOME applications
//! via the session D-Bus.
//!
//! Usage:
//!   triglyphd              # Run as D-Bus service (default)
//!   triglyphd index PATH   # One-shot: index a directory
//!   triglyphd search QUERY # One-shot: search all indices
//!   triglyphd list         # One-shot: list all indices

use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;
use zbus::connection::Builder;

mod dbus_iface;
mod error;
mod extract;
mod index_manager;
mod indexer;
mod paths;
mod searcher;

use dbus_iface::{TriglyphSearchProvider, TriglyphService, DBUS_NAME, DBUS_PATH, SEARCH_PROVIDER_PATH};
use index_manager::IndexManager;

#[derive(Parser)]
#[command(name = "triglyphd")]
#[command(about = "D-Bus daemon for triglyph trigram search")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Index a directory
    Index {
        /// Directory to index
        path: PathBuf,
    },

    /// Search indexed directories
    Search {
        /// Search query
        query: String,

        /// Search only in this directory
        #[arg(long = "in")]
        scope: Option<PathBuf>,
    },

    /// List all indexed directories
    List,

    /// Show status of an index
    Status {
        /// Directory to check
        path: PathBuf,
    },

    /// Remove an index
    Remove {
        /// Directory to remove
        path: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("triglyphd=info".parse()?))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(cmd) => run_oneshot(cmd).await,
        None => run_daemon().await,
    }
}

async fn run_daemon() -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("Starting triglyphd D-Bus service");

    let manager = Arc::new(IndexManager::new()?);

    // Build D-Bus connection and register service
    let connection = Builder::session()?
        .name(DBUS_NAME)?
        .build()
        .await?;

    // Register main Triglyph service
    let service = TriglyphService::new(manager.clone());
    connection
        .object_server()
        .at(DBUS_PATH, service)
        .await?;

    // Register GNOME Shell SearchProvider2
    let search_provider = TriglyphSearchProvider::new(manager);
    connection
        .object_server()
        .at(SEARCH_PROVIDER_PATH, search_provider)
        .await?;

    tracing::info!("Registered on D-Bus as {} at {}", DBUS_NAME, DBUS_PATH);
    tracing::info!("SearchProvider2 at {}", SEARCH_PROVIDER_PATH);
    tracing::info!("Ready for Nautilus/GNOME Shell integration");

    // Run forever
    std::future::pending::<()>().await;
    Ok(())
}

async fn run_oneshot(cmd: Commands) -> Result<(), Box<dyn std::error::Error>> {
    let manager = Arc::new(IndexManager::new()?);

    match cmd {
        Commands::Index { path } => {
            let path = path.canonicalize()?;
            if !path.is_dir() {
                eprintln!("ERR Path is not a directory");
                std::process::exit(1);
            }

            let cancel = manager.cancel_flag();
            let config = extract::ExtractConfig::default();
            let path_clone = path.clone();

            let result = tokio::task::spawn_blocking(move || {
                indexer::build_index_sync(&path_clone, None, cancel, config)
            })
            .await??;

            manager.save_meta(&path, result.file_count)?;
            println!("OK Indexed {} files in {}ms", result.file_count, result.duration_ms);
        }

        Commands::Search { query, scope } => {
            let results = if let Some(path) = scope {
                let path = path.canonicalize()?;
                searcher::Searcher::search_in(&manager, &path, &query).await?
            } else {
                searcher::Searcher::search_all(&manager, &query).await
            };

            // Deduplicate by file path (same file may appear in multiple indices)
            let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
            let mut unique_results: Vec<_> = results
                .iter()
                .filter(|r| seen.insert(&r.path))
                .collect();

            // Sort by mtime descending (most recently modified first)
            unique_results.sort_by(|a, b| b.mtime.cmp(&a.mtime));

            // Limit output
            const MAX_DISPLAY: usize = 50;
            let total = unique_results.len();
            let display_count = total.min(MAX_DISPLAY);

            for r in unique_results.iter().take(display_count) {
                println!("{}", r.path);
            }

            if total > MAX_DISPLAY {
                println!("... and {} more (use --in <dir> to narrow)", total - MAX_DISPLAY);
            }
            println!("({} unique files)", total);
        }

        Commands::List => {
            for s in manager.list_all() {
                println!(
                    "STATUS {}\t{}\t{}\t{}",
                    s.path.display(),
                    s.file_count,
                    s.indexed_at.to_rfc3339(),
                    s.state.as_str()
                );
            }
        }

        Commands::Status { path } => {
            let path = path.canonicalize()?;
            match manager.status(&path).await {
                Some(s) => {
                    println!(
                        "STATUS {}\t{}\t{}\t{}",
                        s.path.display(),
                        s.file_count,
                        s.indexed_at.to_rfc3339(),
                        s.state.as_str()
                    );
                }
                None => {
                    eprintln!("ERR Not indexed");
                    std::process::exit(1);
                }
            }
        }

        Commands::Remove { path } => {
            let path = path.canonicalize()?;
            manager.remove(&path).await?;
            println!("OK Index removed");
        }
    }

    Ok(())
}
