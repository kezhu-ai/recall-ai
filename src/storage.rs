//! SQLite FTS5 store for AI session messages.
//!
//! Schema:
//!   messages(id, source, ts, role, content, source_path, line_no)
//!     FTS5 virtual table `messages_fts(content, source, role)
//!     with content='messages' and content_rowid='id'
//!
//! Indexing is append-only and idempotent on (source_path, line_no).

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug, Clone)]
pub struct Message {
    pub source: String,
    pub ts: Option<DateTime<Utc>>,
    pub role: String,
    pub content: String,
    pub source_path: String,
    pub line_no: i64,
}

#[derive(Debug, Clone)]
pub struct Hit {
    pub source: String,
    pub ts: Option<DateTime<Utc>>,
    pub role: String,
    pub snippet: String,
    pub source_path: String,
    pub line_no: i64,
}

#[derive(Debug, Clone)]
pub struct SourceStats {
    pub source: String,
    pub messages: i64,
}

pub struct Store {
    pub conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let conn = Connection::open(path)?;
        let s = Self { conn };
        s.init_schema()?;
        Ok(s)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS messages (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                source       TEXT NOT NULL,
                ts           TEXT,
                role         TEXT NOT NULL,
                content      TEXT NOT NULL,
                source_path  TEXT NOT NULL,
                line_no      INTEGER NOT NULL,
                UNIQUE(source_path, line_no)
            );
            CREATE INDEX IF NOT EXISTS idx_messages_source ON messages(source);
            CREATE INDEX IF NOT EXISTS idx_messages_ts ON messages(ts);
            CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
                content, source, role,
                content='messages', content_rowid='id',
                tokenize='porter unicode61'
            );
            CREATE TRIGGER IF NOT EXISTS messages_ai AFTER INSERT ON messages BEGIN
                INSERT INTO messages_fts(rowid, content, source, role)
                VALUES (new.id, new.content, new.source, new.role);
            END;
            CREATE TRIGGER IF NOT EXISTS messages_ad AFTER DELETE ON messages BEGIN
                INSERT INTO messages_fts(messages_fts, rowid, content, source, role)
                VALUES ('delete', old.id, old.content, old.source, old.role);
            END;
            CREATE TRIGGER IF NOT EXISTS messages_au AFTER UPDATE ON messages BEGIN
                INSERT INTO messages_fts(messages_fts, rowid, content, source, role)
                VALUES ('delete', old.id, old.content, old.source, old.role);
                INSERT INTO messages_fts(rowid, content, source, role)
                VALUES (new.id, new.content, new.source, new.role);
            END;
            CREATE TABLE IF NOT EXISTS snippets (
                id           INTEGER PRIMARY KEY AUTOINCREMENT,
                name         TEXT NOT NULL UNIQUE,
                message_id   INTEGER NOT NULL,
                description  TEXT,
                tags         TEXT,
                created_at   TEXT NOT NULL,
                FOREIGN KEY(message_id) REFERENCES messages(id) ON DELETE CASCADE
            );
            CREATE INDEX IF NOT EXISTS idx_snippets_created ON snippets(created_at);
            "#,
        )?;
        Ok(())
    }

    pub fn insert(&mut self, m: &Message) -> Result<bool> {
        let ts_str = m.ts.map(|t| t.to_rfc3339());
        let rows = self.conn.execute(
            "INSERT OR IGNORE INTO messages(source, ts, role, content, source_path, line_no) VALUES (?, ?, ?, ?, ?, ?)",
            rusqlite::params![m.source, ts_str, m.role, m.content, m.source_path, m.line_no],
        )?;
        Ok(rows > 0)
    }

    pub fn search(&self, query: &str, days: u32, source: Option<&str>, limit: u32) -> Result<Vec<Hit>> {
        // Escape user query for FTS5 — wrap each token in double quotes so user
        // text like "git status" is parsed as a phrase, not syntax.
        let safe_q = sanitize_fts(query);
        // snippet() columns: 0=content, 1=source, 2=role — we want column 0 (the message text)
        let mut sql = String::from(
            "SELECT m.id, m.source, m.ts, m.role, snippet(messages_fts, 0, '[', ']', '…', 32),
                   m.source_path, m.line_no
             FROM messages_fts
             JOIN messages m ON m.id = messages_fts.rowid
             WHERE messages_fts MATCH ?",
        );
        let mut args: Vec<String> = vec![safe_q];

        if source.is_some() {
            sql.push_str(" AND m.source = ?");
            args.push(source.unwrap().to_string());
        }
        if days > 0 {
            sql.push_str(" AND (m.ts IS NULL OR m.ts >= datetime('now', ?))");
            args.push(format!("-{} days", days));
        }
        sql.push_str(" ORDER BY rank LIMIT ?");
        let limit_str = limit.to_string();
        let mut stmt = self.conn.prepare(&sql)?;
        // bind args by position
        let rows = stmt.query_map(
            rusqlite::params_from_iter(args.iter().chain(std::iter::once(&limit_str))),
            |r| {
                Ok(Hit {
                    source: r.get(1)?,
                    ts: r.get::<_, Option<String>>(2)?.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
                    role: r.get(3)?,
                    snippet: r.get(4)?,
                    source_path: r.get(5)?,
                    line_no: r.get(6)?,
                })
            },
        )?;
        let mut out = Vec::new();
        for r in rows { out.push(r?); }
        Ok(out)
    }

    pub fn stats(&self) -> Result<Vec<SourceStats>> {
        let mut stmt = self.conn.prepare(
            "SELECT source, COUNT(*) AS n FROM messages GROUP BY source ORDER BY n DESC",
        )?;
        let rows = stmt.query_map([], |r| Ok(SourceStats { source: r.get(0)?, messages: r.get(1)? }))?;
        let mut out = Vec::new();
        for r in rows { out.push(r?); }
        Ok(out)
    }

    pub fn digest_days(&self, days: u32) -> Result<String> {
        let mut stmt = self.conn.prepare(
            "SELECT source, role, substr(content, 1, 240), ts FROM messages
             WHERE ts >= datetime('now', ?)
             ORDER BY ts DESC LIMIT 200",
        )?;
        let rows = stmt.query_map(params![format!("-{} days", days)], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
            ))
        })?;
        let mut out = String::new();
        out.push_str(&format!("# AI session digest — last {} day(s)\n\n", days));
        let mut count = 0i64;
        for r in rows {
            let (src, role, content, ts) = r?;
            count += 1;
            let when = ts.unwrap_or_else(|| "?".into());
            out.push_str(&format!("- **{}** [{}] {}: {}\n", when, src, role, content.replace('\n', " ")));
        }
        if count == 0 {
            out.push_str("_No messages indexed yet. Run `recall import <path>` first._\n");
        } else {
            out.insert_str(0, &format!("# AI session digest — last {} day(s) ({})\n\n", days, count));
        }
        Ok(out)
    }
}

