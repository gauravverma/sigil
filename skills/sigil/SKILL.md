---
name: sigil
description: "Use sigil for structural code intelligence — find where a symbol is defined, who calls it, what it calls, list the names in a file, diff a PR structurally, see what breaks if you rename it. ALWAYS use this skill when: the user asks 'where is X defined' or 'who calls X' or 'what does X call' or 'what's in file F' or 'how does X fit in the codebase' or 'what would break if I change X' or 'show me the diff' or 'review this PR' or 'find duplicates' or 'what does this codebase look like', when exploring an unfamiliar repo, when you're about to chain `grep` + `read_file` to answer a structural question, or when a task matches the SWE-bench-Lite phase-1 shape of 'find the method that does X'. Prefer sigil over grep/read_file for any question about relationships (callers, callees, inheritance, rank, blast) or cross-file structural lookups. Do NOT use for pure file enumeration (use `ls`), language-specific syntactic patterns (`grep` for Rust `^pub mod`), or raw text inside a known file."
---

# sigil — Structural Code Intelligence for Agents

sigil gives you entity-level understanding of code: what exists, how it relates, what changed. It replaces multi-step grep+read_file chains with a **single call** that returns a structured answer — file, line, class, signature, overrides, callers — ready to use.

## One-shot command cheat-sheet

Questions that sigil answers in **one call**, ordered by frequency of use. Each row is a complete flow: the question, the command, the shape of the response.

| Question (what the user asks) | One-shot command | Why one-shot |
|---|---|---|
| "where is `X` defined?" | `sigil where X` | Returns file + line + class + signature + override siblings in one row. Tail-segment match: `get_default` finds both `Parameter.get_default` and `Option.get_default`. |
| "who calls `X`?" | `sigil callers X --json` | Structured caller list with file + caller-fn + line, filtered by kind. Add `--group-by file` when you want `{file: count}` distribution only. |
| "what does `X` call?" | `sigil callees X --json` | Same shape in reverse. `--group-by name` for a target-count summary. |
| "list the classes/functions/structs in `F`" | `sigil symbols F --depth 1 --names-only` | Flat JSON array of top-level names, ~300 bytes. Drops imports, nested methods, variables. |
| "full entities in `F` (with sigs + line ranges)" | `sigil symbols F --depth 1 --json` | One call returns sig + kind + line + parent for every top-level item. |
| "how does `X` fit into the codebase?" | `sigil context X --format agent` | Bundle: signature + callers + callees + related types + inheritance overrides. Budget-capped. |
| "what's in this directory structurally?" | `sigil outline --path DIR` | Hierarchical tree of classes + top-level fns grouped by file. |
| "what would break if I rename `X`?" | `sigil blast X --format agent` | Direct callers + files + transitive reach (depth 3). |
| "structural diff of this change" | `sigil diff A..B --markdown` | Entity-level change list classified as breaking / logic / formatting. |
| "review this PR" | `sigil review A..B --markdown` | `diff` + blast radius + co-change misses, rank-ordered. |
| "diff two files without git" | `sigil diff --files OLD NEW` | Any two paths, no index required for the compare itself. |
| "find duplicated function bodies" | `sigil duplicates` | Groups by BLAKE3 body hash; nothing else matches this. |
| "cold-start orientation" | `sigil map --tokens 2000` | Ranked digest in your token budget. Run this **first** in a new repo. |
| "any symbol matching 'foo'" | `sigil search foo --json` | Substring over names, sig-preview included per row, overloads collapsed. |

### Validated one-shot examples (measured)

These aren't hypothetical — they're the command/payload shapes we benchmarked against control arms using only grep + read_file. The token numbers are actual Sonnet medians on real codebases.

**Example 1 — "find the method on class `Parameter` that resolves default values when a callable is passed"** (pallets/click, E4 SWE-bench-Lite-style task)

```bash
sigil where get_default
```

Response (384 bytes):
```
get_default
  Parameter.get_default  src/click/core.py:2249-2251  (method, 3 overloads)
    def get_default(self, ctx: Context, call: bool = True) -> Any
  Option.get_default     src/click/core.py:2891-2905  (method)
    def get_default(self, ctx: Context, call: bool = True) -> Any
```

Measured: one tool call, 2 turns total including the final answer. **Control arm (grep-only): 6 turns, 12,269 tokens.** sigil: 2 turns, 5,521 tokens. **2.22× cheaper**, deterministic across seeds.

**Example 2 — "who calls `parse_file` in the sigil codebase?"**

```bash
sigil callers parse_file --kind call --json
```

