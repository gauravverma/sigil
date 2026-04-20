---
name: sigil
description: "Use sigil for structural code diffs, codebase orientation, symbol context, PR review, and impact analysis. ALWAYS use this skill when: reviewing PRs or commits (use `sigil review` or `sigil diff`, not `git diff`), orienting yourself in an unfamiliar codebase (`sigil map`), understanding a single symbol (`sigil context <name>` — one call replaces reading 6 files), assessing change impact before refactoring (`sigil blast <name>`), finding duplicated code (`sigil duplicates`), searching for symbols/functions/classes, finding callers or callees, or running ad-hoc SQL over the index (`sigil query`). Also use when the user says things like 'what does this PR do', 'help me understand this codebase', 'what does X do', 'what calls X', 'what would break if I change X', 'where is X used', 'show me the diff', 'review this', 'find duplicates', 'what changed', 'how does X fit in', or when exploring an unfamiliar repo. Prefer sigil over Grep/Glob for any question about relationships (who calls X, what does X call, where is X used, impact radius). Do NOT use for simple file reads or text edits — only for structural code intelligence."
---

# sigil — Structural Code Intelligence for Agents

sigil gives you entity-level understanding of code: what exists, how it relates, what changed. It replaces `git diff` with structural diffs that classify changes, replaces `grep` with semantic search, and gives you purpose-built commands for orientation (`map`), symbol focus (`context`), PR review (`review`), and impact (`blast`).

## Core Principle

**Reach for the highest-level command first.** sigil ships an agent-facing layer and a primitive layer. The agent layer composes the primitives with ranking, budgeting, and caller/callee graphs already baked in — one call replaces what would otherwise be four or five grep/read loops.

| If the question is… | Use | Not |
|---|---|---|
| "What does this codebase look like?" | `sigil map` | ls + read each file |
| "What does X do / how does it fit in?" | `sigil context X` | callers + callees + read separately |
| "Review this PR / what changed on this branch?" | `sigil review <ref>` | `git diff` + guessing impact |
| "What would break if I change X?" | `sigil blast X` | scanning callers by hand |
| "Where is X called?" | `sigil callers X` | grep (misses type-only refs, overcounts strings) |
| "What does X call?" | `sigil callees X` | reading the function body and chasing names |

**Always use `--json` (or `--format agent`) when you're the one consuming the output.** JSON gives you structured fields (line ranges, token changes, rank, blast counts) that are unambiguous to parse. Use `--markdown` or plain text only when the *user* needs to read it in their terminal. Default for yourself: `sigil <cmd> --json` or `--format agent` where offered.

## The Agent Loop

Four commands cover 80% of agent-side workflows. Learn these first.

### 1. `sigil map` — cold-start orientation

Drop this in context the first time you touch an unfamiliar repo. It's a ranked, budget-aware digest: top files by PageRank, top entities per file, and auto-detected subsystems via label propagation.

```bash
sigil map --tokens 2000                 # budget-aware markdown digest (default)
sigil map --tokens 4000 --format json   # JSON for structured consumption
sigil map --focus src/auth              # boost entities under a subtree
sigil map --exclude-tests               # drop test-file entities
sigil map --write                       # also writes .sigil/SIGIL_MAP.md
```

Prefer this over "list all files then read the top ten" — it's one command and it ranks by actual import-graph centrality, not by filename.

### 2. `sigil context <symbol>` — focused symbol bundle

When the user asks about a single function/class/type, this is the right hammer. Returns signature + callers + callees + related types in a token-budgeted bundle.

```bash
sigil context parse_file                        # default markdown, 1500-token budget
sigil context parse_file --format agent         # compact JSON for LLM ingestion
sigil context parse_file --budget 3000          # bigger budget
sigil context 'entity.rs::Entity'               # qualified form for disambiguation
sigil context handle_login --depth 15           # more callers/callees per section
```

This replaces the "grep for definition, then grep for callers, then read 6 files" loop. One call, rank-ordered.

### 3. `sigil review <refspec>` — PR review artifact

Bundles structural diff + rank-ordered blast radius + co-change misses. Use this instead of `git diff` or plain `sigil diff` when the user asks to review a PR or commit range.

```bash
sigil review HEAD~1                     # last commit
sigil review main..HEAD                 # current branch
sigil review main..HEAD --format json   # structured (your default)
sigil review main..HEAD --markdown      # paste-ready for a PR comment
sigil review HEAD~3..HEAD --top-k 10    # more impact entries
sigil review HEAD~1 --no-cochange       # skip co-change pass (faster)
```

The co-change section surfaces files that *usually* change together with the files touched in the diff but *didn't* this time — often flags missed edits.

### 4. `sigil blast <symbol>` — impact summary

Before refactoring or renaming a widely-used entity, run this. Reports direct caller count, files reached, transitive reach (depth 3 by default), and the top callers by file rank.

