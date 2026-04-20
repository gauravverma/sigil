# sigil — Agent Adoption Spec & Project Plan

**Thesis.** sigil is the deterministic structural-intelligence layer for AI coding agents. Zero LLM, always-correct, CLI-composable, installed-but-not-embedded. This plan turns that thesis into a shippable 90-day roadmap with concrete specs for every new command, flag, and installer.

**Precursor (Phase 0).** Before the agent-adoption work begins, sigil removes its external codeix dependency and brings parsing + code intelligence in-house. Phase 0 is ~1 sprint (6–8 days). The 90-day plan in §10 starts *after* Phase 0 lands. See §14 for the Phase 0 spec.

---

## 1. Product identity

- **What sigil is**: a stateless CLI that extracts code entities, ranks them, classifies structural changes, and surfaces relationships. All outputs are derivable from BLAKE3 hashes + tree-sitter ASTs + codeix's SearchDb. No inference, no LLM, no hallucination surface.
- **What sigil is not**: an agent, a knowledge graph over multimodal corpora, a long-running daemon (by default), a replacement for LSPs, a ranking service that depends on LLMs.
- **Wedge for AI agents**: two primitives — an orientation digest (`sigil map`) and a task-time context bundle (`sigil context`) — plus a distribution mechanism that makes agents reach for them without prompt leading.

## 2. Non-goals (freeze these in the README)

1. No LLM-generated edges on code. Structural truth is computable; guessing it is worse than omitting it.
2. No multimodal inputs (PDFs, video, images). graphify owns that category.
3. No skill-first framing. The CLI is the product; skill installers and MCP shims are thin adapters.
4. No stateful session model. `sigil serve` is opt-in speed only; default remains fork-exec-print-exit.
5. No preference-level nudging in installed prompts. Describe capabilities; never tell the agent which tool to prefer.
6. No output hedging. "3 callers in 2 files." Never "run sigil to verify."

## 3. Architecture additions (overview)

Three new subsystems on top of the current codebase:

```
src/
  rank.rs            — PageRank over codeix import graph; blast-radius computation
  cochange.rs        — Git-history co-change miner → .sigil/cochange.db (SQLite)
  map.rs             — Budget-aware ranked digest renderer
  context.rs         — Minimum-viable-context bundler for a symbol
  install/
    claude.rs        — Claude Code: settings.json PreToolUse + CLAUDE.md
    codex.rs         — Codex: .codex/hooks.json + AGENTS.md
    cursor.rs        — Cursor: .cursor/rules/sigil.mdc
    gemini.rs        — Gemini CLI: .gemini/settings.json BeforeTool + GEMINI.md
    opencode.rs      — OpenCode: plugin + AGENTS.md
    aider.rs         — Aider: AGENTS.md only
    copilot.rs       — GitHub Copilot CLI: skill file
    githook.rs       — post-commit + post-checkout rebuild
  benchmark.rs       — Token-reduction measurement against canonical queries

evals/               — New top-level directory
  tasks/             — YAML task specs
  runner/            — Python harness driving Claude SDK
  results/           — Per-run JSON artifacts

worked/              — Real-corpus demos with honest review.md each
```

Existing modules that get extended:
- `entity.rs` — add `rank: f64`, `blast_radius: BlastRadius` fields (opt-out via version gate).
- `writer.rs` — emit extended fields in index output.
- `index.rs` — post-index rank pass, triggered once per run.
- `output.rs` — carry rank/blast through the diff → output pipeline.
- `markdown_formatter.rs` — render rank/blast per entity when `--rank` is set.
- `formatter.rs` — same for terminal output.
- `main.rs` — wire new subcommands.

---

## 4. Tier 1 — Build these first (the wedge)

### 4.1 Entity rank + blast radius

**Data model (`entity.rs`):**
```rust
pub struct BlastRadius {
    pub direct_callers: u32,
    pub direct_files: u32,
    pub transitive_callers: u32,
}

pub struct Entity {
    // ... existing fields
    pub rank: Option<f64>,         // None until ranked
    pub blast_radius: Option<BlastRadius>,
}
```

**Algorithm (`rank.rs`):**
1. Build file-to-file import graph from codeix's import edges (or, if codeix does not expose this, from entity references grouped by file).
2. Run PageRank with damping 0.85, 50 iterations, uniform teleportation. File-level rank.
3. Each entity inherits its file's rank × local importance multiplier (exported > private > nested).
4. Blast radius: for each entity, query codeix callers, compute direct/file counts, walk transitive closure up to depth 3 (cap to avoid cycles).
5. Persisted in `.sigil/index.jsonl` alongside entities.

**Performance target:** < 2× current index time on a 10k-file repo. Incremental: ranks recomputed only when the import graph changes; blast radius updated for touched symbols only.

**Flag:** `sigil index --rank` (default on; `--no-rank` to opt out for CI speed).

### 4.2 `sigil map` — budget-aware ranked digest

**Command:**
```
sigil map [--tokens N] [--focus PATH] [--format markdown|json] [--depth N] [--write]
```

**Inputs:**
- `--tokens N` — token budget (default 4000). Uses a tokenizer estimate (bytes / 4 cheap fallback; real tokenizer if Anthropic ships an embeddable one for Rust, else pluggable).
- `--focus PATH` — personalization: entities under PATH and their import neighbors get rank × 2.
- `--format` — markdown (default, for agents reading files) or JSON (for programmatic).
- `--depth N` — max entities per file in output (default 5).
- `--write` — writes to `.sigil/SIGIL_MAP.md` in addition to stdout.

**Output shape (markdown):**
```
# Sigil Map — <repo>

## Top files by impact
### src/core/agent.ts (rank 0.094, used by 24 files)
- export class AgentBus — shared coordination bus for parallel subagents
- export fn dispatch(task) — ...
- export interface Task — ...

### src/core/index.rs (rank 0.071, used by 18 files)
...

## Subsystems (Leiden clustering, Tier 3 — stubbed for now)
- core (12 files) — agent orchestration
- indexing (8 files) — codeix + tree-sitter pipeline
...
```

