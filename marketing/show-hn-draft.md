# Show HN draft — recall-ai

> 复制下面任一标题 + body 到 https://news.ycombinator.com/submit

---

## 标题候选 (4 个)

**A** — Show HN: recall-ai – search every AI conversation you've ever had, locally

**B** — I imported 6,826 of my Claude Code conversations and found the question I'd asked 3 weeks ago

**C** — Show HN: recall-ai – Spotlight for your AI history (Rust + SQLite FTS5, no cloud)

**D** — Show HN: recall-ai – local-first index of ChatGPT, Claude, Claude Code, and Codex history

---

## Body (用 B 标题 + A 体裁)

```text
Hi HN,

I built recall-ai because I lost a SQL query I'd asked Claude Code to write three weeks ago and gave up finding it.

What it does:

  $ recall import ~/.claude/projects/        # 6,826 messages indexed in ~4 s
  $ recall search "ctxguard" --days 365
  3 match(es) for "ctxguard":

  | when        | source      | role      | snippet                                              | path:line |
  | 07-18 22:53 | claude-code | assistant | ★ Insight ──── gh repo view zhuke-ai/[ctxguard]...   | ...jsonl:1453 |
  | 07-18 23:44 | claude-code | assistant | The cwd: /home/runner/work/ctxguard/ctxguard/...      | ...jsonl:1896 |

  $ recall today              # markdown digest, ready to share on Slack
  $ recall serve              # localhost web UI on :7777

Why it matters:

  - ChatGPT just made "search across chats" a top-bar tab.
  - Claude added cross-session search last month.
  - Cursor devs openly say tracking past windows eats their brain.

  All existing fixes are SaaS, locked to one vendor, or both. recall-ai
  is local-first: single 1.1 MB Rust binary + SQLite FTS5, your data
  never leaves the machine.

What works today (v0.1):

  - ChatGPT export (.zip)
  - Claude export (.zip)
  - Claude Code local sessions (~/.claude/projects/*/[0-9a-f]*.jsonl)
  - Codex CLI local sessions (~/.codex/archived_sessions/rollout-*.jsonl)

  Sub-100 ms search across 6k+ messages. Markdown digest. Tiny
  localhost web UI. Zero cloud, zero account.

Why I chose this stack:

  - SQLite FTS5 gives BM25 + prefix match in <50 ms — enough for the 80% case.
  - rusqlite bundles SQLite so the binary stays single-file.
  - Rust 1.94 + clap = 1.1 MB with no runtime deps.

What it doesn't do (yet):

  - No embeddings (semantic search). Coming if corpus > 1M messages.
  - No 10+ provider support. MVP is four sources.
  - No cloud sync, no team mode, no SaaS.

Why I think this can hit 10k stars:

  - The audience is "anyone who uses ChatGPT or Claude Code" — millions,
    not "developers who write CLIs" — hundreds of thousands.
  - ChatGPT/Claude have shipped official search but it's per-vendor and
    cloud-only. Cross-tool, local-first search is the missing layer.
  - Similar shell-history tools (Atuin, 30.6k★) show the appetite.

Install:

  cargo install recall-ai

Source: https://github.com/kezhu-ai/recall-ai
License: MIT OR Apache-2.0

Happy to take feedback from anyone who has a ChatGPT export lying around.
```

## Submit instructions

1. Open https://news.ycombinator.com/submit
2. Paste title (A/B/C/D — recommend B for the story hook)
3. Paste body
4. URL: https://github.com/kezhu-ai/recall-ai
5. Best time: Tuesday-Thursday 8-10 am US Eastern (HN peak)
6. If it doesn't make front page in 24 h, repost Saturday morning with a tweaked title.

## Self-assessment (don't post this)

- Hook strength: medium-high (the "lost a SQL query" story is relatable)
- Novelty: medium (cross-tool + local-first is the wedge)
- Audience: high (any AI user, not just devs)
- Defensibility: medium (any vendor could ship this; first-mover matters)
- 10k probability: ~15-20% (realistic for solo dev, $0 budget, 4-12 week runway)

## Anti-patterns to avoid in launch

- ❌ Don't say "AI memory platform" (collides with mem0/OpenMemory)
- ❌ Don't claim it's a SaaS — it's local-first or nothing
- ❌ Don't bury the demo gif — make it the first link
- ❌ Don't promise embeddings/semantic search in v0.1 (it's literally not there)
- ❌ Don't compare to LangChain/MemGPT — wrong category, confusing

## Cross-posting plan (after HN)

- r/ClaudeAI: "I built a local search across all my Claude Code sessions"
- r/LocalLLaMA: same angle, AI users
- r/ChatGPT: "I built a local index of my ChatGPT export"
- r/commandline: "Rust + SQLite FTS5, single 1.1 MB binary"
- V2EX (中文): "把 ChatGPT / Claude / Claude Code 的对话历史做成本地可搜的工具"
- 掘金 (中文): tech article, "1.1 MB Rust + SQLite FTS5 打造 AI 对话本地搜索引擎"
- X thread: 6 screenshots, one per feature, last one = "star + fork"