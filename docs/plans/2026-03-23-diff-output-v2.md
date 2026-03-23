# Diff Output V2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite sigil diff output to match the v0.2.0 output specification — new terminal format with structured glyphs/columns, markdown output, restructured JSON, exit codes, and --lines/--context/--no-color/--no-emoji flags.

**Architecture:** The output layer is split into three formatters (terminal, JSON, markdown) consuming a shared `DiffOutput` data model. The data model is built from the existing diff pipeline (matcher + classifier) with additions for breaking reasons and pattern IDs. CLI flags control which formatter runs and what detail level to include.

**Tech Stack:** Rust, clap (CLI), serde (JSON), colored (terminal ANSI)

---

## File Structure

### New files
- `src/output.rs` — `DiffOutput` data model and `DiffOutput::from_result()` converter: the canonical intermediate representation consumed by all three formatters. Contains `Meta`, `OutputSummary`, `BreakingEntry`, `OutputPattern`, `MoveEntry`, `FileSection`, `OutputEntity`, `TokenChange`, `SnippetContext`.
- `src/markdown_formatter.rs` — Markdown formatter: renders `DiffOutput` as GitHub-flavored Markdown with emoji/ASCII glyph toggle.

### Modified files
- `src/diff_json.rs` — Keep existing structs for internal pipeline use. Add `base_sha`/`head_sha` to `DiffResult`. Add `breaking_reason` to `EntityDiff`.
- `src/formatter.rs` — Full rewrite of terminal formatting: replace existing `format_terminal()` with `format_terminal_v2()`. New glyph system, column layout, header/separator, patterns section, moves section, per-file sections, summary/breaking footer. Old `format_terminal()` is removed.
- `src/main.rs` — New CLI flags (`--lines`, `--context`, `--markdown`, `--no-emoji`, `--no-color`), exit code logic (codes 0/1/2/3), formatter dispatch via `DiffOutput`.
- `src/diff.rs` — Pass resolved SHAs through to `DiffResult`. Optionally retain source texts when `--context` is requested.
- `src/classifier.rs` — Add `breaking_reason` field to `EntityDiff`.
- `src/matcher.rs` — Add `confidence` field (placeholder, always 1.0) to `EntityMatch` for future fuzzy matching.
- `src/git.rs` — No changes needed; `resolve_ref()` already exists.

### Test files
- `tests/diff_integration.rs` — Update existing `run_sigil_diff` helper to handle new exit codes. Add exit code tests, `--markdown` tests, `--lines` tests, JSON schema tests.

---

## Task 1: Add `breaking_reason` to classifier

**Files:**
- Modify: `src/classifier.rs`
- Modify: `src/diff_json.rs`

- [ ] **Step 1: Add `breaking_reason` field to `EntityDiff`**

In `src/diff_json.rs`, add to `EntityDiff`:
```rust
#[serde(skip_serializing_if = "Option::is_none")]
pub breaking_reason: Option<String>,
```

- [ ] **Step 2: Populate `breaking_reason` in classifier**

In `src/classifier.rs` `classify()`, set `breaking_reason` wherever `breaking: true`:

| Condition | `breaking_reason` value |
|---|---|
| Entity removed and public | `"removed"` |
| Entity renamed and public | `"renamed"` |
| Entity moved and public sig changed | `"moved"` |
| Entity modified and public sig changed | `"sig_changed"` |
| All other cases | `None` |

Note: Currently all public renamed entities are always breaking (regardless of sig change). The reason `"renamed"` is correct for these.

Update all `EntityDiff { ... }` constructors in `classify()` to include the new field.

- [ ] **Step 3: Update test constructors**

