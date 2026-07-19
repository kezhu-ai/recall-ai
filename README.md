# recall-ai

> **Search every AI conversation you've ever had, locally.**
> Single 1.1 MB Rust binary + SQLite FTS5 index. ChatGPT export · Claude export · Claude Code · Codex CLI.

```
$ recall search "ctxguard" --days 365
3 match(es) for "ctxguard":

| when        | source      | role      | snippet                                                                                  | path:line |
|-------------|-------------|-----------|-------------------------------------------------------------------------------------------|-----------|
| 07-18 22:53 | claude-code | assistant | ★ Insight ───────────────────────────────────── …                                     | …jsonl:1453 |
| 07-18 23:44 | claude-code | assistant | The cwd: /home/runner/work/ctxguard/ctxguard/ctxguard does not exist! — 路径有 3 层 ctxguard | …jsonl:1896 |
```

```
$ recall today
# AI session digest — last 1 day(s) (14)
- 2026-07-19T07:18:56Z [claude-code] assistant: ★ Insight ─────────…
- 2026-07-19T07:18:48Z [claude-code] assistant: GitHub 端 3 步全完成 ✅…
```

```
$ recall serve
[recall-web] http://127.0.0.1:7777/  (Ctrl-C to stop)
```

## Why

ChatGPT just made "search across chats" a top-bar tab. Claude added cross-session search. Cursor devs openly say tracking past windows still eats their brain. **The pain is real, the existing fixes are all SaaS, and there's still no local-first way to search across every AI tool you use.**

`recall` is that missing layer. Import once, search forever, in milliseconds.

## What's supported (v0.1)

| Source | How to import |
|---|---|
| **ChatGPT export** | `recall import ~/Downloads/chatgpt-export.zip` |
| **Claude export** | `recall import ~/Downloads/claude-export.zip` |
| **Claude Code local sessions** | `recall import ~/.claude/projects/` |
| **Codex CLI local sessions** | `recall import ~/.codex/archived_sessions/` |

## Install

```bash
cargo install recall-ai
# or grab a binary from GitHub Releases
```

## Usage

```bash
recall import <path>     # .zip (ChatGPT / Claude export) or a session dir
recall search <query>    # FTS5 substring search, ranked, source filterable
recall today             # markdown digest of today's activity
recall week              # markdown digest of the last 7 days
recall stats             # counts by source
recall open <id>         # print path:line for a hit
recall serve             # tiny localhost web UI on :7777
```

Data dir: `$XDG_DATA_HOME/recall` (default `~/.local/share/recall`).
DB file: `<data-dir>/recall.db` (SQLite + FTS5).

## Design choices

- **SQLite + FTS5** for sub-100ms search and zero-server.
- **Append-only, idempotent** — re-importing the same export is a no-op (`UNIQUE(source_path, line_no)`).
- **Local-first, single binary** — no Docker, no daemon, no cloud account.
- **No embeddings yet** — BM25 in FTS5 is enough for the 80% case. v0.5 may add an embedding index if the corpus grows past 1M messages.
- **Tiny localhost web UI** — stdlib `TcpListener` + a 60-line HTML page. No JS framework, no Tauri, no SPA.

## Non-goals (so we don't drift)

- ❌ No cloud sync, no SaaS, no team mode.
- ❌ No agent framework, no "AI memory platform" (we collide with mem0 / OpenMemory).
- ❌ No 10+ provider support in MVP — four sources is enough to prove the thesis.

## Benchmarks (preliminary)

| op | time | memory |
|---|---|---|
| `recall import` 6,826 messages (Claude Code) | ~4 s | ~30 MB |
| `recall search` (warm cache, ~6000 msgs) | <50 ms | <20 MB |
| `recall serve` localhost web UI | ~5 ms / req | ~25 MB |

## Roadmap (per ChatGPT strategic review)

- [x] **v0.1** — SQLite FTS5 index + 4 importers + CLI search + localhost UI
- [ ] **v0.2** — `recall snippets` save / star / bookmark
- [ ] **v0.3** — `recall recap --week` weekly digest for sharing
- [ ] **v0.4** — redact-on-export for safe sharing of digests
- [ ] **v0.5** — optional embedding index for semantic search

## Author

Made by [@kezhu-ai](https://github.com/kezhu-ai) — also the author of [ctxguard](https://github.com/kezhu-ai/ctxguard) (context-window budget for AI agents) and [mcp-sentry](https://github.com/kezhu-ai/mcp-sentry) (policy-as-code firewall for MCP servers).

## License

MIT OR Apache-2.0