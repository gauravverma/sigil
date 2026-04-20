# sigil

**Deterministic structural code intelligence for AI coding agents — and humans.**

sigil cuts the orientation tax AI coding agents pay on every new repo. Instead of grepping through a codebase line by line, agents ask sigil: _"who calls this?"_, _"what's in this file?"_, _"what changed in this PR?"_ — and get back structured answers from a parsed AST index, not text matches.

No LLM in the code path. No embeddings. No cloud. Just tree-sitter + BLAKE3 + PageRank.

```
Median: 35× fewer tokens per agent query on sigil's own source.
Peak:   252× on "focused context for one symbol".
```

Measured with the GPT-4o/o3 BPE tokenizer. Reproduce with `sigil benchmark` on your own repo.

---

## What it does

**For AI agents** — drop-in commands that fit an agent's context window:
- `sigil map` — ranked codebase digest, budget-aware. Cold-start orientation in one tool call.
- `sigil context <symbol>` — signature + callers + callees + related types, in ~500 tokens.
- `sigil review A..B` — PR review: structural diff + blast radius + co-change misses. Replaces `git diff` for review.

**For humans** — fast, precise code navigation that grep can't match:
- `sigil callers <symbol>` — exact reference sites from the parsed AST (not every string match).
- `sigil blast <symbol>` — impact summary: how many files depend on this, how far it propagates.
- `sigil duplicates` — clone report across the codebase (free — sigil already hashes entity bodies).

**For scripts & CI** — JSON output, pipe-composable, deterministic:
- `sigil diff main..HEAD --json` for structural PR checks in GitHub Actions.
- `sigil query "SELECT ..."` for ad-hoc SQL over the materialized index (`--features db`).

---

## Install

### Option 1 — lean default (recommended for most users)

One-liner installer (macOS / Linux):

```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/gauravverma/sigil/releases/latest/download/sigil-installer.sh | sh
```

Or via Cargo:

```bash
cargo install --git https://github.com/gauravverma/sigil
```

Lean binary (~20 MB). Covers `sigil index` / `diff` / `map` / `context` / `review` / `blast` / and every query command. All 8 platform installers included. Fast on small-to-medium repos.

### Option 2 — full build for monorepo scale

```bash
cargo install --git https://github.com/gauravverma/sigil --features db,tokenizer
```

~70 MB binary. Adds:
- **DuckDB backend** — persistent materialized index for codebases above ~5 MB of JSONL. Auto-engages; you don't have to ask. Critical if you're working on fastapi / zod / Linux-kernel-scale codebases.
- **BPE-accurate tokenizer** — `sigil benchmark --tokenizer o200k_base` publishes honest token counts instead of a bytes/4 proxy. Matters if you're citing numbers.
- **`sigil query 'SQL'`** — run arbitrary SQL against the materialized index.

Requires a working C++17 toolchain to compile (Xcode CLT / `build-essential` / MSVC).

### Python bindings

```bash
pip install sigil-diff
```

```python
import sigil

result = sigil.diff_json(old_json_str, new_json_str)
result = sigil.diff_files("old.py", "new.py")
result = sigil.diff_refs(".", "HEAD~1", "HEAD")
```

See [python/README.md](python/README.md) for full API.

---

## 5-minute tour

Clone this repo as the demo corpus:

```bash
git clone https://github.com/gauravverma/sigil && cd sigil
```

### 1. Index

```bash
sigil index
```

~2 seconds on sigil itself. Creates `.sigil/entities.jsonl`, `.sigil/refs.jsonl`, `.sigil/rank.json`. Incremental on re-runs — only touched files re-parse.

### 2. Cold-start orientation

```bash
sigil map --tokens 2000
```

Top files by PageRank over the import graph, top symbols per file ranked by blast radius. The artifact to hand an agent when it first enters your repo.

```
# Sigil Map
100 files, 2985 entities, 12963 refs · sigil 0.2.4

## Subsystems (7)
- **src/parser** (#20) — 12 file(s): src/parser/helpers.rs, src/parser/format.rs, ...
- **src/install** (#42) — 8 file(s): src/install/claude.rs, src/install/codex.rs, ...
- **src/query** (#10) — 6 file(s): src/query/index.rs, src/query/mod.rs, ...
...

## Top files by impact

### src/entity.rs — rank 0.0798 (rust, subsystem #10)
- struct **Entity** [public] — blast 15f/45c/192t
  `pub struct Entity`
- struct **Reference** [public] — blast 9f/24c/101t
...
```

