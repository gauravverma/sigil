# CLAUDE.md

## Project Overview

sigil is a Rust CLI tool for structural code fingerprinting and diffing. It uses tree-sitter to parse source files, extract code entities (functions, classes, methods), compute content hashes, and produce entity-level diffs. Parsing and code-intelligence queries are fully in-house — no external indexer required.

## Build & Test

```bash
cargo build              # Build
cargo test               # Run all tests (unit + integration)
cargo test --lib         # Unit tests only
cargo test --test integration       # Index integration tests
cargo test --test diff_integration  # Diff integration tests
cargo test --test markdown_integration  # Markdown integration tests
```

## Architecture

```
src/
  lib.rs           — Library crate: re-exports all modules for use by Python bindings and tests
  main.rs          — CLI binary (clap): sigil subcommands (index, diff, explore, search, symbols, children, callers, callees)
  entity.rs        — Entity and Reference structs (serde-serializable)
  hasher.rs        — BLAKE3 hashing (struct_hash, body_hash, sig_hash)
  signature.rs     — Signature extraction from source (language-aware)
  meta.rs          — Metaprogramming marker detection (decorators, derives)
  cache.rs         — Incremental indexing cache (.sigil/cache.json)
  writer.rs        — JSONL output writer
  index.rs         — Index orchestration + parse_single_file
  json_index.rs    — JSON file parsing (custom parser, not tree-sitter); array item expansion, derived field marking, minified JSON normalization
  yaml_index.rs    — YAML file parsing (custom parser, not tree-sitter)
  toml_index.rs    — TOML file parsing (custom parser, not tree-sitter)
  markdown_index.rs — Markdown file parsing (custom parser: headings, code blocks, tables, lists, blockquotes, paragraphs, front matter)
  query/mod.rs     — Query helpers (load, explore_text, format_* for CLI output)
  query/index.rs   — In-house Index: loads .sigil/ jsonl, in-memory hash maps, callers/callees/search/explore
  parser/          — Vendored tree-sitter parser layer (11 languages); see src/parser/NOTICE
  git.rs           — Git operations (changed_files, file_at_ref)
  matcher.rs       — Entity matching across versions (exact/moved/renamed); parent-aware matching keys
  classifier.rs    — Change classification (sig/body hash matrix)
  diff.rs          — Diff orchestration (git refs or direct file comparison → parse → match → classify); minified JSON normalization
  diff_json.rs     — Diff output structs (EntityDiff, DiffResult)
  inline_diff.rs   — Line-level diffs within entities
  change_detail.rs — Token-level change extraction
  output.rs        — DiffOutput intermediate model for formatters; derived entity filtering, qualified JSON names, parent suppression
  formatter.rs     — Colored terminal output (format_terminal_v2 with FormatOptions); context truncation, derived line filtering
  markdown_formatter.rs — GitHub-flavored Markdown output (format_markdown with MarkdownOptions); context truncation, derived line filtering

python/
  Cargo.toml       — PyO3 crate (sigil-python) depending on sigil lib
  pyproject.toml   — maturin config; package name: sigil-diff, import name: sigil
  src/lib.rs       — Python bindings: diff_json, diff_files, diff_refs, index_json
  README.md        — Python API documentation
```

## Key Dependencies

- **tree-sitter** — AST parsing (vendored language parsers live in `src/parser/`, originally forked from codeix v0.5.0 under Apache-2.0; see `src/parser/NOTICE`)
- **anyhow** — error handling
- **blake3** — content hashing
- **similar** — line and word diffing
- **clap** — CLI argument parsing
- **colored** — terminal colors
- **toml** — TOML parsing

## Conventions

- All hashes are BLAKE3, truncated to 16 hex characters
- Entity output is sorted deterministically by (file, line_start)
- Incremental indexing: only re-parses changed files
- `sigil diff` shells out to git (no git2 dependency)
- `sigil diff` always exits 0 on success (error handling exits non-zero via `std::process::exit(3)`)
- JSON diff: parent-aware matching `(file, parent, name)` prevents cross-matching (e.g., `body.text` vs `header.text`)
- JSON diff: `_`-prefixed fields are marked as derived and suppressed from output
- JSON diff: array items expanded with identity key heuristic (`id` > `key` > `name` > `text` > `type`), positional fallback
- JSON diff: minified JSON auto-formatted before parsing for correct per-entity hashing
- JSON diff: parent objects suppressed when children carry the detail; qualified names used (e.g., `body.text`)
- Python bindings: `pip install sigil-diff`, `import sigil`; built via PyO3 + maturin

## Useful Commands

```bash
# Run sigil on its own codebase
sigil index -v
sigil diff HEAD~1

# JSON diff for AI review
sigil diff main..HEAD --json --pretty

# Markdown output for PRs
sigil diff main..HEAD --markdown

# Terminal with line numbers and code context
sigil diff HEAD~1 --lines --context

# Compare two files directly (no git required)
sigil diff --files old.py new.py

# Code intelligence queries (powered by the in-house Index over .sigil/)
sigil explore                            # Project structure overview
sigil search "parse_file"                # Full-text search across symbols, files, texts
sigil symbols src/main.rs                # List symbols in a file
sigil children src/entity.rs Entity      # Children of a class/module
sigil callers struct_hash                # Find all callers of a symbol
sigil callees build_index                # Find all callees of a symbol
```
