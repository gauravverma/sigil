# Markdown Entity Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a custom markdown parser that extracts structural entities (headings, code blocks, tables, lists, blockquotes, paragraphs, front matter) from `.md`/`.markdown`/`.mdx` files, enabling entity-level diffs.

**Architecture:** Single-pass line-by-line state machine parser in `src/markdown_index.rs`, following the same pattern as `json_index.rs`/`yaml_index.rs`/`toml_index.rs`. Front matter delegates to the existing YAML parser. Integration touches 4 language-mapping sites across `index.rs` and `diff.rs`.

**Tech Stack:** Rust, BLAKE3 (hashing), existing `hasher` + `yaml_index` modules, no new dependencies.

**Spec:** `docs/superpowers/specs/2026-03-24-markdown-entity-extraction-design.md`

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `src/markdown_index.rs` | Create | Markdown parser: state machine, entity extraction, hashing, name truncation |
| `src/main.rs` | Modify (line 1-22) | Add `mod markdown_index;` declaration |
| `src/index.rs` | Modify (lines 21-29, 141-157, 249-268) | Add `"markdown"` to dispatcher, language mapping, and file discovery |
| `src/diff.rs` | Modify (lines 52-63, 190-197) | Add `"markdown"` to both language mapping sites |
| `tests/fixtures/sample.md` | Create | Markdown test fixture with all construct types |
| `tests/fixtures/sample_v2.md` | Create | Modified version of sample.md for diff testing |
| `tests/markdown_integration.rs` | Create | Integration tests for indexing and diffing markdown |

---

### Task 1: Create test fixture files

**Files:**
- Create: `tests/fixtures/sample.md`
- Create: `tests/fixtures/sample_v2.md`

- [ ] **Step 1: Create the primary markdown fixture**

```markdown
---
title: Getting Started
author: Jane Doe
tags: [rust, cli]
---

# Installation

This guide covers installing sigil on your system.

## Prerequisites

You need the following tools:

- Rust 1.70+
- Git 2.30+
- A terminal emulator

## Steps

Run the following command:

```bash
curl -sSf https://example.com/install.sh | sh
```

Then verify the installation:

```python
import subprocess
result = subprocess.run(["sigil", "--version"], capture_output=True)
print(result.stdout)
```

## Configuration

| Option | Default | Description |
|--------|---------|-------------|
| verbose | false | Enable verbose output |
| color | true | Enable colored output |

> Note: Configuration is optional. Sigil works out of the box
> with sensible defaults for most use cases.

# Usage

Write your first diff command:

```bash
sigil diff HEAD~1
```

## Advanced Usage

For comparing specific files:

1. Choose the old version
2. Choose the new version
3. Run the comparison

Inline code and **bold** text are part of paragraphs.
```

- [ ] **Step 2: Create the modified fixture for diff testing**

`sample_v2.md` has these differences from `sample.md`:
- Changed front matter (`author: John Smith`)
- Modified heading text (`# Setup` instead of `# Installation`)
- Added a new section (`## Troubleshooting` under `# Setup`)
- Modified a code block (different bash command)
- Removed a table row
- Changed a blockquote
- Added a new paragraph

```markdown
---
title: Getting Started
author: John Smith
tags: [rust, cli, tools]
---

# Setup

This guide covers setting up sigil on your system.

## Prerequisites

You need the following tools:

- Rust 1.70+
- Git 2.30+
- A terminal emulator

## Steps

Run the following command:

```bash
cargo install sigil
```

Then verify the installation:

```python
import subprocess
result = subprocess.run(["sigil", "--version"], capture_output=True)
print(result.stdout)
```

## Troubleshooting

If installation fails, check your Rust toolchain version.

## Configuration

| Option | Default |
|--------|---------|
| verbose | false |

> Important: Always configure sigil before first use
> to ensure optimal performance.

# Usage

Write your first diff command:

```bash
sigil diff HEAD~1
```

## Advanced Usage

For comparing specific files:

1. Choose the old version
2. Choose the new version
3. Run the comparison

Inline code and **bold** text are part of paragraphs.

This is a new paragraph added in v2.
```

- [ ] **Step 3: Commit the fixtures**

