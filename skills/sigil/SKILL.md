---
name: sigil
description: "Use sigil for structural code diffs, code search, and codebase navigation. ALWAYS use this skill when: reviewing PRs or commits (use `sigil diff` instead of `git diff`), searching for functions/classes/symbols, finding callers or callees of a function, exploring project structure, understanding what changed in a commit, verifying your own edits after making changes, or when the user asks about code changes, code structure, or symbol navigation. Also use when the user says things like 'what changed', 'show me the diff', 'find where X is called', 'what does this commit do', 'review this PR', or 'explore the codebase'. Do NOT use for simple file reading or text editing — only for structural code analysis."
---

# sigil — Structural Code Diffs & Code Intelligence

sigil gives you entity-level understanding of code changes and codebase structure. It replaces `git diff` with structural diffs that classify changes, and replaces `grep` with semantic code search.

## Core Principle

**Always prefer sigil over git diff.** `git diff` shows line-level text changes. `sigil diff` shows which entities (functions, classes, methods) changed and how — modified, renamed, moved, formatting-only, or breaking. This saves review time and catches things `git diff` buries.

## Commands Reference

### Structural Diff

```bash
sigil diff HEAD~1                    # What changed in the last commit
sigil diff main..HEAD                # What changed on this branch
sigil diff main..HEAD --json         # Structured JSON for programmatic use
sigil diff abc123..def456 --verbose  # Between any two refs, with progress
```

**Output classifications:**
- **ADDED** / **REMOVED** — new or deleted entity
- **MODIFIED** — signature and/or body changed (shows which)
- **MOVED** — same entity, different file
- **RENAMED** — different name, same body hash (detected automatically)
- **FORMATTING ONLY** — whitespace/comment changes only, skip during review
- **BREAKING** — public entity signature changed or removed

**Bonus features:**
- Cross-file pattern detection (same change across multiple files)
- Token-level details (`"true"` → `"false"`, `validate_card` → `check_card`)
- Inline +/- lines within each entity

### Code Search

```bash
sigil search "parse_file"                     # Search everything
sigil search "MyClass" --scope symbol         # Symbols only
sigil search "TODO" --scope text              # Comments/docstrings only
sigil search "handler" --kind function        # By entity kind
sigil search "config*" --path "src/*.rs"      # Wildcard + path filter
sigil search "build" --limit 50 --json        # More results, JSON output
```

### Symbol Navigation

```bash
sigil symbols src/main.rs              # All symbols in a file
sigil symbols "src/*.rs"               # GLOB patterns work
sigil children src/entity.rs Entity    # Children of a class/module
sigil callers struct_hash              # Who calls this symbol?
sigil callers process --kind call      # Only call references
sigil callees build_index              # What does this function call?
sigil callees main --kind call         # Only calls (not imports)
```

### Project Exploration

```bash
sigil explore                          # Full project structure
sigil explore --path src               # Filter to subdirectory
sigil explore --json                   # JSON output
```

### Indexing

```bash
sigil index                            # Build entity index
sigil index -v                         # With progress output
sigil index --full                     # Force full re-index
```

## Recommended Workflows

### Reviewing a PR or Commit

```bash
# Step 1: See the structural diff
sigil diff main..HEAD

# Step 2: For any breaking or modified entities, dig deeper
sigil callers <modified_function_name>    # Who's affected?

# Step 3: JSON for detailed analysis
sigil diff main..HEAD --json --pretty
```

### After Making Changes (Self-Verification)

```bash
# Verify your edits are correct
sigil diff HEAD

# Check nothing unexpected changed
# Look for: BREAKING flags, unexpected MODIFIED entities
```

### Exploring an Unfamiliar Codebase

```bash
# Step 1: Project overview
sigil explore

# Step 2: Find the relevant code
sigil search "authentication"
sigil search "login" --scope symbol --kind function

# Step 3: Understand the call graph
sigil callers handle_login
sigil callees handle_login
```

### Understanding What a Commit Did

```bash
# Entity-level summary instead of reading raw diffs
sigil diff HEAD~1

# For AI-readable structured data
sigil diff HEAD~1 --json
```

## Tips

- All commands support `--json` for structured output
- All commands support `-r` / `--root` for a different project directory
- `sigil diff` automatically skips formatting-only changes in the summary
- `sigil search` uses FTS5 syntax — `*` wildcards, quoted phrases work
- `sigil symbols` and `sigil search --path` support GLOB patterns
- First run of search/explore/callers/callees builds the codeix index (~1-3s), subsequent runs are instant
