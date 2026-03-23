# sigil

Structural code fingerprinting and diffing. **See what actually changed.**

sigil parses source files using [tree-sitter](https://tree-sitter.github.io/) (via [codeix](https://github.com/montanetech/codeix)), extracts code entities (functions, classes, methods, types), computes content hashes, and produces structural diffs that tell you *what kind of change* happened — not just which lines changed.

## Why

`git diff` shows which lines of text changed. sigil shows which **entities** changed and **how**:

| git diff says | sigil diff says |
|---|---|
| 87 lines changed across 3 files | 1 function modified (body), 1 renamed, 12 formatting-only |
| red/green line pairs | `"true"` → `"false"` in `--commit` flag default |
| no signal about impact | ⚠ BREAKING: public signature changed |

For AI agents reviewing code, sigil's `--json` output provides structured change data instead of raw text diffs — enabling precise reviews instead of "this PR modifies several functions."

## How It Works

```
git show HEAD~1:file.py ──→ tree-sitter parse ──→ entities + hashes ──┐
                                                                       ├──→ match → classify → output
git show HEAD:file.py   ──→ tree-sitter parse ──→ entities + hashes ──┘
```

1. **Only changed files** are parsed (via `git diff --name-status`)
2. Each file is parsed into entities with three BLAKE3 hashes:
   - `struct_hash` — raw text (catches ANY change including whitespace)
   - `body_hash` — normalized body (ignores formatting, comments)
   - `sig_hash` — signature only (detects API changes)
3. Entities are matched across versions (name+file → cross-file move → body hash rename)
4. Changes are classified using the hash matrix (formatting-only / body-only / sig-only / both)
5. Token-level details extracted (which specific values changed within a line)

## Install

```bash
cargo install --git https://github.com/AnchorVault/sigil
```

Or build from source:

```bash
git clone https://github.com/AnchorVault/sigil
cd sigil
cargo build --release
# Binary at target/release/sigil
```

## Quick Start

### Structural Diff

```bash
# Diff the last commit
sigil diff HEAD~1

# Diff between branches
sigil diff main..feature-branch

# JSON output for CI/AI agents
sigil diff HEAD~1 --json
```

Example output:

```
src/payments.py
  ▸ MODIFIED  execute_payment (function)
  │ signature changed, body changed
  │   "true" → "false"
  │   validate_card → check_card
  │ ⚠ BREAKING

  ▸ ADDED  PaymentAuditLog (class)

  ▸ FORMATTING ONLY  calculate_total (function)

1 added, 0 removed, 1 modified, 0 moved, 0 renamed, 1 formatting only
⚠ BREAKING CHANGES DETECTED
```

### Entity Index

```bash
# Index the current project
sigil index

# Index with verbose output
sigil index -v

# Output to stdout as JSONL
sigil index --stdout

# Index specific files only
sigil index --files src/main.rs src/lib.rs

# Force full re-index (ignore cache)
sigil index --full
```

This writes `.sigil/entities.jsonl`, `.sigil/refs.jsonl`, and `.sigil/cache.json` to the project root.

### Code Intelligence

```bash
# Explore project structure (files grouped by directory)
sigil explore
sigil explore --path src

# Full-text search across symbols, files, and texts
sigil search "parse_file"
sigil search "MyClass" --scope symbol --kind class
sigil search "TODO" --scope text --json

# List all symbols in a file
sigil symbols src/main.rs
sigil symbols "src/*.rs"          # GLOB patterns supported

# Get children of a class or module
sigil children src/entity.rs Entity

# Find all callers/references to a symbol
sigil callers struct_hash
sigil callers process --kind call

# Find all symbols that a function calls
sigil callees build_index

```

All code intelligence commands support `--json` for structured output and `--root` to specify the project directory.

## Commands

### `sigil diff <REF_SPEC>`

Structural diff between two git refs.

```
sigil diff HEAD~1              # Compare with previous commit
sigil diff main..HEAD          # Compare branch with main
sigil diff abc123..def456      # Compare two specific commits
sigil diff HEAD~3 --json       # JSON output
sigil diff HEAD~1 --pretty     # Pretty-printed JSON
sigil diff HEAD~1 -v           # Verbose (show parse progress)
```

**Change classifications:**
- **ADDED** — new entity
- **REMOVED** — entity deleted (breaking if public)
- **MODIFIED** — entity changed (signature and/or body)
- **MOVED** — entity relocated to different file
- **RENAMED** — different name, same body hash
- **FORMATTING ONLY** — whitespace/comment changes only, no logic change

**Breaking change detection:** Flags when a public entity's signature changes or is removed/renamed.

**Cross-file patterns:** Detects when the same change is applied across multiple files.

### `sigil index`

Build the entity index for a project.

```
sigil index                    # Index current directory
sigil index -r /path/to/repo   # Index a different directory
sigil index --stdout            # JSONL to stdout
sigil index --full              # Ignore cache, re-parse everything
sigil index --no-refs           # Skip reference extraction (faster)
sigil index -v                  # Verbose progress
```

**Incremental indexing:** Only re-parses files that changed since the last run. Second run on an unchanged repo parses zero files.

### `sigil explore`

Project structure overview: files grouped by directory with counts.

```
sigil explore                  # Full project overview
sigil explore --path src       # Filter to a subdirectory
sigil explore --max-entries 50 # Limit output
sigil explore --json           # JSON output
```

### `sigil search <QUERY>`

Full-text search across symbols, files, and text blocks (FTS5 syntax, supports `*` wildcards).

```
sigil search "parse_file"                    # Search everything
sigil search "MyClass" --scope symbol        # Symbols only
sigil search "TODO" --scope text             # Text blocks only
sigil search "handler" --kind function       # Filter by kind
sigil search "utils/*" --path "src/*.rs"     # Filter by file path
sigil search "build" --limit 50 --json       # More results, JSON output
```

### `sigil symbols <FILE>`

List all symbols in a file. Supports GLOB patterns.

```
sigil symbols src/main.rs      # Symbols in one file
sigil symbols "src/*.rs"       # Symbols across matching files
sigil symbols src/main.rs --json
```

### `sigil children <FILE> <PARENT>`

Get children of a class or module.

```
sigil children src/entity.rs Entity
sigil children src/db.rs SearchDb --json
```

### `sigil callers <NAME>`

Find all callers/references to a symbol.

```
sigil callers struct_hash              # All references
sigil callers process --kind call      # Only call references
sigil callers MyClass --kind import    # Only imports
sigil callers build --json
```

### `sigil callees <CALLER>`

Find all symbols that a function calls.

```
sigil callees build_index
sigil callees main --kind call
sigil callees process_event --json
```

## Supported Languages

Tree-sitter grammars via [codeix](https://github.com/montanetech/codeix):

| Language | Extensions |
|---|---|
| Python | `.py` `.pyi` `.pyw` |
| Rust | `.rs` |
| JavaScript | `.js` `.mjs` `.cjs` `.jsx` |
| TypeScript | `.ts` `.mts` `.cts` `.tsx` |
| Go | `.go` |
| Java | `.java` |
| C | `.c` `.h` |
| C++ | `.cpp` `.cc` `.cxx` `.hpp` `.hxx` |
| Ruby | `.rb` `.rake` `.gemspec` |
| C# | `.cs` |
| Markdown | `.md` `.markdown` |

## Git Hook Setup

To automatically run `sigil index` after each commit:

```bash
# Create the hook
cat > .git/hooks/post-commit << 'EOF'
#!/bin/sh
sigil index --verbose 2>&1 | tail -1
EOF

chmod +x .git/hooks/post-commit
```

For a pre-push hook that runs structural diff:

```bash
cat > .git/hooks/pre-push << 'EOF'
#!/bin/sh
remote="$1"
# Show structural diff of what's being pushed
sigil diff origin/main..HEAD
EOF

chmod +x .git/hooks/pre-push
```

## JSON Output

`sigil diff --json` produces structured output for AI agents and CI pipelines:

```json
{
  "base_ref": "HEAD~1",
  "head_ref": "HEAD",
  "entities": [
    {
      "change": "modified",
      "name": "process_payment",
      "kind": "function",
      "file": "src/payments.py",
      "sig_changed": true,
      "body_changed": true,
      "breaking": true,
      "change_details": [
        { "kind": "value_changed", "description": "\"true\" → \"false\"" }
      ]
    }
  ],
  "patterns": [],
  "summary": {
    "added": 0,
    "removed": 0,
    "modified": 1,
    "moved": 0,
    "renamed": 0,
    "formatting_only": 0,
    "has_breaking_change": true
  }
}
```

## Integration with AI Agents

Add to your `CLAUDE.md` or agent instructions:

```markdown
Before reviewing changes, run: sigil diff main..HEAD --json
After making changes, run: sigil diff HEAD to verify your edits.
```

## CI/CD Example

```yaml
# GitHub Actions
- name: Structural diff
  run: |
    sigil diff ${{ github.event.pull_request.base.sha }}..${{ github.sha }} --json > diff.json

- name: Label breaking changes
  run: |
    if jq -e '.summary.has_breaking_change' diff.json; then
      gh pr edit --add-label "breaking-change"
    fi
```

## License

MIT
