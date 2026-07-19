//! Output renderers + a tiny localhost web UI.

use anyhow::Result;
use chrono::{DateTime, Local};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use tabled::{Table, Tabled};

use crate::storage::{Hit, SourceStats, Store};

pub fn print_search(hits: &[Hit], query: &str) {
    if hits.is_empty() {
        println!("(no matches for \"{}\")", query);
        return;
    }
    println!("{} match(es) for \"{}\":\n", hits.len(), query);
    let rows: Vec<SearchRow> = hits.iter().take(20).map(|h| SearchRow {
        when: format_when(h.ts),
        source: h.source.clone(),
        role: h.role.clone(),
        snippet: h.snippet.clone(),
        path: format!("{}:{}", h.source_path, h.line_no),
    }).collect();
    println!("{}", Table::new(rows));
}

#[derive(Tabled)]
struct SearchRow {
    #[tabled(rename = "when")]
    when: String,
    #[tabled(rename = "source")]
    source: String,
    #[tabled(rename = "role")]
    role: String,
    #[tabled(rename = "snippet")]
    snippet: String,
    #[tabled(rename = "path:line")]
    path: String,
}

pub fn print_stats(stats: &[SourceStats]) {
    if stats.is_empty() {
        println!("(empty index — run `recall import <path>` first)");
        return;
    }
    let rows: Vec<StatRow> = stats.iter().map(|s| StatRow {
        source: s.source.clone(),
        messages: s.messages.to_string(),
    }).collect();
    println!("{}", Table::new(rows));
}

#[derive(Tabled)]
struct StatRow {
    #[tabled(rename = "source")]
    source: String,
    #[tabled(rename = "messages")]
    messages: String,
}

fn format_when(ts: Option<DateTime<chrono::Utc>>) -> String {
    ts.map(|t| t.with_timezone(&Local).format("%m-%d %H:%M").to_string()).unwrap_or_else(|| "?".into())
}

pub fn open_hit(store: &mut Store, id: &str) -> Result<()> {
    let rowid: i64 = id.trim_start_matches("r-").parse()
        .map_err(|_| anyhow::anyhow!("bad id {:?}", id))?;
    let mut stmt = store.conn.prepare(
        "SELECT source_path, line_no FROM messages WHERE id = ?",
    )?;
    let (path, line_no): (String, i64) = stmt.query_row(rusqlite::params![rowid], |r| Ok((r.get(0)?, r.get(1)?)))?;
    println!("{}:{}", path, line_no);
    Ok(())
}

#[allow(dead_code)]
pub fn _open_hit_unused() {}

/// Tiny localhost web UI: single-page HTML + JSON API.
/// No JS frameworks, no Tauri, no SaaS — just stdlib + tiny HTML/CSS.
pub fn serve(addr: &str, store: &Store) -> Result<()> {
    let listener = TcpListener::bind(addr)?;
    eprintln!("[recall-web] http://{}/  (Ctrl-C to stop)", addr);
    for stream in listener.incoming() {
        let mut s = match stream { Ok(s) => s, Err(_) => continue };
        let mut buf = [0u8; 8192];
        let n = match s.read(&mut buf) { Ok(n) => n, Err(_) => continue };
        let req = String::from_utf8_lossy(&buf[..n]).to_string();
        let (method, path) = parse_request(&req);
        let response = if method == "GET" && path.starts_with("/search") {
            let q = path.split('?').nth(1).and_then(|s| {
                s.split('&').find_map(|kv| {
                    let mut parts = kv.split('=');
                    let k = parts.next()?;
                    if k == "q" { Some(parts.next()?.to_string()) } else { None }
                })
            }).unwrap_or_default();
            if q.is_empty() {
                page_html("", &[])
            } else {
                let hits = store.search(&q, 365, None, 50).unwrap_or_default();
                let json = serde_json::to_string(&hits.iter().map(|h| {
                    serde_json::json!({
                        "source": h.source,
                        "ts": h.ts.map(|t| t.to_rfc3339()),
                        "role": h.role,
                        "snippet": h.snippet,
                        "path": format!("{}:{}", h.source_path, h.line_no),
                    })
                }).collect::<Vec<_>>()).unwrap_or_else(|_| "[]".into());
                page_html(&q, &serde_json::from_str::<Vec<serde_json::Value>>(&json).unwrap_or_default())
            }
        } else {
            page_html("", &[])
        };
        let http = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            response.len(), response
        );
        let _ = s.write_all(http.as_bytes());
    }
    Ok(())
}

fn parse_request(req: &str) -> (&str, &str) {
    let first = req.lines().next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("/");
    (method, path)
}

fn page_html(q: &str, results: &[serde_json::Value]) -> String {
    let rows: String = results.iter().map(|r| {
        let src = r.get("source").and_then(|v| v.as_str()).unwrap_or("");
        let role = r.get("role").and_then(|v| v.as_str()).unwrap_or("");
        let snip = r.get("snippet").and_then(|v| v.as_str()).unwrap_or("");
        let path = r.get("path").and_then(|v| v.as_str()).unwrap_or("");
        format!(
            r#"<tr><td class="src">{src}</td><td class="role">{role}</td><td class="snip">{snip}</td><td class="path">{path}</td></tr>"#,
            src = html_escape(src), role = html_escape(role),
            snip = html_escape(snip), path = html_escape(path)
        )
    }).collect::<Vec<_>>().join("\n");

    format!(r#"<!doctype html>
<html><head><meta charset="utf-8"><title>recall</title>
<style>
body{{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;background:#0a0a0f;color:#e6e6f0;margin:0;padding:2rem}}
h1{{font-size:1.4rem;margin:0 0 1rem;color:#10b981}}
input{{width:60%;padding:8px 12px;background:#13131a;border:1px solid #2a2a35;color:#e6e6f0;border-radius:6px;font:inherit}}
button{{padding:8px 16px;background:#10b981;color:#050509;border:0;border-radius:6px;font:inherit;font-weight:600;cursor:pointer}}
table{{width:100%;border-collapse:collapse;margin-top:1rem}}
td{{padding:8px 10px;border-bottom:1px solid #1a1a25;font-size:13px;vertical-align:top}}
.src{{color:#10b981;width:120px}}
.role{{color:#8b5cf6;width:80px}}
.path{{color:#6c7086;font-size:11px;width:30%;word-break:break-all}}
em{{color:#f59e0b;font-style:normal;background:#2a1a05}}
</style></head>
<body>
<h1>recall · search every AI conversation</h1>
<form>
<input name="q" placeholder="search your AI history…" value="{q}"/>
<button>Search</button>
</form>
<table>
<thead><tr><th>source</th><th>role</th><th>match</th><th>file:line</th></tr></thead>
<tbody>
{rows}
</tbody>
</table>
</body></html>"#,
        q = html_escape(q))
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}

#[allow(dead_code)]
fn _unused_path() -> PathBuf { PathBuf::new() }