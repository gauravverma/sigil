# Architecture

## Overview

sigil is a Rust CLI tool with two main capabilities:

1. **Structural indexing** — parse source files into entities with content hashes
2. **Structural diffing** — compare entities across git refs, classify changes
3. **Code intelligence** — search, navigate, and explore codebases (powered by codeix)

```
┌─────────────────────────────────────────────────────────────────┐
│                          sigil CLI                              │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────────────────┐│
│  │  index   │  │   diff   │  │  explore/search/symbols/       ││
│  │          │  │          │  │  callers/callees/children        ││
│  └────┬─────┘  └────┬─────┘  └──────────────┬─────────────────┘│
│       │              │                       │                  │
│  ┌────┴─────┐  ┌────┴──────────────┐  ┌────┴─────────────┐    │
│  │ index.rs │  │ diff.rs           │  │ query.rs         │    │
│  │          │  │ matcher.rs        │  │ (codeix SearchDb) │    │
│  │          │  │ classifier.rs     │  │                   │    │
│  │          │  │ inline_diff.rs    │  └───────────────────┘    │
│  │          │  │ change_detail.rs  │                            │
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
│  │              codeix (dependency)                   │          │
│  │  tree-sitter parsing · SearchDb · MountTable      │          │
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
| `query.rs` | Wraps codeix's `SearchDb`. Loads/builds the codeix index, provides formatted output for explore, search, symbols, children, callers, callees. |

## Data Flow

### `sigil index`

```
discover files (ignore crate, .gitignore-aware)
  → for each file: codeix parse → signature extract → meta detect → hash
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
build_index_to_db(root, fts=true, cache=true)
  → loads .codeindex/ if available, parses otherwise
  → creates in-memory SQLite with FTS5
  → db.search() / db.get_file_symbols() / db.get_callers() / db.get_callees()
  → format output: terminal or JSON
```

## Key Design Decisions

1. **No git2 dependency** — shell out to `git` commands. Simpler, always available, no C bindings.
2. **Three hashes per entity** — `struct_hash` (any change), `body_hash` (logic changes), `sig_hash` (API changes). The hash matrix enables precise change classification.
3. **codeix as library dependency** — reuse tree-sitter parsing and SearchDb. Don't reinvent the parser.
4. **Incremental by default** — `.sigil/cache.json` tracks file hashes. Only re-parses changed files.
5. **JSON-first for AI** — every command supports `--json`. The terminal output is for humans; the JSON output is for AI agents and CI.

## Dependencies

| Crate | Purpose |
|---|---|
| `codeix` | Tree-sitter parsing, SearchDb (in-memory SQLite + FTS5), code intelligence |
| `blake3` | Content hashing (fast, 16 hex char truncation) |
| `similar` | Line-level and word-level diffing |
| `clap` | CLI argument parsing (derive macros) |
| `colored` | Terminal color output |
| `serde` / `serde_json` | JSON serialization |
| `ignore` | .gitignore-aware file walking |
| `anyhow` | Error handling (codeix Result types) |