```bash
git add tests/fixtures/sample.md tests/fixtures/sample_v2.md
git commit -m "test: add markdown fixtures for entity extraction"
```

---

### Task 2: Scaffold `markdown_index.rs` with helper types and the public API

**Files:**
- Create: `src/markdown_index.rs`
- Modify: `src/main.rs:1-22`

- [ ] **Step 1: Write the failing test for empty file**

In `src/markdown_index.rs`:

```rust
use crate::entity::{Entity, Reference};
use crate::hasher;

/// Parse a markdown file and extract structural entities.
pub fn parse_markdown_file(
    source: &str,
    file_path: &str,
) -> Result<(Vec<Entity>, Vec<Reference>), String> {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_file() {
        let (entities, refs) = parse_markdown_file("", "test.md").unwrap();
        assert!(entities.is_empty());
        assert!(refs.is_empty());
    }
}
```

- [ ] **Step 2: Add module declaration to main.rs**

Add `mod markdown_index;` after the existing `mod yaml_index;` line (line 14 in `src/main.rs`).

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --lib markdown_index::tests::parse_empty_file`
Expected: FAIL with "not yet implemented"

- [ ] **Step 4: Implement minimal parse_markdown_file that returns empty for empty input**

Replace `todo!()` with:

```rust
pub fn parse_markdown_file(
    source: &str,
    file_path: &str,
) -> Result<(Vec<Entity>, Vec<Reference>), String> {
    if source.trim().is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let lines: Vec<&str> = source.lines().collect();
    let mut entities = Vec::new();

    // TODO: state machine parser

    entities.sort_by(|a: &Entity, b: &Entity| a.line_start.cmp(&b.line_start));
    Ok((entities, Vec::new()))
}
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --lib markdown_index::tests::parse_empty_file`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add src/markdown_index.rs src/main.rs
git commit -m "feat: scaffold markdown_index.rs with public API and empty-file handling"
```

---

### Task 3: Implement heading parsing with section hierarchy

**Files:**
- Modify: `src/markdown_index.rs`

- [ ] **Step 1: Write the failing test for basic headings**

```rust
#[test]
fn parse_heading_hierarchy() {
    let source = "# Top\n\nSome text.\n\n## Sub A\n\nContent A.\n\n## Sub B\n\nContent B.\n\n### Deep\n\nDeep content.\n";
    let (entities, _) = parse_markdown_file(source, "test.md").unwrap();

    let sections: Vec<&Entity> = entities.iter().filter(|e| e.kind == "section").collect();
    assert_eq!(sections.len(), 4);

    let top = sections.iter().find(|e| e.name == "Top").unwrap();
    assert!(top.parent.is_none());
    assert_eq!(top.line_start, 1);
    assert_eq!(top.sig.as_deref(), Some("# Top"));

    let sub_a = sections.iter().find(|e| e.name == "Sub A").unwrap();
    assert_eq!(sub_a.parent.as_deref(), Some("Top"));

    let sub_b = sections.iter().find(|e| e.name == "Sub B").unwrap();
    assert_eq!(sub_b.parent.as_deref(), Some("Top"));

    let deep = sections.iter().find(|e| e.name == "Deep").unwrap();
    assert_eq!(deep.parent.as_deref(), Some("Sub B"));
}

#[test]
fn parse_heading_only_file() {
    let source = "# A\n## B\n## C\n";
    let (entities, _) = parse_markdown_file(source, "test.md").unwrap();
    let sections: Vec<&Entity> = entities.iter().filter(|e| e.kind == "section").collect();
    assert_eq!(sections.len(), 3);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib markdown_index::tests::parse_heading`
Expected: FAIL

- [ ] **Step 3: Implement the name truncation helper and heading stack logic**

Add the `truncate_name` helper (truncate at 57 chars and append `...` so total is at most 60; if the input is <= 60 chars, return as-is) and the core state machine loop that handles heading lines. All entities must set `file: file_path.to_string()`. Use a heading stack `Vec<(usize, String, u32)>` — `(level, name, line_start)`. On each heading:
1. Close all open sections at level >= current level (set their `line_end`, compute hashes, push entity)
2. Push new heading onto the stack

At EOF, close all remaining open sections.

