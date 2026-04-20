# Architecture

## Overview

sigil is a Rust CLI tool with two main capabilities:

1. **Structural indexing** — parse source files into entities with content hashes
2. **Structural diffing** — compare entities across git refs, classify changes
3. **Code intelligence** — search, navigate, and explore codebases via the in-house `query::index::Index`

```
┌─────────────────────────────────────────────────────────────────┐
│                          sigil CLI                              │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────────────────┐│
│  │  index   │  │   diff   │  │  explore/search/symbols/       ││
│  │          │  │          │  │  callers/callees/children      ││
│  └────┬─────┘  └────┬─────┘  └──────────────┬─────────────────┘│
│       │              │                       │                  │
│  ┌────┴─────┐  ┌────┴──────────────┐  ┌────┴─────────────┐    │
│  │ index.rs │  │ diff.rs           │  │ query/mod.rs     │    │
│  │          │  │ matcher.rs        │  │ query/index.rs   │    │
│  │          │  │ classifier.rs     │  │ (Index loader +  │    │
│  │          │  │ inline_diff.rs    │  │  hash-map queries)│    │
│  │          │  │ change_detail.rs  │  └───────────────────┘    │
│  │          │  │ formatter.rs      │                            │
│  └────┬─────┘  └────┬─────────────┘                            │
│       │              │                                          │
│  ┌────┴──────────────┴──────────────────────────────┐          │
│  │            Shared modules                         │          │
│  │  entity.rs  hasher.rs  signature.rs  meta.rs     │          │
│  │  cache.rs   writer.rs  git.rs                    │          │
│  └──────────────────┬───────────────────────────────┘          │
│                     │                                           │
│  ┌──────────────────┴───────────────────────────────┐          │
│  │       parser/  (vendored tree-sitter layer)       │          │
│  │  treesitter.rs  languages.rs  helpers.rs          │          │
│  │  + 11 language extractors (rust, python, ts, …)   │          │
│  │       see src/parser/NOTICE for attribution       │          │
│  └───────────────────────────────────────────────────┘          │
└─────────────────────────────────────────────────────────────────┘
```

## Module Responsibilities

### CLI & Entry Point

| Module | Responsibility |
|---|---|
| `main.rs` | CLI argument parsing (clap). Dispatches to index, diff, or query modules. |

### Indexing Pipeline

| Module | Responsibility |
|---|---|
| `index.rs` | Orchestrates file discovery → parsing → hashing → output. Provides `build_index()` for bulk indexing and `parse_single_file()` for the diff engine. |
| `entity.rs` | `Entity` and `Reference` struct definitions. Serializable with serde. |
| `hasher.rs` | BLAKE3 hash computation: `struct_hash` (raw text), `body_hash` (normalized, ignores formatting), `sig_hash` (signature only). All truncated to 16 hex chars. |
| `signature.rs` | Extracts function/method/class signatures from source. Language-aware (brace vs colon delimiters). Handles multi-line signatures, decorators, where clauses. |
| `meta.rs` | Detects metaprogramming markers: Python decorators, Rust derives, Java/C# annotations, TypeScript decorators, Ruby DSL methods. |
| `cache.rs` | Incremental indexing via `.sigil/cache.json`. Tracks file content hashes to skip unchanged files. |
| `writer.rs` | JSONL serialization to `.sigil/entities.jsonl` and `.sigil/refs.jsonl`. |

### Diff Pipeline

