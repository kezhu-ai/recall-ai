//! Snippet subcommand: save / list / show / rm named bookmarks for messages.
//!
//! Snippets are a local "AI command line history" — when you find a useful
//! prompt or a great answer in `recall search`, you can save it under a
//! short name like `sql-window-function` and recall it later with
//! `recall snippet show sql-window-function`.

use anyhow::{Context, Result};
use chrono::Local;
use tabled::{Table, Tabled};

use crate::SnippetCmd;
use crate::storage::{Snippet, Store};

pub fn run(store: &mut Store, action: SnippetCmd) -> Result<()> {
    match action {
        SnippetCmd::Save { name, message_id, description, tags } => {
            if name.is_empty() || name.contains(' ') {
                anyhow::bail!("snippet name must be a single token (e.g. sql-window-fn)");
            }
            let s = store.save_snippet(&name, message_id, description.as_deref(), &tags)?;
            eprintln!("[recall] saved snippet `{}` → message id {}", s.name, s.message_id);
        }
        SnippetCmd::List { tag } => {
            let items = store.list_snippets(tag.as_deref())?;
            if items.is_empty() {
                println!("(no snippets — try `recall search <q>` then `recall snippet save <name> <id>`)");
                return Ok(());
            }
            let rows: Vec<ListRow> = items.iter().map(|s| ListRow {
                when: format_when(&s.created_at),
                name: s.name.clone(),
                tags: if s.tags.is_empty() { "-".into() } else { s.tags.clone() },
                desc: s.description.clone().unwrap_or_default(),
                msg_id: s.message_id.to_string(),
            }).collect();
            println!("{}", Table::new(rows));
            println!("\n{} snippet(s)", items.len());
        }
        SnippetCmd::Show { name, copy } => {
            let s = store.get_snippet(&name)?
                .with_context(|| format!("snippet {:?} not found — run `recall snippet list`", name))?;
            if copy {
                copy_to_clipboard(&s.message_content)?;
                eprintln!("[recall] copied {} chars to clipboard", s.message_content.len());
            } else {
                println!("--- {} [{}] (msg {}) ---", s.name, s.message_source, s.message_id);
                if !s.tags.is_empty() { println!("tags: {}", s.tags); }
                if let Some(d) = s.description { println!("desc: {}", d); }
                println!();
                println!("{}", s.message_content);
            }
        }
        SnippetCmd::Rm { name } => {
            if store.rm_snippet(&name)? {
                eprintln!("[recall] removed snippet `{}`", name);
            } else {
                anyhow::bail!("snippet {:?} not found", name);
            }
        }
    }
    Ok(())
}

#[derive(Tabled)]
struct ListRow {
    #[tabled(rename = "saved")]
    when: String,
    #[tabled(rename = "name")]
    name: String,
    #[tabled(rename = "tags")]
    tags: String,
    #[tabled(rename = "description")]
    desc: String,
    #[tabled(rename = "msg-id")]
    msg_id: String,
}

fn format_when(iso: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(iso)
        .map(|d| d.with_timezone(&Local).format("%m-%d %H:%M").to_string())
        .unwrap_or_else(|_| iso.to_string())
}

fn copy_to_clipboard(s: &str) -> Result<()> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    // Try the platform's clipboard helper, fall back to printing.
    let candidates: &[(&str, &[&str])] = if cfg!(target_os = "windows") {
        &[("clip", &[])]
    } else if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else {
        &[("wl-copy", &[]), ("xclip", &["-selection", "clipboard"])]
    };
    for (bin, args) in candidates {
        if let Ok(mut child) = Command::new(bin)
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                stdin.write_all(s.as_bytes()).ok();
                drop(stdin);
                let _ = child.wait();
                return Ok(());
            }
        }
    }
    println!("(no clipboard helper found; printed to stdout instead)\n");
    println!("{}", s);
    Ok(())
}