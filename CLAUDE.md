# CLAUDE.md

## Project Overview

sigil is a Rust CLI tool for structural code fingerprinting and diffing. It uses tree-sitter (via codeix) to parse source files, extract code entities (functions, classes, methods), compute content hashes, and produce entity-level diffs.

## Build & Test

```bash
cargo build              # Build
cargo test               # Run all tests (unit + integration)
cargo test --lib         # Unit tests only
cargo test --test integration       # Index integration tests
cargo test --test diff_integration  # Diff integration tests
```

## Architecture

```
src/
  main.rs          — CLI (clap): sigil subcommands (index, diff, explore, search, symbols, children, callers, callees)
  entity.rs        — Entity and Reference structs (serde-serializable)
  hasher.rs        — BLAKE3 hashing (struct_hash, body_hash, sig_hash)
  signature.rs     — Signature extraction from source (language-aware)
  meta.rs          — Metaprogramming marker detection (decorators, derives)
  cache.rs         — Incremental indexing cache (.sigil/cache.json)
  writer.rs        — JSONL output writer
  index.rs         — Index orchestration + parse_single_file
  json_index.rs    — JSON file parsing (custom parser, not tree-sitter)
  yaml_index.rs    — YAML file parsing (custom parser, not tree-sitter)
  toml_index.rs    — TOML file parsing (custom parser, not tree-sitter)
  query.rs         — codeix SearchDb wrapper (load_index, explore, search, format helpers)
  git.rs           — Git operations (changed_files, file_at_ref)
  matcher.rs       — Entity matching across versions (exact/moved/renamed)
  classifier.rs    — Change classification (sig/body hash matrix)
  diff.rs          — Diff orchestration (git refs or direct file comparison → parse → match → classify)
  diff_json.rs     — Diff output structs (EntityDiff, DiffResult)
  inline_diff.rs   — Line-level diffs within entities
  change_detail.rs — Token-level change extraction
  output.rs        — DiffOutput intermediate model for formatters (terminal, markdown, JSON)
  formatter.rs     — Colored terminal output (format_terminal_v2 with FormatOptions)
  markdown_formatter.rs — GitHub-flavored Markdown output (format_markdown with MarkdownOptions)
```

## Key Dependencies

- **codeix** — tree-sitter parser + SearchDb code intelligence (git dependency from github.com/montanetech/codeix)
- **anyhow** — error handling (used by codeix Result types)
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

# Code intelligence queries (powered by codeix SearchDb)
sigil explore                            # Project structure overview
sigil search "parse_file"                # Full-text search across symbols, files, texts
sigil symbols src/main.rs                # List symbols in a file
sigil children src/entity.rs Entity      # Children of a class/module
sigil callers struct_hash                # Find all callers of a symbol
sigil callees build_index                # Find all callees of a symbol
```