fn sanitize_fts(q: &str) -> String {
    // For FTS5 in "query" syntax, escape " and wrap each token in quotes.
    let mut out = String::new();
    let mut in_token = false;
    for c in q.chars() {
        match c {
            '"' => { out.push_str("\"\""); }
            ' ' | '\t' | '\n' if in_token => {
                out.push('"');
                out.push(c);
                in_token = false;
            }
            ' ' | '\t' | '\n' => { out.push(c); }
            _ if !in_token => { out.push('"'); out.push(c); in_token = true; }
            _ => { out.push(c); }
        }
    }
    if in_token { out.push('"'); }
    if out.is_empty() { out.push_str("\"\""); }
    out
}

// keep the bare import quiet
#[allow(dead_code)]
fn _unused_path_buf() -> PathBuf { PathBuf::new() }

// ────────────────────────────────────────────────────────────────────────────
// Snippets (W5-8 feature)
// ────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Snippet {
    pub id: i64,
    pub name: String,
    pub message_id: i64,
    pub description: Option<String>,
    pub tags: String,
    pub created_at: String,
    pub message_content: String,
    pub message_role: String,
    pub message_source: String,
    pub message_ts: Option<DateTime<Utc>>,
}

impl Store {
    pub fn save_snippet(&mut self, name: &str, message_id: i64, description: Option<&str>, tags: &str) -> Result<Snippet> {
        let exists: bool = self.conn.query_row(
            "SELECT 1 FROM messages WHERE id = ?",
            rusqlite::params![message_id],
            |_| Ok(true),
        ).optional()?.unwrap_or(false);
        if !exists {
            anyhow::bail!("message id {} not in index (run `recall import` first?)", message_id);
        }
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO snippets(name, message_id, description, tags, created_at) VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(name) DO UPDATE SET message_id=excluded.message_id, description=excluded.description, tags=excluded.tags, created_at=excluded.created_at",
            rusqlite::params![name, message_id, description, tags, now],
        )?;
        self.get_snippet(name)?.context("just-saved snippet missing")
    }

    pub fn list_snippets(&self, tag_filter: Option<&str>) -> Result<Vec<Snippet>> {
        let mut sql = String::from(
            "SELECT s.id, s.name, s.message_id, s.description, s.tags, s.created_at,
                    m.content, m.role, m.source, m.ts
             FROM snippets s JOIN messages m ON m.id = s.message_id",
        );
        let mut args: Vec<String> = Vec::new();
        if let Some(t) = tag_filter {
            sql.push_str(" WHERE s.tags LIKE ?");
            args.push(format!("%{}%", t));
        }
        sql.push_str(" ORDER BY s.created_at DESC");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args), |r| {
            Ok(Snippet {
                id: r.get(0)?,
                name: r.get(1)?,
                message_id: r.get(2)?,
                description: r.get(3)?,
                tags: r.get(4)?,
                created_at: r.get(5)?,
                message_content: r.get(6)?,
                message_role: r.get(7)?,
                message_source: r.get(8)?,
                message_ts: r.get::<_, Option<String>>(9)?.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
            })
        })?;
        let mut out = Vec::new();
        for r in rows { out.push(r?); }
        Ok(out)
    }

    pub fn get_snippet(&self, name: &str) -> Result<Option<Snippet>> {
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.name, s.message_id, s.description, s.tags, s.created_at,
                    m.content, m.role, m.source, m.ts
             FROM snippets s JOIN messages m ON m.id = s.message_id
             WHERE s.name = ?",
        )?;
        let mut rows = stmt.query_map(rusqlite::params![name], |r| {
            Ok(Snippet {
                id: r.get(0)?,
                name: r.get(1)?,
                message_id: r.get(2)?,
                description: r.get(3)?,
                tags: r.get(4)?,
                created_at: r.get(5)?,
                message_content: r.get(6)?,
                message_role: r.get(7)?,
                message_source: r.get(8)?,
                message_ts: r.get::<_, Option<String>>(9)?.and_then(|s| DateTime::parse_from_rfc3339(&s).ok().map(|d| d.with_timezone(&Utc))),
            })
        })?;
        match rows.next() {
            Some(r) => Ok(Some(r?)),
            None => Ok(None),
        }
    }

    pub fn rm_snippet(&self, name: &str) -> Result<bool> {
        let n = self.conn.execute("DELETE FROM snippets WHERE name = ?", rusqlite::params![name])?;
        Ok(n > 0)
    }
}