Key implementation details:
- Detect headings: line starts with `#` followed by space, count `#` chars for level
- `line_end` of a section = line before next same-or-higher heading, or last line of file
- Use `hasher::struct_hash()`, `hasher::body_hash_raw()`, `hasher::sig_hash()` — **not** `hasher::body_hash()` (would strip `#` headings as comments)
- `sig` = full heading line (e.g., `"## Sub A"`)
- `meta: None` for all entities

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib markdown_index::tests::parse_heading`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/markdown_index.rs
git commit -m "feat: implement heading/section parsing with hierarchy"
```

---

### Task 4: Implement fenced code block parsing

**Files:**
- Modify: `src/markdown_index.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn parse_code_blocks() {
    let source = "# Doc\n\n```python\ndef hello():\n    print(\"hi\")\n```\n\n```\nno language\n```\n";
    let (entities, _) = parse_markdown_file(source, "test.md").unwrap();

    let code_blocks: Vec<&Entity> = entities.iter().filter(|e| e.kind == "code_block").collect();
    assert_eq!(code_blocks.len(), 2);

    let py_block = &code_blocks[0];
    assert_eq!(py_block.sig.as_deref(), Some("python"));
    assert_eq!(py_block.parent.as_deref(), Some("Doc"));
    assert!(py_block.name.contains("def hello()"));

    let no_lang = &code_blocks[1];
    assert!(no_lang.sig.is_none());
    assert!(no_lang.name.contains("no language"));
}

#[test]
fn parse_nested_fence_characters() {
    let source = "# Doc\n\n````\n```\nnested fence\n```\n````\n";
    let (entities, _) = parse_markdown_file(source, "test.md").unwrap();
    let code_blocks: Vec<&Entity> = entities.iter().filter(|e| e.kind == "code_block").collect();
    assert_eq!(code_blocks.len(), 1, "nested fence markers should not split the block");
}

