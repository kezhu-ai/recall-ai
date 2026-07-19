//! Importers for ChatGPT / Claude / Claude Code / Codex CLI exports.
//!
//! Strategy: walk the input path. If it's a .zip, extract first. Then dispatch
//! on filename heuristics to a per-source parser.

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use tempfile::TempDir;
use walkdir::WalkDir;

use crate::storage::{Message, Store};

pub fn import_path(path: &Path, store: &mut Store) -> Result<usize> {
    if !path.exists() {
        anyhow::bail!("import: path does not exist: {}", path.display());
    }
    let tmp: Option<TempDir>;
    let root: PathBuf = if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("zip") {
        let d = TempDir::new()?;
        extract_zip(path, d.path())?;
        tmp = Some(d);
        let dir = tmp.as_ref().unwrap().path().to_path_buf();
        dir
    } else if path.is_dir() {
        tmp = None;
        path.to_path_buf()
    } else {
        anyhow::bail!("unsupported import path: {}", path.display());
    };

    let mut total = 0usize;
    total += import_chatgpt(&root, store)?;
    total += import_claude_export(&root, store)?;
    total += import_claude_code(&root, store)?;
    total += import_codex(&root, store)?;
    Ok(total)
}

fn extract_zip(zip: &Path, dst: &Path) -> Result<()> {
    let f = File::open(zip).with_context(|| format!("opening {}", zip.display()))?;
    let mut zip = zip::ZipArchive::new(f)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let out_path = dst.join(entry.name());
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path).ok();
        } else {
            if let Some(p) = out_path.parent() { std::fs::create_dir_all(p).ok(); }
            let mut out = File::create(&out_path)?;
            std::io::copy(&mut entry, &mut out)?;
        }
    }
    Ok(())
}

/// ChatGPT export: conversations.json (or zip containing it).
fn import_chatgpt(root: &Path, store: &mut Store) -> Result<usize> {
    let mut total = 0;
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.file_name().and_then(|s| s.to_str()) != Some("conversations.json") { continue; }
        let raw = std::fs::read_to_string(p)?;
        let v: serde_json::Value = serde_json::from_str(&raw)?;
        let arr = v.as_array().cloned().unwrap_or_default();
        for chat in arr {
            let title = chat.get("title").and_then(|x| x.as_str()).unwrap_or("(untitled)").to_string();
            let mapping = chat.get("mapping").and_then(|x| x.as_object()).cloned().unwrap_or_default();
            for (_id, node) in mapping {
                let msg = node.get("message").cloned().unwrap_or(serde_json::Value::Null);
                let role = msg.get("author").and_then(|a| a.get("role")).and_then(|x| x.as_str()).unwrap_or("");
                let role = match role {
                    "user" => "user",
                    "assistant" => "assistant",
                    _ => continue,
                };
                let content = msg.get("content").and_then(|c| c.get("parts")).and_then(|p| p.as_array())
                    .map(|parts| parts.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join("\n"))
                    .unwrap_or_default();
                if content.is_empty() { continue; }
                let ts = msg.get("create_time").and_then(|v| v.as_f64())
                    .and_then(|f| chrono::DateTime::from_timestamp(f as i64, 0));
                let m = Message {
                    id: None,
                    source: "chatgpt".into(),
                    ts,
                    role: role.into(),
                    content: format!("[chat: {}] {}", title, content),
                    source_path: p.display().to_string(),
                    line_no: total as i64 + 1,
                };
                if store.insert(&m)? { total += 1; }
            }
        }
    }
    Ok(total)
}

/// Claude export: a zip with conversations.json at the top level.
fn import_claude_export(root: &Path, store: &mut Store) -> Result<usize> {
    let mut total = 0;
    for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        let fname = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if fname != "conversations.json" { continue; }
        // Heuristic: ChatGPT uses "conversation" mapping; Claude uses "chat_messages".
        // Use a simple distinguishing marker: presence of "chat_messages".
        let head = std::fs::read(p).unwrap_or_default();
        let head_str = String::from_utf8_lossy(&head[..head.len().min(2048)]);
        if !head_str.contains("chat_messages") { continue; }
        let raw = std::fs::read_to_string(p)?;
        let v: serde_json::Value = serde_json::from_str(&raw)?;
        let arr = v.as_array().cloned().unwrap_or_default();
        for chat in arr {
            let title = chat.get("name").and_then(|x| x.as_str()).unwrap_or("(untitled)").to_string();
            let ts0 = chat.get("created_at").and_then(|x| x.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc)));
            let msgs = chat.get("chat_messages").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            for (i, m) in msgs.iter().enumerate() {
                let role = m.get("role").and_then(|x| x.as_str()).unwrap_or("");
                let role = match role {
                    "user" | "human" => "user",
                    "assistant" | "claude" => "assistant",
                    _ => continue,
                };
                let content = m.get("content").and_then(|x| x.as_str()).unwrap_or("").to_string();
                if content.is_empty() { continue; }
                let m = Message {
                    id: None,
                    source: "claude-export".into(),
                    ts: ts0,
                    role: role.into(),
                    content: format!("[chat: {}] {}", title, content),
                    source_path: p.display().to_string(),
                    line_no: total as i64 + i as i64 + 1,
                };
                if store.insert(&m)? { total += 1; }
            }
        }
    }
    Ok(total)
}