Returns 128 call-site references across 12 files — including qualified forms like `crate::parser::treesitter::parse_file`. Measured on sigil itself: **control burned 80k tokens across 16 grep-narrow turns; sigil: 10k tokens, 2 turns. 14.8× cheaper for Haiku**, 3.23× cheaper for Sonnet.

**Example 3 — "list every top-level struct in `src/entity.rs`"**

```bash
sigil symbols src/entity.rs --depth 1 --names-only
# → ["Entity","BlastRadius","Reference"]
```

50 bytes instead of 900 bytes of full entity records. If you need signatures / line ranges, drop `--names-only`.

**Example 4 — "what would break if I rename `process_event`?"**

```bash
sigil blast process_event --format agent
```

One call returns direct caller count, files, transitive reach up to depth 3, and top callers by file rank. Replaces "grep for name; read every caller; recursively chase each caller's callers" — a loop that usually costs 10+ turns.

## When NOT to use sigil

sigil is structural. For these question shapes, simpler tools win:

| Question shape | Use instead | Why |
|---|---|---|
| "which files exist under dir D?" | `ls` / `find` / `bash` | Pure file enumeration; sigil's `outline` returns classes+fns, not raw file lists. |
| "text content X inside known file F" | `read_file` / `grep` | sigil indexes symbols, not string contents. |
| "lines matching regex in the repo" | `grep` | Same — text search beats AST search on raw text. |
| language-specific syntactic pattern | `grep` | e.g. Rust `^pub mod` — simpler to regex. |
| sigil returned empty AND no "Did you mean?" suggestion | `grep` | Confirm the name really doesn't exist textually before giving up. |

**Empty sigil results are data, not failure.** On an empty response sigil prints `Did you mean: X, Y, Z?` to stderr when the queried name is close to known entities. Retry with a suggestion *before* falling back to grep — that stderr line is the recovery path.

## Consuming sigil output in code

Every script-facing command (`symbols`, `children`, `callers`, `callees`, `search`) defaults to **minified JSON** with `--json`. Add `--pretty` only for human inspection.

The agent-facing commands (`map`, `context`, `review`, `blast`) accept `--format agent` for a compact token-tuned JSON. Use `--format markdown` when the *user* needs to read it.

Fields you'll get on entity JSON (from 0.4.0 onward):
- `file`, `name`, `kind`, `line_start`, `line_end`
- `parent` (skip when null), `sig` (when present), `meta` (when non-empty)
- `visibility` (skip when "private" — the default)
- `blast_radius: {direct_callers, direct_files, transitive_callers}` (skip when all zero)
- Hash columns (`struct_hash`, `body_hash`, `sig_hash`) appear only with `--with-hashes`

On `References`: `{file, caller?, name, kind, line}`. The field is `kind` (not `ref_kind`) from 0.4.0; older `.sigil/refs.jsonl` with `ref_kind` is still read via a serde alias.

## Zero-config onboarding

First query in a repo without `.sigil/` auto-runs `sigil index` and emits one stderr line — `sigil: no index at .../.sigil — running sigil index once`. That's not an error; it's a heads-up. Set `SIGIL_NO_AUTO_INDEX=1` to disable if you're bulk-scripting and want to control indexing manually.

Once the index exists, sigil uses `.sigil/cache.json` for incremental rebuilds — only touched files re-parse on subsequent `sigil index` runs.

## Full agent loop — a worked flow

"Help me understand this codebase and then refactor `handle_payment`":

```bash
# 1. Orient
sigil map --tokens 3000 --format json

# 2. Find the symbol
sigil where handle_payment
# → {file: src/checkout.rs, line: 142, class: null, sig: "fn handle_payment(...)"}

# 3. Understand its role
sigil context handle_payment --format agent
# → signature + callers + callees + related types + inheritance overrides

# 4. Quantify impact before editing
sigil blast handle_payment --format agent
# → direct_callers: 14, direct_files: 6, transitive_callers: 23

# 5. Edit the code.

# 6. Verify no unintended fan-out
sigil diff HEAD --json
# → look for unexpected MODIFIED/BREAKING entries
```

Six commands, six structured answers. Every step avoids a grep+read_file chain.

## Structural Diff (detailed)

`sigil diff` is the lower-level primitive behind `sigil review`. Use `diff` directly for raw entity-level changes without rank/blast/cochange enrichment.