### 3. Focused context on one symbol

```bash
sigil context Entity --budget 1000
```

```
# `Entity`

**struct** in `src/entity.rs`:7-35 · public · blast 15f/45c/192t

## Signature
`pub struct Entity`

## Callers (4)
- `is_public` _type_annotation_ `src/classifier.rs:116`
- `match_classify_enrich` _type_annotation_ `src/diff.rs:220`
- `EntityDiff` _type_annotation_ `src/diff_json.rs:36`
- _+40 more truncated by budget_

## Related types (4)
- `Entity` → `String` _type_annotation_ `src/entity.rs:8`
- `Entity` → `Option` _type_annotation_ `src/entity.rs:14`
- _+16 more_
```

~350 tokens — the minimum-viable context for editing `Entity`, or for answering "what is this thing?"

### 4. PR review

```bash
sigil review HEAD~3..HEAD
```

Replaces `git diff` for review. Entity-level changes, ranked by impact, with blast radius per entity and co-change misses. Committable as a review artifact.

### 5. Navigation queries

```bash
sigil callers Entity             # exact reference sites
sigil callees build_index        # what a function depends on
sigil symbols src/entity.rs      # what's in a file
sigil search parse --scope symbols
sigil blast Entity --depth 5     # impact summary
```

### 6. Run the benchmark on your repo

```bash
sigil benchmark --refspec HEAD~3..HEAD
# with the full build:
sigil benchmark --refspec HEAD~3..HEAD --tokenizer o200k_base
```

Prints a per-query table: bytes grep would produce vs bytes sigil produces, plus the median reduction ratio.

---

## Install into your AI agent

Each installer writes a capability-describing block (what sigil does, when each command fits) — never a preference statement ("use sigil instead of grep"). Agents discover the tool on the same terms they'd discover any built-in command.

```bash
sigil claude install     # CLAUDE.md + .claude/settings.json PreToolUse hook
sigil cursor install     # .cursor/rules/sigil.mdc (alwaysApply: true)
sigil codex install      # AGENTS.md + .codex/hooks.json Bash hook
sigil gemini install     # GEMINI.md + .gemini/settings.json BeforeTool hook
sigil opencode install   # AGENTS.md + .opencode/plugins/sigil.js
sigil aider install      # AGENTS.md block
sigil copilot install    # ~/.copilot/skills/sigil/SKILL.md
sigil hook install       # git post-commit + post-checkout auto-rebuild
```

Each has a matching `uninstall`. Every installer is idempotent (rerunning with same content is a no-op), preserves user content outside sigil's marker block, and leaves sibling user hooks / rules / plugins untouched.

**`git sigil <cmd>` alias** — piggyback on git's pretrained name recognition so agents that know `git diff` naturally discover `git sigil`:

```bash
ln -s "$(which sigil)" /usr/local/bin/git-sigil
# now: git sigil map / git sigil review / git sigil context — all work
```

---

## Benchmarks

### Multi-language test (full binary, threshold=5 MB)

One OSS repo per language, 3 query shapes each, 3-run median wall-clock. Sigil vs `git grep`. Full writeup in [evals/results/multilang-with-db-2026-04-20.md](evals/results/multilang-with-db-2026-04-20.md).

| Repo | Lang | Entities | Init | Best sigil win |
|---|---|---:|---:|---|
| cobra | Go | 1.6k | 235 ms | 15.9× more compact, 1.5× faster |
| ripgrep | Rust | 5.6k | 1.66 s | 9.2× more compact, ≈ tied on time |
| zod | TypeScript | 30.8k | 9.9 s | **69.5× more compact**, 1.8× faster |
| fastapi | Python | 118.6k | 2.83 s | 16.7× more compact, **6× faster** |