#[test]
fn parse_tilde_fence() {
    let source = "# Doc\n\n~~~bash\necho hello\n~~~\n";
    let (entities, _) = parse_markdown_file(source, "test.md").unwrap();
    let code_blocks: Vec<&Entity> = entities.iter().filter(|e| e.kind == "code_block").collect();
    assert_eq!(code_blocks.len(), 1);
    assert_eq!(code_blocks[0].sig.as_deref(), Some("bash"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib markdown_index::tests::parse_code_blocks && cargo test --lib markdown_index::tests::parse_nested_fence && cargo test --lib markdown_index::tests::parse_tilde_fence`
Expected: FAIL

- [ ] **Step 3: Implement code block state machine**

Add `InFencedCode` state to the parser. Track:
- `fence_marker`: the opening fence string (e.g., ```` ``` ```` or `~~~` or ```` ```` ````) — only close on exact-length match
- `fence_lang`: language tag after the opening fence (if any)
- `fence_start_line`: 1-indexed start line
- Accumulate content lines to derive the name (first content line preview)

On closing fence: create `code_block` entity with:
- `kind`: `"code_block"`
- `name`: first content line, truncated to 60 chars
- `sig`: language tag or `None`
- `parent`: current heading from the heading stack
- `file`: `file_path.to_string()`
- Compute all three hashes from the full fence span (opening ``` to closing ```)

Handle both backtick (`` ` ``) and tilde (`~`) fence markers. The fence closes only when a line starts with the same character repeated at least as many times as the opening fence.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib markdown_index::tests::parse_code_blocks && cargo test --lib markdown_index::tests::parse_nested_fence && cargo test --lib markdown_index::tests::parse_tilde_fence`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/markdown_index.rs
git commit -m "feat: implement fenced code block parsing with language tags"
```

---

### Task 5: Implement front matter delegation to YAML parser

**Files:**
- Modify: `src/markdown_index.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn parse_front_matter() {
    let source = "---\ntitle: Getting Started\nauthor: Jane\ntags:\n  - rust\n  - cli\n---\n\n# Content\n\nSome text.\n";
    let (entities, _) = parse_markdown_file(source, "test.md").unwrap();

    // Should have frontmatter entity + YAML children + section + paragraph
    let fm = entities.iter().find(|e| e.kind == "frontmatter").unwrap();
    assert_eq!(fm.name, "frontmatter");
    assert!(fm.parent.is_none());
    assert_eq!(fm.line_start, 1);

    // YAML keys should be children of frontmatter
    let title = entities.iter().find(|e| e.name == "title").unwrap();
    assert_eq!(title.parent.as_deref(), Some("frontmatter"));

    let tags = entities.iter().find(|e| e.name == "tags").unwrap();
    assert_eq!(tags.parent.as_deref(), Some("frontmatter"));
    assert_eq!(tags.kind, "array");

    // Line offsets should be adjusted (title is on line 2 of the file, not line 1)
    assert_eq!(title.line_start, 2);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib markdown_index::tests::parse_front_matter`
Expected: FAIL

- [ ] **Step 3: Implement front matter detection and YAML delegation**

Add `InFrontMatter` state. Detection: first line of file is exactly `---`.

On closing `---`:
1. Create a `frontmatter` entity spanning the full `---` to `---` block
2. Extract the YAML body between the delimiters (strip `---` lines)
3. Call `crate::yaml_index::parse_yaml_file(yaml_body, file_path)`
4. For each returned entity:
   - Adjust `line_start` and `line_end` by adding the front matter offset (line after opening `---`, which is line 2)
   - Set `parent` to `"frontmatter"`
5. Append all to the entity list

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib markdown_index::tests::parse_front_matter`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/markdown_index.rs
git commit -m "feat: implement front matter with YAML parser delegation"
```

---

### Task 6: Implement table parsing

**Files:**
- Modify: `src/markdown_index.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn parse_tables() {
    let source = "# Config\n\n| Option | Default |\n|--------|--------|\n| verbose | false |\n| color | true |\n\nSome text after.\n";
    let (entities, _) = parse_markdown_file(source, "test.md").unwrap();

    let tables: Vec<&Entity> = entities.iter().filter(|e| e.kind == "table").collect();
    assert_eq!(tables.len(), 1);
    assert!(tables[0].name.contains("Option"));
    assert_eq!(tables[0].parent.as_deref(), Some("Config"));
    assert!(tables[0].sig.is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --lib markdown_index::tests::parse_tables`
Expected: FAIL

- [ ] **Step 3: Implement table detection with lookahead**

In the `Normal` state, when a line starts with `|`:
1. Buffer the line
2. Peek at the next line — if it matches the separator pattern (contains `|` and `-` characters, like `|---|---|`), enter `InTable` state
3. If not a separator, treat the buffered line as paragraph text

In `InTable` state, accumulate lines until a line doesn't start with `|`, then emit the table entity.

Entity fields:
- `kind`: `"table"`
- `name`: first row content (header row), truncated to 60 chars
- `sig`: `None`
- `parent`: current heading

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --lib markdown_index::tests::parse_tables`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/markdown_index.rs
git commit -m "feat: implement table parsing with separator row detection"
```

---

### Task 7: Implement blockquote, list, and paragraph parsing

**Files:**
- Modify: `src/markdown_index.rs`

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn parse_blockquotes() {
    let source = "# Doc\n\n> This is a quote\n> spanning multiple lines.\n\nSome text.\n";
    let (entities, _) = parse_markdown_file(source, "test.md").unwrap();

    let bqs: Vec<&Entity> = entities.iter().filter(|e| e.kind == "blockquote").collect();
    assert_eq!(bqs.len(), 1);
    assert!(bqs[0].name.contains("This is a quote"));
    assert_eq!(bqs[0].parent.as_deref(), Some("Doc"));
}

#[test]
fn parse_lists() {
    let source = "# Doc\n\n- item one\n- item two\n- item three\n\n1. first\n2. second\n";
    let (entities, _) = parse_markdown_file(source, "test.md").unwrap();

    let lists: Vec<&Entity> = entities.iter().filter(|e| e.kind == "list").collect();
    assert_eq!(lists.len(), 2);

    let unordered = &lists[0];
    assert_eq!(unordered.sig.as_deref(), Some("unordered"));
    assert!(unordered.name.contains("item one"));

    let ordered = &lists[1];
    assert_eq!(ordered.sig.as_deref(), Some("ordered"));
    assert!(ordered.name.contains("first"));
}

#[test]
fn parse_paragraphs() {
    let source = "# Doc\n\nFirst paragraph text.\n\nSecond paragraph text.\n";
    let (entities, _) = parse_markdown_file(source, "test.md").unwrap();

    let paras: Vec<&Entity> = entities.iter().filter(|e| e.kind == "paragraph").collect();
    assert_eq!(paras.len(), 2);
    assert!(paras[0].name.contains("First paragraph"));
    assert!(paras[1].name.contains("Second paragraph"));
    assert_eq!(paras[0].parent.as_deref(), Some("Doc"));
}

#[test]
fn parse_list_continuation() {
    let source = "# Doc\n\n- item one\n  continued on next line\n- item two\n";
    let (entities, _) = parse_markdown_file(source, "test.md").unwrap();

    let lists: Vec<&Entity> = entities.iter().filter(|e| e.kind == "list").collect();
    assert_eq!(lists.len(), 1, "continuation lines should not create a separate list");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib markdown_index::tests::parse_blockquotes && cargo test --lib markdown_index::tests::parse_lists && cargo test --lib markdown_index::tests::parse_paragraphs && cargo test --lib markdown_index::tests::parse_list_continuation`
Expected: FAIL

- [ ] **Step 3: Implement accumulator-based parsing for blockquotes, lists, paragraphs**

In `Normal` state, classify non-heading, non-blank lines:

1. **Blockquotes**: line starts with `> ` or `>`. Accumulate consecutive `>` lines. Code blocks inside blockquotes (lines like `> ````) stay as part of the blockquote — do NOT enter `InFencedCode` state.
2. **Lists**: line starts with `- `, `* `, or `N. ` (regex: `^\d+\.\s`). Accumulate consecutive list lines. Indented continuation lines (2+ spaces) are part of the current list.
3. **Paragraphs**: any other non-blank line. Accumulate until blank line or heading.

On flush (blank line, heading, or EOF), emit the accumulated block as an entity:
- `kind`: `"blockquote"` / `"list"` / `"paragraph"`
- `name`: first line content preview, truncated to 60 chars (strip `> ` / `- ` / `N. ` prefix for name)
- `sig`: `"ordered"` or `"unordered"` for lists, `None` for others
- `parent`: current heading from heading stack

Important: when switching between accumulator types (e.g., blockquote followed by list), flush the current accumulator before starting the new one.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --lib markdown_index::tests::parse_blockquotes && cargo test --lib markdown_index::tests::parse_lists && cargo test --lib markdown_index::tests::parse_paragraphs && cargo test --lib markdown_index::tests::parse_list_continuation`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/markdown_index.rs
git commit -m "feat: implement blockquote, list, and paragraph parsing"
```

---

### Task 8: Add mixed-document and edge-case tests

**Files:**
- Modify: `src/markdown_index.rs`

- [ ] **Step 1: Write comprehensive unit tests**

```rust
#[test]
fn parse_mixed_document() {
    let source = "---\ntitle: Test\n---\n\n# Introduction\n\nThis is a guide.\n\n## Setup\n\n```bash\nnpm install\n```\n\n| Col A | Col B |\n|-------|-------|\n| 1     | 2     |\n\n> Important note here.\n\n- step one\n- step two\n\n## Next Steps\n\nFinal paragraph.\n";
    let (entities, _) = parse_markdown_file(source, "test.md").unwrap();

    // Count by kind
    let kinds: Vec<&str> = entities.iter().map(|e| e.kind.as_str()).collect();
    assert!(kinds.contains(&"frontmatter"), "missing frontmatter");
    assert!(kinds.contains(&"section"), "missing section");
    assert!(kinds.contains(&"code_block"), "missing code_block");
    assert!(kinds.contains(&"table"), "missing table");
    assert!(kinds.contains(&"blockquote"), "missing blockquote");
    assert!(kinds.contains(&"list"), "missing list");
    assert!(kinds.contains(&"paragraph"), "missing paragraph");
}

#[test]
fn parse_no_headings() {
    let source = "Just a paragraph.\n\n```rust\nlet x = 1;\n```\n";
    let (entities, _) = parse_markdown_file(source, "test.md").unwrap();

    // All entities should be top-level (no parent)
    for e in &entities {
        assert!(e.parent.is_none(), "entity {} should have no parent", e.name);
    }
}

#[test]
fn hashes_are_present_and_16_chars() {
    let source = "# Heading\n\nSome text.\n";
    let (entities, _) = parse_markdown_file(source, "test.md").unwrap();
    for e in &entities {
        assert_eq!(e.struct_hash.len(), 16, "struct_hash wrong length for {}", e.name);
        assert!(e.struct_hash.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

#[test]
fn meta_is_always_none() {
    let source = "# Heading\n\nText.\n";
    let (entities, _) = parse_markdown_file(source, "test.md").unwrap();
    for e in &entities {
        assert!(e.meta.is_none(), "meta should be None for {}", e.name);
    }
}

#[test]
fn entities_sorted_by_line() {
    let source = "# A\n\nParagraph.\n\n## B\n\n```bash\necho hi\n```\n";
    let (entities, _) = parse_markdown_file(source, "test.md").unwrap();
    for w in entities.windows(2) {
        assert!(w[0].line_start <= w[1].line_start,
            "{} (line {}) should come before {} (line {})",
            w[0].name, w[0].line_start, w[1].name, w[1].line_start);
    }
}

#[test]
fn horizontal_rule_not_frontmatter() {
    let source = "# Doc\n\nSome text.\n\n---\n\nMore text.\n";
    let (entities, _) = parse_markdown_file(source, "test.md").unwrap();
    let fm: Vec<&Entity> = entities.iter().filter(|e| e.kind == "frontmatter").collect();
    assert!(fm.is_empty(), "--- in middle of file should not be treated as frontmatter");
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib markdown_index::tests`
Expected: ALL PASS

- [ ] **Step 3: Fix any failures, then commit**

```bash
git add src/markdown_index.rs
git commit -m "test: add comprehensive unit tests for markdown parser edge cases"
```

---

### Task 9: Wire up integration points (index.rs, diff.rs, main.rs)

**Files:**
- Modify: `src/index.rs:21-29` (dispatcher)
- Modify: `src/index.rs:141-157` (language mapping in build_index)
- Modify: `src/index.rs:249-268` (file discovery)
- Modify: `src/diff.rs:52-63` (compute_diff language mapping)
- Modify: `src/diff.rs:190-197` (detect_lang helper)

- [ ] **Step 1: Write the failing integration test**

Create `tests/markdown_integration.rs`:

```rust
use std::process::Command;

fn run_sigil_index(fixture_dir: &str, extra_args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_sigil"))
        .arg("index")
        .arg("--root")
        .arg(fixture_dir)
        .arg("--stdout")
        .arg("--full")
        .args(extra_args)
        .output()
        .expect("failed to run sigil");

    assert!(output.status.success(), "sigil failed: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).expect("invalid utf8")
}

fn fixture_path() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    format!("{}/tests/fixtures", manifest)
}

#[test]
fn indexes_markdown_fixture() {
    let output = run_sigil_index(&fixture_path(), &["--files", &format!("{}/sample.md", fixture_path())]);
    let entities: Vec<serde_json::Value> = output.lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    // Should find frontmatter, sections, code blocks, tables, lists, blockquotes, paragraphs
    assert!(entities.len() >= 10, "expected at least 10 entities, got {}", entities.len());

    let kinds: Vec<&str> = entities.iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"frontmatter"), "missing frontmatter");
    assert!(kinds.contains(&"section"), "missing section");
    assert!(kinds.contains(&"code_block"), "missing code_block");
    assert!(kinds.contains(&"table"), "missing table");
    assert!(kinds.contains(&"list"), "missing list");
    assert!(kinds.contains(&"blockquote"), "missing blockquote");
    assert!(kinds.contains(&"paragraph"), "missing paragraph");

    // Verify heading hierarchy
    let installation = entities.iter().find(|e| e["name"].as_str() == Some("Installation")).unwrap();
    assert!(installation["parent"].is_null(), "top-level heading should have no parent");

    let prereqs = entities.iter().find(|e| e["name"].as_str() == Some("Prerequisites")).unwrap();
    assert_eq!(prereqs["parent"].as_str(), Some("Installation"));
}

#[test]
fn diff_markdown_files() {
    let output = Command::new(env!("CARGO_BIN_EXE_sigil"))
        .arg("diff")
        .arg("--files")
        .arg(format!("{}/sample.md", fixture_path()))
        .arg(format!("{}/sample_v2.md", fixture_path()))
        .arg("--json")
        .output()
        .expect("failed to run sigil diff");

    assert!(output.status.success(), "sigil diff failed: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("invalid utf8");
    let result: serde_json::Value = serde_json::from_str(&stdout).expect("invalid json");

    // Should have entity-level diffs
    let entities = result["entities"].as_array().expect("missing entities array");
    assert!(!entities.is_empty(), "diff should produce entity-level changes");
}
```

- [ ] **Step 2: Run integration test to verify it fails**

Run: `cargo test --test markdown_integration`
Expected: FAIL (markdown files not recognized by indexing pipeline)

- [ ] **Step 3: Add markdown to parse_single_file dispatcher in index.rs**

In `src/index.rs`, after the `if language == "toml"` block (line 27-29), add:

```rust
if language == "markdown" {
    return crate::markdown_index::parse_markdown_file(source, file_path);
}
```

- [ ] **Step 4: Add markdown to build_index language mapping in index.rs**

In `src/index.rs`, in the `build_index` function's language detection block (around line 141-157), add before the `else` fallback:

```rust
} else if ext == "md" || ext == "markdown" || ext == "mdx" {
    "markdown"
```

- [ ] **Step 5: Add markdown to discover_source_files in index.rs**

In `src/index.rs`, in the `discover_source_files` function (around line 260-263), the file filter uses a single boolean expression inside `.map(|ext| { ... })`. Add the markdown extensions to this boolean expression:

```rust
ext == "md" || ext == "markdown" || ext == "mdx" ||
    ext == "json" || ext == "yaml" || ext == "yml" || ext == "toml"
    || codeix::parser::languages::detect_language(ext).is_some()
```

- [ ] **Step 6: Add markdown to compute_diff language mapping in diff.rs**

In `src/diff.rs`, in the `compute_diff` function's language detection block (around line 52-58), add before the `else` fallback:

```rust
} else if ext == "md" || ext == "markdown" || ext == "mdx" {
    "markdown"
```

- [ ] **Step 7: Add markdown to detect_lang helper in diff.rs**

In `src/diff.rs`, in the `detect_lang` function (around line 190-197), add a match arm:

```rust
"md" | "markdown" | "mdx" => Some("markdown"),
```

- [ ] **Step 8: Run integration tests to verify they pass**

Run: `cargo test --test markdown_integration`
Expected: PASS

- [ ] **Step 9: Run the full test suite to verify nothing is broken**

Run: `cargo test`
Expected: ALL PASS

- [ ] **Step 10: Commit**

```bash
git add src/index.rs src/diff.rs tests/markdown_integration.rs
git commit -m "feat: wire markdown parser into indexing and diff pipelines"
```

---

### Task 10: End-to-end verification and final commit

**Files:**
- No new files

- [ ] **Step 1: Run sigil on a real markdown file**

```bash
cargo run -- index --files README.md --stdout --full 2>/dev/null | head -20
```

Verify: entities with kinds `section`, `paragraph`, `code_block`, etc. appear in output.

- [ ] **Step 2: Run sigil diff on the test fixtures**

```bash
cargo run -- diff --files tests/fixtures/sample.md tests/fixtures/sample_v2.md
```

Verify: terminal output shows added/removed/modified entities with markdown-appropriate kinds.

- [ ] **Step 3: Run the complete test suite**

```bash
cargo test
```

Expected: ALL PASS (unit + integration + diff_integration)

- [ ] **Step 4: Verify the CLAUDE.md is up to date**

Check if `CLAUDE.md` needs updates:
- Add `markdown_index.rs` to the Architecture section
- Add `markdown_integration` to the test commands section if needed

- [ ] **Step 5: Final commit if CLAUDE.md was updated**

```bash
git add CLAUDE.md
git commit -m "docs: add markdown_index.rs to architecture documentation"
```
