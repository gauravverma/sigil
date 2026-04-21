# `sigil diff` — detailed reference

`sigil diff` is the lower-level primitive behind `sigil review`. Use `diff` directly for raw entity-level changes without rank/blast/cochange enrichment. Use `sigil review` when you also want impact + co-change misses.

## Common invocations

```bash
sigil diff HEAD~1                       # what changed in the last commit
sigil diff main..HEAD                   # what changed on this branch
sigil diff main..HEAD --json            # structured JSON (script-friendly)
sigil diff abc123..def456 --verbose     # arbitrary refs, with progress
sigil diff --files old.py new.py        # compare two files directly (no git)
sigil diff HEAD~1 --lines               # show line numbers next to entity names
sigil diff HEAD~1 --context 5           # wider code snippets (default 3)
sigil diff HEAD~1 --no-context          # entity-only, no snippets
sigil diff HEAD~1 --markdown            # GitHub-flavored markdown (paste-ready)
sigil diff HEAD~1 --markdown --no-emoji # ASCII-only markdown
sigil diff HEAD~1 --summary --group     # one-line summary, grouped changes
sigil diff HEAD~1 --no-callers          # skip caller analysis for breaking changes
```

## Entity classifications

`sigil diff` uses three BLAKE3 hashes per entity (`struct_hash`, `body_hash`, `sig_hash`) to classify each change:

- **ADDED** / **REMOVED** — new or deleted entity
- **MODIFIED** — signature and/or body changed (output tells you which)
- **MOVED** — same body, different file
- **RENAMED** — different name, same body hash (detected automatically)
- **FORMATTING ONLY** — whitespace / comment-only changes; usually skip during review
- **BREAKING** — public-entity signature changed or removed

Renames and moves are matched across the commit pair — a `delete foo + add bar` with the same `body_hash` collapses into one **RENAMED** entry instead of two rows of noise.

## Exit codes

- `0` on success, **even when changes are present**
- `3` on error

Don't branch on the exit code to detect "did anything change" — read the output instead. The classification is inside the output, not the exit status.

## JSON output fields

Each entity entry in `--json` output carries:
- `name`, `file`, `line_start`, `line_end`, `kind`, `parent`
- `struct_hash`, `body_hash`, `sig_hash`
- `classification` (one of the kinds above)
- For **BREAKING**: `callers` — the list of references that touch this entity's public surface

## `--files` without an index

`sigil diff --files OLD NEW` works offline, without git and without `.sigil/`:

- Parses each file with its language's tree-sitter grammar (extension-based)
- Computes BLAKE3 hashes, matches entities by body/sig
- Outputs the same classified diff

Works on all 11 tree-sitter languages + JSON + YAML + TOML + Markdown. The data-format parsers handle cases tree-sitter can't: `"port": 8080 → 8443` detected as a structural value change, YAML key moves matched parent-aware, Markdown headings / code blocks / lists / tables entity-extracted.

## Using `sigil review` instead

`sigil review <refspec>` wraps `sigil diff` and adds:
- Rank-ordered blast radius per touched entity
- Co-change misses (files that *usually* change together but didn't this time)
- PR-shaped output that pastes straight into a GitHub comment

Reach for `review` when the user asks "review this PR" or "what changed on this branch" — save `diff` for "I want the raw structural change list for a script."