- **Compactness**: sigil consistently 5–70× smaller than grep output because the parsed reference table skips docstrings, comments, string literals, and type annotations that grep matches.
- **Speed**: small repos, sigil is ~1.5× faster; large repos (with DuckDB auto-engage), **sigil beats grep by 2–6×**. The sweet spot is anything with ≥5 MB of `.sigil/*.jsonl`.
- **Semantic gap**: `git grep` returns **0 lines** on "what's in this file?" queries across all 4 languages — regex can't match Rust multi-line impls, Python indented methods, TS `export const foo = ...`, or Go receiver methods. sigil's AST-based extraction handles each trivially.

### Self-benchmark (sigil on sigil)

| Query | grep tokens | sigil tokens | Ratio |
|---|---:|---:|---:|
| PR review (3 commits) | 195,003 | 5,572 | **35×** |
| Context for `Entity` | 91,937 | 467 | **196×** |
| Cold-start orientation | 44,733 | 2,786 | **16×** |

Median: **35×**. BPE-accurate counts via `o200k_base`. Raw JSON at [evals/results/0.2.4-HEAD-3..HEAD-o200k.json](evals/results/0.2.4-HEAD-3..HEAD-o200k.json).

---

## How it works

```
                  ┌──────────────────────────────────────────┐
                  │  source files (.rs .py .ts .go …)        │
                  └──────────────────┬───────────────────────┘
                                     │ tree-sitter (11 languages)
                                     ▼
                  ┌──────────────────────────────────────────┐
                  │  Entity  — struct/fn/class with 3 BLAKE3 │
                  │  Reference — call / import / type_annot  │
                  └──────┬──────────────────────┬────────────┘
                         │                      │
              ┌──────────▼──┐           ┌──────▼────────────┐
              │ entities.jsonl│         │ refs.jsonl        │
              └──────┬────────┘         └──────┬────────────┘
                     │                         │
              ┌──────▼───────────┐             │
              │ PageRank + blast │◄────────────┘
              │ rank.json        │
              └──────┬───────────┘
                     │
   ┌─────────────────┼─────────────────────────────┐
   ▼                 ▼                             ▼
 in-memory        DuckDB-backed               sigil diff
 HashMap Index    (feature = "db")            (structural match + classify)
 (small repos)    (≥5 MB JSONL)
```

1. **tree-sitter parser** extracts entities (functions, structs, classes, types, imports) with line ranges. 11 languages ship; feature-gated so lean builds can drop unused grammars.
2. **BLAKE3 hashes** per entity — `struct_hash` (raw), `body_hash` (normalized, ignores whitespace), `sig_hash` (signature only). Powers classify: formatting-only vs logic-change vs API-change.
3. **Reference table** — call / import / type_annotation / instantiation / definition rows, linking caller → target.
4. **PageRank** over the file import graph ranks which files are load-bearing. **Blast radius** per entity = BFS over the reverse-reference graph, capped at depth 3.
5. **Two backends** behind a single router:
   - **In-memory** `HashMap<String, Vec<usize>>` lookups. Sub-20 ms queries, zero dependencies.
   - **DuckDB** persistent store, columnar + vectorized. Auto-engages above 5 MB of JSONL. Handles monorepo scale without re-parsing on every invocation.

The two backends serve identical APIs; the router picks based on index size or `SIGIL_BACKEND` env var. Users never think about it.

**On-disk** (per repo):
```
.sigil/
  entities.jsonl       ← one entity per line; source of truth, committable
  refs.jsonl           ← one reference per line
  rank.json            ← PageRank + blast radius
  cache.json           ← per-file BLAKE3 hashes for incremental re-indexing
  SIGIL_MAP.md         ← optional — `sigil map --write` artifact for agents
  index.duckdb         ← derived, gitignored, built lazily on first SQL query
```

---

## Supported languages

Tree-sitter grammars ship as cargo features. Default build includes all 11:

| Language | Extensions |
|---|---|
| Python | `.py` `.pyi` `.pyw` |
| Rust | `.rs` |
| JavaScript | `.js` `.mjs` `.cjs` `.jsx` |
| TypeScript | `.ts` `.mts` `.cts` `.tsx` |
| Go | `.go` |
| Java | `.java` |
| C / C++ | `.c` `.h` `.cpp` `.cc` `.cxx` `.hpp` `.hxx` |
| Ruby | `.rb` `.rake` `.gemspec` |
| C# | `.cs` |
| Markdown | `.md` `.markdown` |