```bash
sigil blast process_event                       # markdown
sigil blast process_event --format agent        # compact JSON
sigil blast process_event --depth 20            # show more top callers
sigil blast process_event --exclude-tests       # production callers only
```

## Structural Diff

`sigil diff` is the lower-level primitive behind `sigil review`. Use `diff` directly when you need raw entity-level changes without the rank/blast/cochange enrichment.

```bash
sigil diff HEAD~1                       # what changed in the last commit
sigil diff main..HEAD                   # what changed on this branch
sigil diff main..HEAD --json            # structured JSON (your default)
sigil diff abc123..def456 --verbose     # between arbitrary refs, with progress
sigil diff --files old.py new.py        # compare two files directly (no git)
sigil diff HEAD~1 --lines               # show line numbers next to entity names
sigil diff HEAD~1 --context 5           # wider code snippets (default 3)
sigil diff HEAD~1 --no-context          # entity-only, no snippets
sigil diff HEAD~1 --markdown            # GitHub-flavored markdown
sigil diff HEAD~1 --markdown --no-emoji # ASCII-only markdown
sigil diff HEAD~1 --summary --group     # one-line summary, grouped changes
sigil diff HEAD~1 --no-callers          # skip caller analysis for breaking changes
```

**Exit codes:** `0` on success (even when changes are present), `3` on error. Don't rely on non-zero as a signal that *something changed* — sigil distinguishes change types inside the output, not in the exit code.

**Entity classifications in the output:**
- **ADDED** / **REMOVED** — new or deleted entity
- **MODIFIED** — signature and/or body changed (output tells you which)
- **MOVED** — same body, different file
- **RENAMED** — different name, same body hash (detected automatically)
- **FORMATTING ONLY** — whitespace / comment-only changes, usually skip during review
- **BREAKING** — public-entity signature changed or removed

JSON output includes: entity name, file, line range, `struct_hash` / `body_hash` / `sig_hash`, and for breaking changes, the list of callers.

## Navigation Primitives

The script-facing layer. Use these when the agent-facing commands don't fit — e.g., answering a scripted "list every caller of X" rather than a natural-language "what does X do."

### Search

```bash
sigil search "parse_file"                    # everything — symbols, files, text
sigil search "MyClass" --scope symbols       # symbols only
sigil search "handler" --kind function       # filter by entity kind
sigil search "build" --limit 50 --json       # more results, JSON
sigil search "config" --path "src/*.rs"      # path filter
```

### Symbol layout

```bash
sigil symbols src/main.rs                    # all symbols in a file
sigil symbols "src/*.rs"                     # glob patterns
sigil children src/entity.rs Entity          # children of a class/module
```

### Call graph

```bash
sigil callers struct_hash                              # who references this symbol?
sigil callers process --kind call                      # calls only (skip imports, types)
sigil callers build_index --kind import                # only import references
sigil callees build_index                              # what does this function call?
```

Valid `--kind` values: `call` | `import` | `type_annotation` | `instantiation` | `definition`.

### Exploration

```bash
sigil explore                               # project structure
sigil explore --path src                    # filter to subtree
sigil explore --json                        # structured output
```

## Advanced: Duplicates, SQL, Benchmark

### `sigil duplicates` — clone detection

Groups entities by `body_hash` — literal duplicate implementations anywhere in the repo.

```bash
sigil duplicates                            # markdown report
sigil duplicates --min-lines 10             # ignore small fragments
sigil duplicates --format json              # structured
sigil duplicates --max-group-size 20        # drop huge groups (usually generated code)
```

Useful when the user asks "where's this copy-pasted," "is this already implemented somewhere," or during cleanup/refactor tasks.

### `sigil query "SQL"` — DuckDB escape hatch

Ad-hoc SQL against the materialized index. Tables: `entities`, `refs`. Views: `rank`, `blast`. Shipped binaries include the DuckDB backend; no extra setup.

```bash
sigil query "SELECT kind, COUNT(*) FROM entities GROUP BY 1 ORDER BY 2 DESC"
sigil query "SELECT name, file FROM entities WHERE visibility = 'public' AND rank > 0.01"
sigil query "SELECT * FROM refs WHERE target_name = 'parse_file' LIMIT 50" --format json
```

Use this when the user asks a question that doesn't fit the built-in commands — "show me every public function with no callers," "which files have the most entities," etc.

### `sigil benchmark` — token accounting

Publishes median token reduction of sigil commands vs raw alternatives (git log, git diff, ls + read).

```bash
sigil benchmark                                 # bytes/4 proxy (fast)
sigil benchmark --tokenizer o200k_base          # BPE-accurate (o200k / cl100k / p50k)
```

Useful only when the user explicitly asks about token efficiency.

### `sigil cochange` — rebuild co-change cache

Reads `git log --name-only` and weights file pairs that change together. Writes to `.sigil/cochange.json`. `sigil review` consumes this automatically — you rarely need to run it yourself unless the cache is stale or the repo has new history.