**Algorithm:**
1. Load index, sort entities by `rank × log(1 + blast_radius.direct_files)`.
2. Group by file; keep top-`depth` entities per file.
3. Greedy packing: add files in rank order until token budget exhausted.
4. Render. Include signatures; skip bodies.

**Reuses:** `output.rs` intermediate model, `markdown_formatter.rs` patterns.

**Done when:** running `sigil map --tokens 2000` on sigil itself produces a coherent one-page orientation that a fresh agent can consume as its primary repo intro.

### 4.3 `sigil context <symbol>` — minimum-viable context

**Command:**
```
sigil context <symbol-or-qualified-path> [--budget N] [--format agent|markdown|json] [--depth N]
```

**Inputs:**
- Positional: symbol name (e.g., `dispatch`) or qualified path (e.g., `src/core/agent.rs::AgentBus::dispatch`). Ambiguous names resolve to all matches with file paths.
- `--budget N` — token cap (default 1500).
- `--format agent` — compact JSON; `markdown` for human; `json` for full structured.
- `--depth N` — how many callers/callees to include (default 10).

**Output bundle:**
```json
{
  "symbol": "AgentBus::dispatch",
  "file": "src/core/agent.rs",
  "lines": [42, 78],
  "signature": "pub fn dispatch(&self, task: Task) -> Result<Dispatch>",
  "doc": "Dispatch a task to a free worker.",
  "direct_callers": [
    {"file": "src/boot.rs", "line": 22, "symbol": "main", "snippet": "bus.dispatch(task)?"}
  ],
  "direct_callees": [
    {"symbol": "Worker::accept", "file": "src/worker.rs", "line": 101}
  ],
  "related_types": [
    {"symbol": "Task", "file": "src/types.rs", "signature": "pub struct Task { ... }"}
  ],
  "blast_radius": {"callers": 14, "files": 6, "transitive": 23}
}
```

**Algorithm:**
1. Resolve symbol via codeix search → candidate list; if multiple, emit all with disambiguator hint.
2. Query codeix: signature, doc/docstring, callers, callees, type references.
3. Prioritize by budget: signature + doc always included; callers and callees filled in rank order until budget met.
4. Token estimation same as `sigil map`.

**Done when:** on a real editing task ("modify `fooBar` to also do X"), an agent running `sigil context fooBar` can start editing without any `Read` or `Grep` calls.

### 4.4 Platform hook installers

**Commands:**
```
sigil claude install       sigil claude uninstall
sigil codex install        sigil codex uninstall
sigil cursor install       sigil cursor uninstall
sigil gemini install       sigil gemini uninstall
sigil opencode install     sigil opencode uninstall
sigil aider install        sigil aider uninstall
sigil copilot install      sigil copilot uninstall
```

**Per-platform behavior:**

| Platform | Writes | Mechanism |
|---|---|---|
| Claude Code | `.claude/settings.json` PreToolUse hook + `CLAUDE.md` block | Fires before Glob/Grep/Read |
| Codex | `.codex/hooks.json` PreToolUse + `AGENTS.md` block | Fires before Bash |
| Cursor | `.cursor/rules/sigil.mdc` with `alwaysApply: true` | Per-conversation injection |
| Gemini CLI | `.gemini/settings.json` BeforeTool + `GEMINI.md` block | Fires before file-read tools |
| OpenCode | `.opencode/plugins/sigil.js` + `opencode.json` + `AGENTS.md` | `tool.execute.before` plugin |
| Aider | `AGENTS.md` block | No hook support; static instruction |
| Copilot CLI | `~/.copilot/skills/sigil/SKILL.md` | Skill file |

**Hook content (CAPABILITY-DESCRIBING, not preference-giving).** Example for Claude Code:
```
sigil — structural code intelligence available in this repo.
Capabilities:
  sigil map --tokens N          ranked codebase digest (use for orientation)
  sigil context <symbol>        signature + callers + callees + related types
  sigil diff A..B --markdown    structural diff (use when git diff is noisy)
  sigil callers <symbol>        exact caller list
  sigil callees <symbol>        exact callee list
  sigil blast <symbol>          impact summary
A prebuilt map exists at .sigil/SIGIL_MAP.md.
```

Language rule: describe what the tool does and when it fits; never "prefer sigil over grep." Matches the tone of built-in tool descriptions in agent harnesses. This is the honest answer to the leading-prompt problem.

**Safety:** installers are idempotent (re-running upgrades the block in place using sentinel markers like `<!-- sigil:begin -->` / `<!-- sigil:end -->`). Uninstallers remove only the sigil block. Never touch user content outside markers.

---

## 5. Tier 2 — Multipliers

### 5.1 `sigil hook install` — git auto-rebuild

**Command:** `sigil hook install [--background]`, `sigil hook uninstall`, `sigil hook status`.

**Writes:** `.git/hooks/post-commit`, `.git/hooks/post-checkout`, each running `sigil index --incremental --quiet` in background (unless `--background=false`).