Plus four sigil-native parsers for data formats: **JSON**, **YAML**, **TOML**, with structural diff (e.g., `"port": 8080 → 8443` detected, not just "line 14 changed").

---

## Command reference

### Agent-facing (narrated, budget-aware)

| Command | What it does |
|---|---|
| `sigil map [--tokens N] [--focus PATH] [--exclude-tests]` | Ranked codebase digest. Pack N tokens of highest-impact orientation into one markdown artifact. |
| `sigil context <symbol> [--budget N] [--format agent\|markdown\|json]` | Focused bundle for one symbol: signature + callers + callees + related types. |
| `sigil review <refspec> [--markdown\|--json]` | PR review: structural diff + blast radius + co-change misses. |
| `sigil blast <symbol> [--depth N]` | Impact summary: direct callers, files, transitive reach. |
| `sigil benchmark [--tokenizer o200k_base]` | Publishes a median token-reduction number for your repo. |

### Script-facing (raw, unbounded, JSON-friendly)

| Command | What it does |
|---|---|
| `sigil search <q> [--scope symbol\|file\|all]` | Substring search over symbols + file paths. |
| `sigil symbols <file>` | All entities in a file. |
| `sigil children <file> <parent>` | Entities under a class / module. |
| `sigil callers <symbol> [--kind call\|import\|...]` | All references targeting a symbol. |
| `sigil callees <caller>` | What a symbol calls. |
| `sigil explore [--path PATH]` | Directory overview with file counts by language. |
| `sigil duplicates [--min-lines N]` | Clone report across the codebase. |
| `sigil cochange [--commits N]` | Mine git history for file-pair co-change weights. |

### Admin & data pipeline

| Command | What it does |
|---|---|
| `sigil index [--full] [--no-rank]` | Build / refresh the `.sigil/` index. Incremental by default. |
| `sigil diff <refspec> [--json\|--markdown]` | Structural diff between two git refs or two files. |
| `sigil query "SQL"` | Ad-hoc SQL against the materialized DuckDB index (full build only). |

### Integrations

| Command | What it writes |
|---|---|
| `sigil claude install` | `CLAUDE.md` block + `.claude/settings.json` PreToolUse hook |
| `sigil cursor install` | `.cursor/rules/sigil.mdc` (alwaysApply: true) |
| `sigil codex install` | `AGENTS.md` + `.codex/hooks.json` |
| `sigil gemini install` | `GEMINI.md` + `.gemini/settings.json` BeforeTool |
| `sigil opencode install` | `AGENTS.md` + `.opencode/plugins/sigil.js` |
| `sigil aider install` | `AGENTS.md` block |
| `sigil copilot install` | `~/.copilot/skills/sigil/SKILL.md` |
| `sigil hook install` | `.git/hooks/post-commit` + `post-checkout` auto-rebuild |

Every integration has `sigil <name> uninstall`. All are idempotent and content-preserving.

---

## Backend selection

When `--features db` is compiled in, the router picks a backend per query:

