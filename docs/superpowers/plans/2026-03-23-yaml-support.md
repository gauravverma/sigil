# YAML Language Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add YAML file support to sigil, extracting nested mapping keys as hierarchical entities — identical entity model to JSON support.

**Architecture:** Same pattern as JSON: a new `yaml_index` module with a custom parser using `serde_yaml`. Parses YAML structure, scans raw source to map keys to line positions, produces standard `Entity` structs. Routes through existing index/diff/cache pipelines.

**Tech Stack:** `serde_yaml = "0.9"` (new dependency)

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `Cargo.toml` | Modify | Add `serde_yaml = "0.9"` |
| `src/yaml_index.rs` | Create | YAML parsing, recursive entity extraction with line positions |
| `src/main.rs` | Modify | Add `mod yaml_index;` |
| `src/index.rs` | Modify | Add `yaml`/`yml` to routing in `parse_single_file`, `discover_source_files`, `build_index` |
| `src/diff.rs` | Modify | Add `yaml`/`yml` to routing in `compute_diff` |
| `tests/fixtures/sample.yaml` | Create | Test fixture for integration tests |
| `tests/integration.rs` | Modify | Add YAML indexing integration test |
| `README.md` | Modify | Add YAML to supported languages table |
| `CLAUDE.md` | Modify | Add `yaml_index.rs` to architecture listing |

---

### Task 1: Create YAML Parser Module

**Files:**
- Modify: `Cargo.toml` (add `serde_yaml = "0.9"`)
- Add `mod yaml_index;` to `src/main.rs`
- Create: `src/yaml_index.rs`

- [ ] **Step 0: Add serde_yaml dependency**

In `Cargo.toml`, add after the `serde_json` line:
```toml
serde_yaml = "0.9"
```

- [ ] **Step 1: Add `mod yaml_index` to main.rs**

In `src/main.rs`, add after `mod json_index;` (line 11):
```rust
mod yaml_index;
```

- [ ] **Step 2: Write unit tests for the YAML parser**