| Module | Responsibility |
|---|---|
| `diff.rs` | Orchestrates: git changed files → parse both versions → match → classify → inline diff → change details → cross-file patterns. |
| `git.rs` | Git operations via `std::process::Command`. `changed_files`, `file_at_ref`, `resolve_ref`, `parse_ref_spec`. No git2 dependency. |
| `matcher.rs` | 4-layer entity matching: exact (file+name) → moved (cross-file) → renamed (body hash) → added/removed. |
| `classifier.rs` | Change classification using sig_hash/body_hash matrix. Detects formatting-only, modified, moved, renamed. Flags breaking changes on public entities. |
| `inline_diff.rs` | Line-level diffs within entities using `similar` crate. Shows actual +/- lines. |
| `change_detail.rs` | Token-level change extraction. Pairs similar lines, does word-level diff to find specific changed tokens (e.g., `"true"` → `"false"`). |
| `diff_json.rs` | Output structs: `EntityDiff`, `DiffResult`, `DiffSummary`, `CrossFilePattern`, `ChangeDetail`. |
| `formatter.rs` | Colored terminal output. Groups changes by file. Shows inline diffs and change details. |

### Code Intelligence

| Module | Responsibility |
|---|---|
| `query/mod.rs` | Loads the in-house Index and renders CLI output (`explore_text`, `format_entities`, `format_refs`, `format_search_hits`). |
| `query/index.rs` | The `Index` struct: loads `.sigil/entities.jsonl` + `refs.jsonl` into in-memory hash maps, exposes `get_callers`, `get_callees`, `get_file_symbols`, `get_children`, `search`, `explore_dir_overview`, `explore_files_capped`, `list_projects`. |
| `parser/` | Vendored tree-sitter extractors for 11 languages (originally forked from codeix v0.5.0 under Apache-2.0; see `src/parser/NOTICE`). |

## Data Flow

### `sigil index`

```
discover files (ignore crate, .gitignore-aware)
  → for each file: tree-sitter parse (src/parser/) → signature extract → meta detect → hash
  → sort by (file, line_start)
  → write .sigil/entities.jsonl, refs.jsonl, cache.json
```

### `sigil diff`

```
git diff --name-status base..head → changed files
  → for each file: git show ref:file → parse_single_file (both versions)
  → match_entities(old, new) → 4-pass matching
  → classify each match → EntityDiff
  → compute inline_diff (line-level) per modified entity
  → extract change_details (token-level) per modified entity
  → detect_patterns (cross-file)
  → output: terminal (colored) or JSON
```

### `sigil search/symbols/callers/callees`

```
Index::load(root)
  → read .sigil/entities.jsonl + refs.jsonl into Vec<Entity> / Vec<Reference>
  → precompute 5 lookup maps (by name, by file, ref target, ref caller, ref file)
  → idx.search() / get_file_symbols() / get_callers() / get_callees() / get_children()
  → format output: terminal or JSON
```

No `.codeindex/` directory. No external indexer. The in-memory Index fits
comfortably up to ~500k entities; above that, Phase 0.5 of the adoption
plan adds a DuckDB-backed backend built lazily from the same JSONL.

## Key Design Decisions

1. **No git2 dependency** — shell out to `git` commands. Simpler, always available, no C bindings.
2. **Three hashes per entity** — `struct_hash` (any change), `body_hash` (logic changes), `sig_hash` (API changes). The hash matrix enables precise change classification.
3. **Self-contained parsing + code intelligence** — tree-sitter grammars are direct dependencies; symbol extraction and queries live in `src/parser/` and `src/query/`. No external indexer, no SQLite file, no .codeindex/ directory.
4. **Incremental by default** — `.sigil/cache.json` tracks file hashes. Only re-parses changed files.
5. **JSON-first for AI** — every command supports `--json`. The terminal output is for humans; the JSON output is for AI agents and CI.

## Dependencies

| Crate | Purpose |
|---|---|
| `tree-sitter` + `tree-sitter-<lang>` | AST parsing (feature-gated per language) |
| `blake3` | Content hashing (fast, 16 hex char truncation) |
| `similar` | Line-level and word-level diffing |
| `clap` | CLI argument parsing (derive macros) |
| `colored` | Terminal color output |
| `serde` / `serde_json` | JSON serialization |
| `ignore` | .gitignore-aware file walking |
| `anyhow` | Error handling |