1. `SIGIL_BACKEND=memory` → force in-memory.
2. `SIGIL_BACKEND=db` → force DuckDB (fails loudly if feature wasn't compiled).
3. Otherwise, auto-engage DuckDB when total `.sigil/*.jsonl` size ≥ `SIGIL_AUTO_ENGAGE_THRESHOLD_MB` (default 5 MB).
4. Fall back to in-memory.

Unknown `SIGIL_BACKEND` values are a hard error — no silent fallbacks. Reproducibility > convenience.

---

## CI / CD example

```yaml
# .github/workflows/review.yml
- name: sigil structural diff
  run: |
    curl -LsSf https://github.com/gauravverma/sigil/releases/latest/download/sigil-installer.sh | sh
    sigil index
    sigil review origin/main..HEAD --markdown > review.md

- name: Comment on PR
  uses: actions/github-script@v7
  with:
    script: |
      const fs = require('fs');
      const body = fs.readFileSync('review.md', 'utf8');
      github.rest.issues.createComment({
        issue_number: context.issue.number,
        owner: context.repo.owner,
        repo: context.repo.repo,
        body,
      });

- name: Block breaking changes without label
  run: |
    if sigil diff origin/main..HEAD --json | jq -e '.summary.has_breaking'; then
      gh pr view ${{ github.event.number }} --json labels | \
        jq -e '.labels[] | select(.name == "breaking-change")' || \
        (echo "breaking changes require the 'breaking-change' label"; exit 1)
    fi
```

---

## Honest caveats

- **sigil needs `sigil index` first.** ~200 ms on tiny repos; ~3 s on fastapi-size (2,500 files); ~10 s on TypeScript-heavy codebases (zod). One-time cost per session; `sigil hook install` amortizes via git hooks.
- **Output is precise, not exhaustive by default.** `sigil map --tokens 4000` hits its budget and truncates; `sigil context` hits a depth cap. The script-facing commands (`callers`, `symbols`, `children`) are unbounded — use those when you need every row.
- **No semantic inference.** sigil tells you who calls what and what changed structurally. It doesn't tell you "this function implements the observer pattern" or "this has a race condition." Those need an LLM — sigil feeds one, it doesn't replace one.
- **Tree-sitter parsing isn't 100%.** Some language edge cases (Rust macros, Python dynamic imports, TS complex generics) don't extract cleanly. The 4 data parsers (JSON/YAML/TOML/Markdown) are sigil-native and handle edge cases that tree-sitter grammars don't.
- **Small-repo performance:** the lean default build is fastest on small repos. The full build (`--features db`) adds ~10 ms of startup overhead per invocation from the bigger binary. For monorepo-scale use, that's noise; for one-shot scripts on tiny repos, stick with the lean build.

---

## FAQ

**Q: Do I need to commit `.sigil/` to git?**
Depends. `.sigil/entities.jsonl` + `refs.jsonl` + `rank.json` + `SIGIL_MAP.md` are committable (human-readable, diffable, small on small repos). `.sigil/index.duckdb` is derived and `.gitignore`'d by default. Committing the JSONL lets every teammate / CI agent read the map without running `sigil index` first. Not committing them keeps the tree cleaner.

**Q: How does sigil compare to ripgrep?**
Different tools. ripgrep is line-oriented text search; sigil is structural AST search. Sigil beats grep on (a) output compactness — 5–70× fewer bytes because no noise from docstrings / strings / comments, and (b) semantic queries like "what's defined in this file?" that grep can't express. Grep beats sigil on one-shot queries against unindexed repos. Detailed numbers in [evals/results/multilang-with-db-2026-04-20.md](evals/results/multilang-with-db-2026-04-20.md).

**Q: How does sigil compare to LSP / language servers?**
LSPs are per-language, resident processes with deep semantic understanding (types, generic resolution, incremental state). sigil is cross-language, stateless, deterministic. Complementary, not competitive — an LSP-upgrade path is on the roadmap (§14.10 of the plan) for precision on TS generics / Rust traits / Python dynamic dispatch.

**Q: Why BLAKE3 for hashing?**
Faster than SHA-256, faster than xxhash3 at most sizes. 16-hex-char truncation is sigil's storage form — enough to distinguish entities within any plausible repo size.

**Q: What happens if `sigil index` can't parse a file?**
Skipped silently (with `-v` flag: printed to stderr). sigil never errors out on parse failures — one broken file doesn't block the other 2,000.

**Q: Can I run sigil without the `.sigil/` directory?**
Yes — `sigil diff --files old.py new.py` compares two files directly without an index. For agent commands (`map`, `context`, `blast`, `review`), you need to run `sigil index` first.

---

## Roadmap & plan

The full strategic roadmap — agent-adoption plan, command surface rationale, scale strategy, eval methodology — lives in **[agent-adoption-plan.md](agent-adoption-plan.md)** (~900 lines). Release notes in **[blog-agent-adoption.md](blog-agent-adoption.md)**.

## Worked examples

Real sigil outputs on real repos, with honest notes on wins and misses:
- [worked/sigil-self/](worked/sigil-self/) — sigil indexed against its own source
- [evals/results/](evals/results/) — benchmark snapshots

Contributions welcome — see [worked/README.md](worked/README.md) for the rubric.

## License

MIT. See [LICENSE](LICENSE).