/// Claude Code local sessions: ~/.claude/projects/<proj>/<sid>.jsonl
fn import_claude_code(root: &Path, store: &mut Store) -> Result<usize> {
    let mut total = 0;
    for entry in WalkDir::new(root).max_depth(3).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.extension().and_then(|s| s.to_str()) != Some("jsonl") { continue; }
        if p.to_string_lossy().contains("subagents") { continue; }
        // Heuristic: Claude Code JSONLs have a top-level "type":"user|assistant"
        let f = File::open(p)?;
        let reader = BufReader::new(f);
        let mut line_no: i64 = 0;
        for line in reader.lines().map_while(Result::ok) {
            line_no += 1;
            if line.is_empty() { continue; }
            let val: serde_json::Value = match serde_json::from_str(&line) { Ok(v) => v, Err(_) => continue };
            let t = val.get("type").and_then(|v| v.as_str()).unwrap_or("");
            let (role, content) = match t {
                "user" | "assistant" => {
                    let role = if t == "user" { "user" } else { "assistant" };
                    let msg = val.get("message").and_then(|m| m.get("content"));
                    let content = if let Some(c) = msg.and_then(|c| c.as_str()) {
                        c.to_string()
                    } else if let Some(arr) = msg.and_then(|c| c.as_array()) {
                        arr.iter().filter_map(|p| p.get("text").and_then(|t| t.as_str())).collect::<Vec<_>>().join("\n")
                    } else { continue; };
                    (role.to_string(), content)
                }
                _ => continue,
            };
            if content.is_empty() { continue; }
            let ts = val.get("timestamp").and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc)));
            let m = Message {
                id: None,
                source: "claude-code".into(),
                ts,
                role,
                content,
                source_path: p.display().to_string(),
                line_no,
            };
            if store.insert(&m)? { total += 1; }
        }
    }
    Ok(total)
}

/// Codex CLI: ~/.codex/archived_sessions/rollout-*.jsonl
fn import_codex(root: &Path, store: &mut Store) -> Result<usize> {
    let mut total = 0;
    for entry in WalkDir::new(root).max_depth(2).into_iter().filter_map(|e| e.ok()) {
        let p = entry.path();
        if p.extension().and_then(|s| s.to_str()) != Some("jsonl") { continue; }
        let fname = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !fname.starts_with("rollout-") { continue; }
        let f = File::open(p)?;
        let reader = BufReader::new(f);
        let mut line_no: i64 = 0;
        let mut msg_ts: Option<DateTime<Utc>> = None;
        for line in reader.lines().map_while(Result::ok) {
            line_no += 1;
            if line.is_empty() { continue; }
            let val: serde_json::Value = match serde_json::from_str(&line) { Ok(v) => v, Err(_) => continue };
            let ts = val.get("timestamp").and_then(|v| v.as_str())
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc)));
            if ts.is_some() { msg_ts = ts; }
            let (role, content) = match val.get("type").and_then(|v| v.as_str()).unwrap_or("") {
                "response_item" => {
                    let p = val.get("payload");
                    let pt = p.and_then(|x| x.get("type")).and_then(|x| x.as_str()).unwrap_or("");
                    if pt != "message" { continue; }
                    let role = p.and_then(|x| x.get("role")).and_then(|x| x.as_str()).unwrap_or("");
                    let role = match role { "user" => "user", "assistant" => "assistant", _ => continue };
                    let arr = p.and_then(|x| x.get("content")).and_then(|x| x.as_array());
                    let content = arr.map(|a| a.iter().filter_map(|p| p.get("text").and_then(|t| t.as_str())).collect::<Vec<_>>().join("\n")).unwrap_or_default();
                    (role.to_string(), content)
                }
                _ => continue,
            };
            if content.is_empty() { continue; }
            let m = Message {
                id: None,
                source: "codex".into(),
                ts: msg_ts,
                role,
                content,
                source_path: p.display().to_string(),
                line_no,
            };
            if store.insert(&m)? { total += 1; }
        }
    }
    Ok(total)
}