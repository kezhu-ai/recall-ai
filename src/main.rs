//! recall — Search every AI conversation you've ever had, locally.
//!
//! Architecture: local SQLite FTS5 index of all parsed AI session JSONLs
//! (ChatGPT export, Claude export, Claude Code, Codex CLI). Single binary,
//! zero-config, single user, no cloud.
//!
//! Subcommands:
//!   import <path>      Import a ChatGPT export zip OR a Claude export zip
//!   import <dir>       Scan a directory of Claude Code / Codex CLI sessions
//!   search <query>     Full-text FTS5 search across all sources
//!   open <id>          Open the source chat at the matching line
//!   snippet save       Bookmark a message as a named snippet
//!   snippet list       List saved snippets (with optional tag filter)
//!   snippet show       Print a snippet's content (optionally copy to clipboard)
//!   snippet rm         Remove a saved snippet
//!   today / week       Markdown digest (chronological)
//!   stats              Counts by source
//!   serve              Localhost web UI (default :7777)
//!
//! Data dir: $XDG_DATA_HOME/recall (fallback ~/.local/share/recall)
//! DB file:  <data-dir>/recall.db

use std::path::PathBuf;
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

mod ingest;
mod storage;
mod output;
mod snippet;

#[derive(Parser, Debug)]
#[command(name = "recall", version, about = "Search every AI conversation you've ever had, locally.")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Import: a ChatGPT export zip, a Claude export zip, or a session dir.
    Import {
        /// Path to a .zip (ChatGPT / Claude export) or a directory of session JSONLs.
        path: PathBuf,
    },

    /// Full-text search across all indexed sources.
    Search {
        query: String,
        /// Restrict to last N days
        #[arg(long, default_value_t = 365)]
        days: u32,
        /// Filter by source: chatgpt | claude-export | claude-code | codex
        #[arg(long)]
        source: Option<String>,
        /// Limit number of results
        #[arg(long, default_value_t = 20)]
        limit: u32,
    },

    /// Open the source chat at the matching line (best-effort).
    Open {
        /// Result id from `recall search` (e.g. r-42)
        id: String,
    },

    /// Save / list / show / rm named snippets (bookmarked messages).
    Snippet {
        #[command(subcommand)]
        action: SnippetCmd,
    },

    /// Markdown summary of today's AI activity.
    Today,

    /// Markdown summary of the last 7 days.
    Week,

    /// Counts by source.
    Stats,

    /// Run a tiny localhost web UI for browsing (default :7777).
    Serve {
        #[arg(long, default_value = "127.0.0.1:7777")]
        addr: String,
    },
}

#[derive(Subcommand, Debug)]
enum SnippetCmd {
    /// Save a search result as a named snippet.
    Save {
        /// Snippet name (lowercase, no spaces; e.g. "sql-window-function")
        name: String,
        /// Source message row id (from `recall search` results, e.g. r-42 → 42)
        message_id: i64,
        /// Free-form description (one line)
        #[arg(long)]
        description: Option<String>,
        /// Comma-separated tags, e.g. "sql,postgres,window"
        #[arg(long, default_value = "")]
        tags: String,
    },
    /// List saved snippets (newest first).
    List {
        /// Filter by tag (substring match)
        #[arg(long)]
        tag: Option<String>,
    },
    /// Print a saved snippet's content to stdout.
    Show {
        name: String,
        /// Copy to clipboard via `clip.exe` (Windows) or `pbcopy` (macOS) or `wl-copy` (Linux).
        /// If none of those are available, prints to stdout.
        #[arg(long)]
        copy: bool,
    },
    /// Remove a saved snippet.
    Rm {
        name: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let data_dir = data_dir();
    std::fs::create_dir_all(&data_dir).ok();
    let db_path = data_dir.join("recall.db");

    let mut store = storage::Store::open(&db_path)
        .with_context(|| format!("opening db {}", db_path.display()))?;

    match cli.cmd {
        Cmd::Import { path } => {
            let n = ingest::import_path(&path, &mut store)?;
            eprintln!("[recall] imported {} messages from {}", n, path.display());
        }
        Cmd::Search { query, days, source, limit } => {
            let hits = store.search(&query, days, source.as_deref(), limit)?;
            output::print_search(&hits, &query);
        }
        Cmd::Open { id } => {
            output::open_hit(&mut store, &id)?;
        }
        Cmd::Snippet { action } => {
            snippet::run(&mut store, action)?;
        }
        Cmd::Today => {
            let digest = store.digest_days(1)?;
            println!("{}", digest);
        }
        Cmd::Week => {
            let digest = store.digest_days(7)?;
            println!("{}", digest);
        }
        Cmd::Stats => {
            output::print_stats(&store.stats()?);
        }
        Cmd::Serve { addr } => {
            output::serve(&addr, &store)?;
        }
    }
    Ok(())
}

fn data_dir() -> PathBuf {
    if let Some(p) = std::env::var_os("RECALL_DATA_DIR") {
        return PathBuf::from(p);
    }
    if let Some(p) = std::env::var_os("XDG_DATA_HOME") {
        let mut pp = PathBuf::from(p);
        pp.push("recall");
        return pp;
    }
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut p = home;
    p.push(".local");
    p.push("share");
    p.push("recall");
    p
}