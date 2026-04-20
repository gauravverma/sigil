# Giving AI coding agents a deterministic map

_sigil 0.3.0 · released 2026-04-20_

AI coding agents spend most of their token budget on one thing: figuring
out where they are. An agent lands in an unfamiliar repo and starts
grepping — cast a wide net, read a few files, grep again, follow the
refs, read more files. By the time it's oriented, you've spent $0.80 on
warm-up and the agent hasn't edited anything yet.

This is the pattern sigil 0.3.0 is built against.

## What changed

Up to 0.2.4, sigil was a structural diff tool: two git refs in, an
entity-level change report out. Useful for PR review, but not reaching
far into the agent workflow. 0.3.0 keeps the diff engine and adds the
pieces that make it a real orientation layer:

- **`sigil map`** — budget-aware ranked digest. Top files by PageRank
  over the repo's own import graph, top symbols per file by blast
  radius. Drop into an agent's context for cold-start.
- **`sigil context <symbol>`** — signature, direct callers, direct
  callees, related types. One call replaces the "read six files to
  understand one function" loop.
- **`sigil review A..B`** — structural PR review with rank, blast
  radius, and co-change misses. Instead of `git diff`.
- **`sigil blast <symbol>`** — impact summary: how many files depend on
  this, how far the change propagates.
- **`sigil duplicates`** — clone detection. Free, because sigil already
  computes content hashes per entity.
- **Platform installers** for Claude Code, Cursor, Codex, Gemini CLI,
  OpenCode, Aider, GitHub Copilot CLI, plus `sigil hook install` for
  git-driven auto-rebuild.

All of it deterministic. No LLM in the code path. sigil parses source
with tree-sitter, hashes with BLAKE3, and runs PageRank over the
reference table it already builds for diffing. Same engine, wider
surface.

## Why this matters for agents

The thing an agent needs most and grep provides least is **structural
precision**. `grep foo` matches a symbol name inside a string literal,
inside a comment, inside a doc example. `sigil callers foo` returns the
exact set of references — aliased uses included, strings excluded —
because it's querying a parsed index, not matching text.

Run this on sigil itself, 0.2.4 refspec `HEAD~3..HEAD`, BPE-accurate
counts via `o200k_base` (GPT-4o/o3 tokenizer):

| Query | Raw tokens | Sigil tokens | Ratio |
|---|---:|---:|---:|
| PR review (3 commits) | 195,003 | 5,572 | **35.00×** |
| Context for `Entity` | 91,937 | 467 | **196.87×** |
| Cold-start orientation | 44,733 | 2,786 | **16.06×** |

Median reduction: **35.00×**. The eye-catcher is the context query —
reading every file that references `Entity` is ~92 K tokens of source;
the bundle sigil produces is 467 tokens. That's the difference between
the agent burning nearly half its context window on orientation vs one
tool call returning 0.2% of it.

These numbers come from `sigil benchmark`, which is [a shipped command](https://github.com/gauravverma/sigil/blob/main/src/benchmark.rs).
Reproducible on any codebase:

```bash
cargo install sigil --features tokenizer
cd your-repo
sigil index
sigil benchmark --refspec main..HEAD --tokenizer o200k_base
```

## The honest parts

Ratios scale with corpus size. On a 500-line library, sigil gives you
structural clarity, not compression — the whole repo fits in an
agent's context anyway. The wins come on real codebases with enough
coupling that "read what you need" is expensive.

Token reduction isn't the product. It's the consequence. **Success
metric is whether an agent can edit your code correctly with less
context.** We'll measure that with proper SWE-bench-style evaluation
in the next phase; for now, token-to-answer is the honest smoke test.

Sigil exports nothing about semantics it doesn't compute deterministically.
It won't tell you "this function implements the observer pattern" or
"this has a race condition." It tells you who calls what, how far
changes propagate, and what's duplicated. That's the whole pitch.

## Platform distribution

A tool nobody reaches for doesn't help anyone. 0.3.0 ships seven
platform integrations that surface sigil's commands to the agent
without prompt-leading:

```bash
sigil claude install     # CLAUDE.md + PreToolUse hook on Grep|Glob
sigil cursor install     # .cursor/rules/sigil.mdc (alwaysApply)
sigil codex install      # AGENTS.md + .codex/hooks.json Bash hook
sigil gemini install     # GEMINI.md + .gemini/settings.json BeforeTool
sigil opencode install   # .opencode/plugins/sigil.js + opencode.json
sigil aider install      # AGENTS.md block (no hook mechanism)
sigil copilot install    # ~/.copilot/skills/sigil/SKILL.md
sigil hook install       # .git/hooks/post-commit auto-rebuild
```

Each installer writes a **capability-describing block** — it lists
what sigil commands exist and when each one fits. It never says
"prefer sigil over grep" or similar preference language. A unit test
in `src/install/mod.rs` enforces this: we parse the capability text
and assert it contains zero preference phrases. The difference matters:
description is fair environmental signal; preference is a prompt-lead
that invalidates any eval.

Every installer is idempotent, preserves user content outside sigil's
marker block, preserves sibling user hooks, and cleans up on uninstall
(deletes files it created solely). The installer pattern is lifted
from [graphify](https://github.com/safishamsi/graphify), which proved
it works across 15+ agent platforms — we adapted it for sigil's
deterministic stack.

## Under the hood

Phase 0 (eight commits on `agent-adoption`) removed sigil's last
external runtime dependency on `codeix`. The parser layer was vendored
(Apache-2.0) into `src/parser/`, the query layer was rewritten in-house
as a hash-map Index over `.sigil/*.jsonl`. Sigil now owns the full
stack: parsing, indexing, ranking, diffing, output. Next planned
addition — a DuckDB backend for indexes above 500k entities (§14.9 of
the plan) — stays behind a feature flag; the JSONL format remains the
portable source of truth.

Phase 1 added rank + blast + map + context + review + blast + benchmark +
duplicates, then the eight installers. ~525 unit tests cover every new
command; installers run through marker-preservation fuzz tests. The
invariant that *sigil must never invalidate a user's custom hook or
capability-document content* is enforced in CI.

Phase 2 is where the open questions live: real agent-in-the-loop
evaluation, cross-repo benchmarks, LSP integration for higher-precision
caller resolution, DuckDB-backed scale past 500k entities, and a wider
worked-example corpus. The roadmap is in `agent-adoption-plan.md` —
it's public, opinionated, and subject to correction.

## Try it

```bash
cargo install sigil
cd your-repo
sigil index
sigil claude install       # or cursor / codex / gemini / opencode / …
sigil map --tokens 4000    # drop this into an agent context
sigil context <some-symbol> --budget 1500
sigil benchmark            # your own number, not ours
```

Repo: https://github.com/gauravverma/sigil

The [`worked/`](worked/) directory has sigil indexed against its own
source — every artifact, honest notes about wins and misses, reproducible
numbers. Same format we'll accept for external contributions.

No LLM in the code path. No embeddings. No cloud service. Just
tree-sitter and BLAKE3 and a ranked reference table — enough structure
to make an agent 16–200× cheaper per answer on the questions it asks
most.
