//! Output renderers: search hits table, daily/weekly markdown digest, stats.

use std::path::Path;
use chrono::{DateTime, Local, Utc};
use tabled::{Table, Tabled};

use crate::ingest::{SearchHit, Session};

pub fn print_search(hits: &[SearchHit], query: &str, full: bool) {
    if hits.is_empty() {
        println!("(no matches for \"{}\")", query);
        return;
    }
    println!("{} match(es) for \"{}\":\n", hits.len(), query);

    if full {
        for h in hits.iter().take(50) {
            println!("--- {} [{}] line {} ---", h.session_path.display(), h.role, h.line_no);
            println!("  {}", h.snippet);
            println!();
        }
    } else {
        let rows: Vec<SearchRow> = hits.iter().take(50).map(|h| SearchRow {
            when: format_when(h.first_ts),
            tool: h.tool.clone(),
            role: h.role.clone(),
            snippet: h.snippet.clone(),
            path: h.session_path.display().to_string(),
        }).collect();
        println!("{}", Table::new(rows));
    }
}

#[derive(Tabled)]
struct SearchRow {
    #[tabled(rename = "when")]
    when: String,
    #[tabled(rename = "tool")]
    tool: String,
    #[tabled(rename = "role")]
    role: String,
    #[tabled(rename = "snippet")]
    snippet: String,
    #[tabled(rename = "path")]
    path: String,
}

pub fn build_digest(sessions: &[Session], label: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!("# AI session digest — {}\n\n", label));

    if sessions.is_empty() {
        out.push_str("_No AI sessions found in this window._\n");
        return out;
    }

    let total_turns: u64 = sessions.iter().map(|s| s.turns).sum();
    let total_tokens: u64 = sessions.iter().map(|s| s.input_tokens + s.output_tokens).sum();
    let total_sessions = sessions.len();

    out.push_str(&format!(
        "**{} sessions** · **{} turns** · **{} tokens**\n\n",
        total_sessions, total_turns, human(total_tokens)
    ));

    // Group by day
    let mut by_day: std::collections::BTreeMap<String, Vec<&Session>> = std::collections::BTreeMap::new();
    for s in sessions {
        let day = s.first_ts.map(|t| t.format("%Y-%m-%d").to_string()).unwrap_or_else(|| "?".into());
        by_day.entry(day).or_default().push(s);
    }

    for (day, items) in by_day.iter().rev() {
        out.push_str(&format!("## {}\n\n", day));
        for s in items {
            let when = format_when(s.first_ts);
            let tool = if s.tool == "claude" { "Claude Code" } else { "Codex CLI" };
            out.push_str(&format!(
                "- **{}** {} ({} turns, {} tokens)\n",
                when, tool, s.turns, human(s.input_tokens + s.output_tokens)
            ));
            // Show first user message + last assistant message as a hint
            if let Some(u) = s.user_messages.first() {
                out.push_str(&format!("  > you: {}\n", truncate_line(u, 120)));
            }
            if let Some(a) = s.assistant_messages.last() {
                out.push_str(&format!("  > ai:  {}\n", truncate_line(a, 120)));
            }
        }
        out.push('\n');
    }

    out
}

pub fn print_stats(sessions: &[Session]) {
    println!("{} sessions found.\n", sessions.len());

    let by_tool = {
        let mut map = std::collections::HashMap::<&str, (u64, u64)>::new();
        for s in sessions {
            let entry = map.entry(s.tool.as_str()).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += s.input_tokens + s.output_tokens;
        }
        map
    };
    let mut rows: Vec<StatRow> = by_tool.iter().map(|(tool, (n, t))| StatRow {
        tool: tool.to_string(),
        sessions: n.to_string(),
        tokens: human(*t),
    }).collect();
    rows.sort_by(|a, b| b.sessions.cmp(&a.sessions));
    println!("{}", Table::new(rows));

    if let Some(top) = sessions.iter().max_by_key(|s| s.input_tokens + s.output_tokens) {
        println!("\nHeaviest session:");
        println!("  {} ({})", top.path.display(), human(top.input_tokens + top.output_tokens));
        println!("  {} turns · first {} last {}",
            top.turns,
            format_when(top.first_ts),
            format_when(top.last_ts));
    }
}

#[derive(Tabled)]
struct StatRow {
    #[tabled(rename = "tool")]
    tool: String,
    #[tabled(rename = "sessions")]
    sessions: String,
    #[tabled(rename = "tokens")]
    tokens: String,
}

fn human(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn format_when(ts: Option<DateTime<Utc>>) -> String {
    ts.map(|t| {
        let local = t.with_timezone(&Local);
        local.format("%m-%d %H:%M").to_string()
    }).unwrap_or_else(|| "?".into())
}

fn truncate_line(s: &str, max: usize) -> String {
    let clean: String = s.chars().filter(|c| *c != '\n' && *c != '\r').collect();
    if clean.chars().count() <= max {
        clean
    } else {
        let mut out: String = clean.chars().take(max).collect();
        out.push('…');
        out
    }
}