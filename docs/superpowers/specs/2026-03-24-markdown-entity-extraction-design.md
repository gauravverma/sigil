# Markdown Entity Extraction for Sigil

**Date:** 2026-03-24
**Status:** Approved

## Problem

Sigil recognizes markdown files during indexing (they appear in "files changed" counts), but its parser extracts zero entities from them. Since sigil diffs operate at the entity level, markdown files are silently dropped from diff output. Users must fall back to `git diff` for any markdown changes.

## Goal

Add a custom markdown parser (`markdown_index.rs`) that extracts structural entities from `.md`/`.markdown`/`.mdx` files, enabling entity-level diffs for markdown content. This applies to all use cases: documentation repos, in-repo docs, MDX components, content management systems, and general completeness.

## Entity Model

| Construct | `kind` | `name` | `parent` | `sig` |
|---|---|---|---|---|
| `# Heading` | `"section"` | Heading text (e.g., `"Installation"`) | parent section or `None` | Full heading line (e.g., `"# Installation"`) |
| Front matter block | `"frontmatter"` | `"frontmatter"` | `None` | `None` |
| Front matter keys | (from YAML parser) | Key name | `"frontmatter"` | (YAML parser provides) |
| Fenced code block | `"code_block"` | First line preview, truncated to 60 chars | enclosing section | Language tag (e.g., `"python"`) or `None` |
| Table | `"table"` | First row preview, truncated to 60 chars | enclosing section | `None` |
| Blockquote | `"blockquote"` | First line preview, truncated to 60 chars | enclosing section | `None` |
| List | `"list"` | First item preview, truncated to 60 chars | enclosing section | `"ordered"` or `"unordered"` |
| Paragraph | `"paragraph"` | First line preview, truncated to 60 chars | enclosing section | `None` |

All entities have `meta: None` — no metaprogramming markers apply to markdown constructs.

### Heading Nesting

Headings form a hierarchy: `h2` parents under `h1`, `h3` under `h2`, etc. A heading at the same or higher level closes the previous section at that level. For example, `## A` followed by `## B` are siblings under their parent `h1`, not nested.

### Section Spans

A heading entity's `line_start` is the heading line. Its `line_end` is the line before the next same-or-higher-level heading, or EOF. The body hash covers this full span, so content changes within a section are detected even if no leaf entity specifically changed.

**Overlapping spans are intentional.** A section entity's span encompasses all child entities within it (code blocks, paragraphs, sub-headings, etc.). This means a content change in a child entity will also change the parent section's `struct_hash` and `body_hash`. This is consistent with JSON/YAML/TOML parsers where parent objects span their children, and is the correct behavior — it enables detecting "something changed in this section" at every level of the hierarchy.

### Content-Derived Names

Block elements (code blocks, tables, blockquotes, lists, paragraphs) use the first line/item of their content as their name, truncated to 60 characters with `...` suffix. This provides stable identity across insertions (unlike positional numbering) and useful context in diff output.

### Code Block Language Tags