**On rebuild failure:** exits non-zero so git surfaces the error (graphify's pattern).

**Effect:** `SIGIL_MAP.md` and the index are always fresh without a daemon.

### 5.2 `sigil review A..B`

**Command:**
```
sigil review <refspec> [--markdown|--json] [--budget N] [--cochange]
```

Superset of `sigil diff`. Adds per-entity rank + blast radius, co-change misses, and rank-ordered sections. Intended as the PR-review artifact — the thing a reviewer (human or agent) reads instead of `git diff`.

**Output sections (markdown):**
1. **Most impactful changes** — top-K by `rank × blast`.
2. **Structural deltas** — the existing diff content with rank/blast tags.
3. **Co-change misses** — files that historically change with touched files but didn't in this PR. Flagged, not fatal.
4. **Clones introduced** — new entities whose `body_hash` matches an existing entity (opportunity flag).

**Implementation:** wraps `diff.rs` + joins against `rank` and `cochange.rs`.

### 5.3 `sigil blast <symbol>`

**Command:** `sigil blast <symbol> [--transitive] [--format agent|json|markdown]`

Prints:
```
processPayment — called from 14 sites in 6 files.
Transitive: reaches 23 symbols across 9 files.
Top callers by rank:
  src/api/checkout.ts:42  processCart → processPayment
  ...
```

Data source: codeix callers + blast_radius cache.

### 5.4 `sigil duplicates`

**Command:** `sigil duplicates [--min-lines N] [--format json|markdown]`

Groups index by `body_hash` where count > 1 and body exceeds `--min-lines` (default 3). Nearly free — the hashes already exist.

**Use case:** clone-report CLI, and an input to `sigil review` for flagging introduced duplication.

### 5.5 Cross-cutting flags on every command

- `--format agent` — compact JSON with short keys (`{f,n,c,br,r}`). Drops whitespace. Design goal: ≤ half the tokens of the verbose JSON.
- `--budget <tokens>` — respected wherever a list is emitted. Drops lowest-rank entries first, not truncates arbitrarily.
- `--jsonl` — streaming, one object per line.

Implement once in `output.rs`; every command inherits.

### 5.6 `sigil benchmark`

**Command:** `sigil benchmark [--corpus PATH] [--queries FILE]`

Runs a fixed suite of canonical queries against two conditions:
1. Control: estimate tokens to answer via raw `git` + file reads.
2. Treatment: estimate tokens via sigil commands.

**Output:**
```
Benchmark (sigil 0.3.0)
  Q1: "summarize PR main..HEAD"    raw: 4820 → sigil: 612   (7.9×)
  Q2: "find callers of process"    raw: 2140 → sigil: 180   (11.9×)
  Q3: "explain fooBar function"    raw: 1450 → sigil: 310   (4.7×)
  Median reduction: 7.9×
```

Prints after every `sigil index` when `--benchmark` is set, and as a standalone command.

**Corpus:** `worked/` fixtures (see §7).

---

## 6. Tier 3 — Platform maturity (quarter 2+)

Listed briefly; specs to be written when promoted.

- **`sigil serve`** — unix-socket daemon; CLI auto-detects and routes.
- **`sigil dead A..B`** — dangling-reference detection across a diff.
- **LSP upgrade path** — tree-sitter default; optional LSP backend for callers/references per-language.
- **Community detection** — Leiden pass over the call graph → subsystem clusters in `sigil map`.
- **`sigil wiki`** — per-module markdown output with `index.md`.
- **`git-sigil`** — installable git subcommand so `git sigil diff` works (name-recognition lever).
- **MCP shim** — thin adapter over the CLI for permission-gating parity.

---

## 7. Worked examples (trust layer)

Directory structure:
```
worked/
  <repo-slug>/
    <pr-or-commit-sha>/
      raw/                 # input files (or a gitignore'd submodule)
      sigil-output/        # SIGIL_MAP.md, review.md, context-for-<symbol>.md
      review.md            # honest evaluation: what sigil got right, what it missed
      tokens.md            # raw-vs-sigil token counts with methodology
```

**First five targets (initial commit):**
1. A noisy-refactor PR (import reshuffles + 1 logic change).
2. A pure-logic-change PR.
3. A rename-heavy PR (tests `moved`/`renamed` classification).
4. A cross-file refactor (tests blast-radius claims).
5. A monorepo subsystem PR (tests `--focus`).

**Rule:** every `review.md` must include a misses section. Cherry-picked wins erode trust.

---

## 8. Evaluation harness

### 8.1 Structure

```
evals/
  runner/
    run.py              # drives Anthropic SDK or Claude CLI
    grade.py            # deterministic + LLM-judge graders
    report.py           # aggregates results into tables
  tasks/
    E1_pr_review/       # 50 YAML specs, each {repo, sha, prompt, answer_key}
    E2_navigation/      # 50 YAML specs, each {repo, symbol, expected_callers}
    E3_orientation/     # 20 YAML specs
    E4_swe_bench/       # SWE-bench Lite subset
    E5_targeted_edit/   # 20 YAML specs
  results/
    <date>/<commit>/<task>/run.json
```

### 8.2 Arms

- **Control** — agent with baseline tools (Read, Grep, Glob, Bash + git).
- **Treatment** — control + sigil on PATH + installed hooks.
- **Ablations** — control + one sigil command at a time (`map` only, `context` only, `diff` only, `callers` only). Identifies which commands pull weight.
- Run across Haiku + Sonnet + Opus.

### 8.3 Metrics

Per run: `tokens_in`, `tokens_out`, `turns`, `wall_clock_ms`, `success` (bool from deterministic grader), `quality` (from LLM judge where applicable).

Per eval: token ratio (treatment/control) with bootstrap 95% CI; success-rate delta with Wilson interval.

### 8.4 Pipeline

1. For each task × arm × model × seed (N=5), run in isolated git worktree.
2. Capture every tool call + API usage.
3. Grader runs deterministically where possible; LLM-judge (using a different/stronger model than the runner) where not, with 10% human spot-check.
4. Nightly smoke run on 20 tasks; full suite pre-release.
5. Results published in `evals/results/latest.md`.

### 8.5 First-quarter deliverables

- Week 11: E2 (navigation) harness + 50 tasks + first numbers.
- Week 12: E1 (PR review) harness + 30 tasks.
- Quarter 2: E4 (SWE-bench Lite) harness — the money shot.

### 8.6 Fairness rules

1. Prompts never mention sigil. Treatment arm gets it via tool surface + installed hooks, matching production use.
2. Random task ordering per run.
3. Model versions pinned per eval run.
4. Eval corpus frozen per release; don't tune sigil against moving targets.
5. Publish a **separate** adoption eval (sigil on PATH but no hooks, no tool-manifest mention) to measure cold discovery rate. Both numbers matter.

---

## 9. Distribution & positioning

### 9.1 Hook installer rollout

Ship the three highest-value installers in week 7; rest in week 8. Priority:
1. Claude Code (largest audience, hook support mature).
2. Cursor (second-largest, simple rules file).
3. Codex (official hook support in new versions).
4. Gemini CLI, OpenCode, Aider, Copilot CLI.

### 9.2 Training-data seeding (long play, start now)

- One technical blog post per month for the first two quarters: "sigil review vs git diff for PR automation," "sigil context vs grep for code agents," "Deterministic code intelligence without LLMs."
- Demo video per quarter.
- Comment participation in Claude Code, Codex CLI, Aider, SoulForge issue trackers when code-intelligence topics arise.
- Answer "best way to give Claude Code codebase awareness" type threads with worked examples.

Clock is 6–12 months until models actually see "sigil" in training corpora. Start now.

### 9.3 README / positioning rewrite

- Lead with one-sentence identity: deterministic structural intelligence, no LLM, CLI-composable.
- Second: the two primitives (`sigil map`, `sigil context`) and what they replace.
- Third: install snippet for each supported agent.
- Fourth: the benchmark table from `sigil benchmark` on worked examples.
- Hide "advanced" commands (diff internals, JSON/YAML normalization) below the fold.

---

## 10. 90-day timeline

| Week | Milestone | Deliverable |
|---|---|---|
| 1 | Foundation | `rank.rs` skeleton + import-graph extraction from codeix |
| 2 | Rank lands | PageRank working; `BlastRadius` populated in index; `--rank`/`--no-rank` flag |
| 3 | Map MVP | `sigil map --tokens N --format markdown`, writes to stdout |
| 4 | Map polished | `--focus`, `--write` → `.sigil/SIGIL_MAP.md`, `--depth`, JSON format |
| 5 | Context MVP | `sigil context <symbol>` — signature + direct callers/callees |
| 6 | Context polished | Budget handling, `--format agent`, type references, markdown format |
| 7 | Distribution wave 1 | `sigil claude install`, `sigil cursor install`, `sigil codex install` with capability-describing hooks |
| 8 | Distribution wave 2 | `sigil gemini install`, `sigil opencode install`, `sigil aider install`, `sigil copilot install`, `sigil hook install` |
| 9 | Review command | `sigil review A..B --markdown` with rank + blast + co-change; `cochange.rs` + `.sigil/cochange.db` |
| 10 | Impact + duplicates | `sigil blast`, `sigil duplicates`, `sigil benchmark` |
| 11 | Eval harness part 1 | E2 (navigation) harness + 50 tasks + first numbers published in `evals/results/` |
| 12 | Eval harness part 2 + worked examples | E1 (PR review) harness + 30 tasks; 5 `worked/` examples committed; README rewrite; blog post #1 |

**Release cadence:**
- `v0.3` at week 4 (rank + map).
- `v0.4` at week 6 (context).
- `v0.5` at week 8 (installers + git hooks).
- `v0.6` at week 10 (review + blast + duplicates + benchmark).
- `v0.7` at week 12 (eval-backed claims in README).

Each release ships with a changelog, a blog post (from week 4 onward), and a worked-example addition.

---

## 11. Risks & open questions

1. ~~**codeix graph access.**~~ **Resolved.** codeix does not expose file-level import edges; `get_callers/get_callees` are the only symbol-level APIs. Obsolete after Phase 0 (§14) — sigil owns both parsing and querying after decodeix. `rank.rs` derives the graph from sigil's native `Reference` table (Path B from the scoping note).
2. **Token estimation.** Rust has no first-class Anthropic tokenizer. Options: (a) bytes/4 approximation (cheap, ±20% off), (b) spawn a tiktoken Python subprocess (slow, accurate), (c) ship our own tokenizer lite. Recommend (a) for v0.3 with a flag for (b) when precision matters.
3. **Hook-installer drift.** Agent harnesses change their hook formats. Maintainability strategy: version each installer, include a compatibility matrix in the README, test against a pinned version of each agent in CI.
4. **Eval cost.** SWE-bench-scale evals on Opus get expensive. Start with Haiku + Sonnet; Opus only for headline release numbers. Budget $500/release for evals initially.
5. **Adoption without training signal.** Cold-discovery adoption will be low for 6–12 months. Hook installers bridge the gap but require users to run `sigil <platform> install`. Reduce friction: single `sigil install` command that auto-detects installed platforms and offers a multi-select.
6. **Competitive overlap with graphify.** graphify is MIT and rapidly shipping. Overlap is thin (they're knowledge-graph-over-multimodal; sigil is structural-diff-over-code), but distribution patterns will converge. Stay sharper on the determinism story and the diff story — those are differentiators they can't match without rewriting.
7. **"Always-on" hook fatigue.** Multiple tools installing PreToolUse hooks can bloat agent prompts. Mitigation: keep the sigil hook text to ≤ 120 tokens; measure token cost of the hook itself and report it in `sigil benchmark`.

---

## 12. Success criteria (end of 90 days)

Hard numbers to hit before claiming success.

1. **Product**: `sigil map`, `sigil context`, `sigil review`, `sigil blast`, `sigil duplicates`, `sigil benchmark`, plus 7 platform installers shipped and documented.
2. **Distribution**: `sigil <platform> install` tested on Claude Code, Cursor, Codex, Gemini CLI, OpenCode, Aider, Copilot CLI. At least one external user per platform confirms adoption.
3. **Evals**: E2 and E1 published with ≥ 30% median token reduction at equal success/quality (Sonnet, N≥5 per task); no quality regression. Ablation table showing per-command contribution.
4. **Trust**: 5 worked examples in-repo with honest `review.md` each.
5. **Reach**: two blog posts, one demo video, a v0.7 release with eval-backed claims.

If any of the first three miss by more than 20%, stop and diagnose before shipping v0.8.

---

## 13. One-line version (for internal reminder)

Ship a ranked map, a context primitive, and the graphify-style hook installers — backed by an honest eval that proves token reduction at equal quality. Keep the code path LLM-free. Everything else is optional.

---

## 14. Phase 0 — Remove codeix, bring everything in-house (precursor)

**Goal.** sigil becomes self-contained. Drop `codeix` as a git dependency. Own parsing + code intelligence end-to-end. Zero behavior change visible to users.

**Why first.** Phase 0 is a prerequisite for the 90-day plan:
- Adding `rank: f64` and `BlastRadius` to `Entity` requires schema ownership.
- `.codeindex/` duplicates `.sigil/` — collapsing to one store is clean-up we've been deferring.
- Removes supply-chain risk (codeix is pinned to a GitHub tag, 7 stars, last push Feb 2026).
- Aligns implementation with stated product identity ("deterministic, self-contained, zero LLM").

### 14.1 Current codeix surface used by sigil

| codeix symbol | Used at | Replacement |
|---|---|---|
| `codeix::parser::treesitter::parse_file(bytes, lang, path)` | `src/index.rs:34` | **Vendor** `parser/*.rs` into `sigil/src/parser/` |
| `codeix::parser::languages::detect_language(ext)` | `src/diff.rs:61,214`, `src/index.rs:153,268` | **Vendor** `parser/languages.rs` |
| `codeix::cli::build::build_index_to_db(root, …)` | `src/query.rs:12` | **Rewrite** as `Index::load(root)` over `.sigil/index.jsonl` |
| `SearchDb::{get_callers, get_callees, get_file_symbols, get_children, search, explore_*, list_projects}` | `src/main.rs`, `src/query.rs` | **Rewrite** in `src/query/index.rs` as in-memory methods |
| `codeix::index::format::{SymbolEntry, ReferenceEntry}`, `codeix::server::db::SearchResult` | `src/query.rs`, `src/main.rs` | **Rewrite** as sigil-native types |
| `codeix::mount::MountTable` | `src/query.rs` | **Drop** — sigil is single-project |

### 14.2 Parser: vendor from codeix

codeix is dual-licensed **MIT OR Apache-2.0** — vendoring is clean.

**Files to import** (from `codeix/src/parser/` at the v0.5.0 tag):
```
parser/mod.rs            (39 LOC)
parser/treesitter.rs    (308 LOC) — parse_file dispatcher
parser/languages.rs     ( 73 LOC) — extension → lang
parser/helpers.rs       (387 LOC) — shared utils
parser/metadata.rs      (733 LOC) — decorator/metadata extraction
parser/c_lang.rs       (1054 LOC)
parser/cpp.rs          (1451 LOC)
parser/csharp.rs       (1545 LOC)
parser/go.rs           (1008 LOC)
parser/java.rs         (1087 LOC)
parser/javascript.rs   (1226 LOC)
parser/python.rs       (1133 LOC)
parser/ruby.rs         (1090 LOC)
parser/rust_lang.rs    (1085 LOC)
parser/typescript.rs   (1763 LOC)
parser/markdown.rs      (585 LOC)
parser/sfc.rs           (338 LOC) — Vue/Svelte
```

~13.7k LOC total. Committed under `sigil/src/parser/` with a `NOTICE` file preserving codeix's Apache-2.0 attribution.

**Cargo.toml deps to add** (matching codeix v0.5.0 pins):
```toml
tree-sitter = "0.26"
tree-sitter-python    = { version = "0.25", optional = true }
tree-sitter-rust      = { version = "0.24", optional = true }
tree-sitter-javascript = { version = "0.25", optional = true }
tree-sitter-typescript = { version = "0.23", optional = true }
tree-sitter-go        = { version = "0.25", optional = true }
tree-sitter-java      = { version = "0.23", optional = true }
tree-sitter-c         = { version = "0.24", optional = true }
tree-sitter-cpp       = { version = "0.23", optional = true }
tree-sitter-ruby      = { version = "0.23", optional = true }
tree-sitter-c-sharp   = { version = "0.23", optional = true }
tree-sitter-md        = { version = "0.5",  features = ["parser"], optional = true }
```

Replicate codeix's feature flags (`lang-python`, `lang-rust`, …) so sigil users can opt out of languages they don't need.

**Cargo.toml deps to remove:** `codeix` (git dependency at the top).

### 14.3 Intelligence layer: rewrite in-house

No SQLite. No MountTable. In-memory over `.sigil/index.jsonl`.

**New module: `src/query/` (replaces the SearchDb usage in the current `src/query.rs`).**

```rust
// src/query/index.rs
pub struct Index {
    pub entities: Vec<Entity>,
    pub references: Vec<Reference>,
    // precomputed lookups, built in Index::load()
    by_name: HashMap<String, SmallVec<[usize; 4]>>,        // entity name → entity idxs
    refs_by_target: HashMap<String, SmallVec<[usize; 4]>>, // ref name → ref idxs (for get_callers)
    refs_by_caller: HashMap<String, SmallVec<[usize; 4]>>, // ref caller → ref idxs (for get_callees)
    by_file: HashMap<String, SmallVec<[usize; 16]>>,       // file → entity idxs
}

impl Index {
    pub fn load(root: &Path) -> Result<Self>;
    pub fn build_from(entities: Vec<Entity>, references: Vec<Reference>) -> Self;

    pub fn get_callers(&self, name: &str, kind: Option<&str>, limit: usize) -> Vec<&Reference>;
    pub fn get_callees(&self, caller: &str, kind: Option<&str>, limit: usize) -> Vec<&Reference>;
    pub fn get_file_symbols(&self, file: &str, kind: Option<&str>, limit: usize) -> Vec<&Entity>;
    pub fn get_children(&self, file: &str, parent: &str, kind: Option<&str>, limit: usize) -> Vec<&Entity>;
    pub fn search(&self, q: &str, scope: Scope, kind: Option<&str>, path_prefix: Option<&str>, limit: usize) -> Vec<SearchHit>;
    pub fn explore(&self, path_prefix: Option<&str>, max_per_dir: usize) -> DirOverview;
    pub fn list_projects(&self) -> Vec<String>;  // always returns [""] for single-project
}
```

**Search strategy:**
- Phase 0: substring + fuzzy match over `entity.name`. Good enough for ≤ 100k entities.
- Phase 2+ (post-90-day): add `tantivy` as an optional feature if real FTS becomes necessary. Keep the simple path as default.

**Data types to replace:**
```rust
// src/query/types.rs
pub enum SearchHit {
    Symbol(SymbolHit),
    File(FileHit),
    Text(TextHit),
}

pub struct SymbolHit { /* fields matching what main.rs currently prints */ }
// ... etc
```

### 14.4 File-by-file migration plan

| # | Change | File(s) |
|---|---|---|
| 1 | Add `src/parser/` with vendored codeix sources + NOTICE | `src/parser/*.rs`, `src/parser/NOTICE` |
| 2 | Expose `parser::parse_file` and `parser::detect_language` via `src/lib.rs` | `src/lib.rs` |
| 3 | Replace `codeix::parser::treesitter::parse_file` in `index.rs` | `src/index.rs:34` |
| 4 | Replace `codeix::parser::languages::detect_language` in `diff.rs` and `index.rs` | `src/diff.rs:61,214`, `src/index.rs:153,268` |
| 5 | Add `src/query/` module (index.rs, types.rs, search.rs) | new dir |
| 6 | Rewrite `src/query.rs` to wrap `src/query/Index` instead of codeix SearchDb | `src/query.rs` |
| 7 | Update `src/main.rs` call sites (`get_callers`, `get_callees`, `get_file_symbols`, `get_children`, `search`, `explore_*`) | `src/main.rs:285,338,356,369,382,395,408` |
| 8 | Delete `.codeindex/` creation; use `.sigil/` only | wherever `build_index_to_db` was called |
| 9 | Update `Cargo.toml`: drop codeix, add tree-sitter + tree-sitter-\<lang\> | `Cargo.toml` |
| 10 | Update `python/Cargo.toml` transitively (PyO3 crate depends on sigil lib) | `python/Cargo.toml` |
| 11 | Update `CLAUDE.md`, `ARCHITECTURE.md`, `README.md`: remove codeix references | those files |
| 12 | Add `.codeindex/` to `.gitignore` (or remove if already there); ensure it's no longer generated | `.gitignore` |
| 13 | Tests: parser integration (fixture per language), query-layer unit tests, end-to-end `sigil diff`/`sigil explore`/`sigil callers` parity tests | `tests/*` |

### 14.5 Phase 0 timeline

| Day | Deliverable |
|---|---|
| 1 | Vendor `src/parser/` from codeix. Add tree-sitter deps to Cargo.toml. Keep codeix dep in place; `cargo build` green with both paths available. |
| 2 | Switch `parse_file` + `detect_language` call sites over to `crate::parser::*`. Keep codeix dep for SearchDb only. Run existing tests — all pass. |
| 3 | Write `src/query/index.rs` (Index, build_from, lookups). Unit tests against synthetic fixtures. |
| 4 | Implement `get_callers`, `get_callees`, `get_file_symbols`, `get_children`. Parity tests vs old codeix-backed output on sigil's own index. |
| 5 | Implement `search`, `explore`, `list_projects`. Parity tests. |
| 6 | Swap call sites in `main.rs` + `query.rs`. Delete `.codeindex/` code paths. Remove codeix from Cargo.toml. |
| 7 | Cleanup: `CLAUDE.md`, `ARCHITECTURE.md`, `README.md` updates. Python crate rebuild + PyPI smoke test. |
| 8 | Buffer / overflow / polish. Merge to main. Tag as `v0.3.0-alpha` (not a public release — just a checkpoint). |

### 14.6 Acceptance criteria

1. `cargo build` and `cargo test` green with no `codeix` in `Cargo.toml` or `Cargo.lock`.
2. `sigil index`, `sigil diff`, `sigil explore`, `sigil search`, `sigil symbols`, `sigil children`, `sigil callers`, `sigil callees` produce byte-identical output on a reference test corpus vs the pre-Phase-0 binary. (Small formatting diffs acceptable if intentional; must be documented.)
3. Python bindings (`import sigil; sigil.diff_json(...)`) still work.
4. `.codeindex/` is no longer generated. Only `.sigil/` exists.
5. README and ARCHITECTURE.md reflect in-house ownership.

### 14.7 Risks specific to Phase 0

1. **Tree-sitter grammar version drift.** codeix pins specific tree-sitter-\<lang\> versions. Match them exactly initially; upgrade only after green tests on a corpus of real-world files.
2. **Vendored code ownership.** Sigil now owns ~13.7k LOC of tree-sitter walking code. Acceptable cost for the strategic benefits, but it's real maintenance.
3. **Parity bugs.** The output of sigil queries may diverge subtly from codeix's (e.g., ordering, limit handling, visibility filtering). Mitigation: golden-file parity tests on a fixed test corpus before the switch-over in day 6.
4. **Python bindings.** The `python/` crate depends on the lib — rebuild + test before committing. PyPI wheel builds tested in CI.
5. **Search regression at scale.** codeix uses SQLite FTS5; sigil's in-memory substring/fuzzy search is simpler. Fine for ≤ 500k entities. See §14.9 for the DuckDB-backed scale path.

### 14.8 Index files — spec

#### 14.8.1 Today (pre-Phase 0) — two parallel stores

**`.sigil/` — sigil-owned, written by `writer.rs`:**

| File | Purpose | Schema |
|---|---|---|
| `cache.json` | Incremental-parse cache | `{version: "1", files: {path → blake3_hex}}` |
| `entities.jsonl` | One `Entity` per line | `{file, name, kind, line_start: u32, line_end: u32, parent?, sig?, meta?: [String], body_hash?, sig_hash?, struct_hash}` |
| `refs.jsonl` | One `Reference` per line | `{file, caller?, name, ref_kind, line: u32}` |

**`.codeindex/` — codeix-owned, written by `build_index_to_db`:**

| File | Purpose | Schema |
|---|---|---|
| `index.json` | Manifest | `{version, name, root, languages: [String]}` |
| `files.jsonl` | Per-file metadata | `{path, lang?, hash, lines, title?, description?, project?}` |
| `symbols.jsonl` | codeix's entity table | `{file, name, kind, line: [u32;2], parent?, tokens?, alias?, visibility?, project?}` |
| `references.jsonl` | codeix's ref table | `{file, caller?, name, kind, line: [u32;2]}` |
| SQLite files | FTS5 index for `search()` | binary |

**Issues with current state:**
- `.codeindex/index.json` in this repo is 0 bytes (never populated) — every sigil query re-parses from source via `build_index_to_db`. Already a performance bug.
- `entities.jsonl` vs `symbols.jsonl` overlap heavily but aren't schema-compatible (`line_start/line_end` vs `line: [u32;2]`; sigil carries hash columns, codeix carries `tokens/alias/visibility`).
- Two caches, two truths.

#### 14.8.2 After Phase 0 — one store, sigil-owned

**`.sigil/` becomes the only index directory:**

| File | Status | Purpose | Schema |
|---|---|---|---|
| `index.json` | **New** | Top-level manifest | `{version: "2", sigil_version, root, languages: [String], built_at: RFC3339, file_count, entity_count, ref_count}` |
| `files.jsonl` | **New** | Per-file metadata (adopt codeix's useful fields) | `{path, lang?, hash, lines, title?, description?}` |
| `entities.jsonl` | Extended | One `Entity` per line | adds **`visibility?`**; reserves `rank?` and `blast_radius?` for Phase 1 (nullable) |
| `refs.jsonl` | Unchanged | One `Reference` per line | same as today |
| `cache.json` | Unchanged | Incremental-parse cache | same as today |

**`.codeindex/` — deleted** from disk, removed from code paths, added to `.gitignore`.

**Updated `Entity` struct:**
```rust
pub struct Entity {
    pub file: String,
    pub name: String,
    pub kind: String,
    pub line_start: u32,
    pub line_end: u32,
    pub parent: Option<String>,
    pub sig: Option<String>,
    pub meta: Option<Vec<String>>,
    pub body_hash: Option<String>,
    pub sig_hash: Option<String>,
    pub struct_hash: String,
    pub visibility: Option<String>,      // Phase 0: adopt from vendored parser
    pub rank: Option<f64>,                // Phase 1 — reserved slot
    pub blast_radius: Option<BlastRadius>, // Phase 1 — reserved slot
}

pub struct BlastRadius {
    pub direct_callers: u32,
    pub direct_files: u32,
    pub transitive_callers: u32,
}
```

Old v1 `entities.jsonl` remains deserializable — missing fields → `None` via `#[serde(default)]`.

**Rationale for the adds:**
- **`visibility`** — emitted for free by the vendored parser; needed as a rank multiplier (exported > private) and for `sigil symbols --public-only`. Cheap to capture now, expensive to retrofit.
- **`index.json` manifest** — removes ambiguity about what was indexed, when, and with which sigil version. Enables cache-busting on sigil version bumps.
- **`files.jsonl`** — `sigil map` (Phase 1) needs per-file metadata (lang, lines, title). Adopting codeix's schema avoids a second migration.

**Rationale for NOT adding `tokens` / `alias` / `project`:**
- No consumer in the 90-day roadmap.
- `project` is multi-project mount semantics we're explicitly dropping (§14.1 — MountTable goes away).

#### 14.8.3 Forward-compat reservations (Phase 1+)

Planned additions — called out here so we don't break compat again:

- `entities.jsonl` Phase 1: `rank?: f64`, `blast_radius?: {direct_callers, direct_files, transitive_callers}`.
- New files Phase 1+:
  - `.sigil/rank.json` — file-level PageRank scores (kept separate from per-entity rank for cheap recomputation).
  - `.sigil/cochange.json` — file-pair co-change weights mined from `git log --name-only`.

#### 14.8.4 Migration plan (Phase 0 day 1)

1. Bump manifest version to `"2"`.
2. Readers accept both `"1"` and `"2"`. Writers emit only `"2"`.
3. First `sigil index` run after upgrade regenerates `.sigil/*` in the new shape.
4. `.codeindex/` deleted if found, with a one-line stderr note on first encounter.

### 14.9 Scale — JSONL for portability + DuckDB for performance (Phase 0.5)

#### 14.9.1 Where JSONL breaks down

Per-record JSON sizes: Entity ~200 B, Reference ~100 B.

| Repo class | LOC | Entities | Refs | entities.jsonl | refs.jsonl | JSONL cold load |
|---|---|---|---|---|---|---|
| Small lib (sigil today) | 10k | 1k | 5k | 330 KB | 550 KB | < 50 ms |
| Typical product | 100k | 10k | 100k | 2 MB | 10 MB | ~200 ms |
| Large monorepo | 1M | 100k | 1M | 20 MB | 100 MB | 1–2 s |
| Very large (Linux, Chromium) | 30M | 1M | 10M | 200 MB | 1 GB | 10–30 s |

**Threshold where in-memory breaks: ~100 MB total, ~500k entities.** Above that, cold-start latency becomes unacceptable for interactive use.

#### 14.9.2 Decision: hybrid — JSONL source of truth, DuckDB derived

```
.sigil/
  index.json         ← source of truth, committable, diffable
  files.jsonl        ← source of truth
  entities.jsonl     ← source of truth
  refs.jsonl         ← source of truth
  cache.json         ← incremental-parse cache
  index.duckdb       ← DERIVED, gitignored, rebuilt on staleness
  index.duckdb.stamp ← staleness tracker (JSONL mtime + size hash)
```

**JSONL stays the contract.** Portable, human-readable, greppable, `jq`-able, git-diffable. Sigil's identity depends on this.

**DuckDB is the performance layer.** Built lazily; engaged automatically when the index exceeds a threshold.

#### 14.9.3 Why DuckDB (not SQLite, not Polars, not Parquet alone)

| Axis | JSONL + in-memory | SQLite + FTS5 | **DuckDB** | Polars | Parquet + DataFusion |
|---|---|---|---|---|---|
| Point lookups | hot: fast | fastest | fast | fast | fast |
| Analytical (rank/map/blast) | slow | **slow (row-oriented)** | **fastest (columnar + vectorized)** | fast | fast |
| FTS / search | substring | best-in-class | `LIKE` + `fts` ext | `LIKE` | `LIKE` |
| JSONL ingestion | native | INSERT loop | **`read_json_auto()` — zero import** | native | import step |
| Recursive CTEs (transitive blast) | N/A | yes | **yes** | no (dataframe) | yes |
| SQL REPL escape hatch | no | yes | **yes (`duckdb .sigil/index.duckdb`)** | no | no |
| Parquet path | no | no | **native** | yes | native |
| Binary footprint | 0 | ~1.5 MB | ~10 MB | ~8 MB | ~12 MB |
| Git-friendly | **yes** | no | no (derived, gitignored) | no | no |

**SQLite loses on analytical workload** — which is our entire Phase 1+ roadmap (rank, map, blast, context, review). Row-oriented scans of 10M-row tables for group-by queries are 5–20× slower than DuckDB's vectorized engine. SQLite's one compelling edge — FTS5 — doesn't actually matter for sigil, because `sigil search` is substring/prefix match on symbol names, not full-text over bodies.

**Polars** is a dataframe library, not a query engine. No SQL user-facing surface, no recursive CTEs for transitive-blast traversal.

**Parquet alone** — no query engine; you need DataFusion on top. At that point DuckDB is simpler and reads Parquet natively if we ever switch.

**Pure DuckDB (no JSONL)** — tempting but loses the portability contract. Teams committing `.sigil/` to git (graphify's pattern, which we want to enable) need diffable files. Tethers CI to a DuckDB binary. Debuggability drops.

#### 14.9.4 Auto-upgrade behavior

```
if total_jsonl_size > 50 MB OR --db flag:
    ensure .sigil/index.duckdb is fresh (rebuild if JSONL newer)
    route queries through DuckDB
else:
    in-memory Index from JSONL
```

Small repos stay fast and simple. Large repos get scale transparently. The user never has to choose. Threshold tunable via config.

#### 14.9.5 DuckDB schema (derived from JSONL)

No manual ETL. DuckDB reads JSONL natively:

```sql
CREATE VIEW entities AS SELECT * FROM read_json_auto('.sigil/entities.jsonl');
CREATE VIEW refs     AS SELECT * FROM read_json_auto('.sigil/refs.jsonl');
CREATE VIEW files    AS SELECT * FROM read_json_auto('.sigil/files.jsonl');

-- Materialized for query speed
CREATE TABLE entities_mat AS SELECT * FROM entities;
CREATE INDEX idx_entities_name ON entities_mat(name);
CREATE INDEX idx_entities_file ON entities_mat(file);

CREATE TABLE refs_mat AS SELECT * FROM refs;
CREATE INDEX idx_refs_name   ON refs_mat(name);
CREATE INDEX idx_refs_caller ON refs_mat(caller);
CREATE INDEX idx_refs_file   ON refs_mat(file);
```

Staleness check: compare `index.duckdb.stamp` to current JSONL mtimes + sizes. Mismatch → drop + rebuild. Rebuild is fast (zero-import from JSONL).

#### 14.9.6 Commands that benefit most

| Command | JSONL/in-memory | DuckDB | Speedup at scale (1M entities) |
|---|---|---|---|
| `sigil callers foo` | 100 ms (cold: 2 s) | 10 ms (cold: 200 ms) | 10× cold |
| `sigil search "parse"` | 200 ms | 30 ms | 7× |
| `sigil map --tokens N` | 1 s (heavy join) | 100 ms | 10× |
| `sigil blast foo` (transitive CTE) | hand-rolled, slow | native recursive CTE | 20× |
| `sigil review A..B` (diff + rank join) | 500 ms | 60 ms | 8× |
| `sigil query 'SELECT ...'` (SQL REPL) | not offered | **offered** | N/A |

#### 14.9.7 Phase 0.5 timeline (after Phase 0 ships)

| Day | Deliverable |
|---|---|
| 1 | Add `duckdb` crate as optional feature `db` (default off). Skeleton `src/query/duckdb_backend.rs`. |
| 2 | `Index::ensure_db()` — lazy init + staleness check. Views over JSONL + materialized tables + indexes. |
| 3 | Route `get_callers`, `get_callees`, `get_file_symbols`, `get_children`, `search` through DuckDB when engaged. Parity tests against in-memory backend. |
| 4 | Threshold-triggered auto-upgrade. Benchmark on Linux kernel corpus. |
| 5 | `sigil query 'SELECT ...'` REPL for power users. Update README with scale story. |

Total: ~1 week, deferred until after Phase 0 stabilizes. Users under 50 MB of index never notice it exists.

#### 14.9.8 `.gitignore` additions

```
.sigil/index.duckdb
.sigil/index.duckdb.stamp
.sigil/index.duckdb.wal
```

The JSONL artifacts stay committable; the derived DB never enters git.

#### 14.9.9 Acceptance criteria for Phase 0.5

1. `cargo build --no-default-features` works (DuckDB is opt-in).
2. `cargo build --features db` pulls in DuckDB; binary size increase ≤ 12 MB.
3. On a 1 GB synthetic index (simulated Chromium-scale), every query completes in < 100 ms hot, < 2 s cold.
4. Parity tests: every query returns identical results (modulo stable ordering) between in-memory and DuckDB backends.
5. `.sigil/index.duckdb` is correctly invalidated when JSONL changes.
6. `sigil query 'SQL'` is documented as unstable/power-user; output format is tabular markdown.