Create `src/yaml_index.rs` with the module structure and tests at the bottom. The parser mirrors `json_index.rs` exactly in its entity model:
- Parse YAML using `serde_yaml::Value`
- Scan raw source text to find line positions for each key
- Recursively extract entities for all mapping keys
- Entity kinds: `"object"` for mappings, `"array"` for sequences, `"property"` for scalars
- Signatures: `"key": <type>` format (e.g., `"name": string`, `"settings": object`)
- Parent: immediate parent key name, `None` for root-level keys
- Hashes: `struct_hash` on raw text, `body_hash` on serialized value content, `sig_hash` on the signature
- `meta`: always None

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_yaml() {
        let source = "name: myapp\nversion: 1.0.0\n";
        let (entities, refs) = parse_yaml_file(source, "test.yaml").unwrap();
        assert!(refs.is_empty());
        assert_eq!(entities.len(), 2);

        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"name"));
        assert!(names.contains(&"version"));

        let name_entity = entities.iter().find(|e| e.name == "name").unwrap();
        assert_eq!(name_entity.kind, "property");
        assert_eq!(name_entity.sig.as_deref(), Some("\"name\": string"));
        assert!(name_entity.parent.is_none());
        assert_eq!(name_entity.line_start, 1);
        assert_eq!(name_entity.line_end, 1);
    }

    #[test]
    fn parse_nested_mappings() {
        let source = "settings:\n  theme:\n    color: dark\n  debug: true\n";
        let (entities, _) = parse_yaml_file(source, "test.yaml").unwrap();
        assert_eq!(entities.len(), 4);

        let settings = entities.iter().find(|e| e.name == "settings").unwrap();
        assert_eq!(settings.kind, "object");
        assert!(settings.parent.is_none());
        assert_eq!(settings.line_start, 1);

        let theme = entities.iter().find(|e| e.name == "theme").unwrap();
        assert_eq!(theme.kind, "object");
        assert_eq!(theme.parent.as_deref(), Some("settings"));

        let color = entities.iter().find(|e| e.name == "color").unwrap();
        assert_eq!(color.kind, "property");
        assert_eq!(color.parent.as_deref(), Some("theme"));

        let debug = entities.iter().find(|e| e.name == "debug").unwrap();
        assert_eq!(debug.kind, "property");
        assert_eq!(debug.parent.as_deref(), Some("settings"));
    }

    #[test]
    fn parse_sequences() {
        let source = "tags:\n  - fast\n  - structural\nitems:\n  - 1\n  - 2\n";
        let (entities, _) = parse_yaml_file(source, "test.yaml").unwrap();
        let tags = entities.iter().find(|e| e.name == "tags").unwrap();
        assert_eq!(tags.kind, "array");
        assert_eq!(tags.sig.as_deref(), Some("\"tags\": array"));
    }

    #[test]
    fn parse_all_value_types() {
        let source = "str_val: hello\nnum_val: 42\nfloat_val: 3.14\nbool_val: true\nnull_val: null\n";
        let (entities, _) = parse_yaml_file(source, "test.yaml").unwrap();
        assert_eq!(entities.len(), 5);

        let sigs: Vec<(&str, &str)> = entities.iter()
            .map(|e| (e.name.as_str(), e.sig.as_deref().unwrap()))
            .collect();
        assert!(sigs.contains(&("str_val", "\"str_val\": string")));
        assert!(sigs.contains(&("num_val", "\"num_val\": number")));
        assert!(sigs.contains(&("float_val", "\"float_val\": number")));
        assert!(sigs.contains(&("bool_val", "\"bool_val\": boolean")));
        assert!(sigs.contains(&("null_val", "\"null_val\": null")));
    }

    #[test]
    fn hashes_are_present_and_16_chars() {
        let source = "key: value\n";
        let (entities, _) = parse_yaml_file(source, "test.yaml").unwrap();
        assert_eq!(entities.len(), 1);
        let e = &entities[0];
        assert_eq!(e.struct_hash.len(), 16);
        assert!(e.struct_hash.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(e.body_hash.is_some());
        assert_eq!(e.body_hash.as_ref().unwrap().len(), 16);
        assert!(e.sig_hash.is_some());
        assert_eq!(e.sig_hash.as_ref().unwrap().len(), 16);
    }

    #[test]
    fn parse_empty_mapping() {
        let source = "{}";
        let (entities, refs) = parse_yaml_file(source, "test.yaml").unwrap();
        assert!(entities.is_empty());
        assert!(refs.is_empty());
    }

    #[test]
    fn parse_invalid_yaml() {
        let result = parse_yaml_file(":\n  - :\n  - :\n    -", "test.yaml");
        assert!(result.is_err());
    }

    #[test]
    fn entities_sorted_by_line() {
        let source = "z_last: 1\na_first: 2\n";
        let (entities, _) = parse_yaml_file(source, "test.yaml").unwrap();
        assert!(entities[0].line_start <= entities[1].line_start);
    }

    #[test]
    fn multiline_mapping_span() {
        let source = "config:\n  a: 1\n  b: 2\nother: 3\n";
        let (entities, _) = parse_yaml_file(source, "test.yaml").unwrap();
        let config = entities.iter().find(|e| e.name == "config").unwrap();
        assert_eq!(config.line_start, 1);
        // config spans lines 1-3 (a: 1, b: 2 are children)
        assert!(config.line_end >= 3);
    }

    #[test]
    fn duplicate_keys_in_different_parents() {
        let source = "a:\n  id: 1\nb:\n  id: 2\n";
        let (entities, _) = parse_yaml_file(source, "test.yaml").unwrap();
        let ids: Vec<&Entity> = entities.iter().filter(|e| e.name == "id").collect();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0].parent, ids[1].parent);
    }

    #[test]
    fn parse_root_sequence_returns_empty() {
        let source = "- 1\n- 2\n- 3\n";
        let (entities, _) = parse_yaml_file(source, "test.yaml").unwrap();
        assert!(entities.is_empty());
    }

    #[test]
    fn meta_is_always_none() {
        let source = "key: value\n";
        let (entities, _) = parse_yaml_file(source, "test.yaml").unwrap();
        assert!(entities[0].meta.is_none());
    }

    #[test]
    fn keys_with_special_characters() {
        let source = "\"my.dotted.key\": 1\n\"key with spaces\": 2\n";
        let (entities, _) = parse_yaml_file(source, "test.yaml").unwrap();
        assert_eq!(entities.len(), 2);
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"my.dotted.key"));
        assert!(names.contains(&"key with spaces"));
    }

    #[test]
    fn comments_are_ignored_in_parsing() {
        let source = "# This is a comment\nname: value\n# Another comment\nother: data\n";
        let (entities, _) = parse_yaml_file(source, "test.yaml").unwrap();
        assert_eq!(entities.len(), 2);
    }

    #[test]
    fn flow_style_mapping() {
        let source = "config: {a: 1, b: 2}\n";
        let (entities, _) = parse_yaml_file(source, "test.yaml").unwrap();
        let config = entities.iter().find(|e| e.name == "config").unwrap();
        assert_eq!(config.kind, "object");
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --lib yaml_index 2>&1`
Expected: compilation errors since `parse_yaml_file` doesn't exist yet

- [ ] **Step 4: Implement the YAML parser**

Implement `parse_yaml_file` in `src/yaml_index.rs`. The structure mirrors `json_index.rs`:

```rust
use crate::entity::{Entity, Reference};
use crate::hasher;

/// Parse a YAML file and extract nested keys as entities.
pub fn parse_yaml_file(
    source: &str,
    file_path: &str,
) -> Result<(Vec<Entity>, Vec<Reference>), String> {
    let value: serde_yaml::Value = serde_yaml::from_str(source)
        .map_err(|e| format!("YAML parse error: {}", e))?;

    let mut entities = Vec::new();
    let line_positions = build_line_index(source);

    if let serde_yaml::Value::Mapping(map) = &value {
        let mut search_start = 0usize;
        extract_mapping_entities(source, file_path, map, None, &line_positions, &mut search_start, &mut entities);
    }

    entities.sort_by(|a, b| a.line_start.cmp(&b.line_start));
    Ok((entities, Vec::new()))
}
```

Key helper functions (same pattern as json_index.rs):

1. **`build_line_index(source) -> Vec<usize>`** — byte offsets for line starts (reuse same logic as json_index).

2. **`byte_offset_to_line(line_positions, offset) -> u32`** — binary search for line number.

3. **`find_key_line(source, key, search_start, line_positions) -> (u32, usize)`** — scan source for `key:` (YAML uses unquoted keys typically). Must handle both quoted (`"key":`, `'key':`) and unquoted (`key:`) forms. The search needle should try the unquoted form first, then quoted forms.

4. **`find_value_end_line(source, key_line, value, line_positions) -> u32`** — for mappings/sequences, determine the end line. YAML is indentation-based, so the end of a block value is where indentation returns to the parent level or a new key at the same level starts. For flow-style (`{...}` or `[...]`), track braces/brackets. For scalars, the end line is the key line.

5. **`extract_mapping_entities(source, file_path, map, parent, line_positions, search_start, entities)`** — iterate `serde_yaml::Mapping` entries, extract entities (same as `extract_object_entities` in json_index).

6. **`yaml_type_name(value) -> &str`** — returns type string for a `serde_yaml::Value`. Note: `serde_yaml::Value` has `Null`, `Bool`, `Number`, `String`, `Sequence`, `Mapping` variants.

7. **`entity_kind(value) -> &str`** — `"object"` for Mapping, `"array"` for Sequence, `"property"` for others.

**Key YAML-specific difference from JSON:**

YAML keys are found in source as `key:` (unquoted) or `"key":` / `'key':` (quoted). The `find_key_line` function should search for `key:` where `key` is followed by `:` and then a space, newline, or end of line. This is simpler than JSON's `"key":` pattern but needs care to avoid matching substrings (e.g., `keyname:` when looking for `key:`).

Strategy for `find_key_line`: Build multiple needle candidates in order of likelihood:
1. `key:` (unquoted, most common in YAML)
2. `"key":` (double-quoted)
3. `'key':` (single-quoted)

For each candidate, scan forward from `search_start` and verify the match is at the start of a line (after optional whitespace only). This prevents matching `keyname:` when looking for `key:`.

**Key YAML-specific difference for `find_value_end_line`:**

YAML block-style mappings and sequences use indentation rather than braces. To find the end of a block mapping value:
1. Get the indentation level of the key line
2. Scan forward until a line with equal or less indentation is found (that's the start of the next sibling or parent)
3. The end line is the last line before that point

For flow-style (`{...}`, `[...]`), use brace/bracket tracking like JSON.

For scalar values, the end line equals the key line (unless it's a multi-line scalar with `|` or `>`, in which case scan indented continuation lines).

- [ ] **Step 5: Run unit tests to verify they pass**

Run: `cargo test --lib yaml_index`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/main.rs src/yaml_index.rs
git commit -m "feat: add YAML parser module for structural entity extraction"
```

---

### Task 2: Route YAML Files in Index and Diff Pipelines

**Files:**
- Modify: `src/index.rs` (3 places: `parse_single_file`, `build_index`, `discover_source_files`)
- Modify: `src/diff.rs` (1 place: `compute_diff`)

- [ ] **Step 1: Route YAML in `parse_single_file`**

In `src/index.rs`, after the JSON early return (line 21-23), add:

```rust
if language == "yaml" {
    return crate::yaml_index::parse_yaml_file(source, file_path);
}
```

- [ ] **Step 2: Route YAML in `build_index` language detection**

In `src/index.rs`, modify the language detection (around line 135) to add YAML:

```rust
let lang = if ext == "json" {
    "json"
} else if ext == "yaml" || ext == "yml" {
    "yaml"
} else {
    match codeix::parser::languages::detect_language(ext) {
        Some(l) => l,
        None => {
            if verbose {
                eprintln!("skip (unsupported): {}", relative_str);
            }
            continue;
        }
    }
};
```

- [ ] **Step 3: Route YAML in `discover_source_files`**

In `src/index.rs`, modify the filter closure (around line 250-251):

```rust
.map(|ext| {
    ext == "json" || ext == "yaml" || ext == "yml"
        || codeix::parser::languages::detect_language(ext).is_some()
})
```

- [ ] **Step 4: Route YAML in `compute_diff`**

In `src/diff.rs`, modify the language detection (around line 44):

```rust
let lang: &str = if ext == "json" {
    "json"
} else if ext == "yaml" || ext == "yml" {
    "yaml"
} else {
    match codeix::parser::languages::detect_language(ext) {
        Some(l) => l,
        None => continue,
    }
};
```

- [ ] **Step 5: Run tests**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/index.rs src/diff.rs
git commit -m "feat: route YAML files to custom parser in index and diff pipelines"
```

---

### Task 3: Integration Tests

**Files:**
- Create: `tests/fixtures/sample.yaml`
- Modify: `tests/integration.rs`

- [ ] **Step 1: Create the YAML test fixture**

Create `tests/fixtures/sample.yaml`:

```yaml
name: sigil-test
version: 1.0.0
settings:
  theme:
    color: dark
    font_size: 14
  debug: false
  tags:
    - fast
    - structural
dependencies:
  serde: "1.0"
  blake3: "1.0"
```

- [ ] **Step 2: Write the integration test**

Add to `tests/integration.rs`:

```rust
#[test]
fn indexes_yaml_fixture() {
    let output = run_sigil_index(
        &fixture_path(),
        &["--files", &format!("{}/sample.yaml", fixture_path())],
    );
    let entities: Vec<serde_json::Value> = output.lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    // Root keys: name, version, settings, dependencies
    // Nested: theme (under settings), color, font_size (under theme),
    //         debug, tags (under settings), serde, blake3 (under dependencies)
    assert!(entities.len() >= 10, "expected at least 10 entities, got {}", entities.len());

    let names: Vec<&str> = entities.iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    assert!(names.contains(&"name"), "missing 'name' key");
    assert!(names.contains(&"settings"), "missing 'settings' key");
    assert!(names.contains(&"theme"), "missing 'theme' key");
    assert!(names.contains(&"color"), "missing 'color' key");
    assert!(names.contains(&"dependencies"), "missing 'dependencies' key");

    // Check kinds
    let settings = entities.iter().find(|e| e["name"] == "settings").unwrap();
    assert_eq!(settings["kind"].as_str().unwrap(), "object");

    let tags = entities.iter().find(|e| e["name"] == "tags").unwrap();
    assert_eq!(tags["kind"].as_str().unwrap(), "array");

    let color = entities.iter().find(|e| e["name"] == "color").unwrap();
    assert_eq!(color["kind"].as_str().unwrap(), "property");
    assert_eq!(color["parent"].as_str().unwrap(), "theme");

    // Check signatures
    assert_eq!(settings["sig"].as_str().unwrap(), "\"settings\": object");
    assert_eq!(color["sig"].as_str().unwrap(), "\"color\": string");

    // All struct_hashes must be 16 hex chars
    for entity in &entities {
        let sh = entity["struct_hash"].as_str().unwrap();
        assert_eq!(sh.len(), 16, "struct_hash wrong length for {}", entity["name"]);
    }
}
```

- [ ] **Step 3: Run integration tests**

Run: `cargo test --test integration indexes_yaml_fixture`
Expected: PASS

- [ ] **Step 4: Run full test suite**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add tests/fixtures/sample.yaml tests/integration.rs
git commit -m "test: add YAML indexing integration tests and fixture"
```

---

### Task 4: Update Documentation

**Files:**
- Modify: `README.md` (supported languages table)
- Modify: `CLAUDE.md` (architecture listing)

- [ ] **Step 1: Add YAML to the supported languages table in README.md**

In the "Supported Languages" section, add a row after the JSON row:

```markdown
| YAML | `.yaml` `.yml` (built-in parser) |
```

- [ ] **Step 2: Add yaml_index.rs to CLAUDE.md architecture listing**

In the `Architecture` section of `CLAUDE.md`, add after the `json_index.rs` entry:

```
  yaml_index.rs    — YAML file parsing (custom parser, not tree-sitter)
```

- [ ] **Step 3: Commit**

```bash
git add README.md CLAUDE.md
git commit -m "docs: add YAML to supported languages and architecture listing"
```

---

### Task 5: Smoke Test

- [ ] **Step 1: Run sigil on its own codebase**

```bash
cargo run -- index -v
```

Verify any `.yaml`/`.yml` files in the project are indexed.

- [ ] **Step 2: Test with a standalone YAML file**

```bash
echo -e "name: test\nnested:\n  key: val" > /tmp/test.yaml
cargo run -- index --files /tmp/test.yaml --stdout --full
```

Expected: 3 entities (name, nested, key) with correct kinds, parents, and hashes.