Any test in `diff_json.rs`, `formatter.rs`, or `classifier.rs` that constructs `EntityDiff` needs the new field added as `breaking_reason: None` (or appropriate value for breaking tests).

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: All 153+ tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/classifier.rs src/diff_json.rs
git commit -m "feat: add breaking_reason to EntityDiff"
```

---

## Task 2: Add `confidence` placeholder to EntityMatch

**Files:**
- Modify: `src/matcher.rs`

Note: All confidence values are 1.0 for now since all matches are deterministic (exact name or exact body_hash). This field is a placeholder for future fuzzy/heuristic matching. The spec requires it in JSON output for consumers.

- [ ] **Step 1: Add `confidence` field to `EntityMatch`**

```rust
pub struct EntityMatch {
    pub old: Option<Entity>,
    pub new: Option<Entity>,
    pub match_kind: MatchKind,
    pub confidence: f64,  // 0.0-1.0; always 1.0 until fuzzy matching is added
}
```

- [ ] **Step 2: Set confidence to 1.0 in all match paths**

All four passes in `match_entities()` set `confidence: 1.0`.

- [ ] **Step 3: Update all `EntityMatch` constructors in tests**

Add `confidence: 1.0` to all test constructors in `matcher.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add src/matcher.rs
git commit -m "feat: add confidence placeholder to EntityMatch"
```

---

## Task 3: Thread resolved SHAs through DiffResult

**Files:**
- Modify: `src/diff.rs`
- Modify: `src/diff_json.rs`

`resolve_ref()` already exists in `src/git.rs` — no changes needed there.

- [ ] **Step 1: Add `base_sha` and `head_sha` to `DiffResult`**

In `src/diff_json.rs`:
```rust
pub struct DiffResult {
    pub base_ref: String,
    pub head_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_sha: Option<String>,
    // ... rest unchanged
}
```

- [ ] **Step 2: Resolve SHAs in `compute_diff()`**

In `src/diff.rs`, after parsing refs, resolve to full SHAs:
```rust
let base_sha = git::resolve_ref(root, &base_ref).ok();
let head_sha = git::resolve_ref(root, &head_ref).ok();
```

Pass into `DiffResult`. For `compute_file_diff()`, set both to `None`.

- [ ] **Step 3: Update DiffResult constructors in tests**

Add `base_sha: None, head_sha: None` to all test `DiffResult` literals in `diff_json.rs` and `formatter.rs`.

- [ ] **Step 4: Run tests**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 5: Commit**

```bash
git add src/diff.rs src/diff_json.rs
git commit -m "feat: resolve and thread git SHAs through DiffResult"
```

---

## Task 4: Create `DiffOutput` intermediate model and `--context` snippet support

**Files:**
- Create: `src/output.rs`
- Modify: `src/main.rs` (add `mod output;`)
- Modify: `src/diff.rs` (optionally retain source texts)
- Modify: `src/diff_json.rs` (add optional source maps)

This task also includes `SnippetContext` support for `--context`, since Tasks 5 and 6 need it.

- [ ] **Step 1: Define all structs in `src/output.rs`**

```rust
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub base_ref: String,
    pub head_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_sha: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_sha: Option<String>,
    pub generated_at: String,
    pub sigil_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputSummary {
    pub files_changed: usize,
    pub patterns: usize,
    pub moves: usize,
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
    pub renamed: usize,
    pub formatting_only: usize,
    pub has_breaking: bool,
    pub natural_language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakingEntry {
    pub entity: String,
    pub kind: String,
    pub file: String,
    pub line: u32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputPattern {
    pub id: String,
    #[serde(rename = "type")]
    pub pattern_type: String,
    pub entity_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_glob: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_glob: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity_name: Option<String>,
    pub file_count: usize,
    pub files: Vec<String>,
    pub entities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoveEntry {
    pub entity: String,
    pub kind: String,
    pub from_file: String,
    pub to_file: String,
    pub from_line: u32,
    pub to_line: u32,
    pub breaking: bool,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenChange {
    #[serde(rename = "type")]
    pub change_type: String,
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnippetContext {
    pub base_snippet: String,
    pub head_snippet: String,
    pub language: String,
    pub snippet_kind: String,  // "signature", "diff", "full"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputEntity {
    pub change: String,
    pub name: String,
    pub kind: String,
    pub line: u32,
    pub line_end: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sig_changed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_changed: Option<bool>,
    pub breaking: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub breaking_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern_ref: Option<String>,
    pub token_changes: Vec<TokenChange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<SnippetContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSummary {
    pub added: usize,
    pub modified: usize,
    pub removed: usize,
    pub renamed: usize,
    pub formatting_only: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileSection {
    pub file: String,
    pub summary: FileSummary,
    pub entities: Vec<OutputEntity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffOutput {
    pub meta: Meta,
    pub summary: OutputSummary,
    pub breaking: Vec<BreakingEntry>,
    pub patterns: Vec<OutputPattern>,
    pub moves: Vec<MoveEntry>,
    pub files: Vec<FileSection>,
}
```

- [ ] **Step 2: Implement `DiffOutput::from_result()`**

Signature: `pub fn from_result(result: &DiffResult, include_context: bool) -> Self`

Key conversion logic:

**Move separation:** Entities with `change == Moved` go into `moves[]`, NOT into `files[]`. For `BreakingEntry` for moved entities, use the destination file (`new.file`).

**Pattern conversion from `CrossFilePattern`:**
1. Assign sequential IDs: `pat_1`, `pat_2`, etc.
2. `pattern_type`: look at the `change` field of the `CrossFilePattern` — if `Renamed`, type is `"rename"`; otherwise `"body_identical"`.
3. `entity_kind`: take the `kind` from the first matching entity in `result.entities`.
4. `from_glob` / `to_glob`: for rename patterns, find the common prefix/suffix of old names vs new names. If names share a prefix/suffix, generate a glob (e.g., `validate_*` → `check_*`). For non-rename patterns, set both to `None`.
5. `entity_name`: for `body_identical` patterns, set to the shared base name from `CrossFilePattern.description`.
6. Tag matching entities in `files[]` with `pattern_ref: Some("pat_N")`.

**Token changes mapping:** Map `ChangeDetail` to `TokenChange`:
- `DetailKind::ValueChanged` → `change_type: "value_changed"`
- `DetailKind::IdentifierChanged` → `change_type: "identifier_renamed"`
- `DetailKind::ArgumentAdded` → `change_type: "param_added"` (from/to extracted from description)
- `DetailKind::ArgumentRemoved` → `change_type: "param_removed"`
- `DetailKind::LineAdded` / `LineRemoved` → skip (these are inline diff lines, not token changes)
- `DetailKind::Comment` → skip

**Natural language generation algorithm:**
1. Start with an empty list of phrases.
2. If patterns > 0: `"N cross-file patterns"` (describe first pattern briefly if rename: `"validate_* → check_*"`)
3. If moves > 0: `"N entity moves"`
4. Always: `"N added, N modified, N removed"` (omit zero counts)
5. If has_breaking: append `"with breaking changes"`
6. If all counts are 0: `"No structural changes."`
7. Join with `, ` and end with `.`

**Context snippets** (when `include_context` is true): see Task 4 Step 3.

- [ ] **Step 3: Implement snippet extraction for `--context`**

Add optional `old_sources` and `new_sources` fields to `DiffResult`:
```rust
// In diff_json.rs:
#[serde(skip)]
pub old_sources: Option<HashMap<String, String>>,
#[serde(skip)]
pub new_sources: Option<HashMap<String, String>>,
```

In `diff.rs`, retain the source maps in `DiffResult` when `DiffOptions.include_context` is true.

In `from_result()`, when `include_context` is true and sources are available, build `SnippetContext`:
- `sig_changed && !body_changed` → extract signature lines only → `snippet_kind: "signature"`
- `body_changed && !sig_changed` → changed lines in unified diff format → `snippet_kind: "diff"`
- Both changed → full entity body → `snippet_kind: "full"`
- `formatting_only` → no snippet

- [ ] **Step 4: Add `mod output;` to `main.rs`**

- [ ] **Step 5: Write unit tests for `DiffOutput::from_result()`**

Tests:
- Basic conversion: given a DiffResult with 1 added, 1 modified, 1 removed → verify counts, file sections, entity fields
- Move separation: given a DiffResult with a Moved entity → verify it appears in `moves[]`, not in `files[]`
- Pattern ID assignment: given a DiffResult with patterns → verify `pat_1` etc. and `pattern_ref` on entities
- Breaking array: given breaking entities → verify BreakingEntry with correct reason and file
- Natural language: verify generated string for various change combinations
- Empty diff: verify all arrays empty, natural language says "No structural changes."

- [ ] **Step 6: Run tests**

Run: `cargo test output`
Expected: All new tests pass.

- [ ] **Step 7: Commit**

```bash
git add src/output.rs src/main.rs src/diff.rs src/diff_json.rs
git commit -m "feat: add DiffOutput model with from_result converter and snippet support"
```

---

## Task 5: Rewrite terminal formatter

**Files:**
- Modify: `src/formatter.rs`

The old `format_terminal()` function is replaced entirely by `format_terminal_v2()`. Existing tests for `format_terminal()` are rewritten.

- [ ] **Step 1: Define glyph constants**

```rust
const GLYPH_MODIFIED: &str = "~";
const GLYPH_ADDED: &str = "+";
const GLYPH_REMOVED: &str = "\u{2212}"; // −
const GLYPH_RENAMED: &str = "\u{2248}"; // ≈
const GLYPH_FORMAT: &str = "\u{00B7}";  // ·
const GLYPH_PATTERN: &str = "\u{2261}"; // ≡
const GLYPH_MOVE: &str = "\u{21D2}";    // ⇒
const GLYPH_BREAKING: &str = "\u{26A0}"; // ⚠
const GLYPH_SEPARATOR: &str = "\u{2500}"; // ─
const GLYPH_ARROW: &str = "\u{2192}";   // →
```

- [ ] **Step 2: Implement `format_terminal_v2()` taking `DiffOutput` and options**

```rust
pub struct FormatOptions {
    pub show_lines: bool,
    pub show_context: bool,
    pub use_color: bool,
}

pub fn format_terminal_v2(output: &DiffOutput, opts: &FormatOptions) -> String {
    let mut out = String::new();
    // 1. Header with separator
    // 2. Patterns section (if any)
    // 3. Moves section (if any)
    // 4. Separator
    // 5. Per-file sections
    // 6. Separator
    // 7. Summary line
    // 8. Breaking line (if any)
    // 9. Separator
    out
}
```

Note: When `use_color` is false, use `colored::control::set_override(false)` so all `.bold()`, `.red()`, etc. calls produce plain text. The `colored` crate handles this transparently — test runners are non-TTY so colors are off by default in tests.

- [ ] **Step 3: Implement header rendering**

```
──────────────────────────────────────────────────────────
sigil diff  HEAD~1                     4 files  ⚠ 2 breaking
──────────────────────────────────────────────────────────
```

- `GLYPH_SEPARATOR` repeated to 60 chars.
- Right-pad refspec to ~40 columns, then file count, then breaking count (omit if 0).
- If entire diff is formatting-only, show `(formatting only)` instead of breaking count.

- [ ] **Step 4: Implement patterns section**

```
  ≡  rename    validate_* → check_*   ×8 files   function
     src/a.py  src/b.py  src/c.py  +5 more
```

- Only show if `output.patterns` is non-empty.
- Show first 3 files inline, then `+N more`.
- Note: `--expand pattern` is a future feature; do not emit the hint yet.

- [ ] **Step 5: Implement moves section**

```
  ⇒  execute_payment      function       ⚠ breaking
     src/old.py → src/new.py
```

- Only show if `output.moves` is non-empty.
- Breaking flag on first line if applicable.

- [ ] **Step 6: Implement per-file sections**

File header:
```
src/payments.py                                    +1 ~2 ·3
```
- Filename bold, right-aligned change glyphs with counts (omit zero counts).

Entity rows (fixed column widths):
```
  ~  modified   execute_payment      function           ⚠ breaking
     sig+body  ·  "true" → "false"  ·  validate_card → check_card  ⊂ pat_1
```
- Verb padded to 9 chars, name padded to 20 chars.
- Modified continuation: hash dims (`sig+body`, `sig only`, `body only`) + token diffs (max 4, then `+N more`) + `⊂ pat_N` if applicable.
- Formatting-only: with `use_color == false`, collapse to count line per file:
  ```
  ·  3 formatting only: name1  name2  name3  +N more
  ```

- [ ] **Step 7: Implement `--lines` rendering**

When `show_lines` is true, append `:line` to entity names in dim:
```
  ~  modified   execute_payment:47   function
```
For removed entities, show the last-known line from the base ref (use `line` from `OutputEntity`).

- [ ] **Step 8: Implement `--context` rendering**

When `show_context` is true and `OutputEntity.context` is `Some`, render indented code snippets after the continuation line. Skip for `formatting_only` entities.

- [ ] **Step 9: Implement summary and breaking footer**

```
──────────────────────────────────────────────────────────
  2 patterns  ·  1 moves  ·  3 added  ·  2 modified  ·  1 removed  ·  5 formatting
  ⚠ breaking:  execute_payment (sig changed)  ·  old_handler (removed)
──────────────────────────────────────────────────────────
```

Omit zero counts. If no changes, show `  no structural changes`.

- [ ] **Step 10: Remove old `format_terminal()` and rewrite tests**

Delete the old `format_terminal()` and `format_entity_diff()` functions. Rewrite existing tests in `formatter.rs` to test `format_terminal_v2()` with `DiffOutput` inputs. Tests should use `FormatOptions { use_color: false, .. }` so assertions work without ANSI codes.

- [ ] **Step 11: Run tests**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 12: Commit**

```bash
git add src/formatter.rs
git commit -m "feat: rewrite terminal formatter with new glyph/column layout"
```

---

## Task 6: Create markdown formatter

**Files:**
- Create: `src/markdown_formatter.rs`
- Modify: `src/main.rs` (add `mod markdown_formatter;`)

- [ ] **Step 1: Implement `format_markdown()`**

```rust
pub struct MarkdownOptions {
    pub use_emoji: bool,
    pub show_context: bool,
}

pub fn format_markdown(output: &DiffOutput, opts: &MarkdownOptions) -> String {
    // 1. Header line: ⚠️ **sigil diff** `<BASE_REF>` — <N> files, **<N> breaking**
    // 2. --- separator
    // 3. Patterns section (bullet list, if any)
    // 4. Moves section (bullet list, if any)
    // 5. --- separator
    // 6. Per-file sections: `filename` +N ~N ·N
    //    - entity bullets
    //    - > blockquote for token diffs
    // 7. --- separator
    // 8. Summary line
    // 9. Breaking line (if any)
}
```

- [ ] **Step 2: Implement emoji/ASCII glyph selection**

```rust
fn glyph(concept: &str, use_emoji: bool) -> &'static str {
    match (concept, use_emoji) {
        ("breaking", true) => "\u{26A0}\u{FE0F}",  // ⚠️
        ("breaking", false) => "!",
        ("added", true) => "\u{2726}",  // ✦
        ("added", false) => "+",
        ("modified", _) => "~",
        ("removed", true) => "\u{2212}",  // −
        ("removed", false) => "-",
        ("pattern", _) => "\u{2261}",  // ≡
        ("move", true) => "\u{2197}",  // ↗
        ("move", false) => "=>",
        _ => "?",
    }
}
```

- [ ] **Step 3: Implement formatting-only collapse**

In markdown, formatting-only entities are always collapsed to a count line:
```markdown
- · 3 formatting only: `name1` `name2` `name3` +2 more
```

- [ ] **Step 4: Implement `--context` with fenced code blocks**

When `show_context` is true and entity has a `SnippetContext`, render after the blockquote:
````markdown
- ⚠️ ~ modified `execute_payment` (function) — sig+body
  > `"true"` → `"false"` · `validate_card` → `check_card`
  ```python
  # before
  def execute_payment(commit: bool = True):
  # after
  def execute_payment(commit: bool = False):
  ```
````

- [ ] **Step 5: Implement empty diff output**

If no structural changes: `✓ sigil diff \`HEAD~1\` — no structural changes`

- [ ] **Step 6: Write tests**

Test full markdown output, --no-emoji mode, empty diff, formatting-only diff, context mode, patterns, moves.

- [ ] **Step 7: Run tests**

Run: `cargo test markdown`
Expected: All pass.

- [ ] **Step 8: Commit**

```bash
git add src/markdown_formatter.rs src/main.rs
git commit -m "feat: add markdown output formatter"
```

---

## Task 7: Wire up CLI flags, formatter dispatch, and exit codes

**Files:**
- Modify: `src/main.rs`
- Modify: `tests/diff_integration.rs`

This task merges the original Tasks 7 and 8 since they both modify `main.rs` and the exit code logic belongs in the dispatch.

- [ ] **Step 1: Add new CLI flags to Diff variant**

```rust
Diff {
    // ... existing fields ...

    /// Show line numbers next to entity names
    #[arg(long)]
    lines: bool,

    /// Include code snippets in output
    #[arg(long)]
    context: bool,

    /// Output as GitHub-flavored Markdown
    #[arg(long)]
    markdown: bool,

    /// Use ASCII glyphs instead of emoji (with --markdown)
    #[arg(long)]
    no_emoji: bool,

    /// Disable ANSI color output
    #[arg(long)]
    no_color: bool,
}
```

- [ ] **Step 2: Handle `--no-color` via `colored` crate**

```rust
if no_color {
    colored::control::set_override(false);
}
```

No need for `atty` crate — `colored` handles TTY detection internally.

- [ ] **Step 3: Update Diff handler to build `DiffOutput` and dispatch formatter**

```rust
Cli::Diff { ref_spec, files, root, json, pretty, verbose, lines, context, markdown, no_emoji, no_color } => {
    if no_color {
        colored::control::set_override(false);
    }

    let mut diff_opts = diff::DiffOptions { include_unchanged: false, verbose, include_context: context };

    let result = if files.len() == 2 {
        diff::compute_file_diff(&files[0], &files[1], &diff_opts)
            .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(3); })
    } else {
        let ref_spec = ref_spec.unwrap();
        let (base_ref, head_ref) = git::parse_ref_spec(&ref_spec)
            .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(3); });
        diff::compute_diff(&root, &base_ref, &head_ref, &diff_opts)
            .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(3); })
    };

    let output = output::DiffOutput::from_result(&result, context);

    if json {
        let out = std::io::stdout();
        let mut out = out.lock();
        if pretty {
            serde_json::to_writer_pretty(&mut out, &output)
        } else {
            serde_json::to_writer(&mut out, &output)
        }.expect("Failed to write JSON");
        println!();
    } else if markdown {
        let opts = markdown_formatter::MarkdownOptions {
            use_emoji: !no_emoji,
            show_context: context,
        };
        print!("{}", markdown_formatter::format_markdown(&output, &opts));
    } else {
        let opts = formatter::FormatOptions {
            show_lines: lines,
            show_context: context,
            use_color: !no_color,
        };
        print!("{}", formatter::format_terminal_v2(&output, &opts));
    }

    // Exit codes (only for Diff command)
    let s = &output.summary;
    let exit_code = if s.has_breaking { 2 }
        else if s.added + s.removed + s.modified + s.moves + s.renamed > 0 { 1 }
        else { 0 };
    std::process::exit(exit_code);
}
```

IMPORTANT: Only change `std::process::exit(1)` to `std::process::exit(3)` within the Diff handler. Do NOT change exit codes for other commands (Explore, Search, Symbols, etc.).

- [ ] **Step 4: Update existing integration test helper**

The existing `run_sigil_diff()` in `tests/diff_integration.rs` asserts `output.status.success()` which is only true for exit code 0. With the new exit codes, structural changes return 1 or 2. Update the helper:

```rust
fn run_sigil_diff(extra_args: &[&str]) -> std::process::Output {
    let output = Command::new(env!("CARGO_BIN_EXE_sigil"))
        .arg("diff")
        // ... existing args ...
        .args(extra_args)
        .output()
        .expect("failed to run sigil");

    // Accept exit codes 0, 1, 2 as valid (only 3 is an error)
    assert!(
        output.status.code().unwrap_or(3) < 3,
        "sigil diff failed with error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}
```

- [ ] **Step 5: Add exit code integration tests**

```rust
#[test]
fn exit_code_0_for_no_changes() {
    let output = Command::new(env!("CARGO_BIN_EXE_sigil"))
        .args(["diff", "HEAD..HEAD"])
        .output().unwrap();
    assert_eq!(output.status.code(), Some(0));
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 7: Commit**

```bash
git add src/main.rs tests/diff_integration.rs
git commit -m "feat: wire up CLI flags, formatter dispatch, and exit codes 0/1/2/3"
```

---

## Task 8: Edge cases

**Files:**
- Modify: `src/formatter.rs`
- Modify: `src/markdown_formatter.rs`
- Modify: `src/output.rs`

- [ ] **Step 1: Empty diff**

In `format_terminal_v2()`: if all summary counts are 0, emit `  no structural changes` in the summary section.
In `format_markdown()`: emit `✓ sigil diff \`<ref>\` — no structural changes`.
In JSON: all arrays are already `[]` and counts `0` by default.

- [ ] **Step 2: Formatting-only diff**

Exit code 0 (already handled by the exit code logic — formatting_only is not counted in the structural change sum).
Terminal header shows `(formatting only)`.
Breaking line is omitted. `has_breaking: false`.

- [ ] **Step 3: Same-file move reclassification**

In `output.rs` `DiffOutput::from_result()`, when processing Moved entities, check `old.file == new.file` (same file). If so, reclassify as `modified` (if body/sig changed) or `formatting_only` and place in `files[]` instead of `moves[]`.

- [ ] **Step 4: Pattern minimum**

Verify `DiffResult::detect_patterns()` already requires 2+ files (it does — `if files.len() < 2 { continue; }` at line 109 of `diff_json.rs`). No changes needed.

- [ ] **Step 5: Write edge case tests**

```rust
#[test]
fn empty_diff_output() {
    // Build a DiffResult with no entities
    // Verify DiffOutput has all empty arrays, natural_language = "No structural changes."
}

#[test]
fn formatting_only_diff_has_breaking_false() {
    // Build a DiffResult with only formatting_only entities
    // Verify has_breaking = false
}

#[test]
fn same_file_move_reclassified_to_modified() {
    // Build a DiffResult with a Moved entity where old_file == new_file
    // Verify it appears in files[], not moves[]
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test`
Expected: All pass.

- [ ] **Step 7: Commit**

```bash
git add src/formatter.rs src/markdown_formatter.rs src/output.rs tests/
git commit -m "fix: handle edge cases — empty diff, formatting-only, same-file moves"
```

---

## Task 9: Update documentation

**Files:**
- Modify: `README.md`
- Modify: `CLAUDE.md`
- Modify: `skills/sigil/SKILL.md`

- [ ] **Step 1: Update README**

- Add new flags to `sigil diff` command section: `--lines`, `--context`, `--markdown`, `--no-emoji`, `--no-color`
- Update JSON output example to show new schema (meta, summary, breaking, patterns, moves, files)
- Add "Exit Codes" section documenting codes 0/1/2/3
- Update example terminal output to show new glyph format
- Add `--markdown` example to CI/CD section

- [ ] **Step 2: Update CLAUDE.md**

- Add `output.rs` and `markdown_formatter.rs` to architecture listing
- Update `diff.rs` description to mention context/snippet support
- Update Useful Commands with new flags

- [ ] **Step 3: Update SKILL.md**

- Add new flags to Structural Diff command reference
- Add `--markdown` to recommended workflows (e.g., for PR comments)
- Update the sigil vs Grep/Glob table if needed

- [ ] **Step 4: Commit**

```bash
git add README.md CLAUDE.md skills/sigil/SKILL.md
git commit -m "docs: update README, CLAUDE.md, and SKILL.md with new diff output flags and exit codes"
```

---

## Task 10: Final integration test pass

**Files:**
- Modify: `tests/diff_integration.rs`

- [ ] **Step 1: Add `--json` schema validation test**

Parse JSON output and verify top-level keys: `meta`, `summary`, `breaking`, `patterns`, `moves`, `files`. Verify `meta` has `base_ref`, `head_ref`, `sigil_version`. Verify `summary` has all count fields.

- [ ] **Step 2: Add `--markdown` output test**

Run with `--markdown`, verify output contains expected markdown elements (`**bold**`, backticks, `---` separators, bullet lists).

- [ ] **Step 3: Add `--lines` output test**

Run with `--lines`, verify entity names include `:N` line number suffix.

- [ ] **Step 4: Add `--no-color` output test**

Run with `--no-color`, verify no ANSI escape sequences (`\x1b[`) in output.

- [ ] **Step 5: Add combined flag tests**

Test `--json --context`, `--markdown --no-emoji`, `--no-color --lines`.

- [ ] **Step 6: Run full test suite**

Run: `cargo test`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add tests/diff_integration.rs
git commit -m "test: add integration tests for new diff output modes"
```

---

## Execution order and dependencies

```
Task 1 (breaking_reason)  ─┐
Task 2 (confidence)        ─┼─→ Task 4 (DiffOutput + snippets) ─→ Task 5 (terminal formatter) ─┐
Task 3 (SHAs)              ─┘                                   ─→ Task 6 (markdown formatter) ─┤
                                                                 ─→ Task 7 (CLI + exit codes)   ─┤
                                                                                                  ├─→ Task 8 (edge cases) ─→ Task 9 (docs) ─→ Task 10 (final tests)
                                                                                                  │
```

Tasks 1-3 can run in parallel. Task 4 depends on 1-3 and includes snippet support. Tasks 5, 6, 7 depend on Task 4. Tasks 8-10 are sequential finalization after all formatters are wired up.