```bash
sigil cochange                              # default 500 commits
sigil cochange --commits 2000               # wider history
```

## Indexing

sigil reads from `.sigil/` (JSONL + rank.json + optional DuckDB materialization). The index must exist before search/navigation commands work.

```bash
sigil index                                 # incremental rebuild
sigil index -v                              # with progress
sigil index --full                          # force full reparse
sigil index --no-rank                       # skip PageRank + blast-radius pass
```

**Most agents don't need to invoke this directly.** When the `sigil hook` integration is installed, a git post-commit hook rebuilds `.sigil/` automatically after each commit. Check for `.sigil/entities.jsonl` before running anything that reads the index — if present, it's ready.

## Agent-facing Workflows

### Orienting in an unfamiliar repo

```bash
sigil map --tokens 3000 --format json       # what does this codebase look like?
sigil context <interesting-name> --format agent  # drill into a specific piece
```

Skip the "list files, read README, read a few source files" loop — the map already encodes import-graph centrality.

### Reviewing a PR

```bash
sigil review main..HEAD --format json       # your primary artifact
# If the user wants a pasteable comment:
sigil review main..HEAD --markdown
# For a deep dive on a specific modified function:
sigil blast <modified_name> --format agent
```

`review` already includes co-change misses, so entities the diff *should* have touched but didn't are surfaced for you.

### Understanding a symbol

```bash
sigil context <name> --format agent         # one call, done
# Fallback if context isn't enough or the symbol is ambiguous:
sigil search <name> --scope symbols         # disambiguate
sigil callers <name> --kind call --json     # raw call sites
sigil callees <name> --kind call --json
```

### Planning a rename or refactor

```bash
sigil blast <name> --format agent           # impact scope
sigil callers <name> --kind call --json     # exact call sites to edit
sigil duplicates --min-lines 5              # any copies to consolidate?
```

### Verifying your own edits

After you've modified code:

```bash
sigil diff HEAD --json                      # what changed (uncommitted)
sigil diff HEAD~1 --json                    # what changed (committed)
```

Look for unexpected MODIFIED or BREAKING entries — often catches edits that rippled further than intended.

## When to use sigil vs Grep/Glob/Read

| Question type | Use sigil | Use Grep/Glob/Read |
|---|---|---|
| Codebase orientation | `sigil map` | No — too coarse |
| Symbol impact / call graph | `sigil callers` / `blast` / `context` | No — grep misses type-only references and overcounts string hits |
| PR review | `sigil review` / `sigil diff` | No — `git diff` is line-level noise |
| Compare two specific files | `sigil diff --files` | No — unstructured |
| Cross-file duplication | `sigil duplicates` | No |
| Ad-hoc analytics ("public fns with no callers") | `sigil query` | No |
| Symbol location | `sigil search --scope symbols` | Grep `^fn X` is also fine |
| Free-text search in comments/strings | `sigil search` or `Grep` | Either |
| File discovery by name | — | `Glob "**/*.go"` |
| Read a specific file | — | `Read` |

**Rule of thumb:** Any question about *relationships* (callers, callees, impact, rank, cochange) → sigil. Any question that's pure text or filename matching → Grep / Glob. Any question that's "what does this codebase look like" or "what does this symbol do" or "what does this PR do" → the agent-facing sigil command (`map` / `context` / `review`), not a collection of lower-level calls.

## Tips

- **All commands accept `-r <path>` / `--root <path>`** for running against a directory that isn't `$PWD`.
- **All agent-facing commands offer `--format agent`** — compact JSON tuned for LLM ingestion. Prefer it over `--format markdown` when you're consuming the output yourself.
- **`sigil diff` always exits 0 on success.** Don't branch on the exit code for "did anything change"; read the output instead. Exit 3 is error-only.
- **Entity JSON includes `rank`, `blast_radius`, `visibility`** — entities from v0.3.0+ carry these fields, so you can order results or filter by importance without a separate call.
- **`sigil context` / `sigil blast` accept qualified names** like `file.rs::name`, `Parent::name`, `file.rs::Parent::name` when a bare name is ambiguous.
- **`sigil review` skips co-change analysis** with `--no-cochange` if the git history is very deep and you only need the diff+blast portion.
- **Shipped binaries include the DuckDB backend and BPE tokenizer** — no feature flags or extra installs needed. The backend auto-engages above ~5 MB of `.sigil/` JSONL (tunable via `SIGIL_AUTO_ENGAGE_THRESHOLD_MB`).
- **`sigil` uses `.sigil/cache.json` for incremental indexing** — re-runs of `sigil index` only reparse changed files. Safe to commit `.sigil/entities.jsonl` + `refs.jsonl` + `rank.json` for teammates / CI agents; `.sigil/index.duckdb` is derived and gitignored.