The language info string on fenced code blocks (e.g., `` ```python ``) is stored in the `sig` field, not in the kind or name. This means:
- A language tag change is a `sig_hash` change
- A content change is a `body_hash` change
- Clean separation of metadata vs content changes

## Parser Architecture

### New File: `src/markdown_index.rs`

Public API matching the existing custom parser signature:

```rust
pub fn parse_markdown_file(
    source: &str,
    file_path: &str,
) -> Result<(Vec<Entity>, Vec<Reference>), String>
```

### Single-Pass State Machine

States:

```
Normal → InFrontMatter  (if `---` at line 1)
Normal → InFencedCode   (if ``` or ~~~)
Normal → InTable         (if line matches `| ... |` and next line is a separator row like `|---|---|`)
Normal ← InFrontMatter  (on closing `---`)
Normal ← InFencedCode   (on matching closing fence)
Normal ← InTable         (on first non-table line)
```

In `Normal` state, each line is classified:
- `# ...` — heading: push/pop heading stack
- `> ...` — accumulate blockquote
- `- ` / `* ` / `1. ` — accumulate list
- blank line — flush accumulated paragraph/list/blockquote
- anything else — accumulate paragraph

### Heading Stack

A `Vec<(level, name, line_start)>` tracks open sections. When a new heading at level N is encountered, all sections at level >= N are closed (their `line_end` is set to the previous line), and the new heading is pushed.

### Front Matter Delegation

When `---` is detected at line 1, the YAML content between the opening and closing `---` delimiters is extracted (**delimiters stripped, only the YAML body is passed**) and fed to the existing `parse_yaml_file()`. The markdown file's path is passed as `file_path` so entities get the correct `file` field. Returned entities get:
- Line offsets adjusted (shifted by front matter start line)
- Parent set to `"frontmatter"`

This reuses proven YAML parsing code and provides key-level diff granularity for free.

### Hashing

Same as other custom parsers:
- `struct_hash` — from raw source bytes of the entity span via `hasher::struct_hash()`
- `body_hash` — via `hasher::body_hash_raw()` (**not** `body_hash()` — the non-raw variant strips lines starting with `#` as comments, which would incorrectly discard heading lines)
- `sig_hash` — from the `sig` field via `hasher::sig_hash()`

### Output

Entities sorted by `(file, line_start)`, deterministic — consistent with all other parsers.

## Integration

Language detection for markdown extensions (`md`, `markdown`, `mdx` → `"markdown"`) must be added in **all four** locations where extension-to-language mapping exists. These are currently duplicated across the codebase:

### 1. `src/index.rs` — `build_index()` inline mapping

Add `"md" | "markdown" | "mdx" => "markdown"` to the extension match block.

### 2. `src/index.rs` — `parse_single_file()` dispatcher

Add `"markdown" => parse_markdown_file(source, file_path)` branch.

### 3. `src/index.rs` — `discover_source_files()` extension filter

Add `md`, `markdown`, `mdx` to the recognized extensions so markdown files are discovered during project indexing.

### 4. `src/diff.rs` — `compute_diff()` and `detect_lang()` inline mappings

Add `"md" | "markdown" | "mdx" => "markdown"` to **both** mapping locations in diff.rs. Without this, `sigil diff` would silently skip markdown files even though `sigil index` processes them.

### Changes to `src/main.rs`

- Add `mod markdown_index;` declaration.

### No Changes Needed

The following are entity-generic and work with any entity kind:
- `entity.rs` — kind is a free-form string
- `matcher.rs` / `classifier.rs` — operate on `Entity` structs
- `formatter.rs` / `markdown_formatter.rs` — format any entity kind
- `cache.rs` — incremental caching is file-path-based

## Edge Cases

### Handled

- **Nested fences** — state machine tracks the opening fence string, only closes on exact match
- **Code blocks inside blockquotes** — fenced code blocks within `>` prefixed lines are treated as part of the enclosing blockquote entity (not extracted as separate code blocks), since every line including the fence markers has the `>` prefix. The entire blockquote including any embedded code is one entity.
- **Empty sections** — heading with no content produces entity with matching `line_start`/`line_end` and empty body hash
- **No headings** — all elements are top-level (`parent = None`)
- **Front matter only at start** — `---` on line 1 triggers front matter; `---` elsewhere is a horizontal rule (ignored)
- **List continuation** — indented lines after a list item are part of the same list; blank line + non-indented non-list line ends the list

### Explicit Non-Goals (v1)

- **Setext headings** (`===`/`---` underlines) — treated as paragraph text; rare in practice
- **Inline markup** — bold, italic, links, images are not entities; captured in body hashes
- **HTML blocks** — raw HTML treated as paragraph text
- **Nested blockquotes** (`>> `) — flattened into single blockquote entity
- **Nested lists** — sub-lists are part of parent list entity
- **MDX components** — JSX/TSX components embedded in `.mdx` files are treated as paragraph text
- **Indented code blocks** — only fenced code blocks are recognized; 4-space indented code blocks are treated as paragraph content

All can be added incrementally without breaking the entity model.

## Testing Strategy

### Unit Tests (in `src/markdown_index.rs`, `#[cfg(test)]` module)

1. Basic heading hierarchy — h1 with nested h2/h3, verify parent relationships and line spans
2. Front matter delegation — YAML keys as children of `frontmatter` with correct line offsets
3. Code blocks — fenced with language tag, fenced without, nested fence characters
4. Tables — entity span and name preview
5. Blockquotes — single and multi-line accumulation
6. Lists — ordered/unordered, sig values, continuation lines
7. Paragraphs — blank-line separation, each becomes own entity
8. Mixed document — realistic file with all construct types, verify complete entity list
9. No headings — all elements top-level
10. Empty file — returns empty entity vec
11. Heading-only file — sections with no content between them

### Integration Tests (`tests/markdown_integration.rs`)

- Markdown fixture file in `tests/fixtures/`
- Run `parse_single_file()` and verify entity count, kinds, parent structure
- Diff between two markdown fixtures, verify changes are detected