```bash
sigil diff HEAD~1                       # what changed in the last commit
sigil diff main..HEAD --json            # structured (your default)
sigil diff --files old.py new.py        # compare two files directly (no git)
sigil diff HEAD~1 --lines               # show line numbers next to entity names
sigil diff HEAD~1 --context 5           # wider code snippets (default 3)
sigil diff HEAD~1 --markdown            # GitHub-flavored markdown (paste-ready)
sigil diff HEAD~1 --summary --group     # one-line summary, grouped changes
sigil diff HEAD~1 --no-callers          # skip caller analysis for breaking changes
```

Entity classifications in the output:
- **ADDED** / **REMOVED** — new or deleted entity
- **MODIFIED** — signature and/or body changed (output tells you which)
- **MOVED** — same body, different file
- **RENAMED** — different name, same body hash (detected automatically)
- **FORMATTING ONLY** — whitespace / comment-only; skip during review
- **BREAKING** — public-entity signature changed or removed

**Exit codes:** `0` on success (even with changes), `3` on error. Don't branch on the exit code for "did anything change" — read the output.

## Navigation Primitives (less common)

When the one-shot commands above don't fit, these lower-level tools remain available.

### Search

```bash
sigil search "handler" --kind function       # filter by entity kind
sigil search "build" --limit 50 --json       # more results
sigil search "config" --path "src/*.rs"      # path filter
sigil search "parse" --scope all             # broaden — default is symbol-only
```

`--scope` defaults to `symbol`. Use `file` or `all` to widen.

### Symbols + children

```bash
sigil symbols src/main.rs                    # all entities in a file
sigil symbols "src/*.rs"                     # glob patterns
sigil children src/entity.rs Entity          # children under a class/module
```

### Call graph filters

```bash
sigil callers process --kind call            # call-sites only (skip imports, types)
sigil callers build_index --kind import      # only import references
sigil callers foo --group-by file            # {file: count} aggregation
sigil callees build_index --group-by name    # what does X call most?
```

Valid `--kind` values: `call` | `import` | `type_annotation` | `instantiation`.

### Exploration

```bash
sigil explore                               # project structure
sigil explore --path src --json             # subtree, structured
```

## Advanced: Duplicates, SQL, Benchmark

### `sigil duplicates` — clone detection

```bash
sigil duplicates                            # markdown report
sigil duplicates --min-lines 10             # ignore small fragments
sigil duplicates --format json              # structured
sigil duplicates --max-group-size 20        # drop huge groups (usually generated code)
```

Groups entities by `body_hash`. Useful when the user asks "where's this copy-pasted," "is this already implemented," or during cleanup.

### `sigil query "SQL"` — DuckDB escape hatch

Ad-hoc SQL against the materialized index. Tables: `entities`, `refs`. Views: `rank`, `blast`. Shipped binaries include DuckDB — no extra setup.

```bash
sigil query "SELECT kind, COUNT(*) FROM entities GROUP BY 1 ORDER BY 2 DESC"
sigil query "SELECT name, file FROM entities WHERE visibility = 'public' AND rank > 0.01"
sigil query "SELECT * FROM refs WHERE name = 'parse_file' LIMIT 50" --format json
```

Use when the question doesn't fit the built-in commands — "every public function with no callers," "files with the most entities."

### `sigil benchmark` — token accounting

```bash
sigil benchmark                                 # bytes/4 proxy (fast)
sigil benchmark --tokenizer o200k_base          # BPE-accurate
```

### `sigil cochange` — rebuild co-change cache

```bash
sigil cochange                              # default 500 commits
sigil cochange --commits 2000               # wider history
```

Writes `.sigil/cochange.json`. `sigil review` consumes this automatically; rarely needed directly unless the cache is stale.

## Tips

- **All commands accept `-r <path>` / `--root <path>`** — run against a directory that isn't `$PWD`.
- **Script-facing commands default to unbounded results** (`--limit 0`). The old `--limit 100` default is gone — pass `--limit N` to cap explicitly.
- **`sigil callers` matches qualified-tail names** — `sigil callers parse_file` finds refs stored as `crate::parser::treesitter::parse_file`, not just bare `parse_file`.
- **`sigil context` / `sigil blast` accept qualified names** like `file.rs::name`, `Parent::name`, `file.rs::Parent::name` when the bare name is ambiguous.
- **Empty results include stderr suggestions** — `Did you mean: X, Y, Z?` when the queried name is close to something known. Use it as the recovery path before grep.
- **Shipped binaries are single-build** — `cargo install sigil` includes all 11 languages + DuckDB + tokenizer. No `--features` flags needed.
- **`.sigil/entities.jsonl` + `refs.jsonl` + `rank.json` are committable** (human-readable, diffable, small). `.sigil/index.duckdb` is derived and gitignored.
