# JSON Language Support Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add JSON file support to sigil, extracting nested object keys as hierarchical entities with proper line positions, hashing, and diff integration.

**Architecture:** JSON files bypass the codeix tree-sitter pipeline. A new `json_index` module uses `serde_json` to parse JSON structure, then a line-scanning pass maps each key to its source line positions. The module produces standard `Entity` structs that flow through the existing index, diff, and cache pipelines unchanged.

**Tech Stack:** `serde_json` with `preserve_order` feature (ensures Map iteration matches source key order for accurate line position scanning).

**Important:** `serde_json` is already a dependency but needs `features = ["preserve_order"]` added in `Cargo.toml`. Without this, `serde_json::Map` uses `BTreeMap` (alphabetical order), which breaks the sequential source-scanning algorithm since keys would be iterated in a different order than they appear in the source file.

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `Cargo.toml` | Modify | Add `preserve_order` feature to `serde_json` |
| `src/json_index.rs` | Create | JSON parsing, recursive entity extraction with line positions |
| `src/main.rs` | Modify | Add `mod json_index;` |
| `src/index.rs` | Modify | Route `.json` files to `json_index::parse_json_file` in both `parse_single_file` and `discover_source_files` |
| `src/diff.rs` | Modify | Route `.json` files to `json_index::parse_json_file` |
| `tests/fixtures/sample.json` | Create | Test fixture for integration tests |
| `tests/integration.rs` | Modify | Add JSON indexing integration test |
| `README.md` | Modify | Add JSON to supported languages table |
| `CLAUDE.md` | Modify | Add `json_index.rs` to architecture listing |

---

### Task 1: Create JSON Parser Module

**Files:**
- Modify: `Cargo.toml` (add `preserve_order` feature to `serde_json`)
- Create: `src/json_index.rs`

- [ ] **Step 0: Enable `preserve_order` feature for serde_json**

In `Cargo.toml`, change:
```toml
serde_json = "1"
```
to:
```toml
serde_json = { version = "1", features = ["preserve_order"] }
```

This makes `serde_json::Map` use `IndexMap` (insertion order) instead of `BTreeMap` (alphabetical order), which is critical for the sequential source-scanning algorithm to produce correct line positions.

- [ ] **Step 1: Write unit tests for the JSON parser**

Create `src/json_index.rs` with the module structure and tests at the bottom. The parser should:
- Parse JSON using `serde_json::Value`
- Scan the raw source text to find line positions for each key
- Recursively extract entities for all object keys
- Entity kinds: `"object"` for object values, `"array"` for array values, `"property"` for leaf values (string, number, boolean, null)
- Signatures: `"key": <type>` format (e.g., `"name": string`, `"settings": object`)
- Parent: immediate parent key name (not dotted path), `None` for root-level keys
- Hashes: `struct_hash` on raw text, `body_hash` on normalized value content, `sig_hash` on the signature

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_json() {
        let source = r#"{
  "name": "myapp",
  "version": "1.0.0"
}"#;
        let (entities, refs) = parse_json_file(source, "test.json").unwrap();
        assert!(refs.is_empty());
        assert_eq!(entities.len(), 2);

        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"name"));
        assert!(names.contains(&"version"));

        let name_entity = entities.iter().find(|e| e.name == "name").unwrap();
        assert_eq!(name_entity.kind, "property");
        assert_eq!(name_entity.sig.as_deref(), Some("\"name\": string"));
        assert!(name_entity.parent.is_none());
        assert_eq!(name_entity.line_start, 2);
        assert_eq!(name_entity.line_end, 2);
    }

    #[test]
    fn parse_nested_objects() {
        let source = r#"{
  "settings": {
    "theme": {
      "color": "dark"
    },
    "debug": true
  }
}"#;
        let (entities, _) = parse_json_file(source, "test.json").unwrap();
        assert_eq!(entities.len(), 4);

        let settings = entities.iter().find(|e| e.name == "settings").unwrap();
        assert_eq!(settings.kind, "object");
        assert!(settings.parent.is_none());
        assert_eq!(settings.line_start, 2);
        assert_eq!(settings.line_end, 7);

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
    fn parse_arrays() {
        let source = r#"{
  "tags": ["a", "b"],
  "items": [1, 2, 3]
}"#;
        let (entities, _) = parse_json_file(source, "test.json").unwrap();
        let tags = entities.iter().find(|e| e.name == "tags").unwrap();
        assert_eq!(tags.kind, "array");
        assert_eq!(tags.sig.as_deref(), Some("\"tags\": array"));
    }

    #[test]
    fn parse_all_value_types() {
        let source = r#"{
  "str_val": "hello",
  "num_val": 42,
  "float_val": 3.14,
  "bool_val": true,
  "null_val": null
}"#;
        let (entities, _) = parse_json_file(source, "test.json").unwrap();
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
        let source = r#"{"key": "value"}"#;
        let (entities, _) = parse_json_file(source, "test.json").unwrap();
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
    fn parse_empty_object() {
        let source = "{}";
        let (entities, refs) = parse_json_file(source, "test.json").unwrap();
        assert!(entities.is_empty());
        assert!(refs.is_empty());
    }

    #[test]
    fn parse_invalid_json() {
        let result = parse_json_file("{invalid", "test.json");
        assert!(result.is_err());
    }

    #[test]
    fn entities_sorted_by_line() {
        let source = r#"{
  "z_last": 1,
  "a_first": 2
}"#;
        let (entities, _) = parse_json_file(source, "test.json").unwrap();
        assert!(entities[0].line_start <= entities[1].line_start);
    }

    #[test]
    fn multiline_object_value_span() {
        let source = r#"{
  "config": {
    "a": 1,
    "b": 2
  }
}"#;
        let (entities, _) = parse_json_file(source, "test.json").unwrap();
        let config = entities.iter().find(|e| e.name == "config").unwrap();
        assert_eq!(config.line_start, 2);
        assert_eq!(config.line_end, 5);
    }

    #[test]
    fn duplicate_keys_in_different_parents() {
        let source = r#"{
  "a": { "id": 1 },
  "b": { "id": 2 }
}"#;
        let (entities, _) = parse_json_file(source, "test.json").unwrap();
        let ids: Vec<&Entity> = entities.iter().filter(|e| e.name == "id").collect();
        assert_eq!(ids.len(), 2);
        assert_ne!(ids[0].parent, ids[1].parent);
    }

    #[test]
    fn parse_root_array_returns_empty() {
        let source = r#"[1, 2, 3]"#;
        let (entities, _) = parse_json_file(source, "test.json").unwrap();
        assert!(entities.is_empty());
    }

    #[test]
    fn meta_is_always_none() {
        let source = r#"{"key": "value"}"#;
        let (entities, _) = parse_json_file(source, "test.json").unwrap();
        assert!(entities[0].meta.is_none());
    }

    #[test]
    fn keys_with_special_characters() {
        let source = r#"{
  "my.dotted.key": 1,
  "key with spaces": 2
}"#;
        let (entities, _) = parse_json_file(source, "test.json").unwrap();
        assert_eq!(entities.len(), 2);
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"my.dotted.key"));
        assert!(names.contains(&"key with spaces"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --lib json_index 2>&1`
Expected: compilation errors since `parse_json_file` doesn't exist yet

- [ ] **Step 3: Implement the JSON parser**

Implement `parse_json_file` in `src/json_index.rs`:

```rust
use crate::entity::{Entity, Reference};
use crate::hasher;

/// Parse a JSON file and extract nested keys as entities.
pub fn parse_json_file(
    source: &str,
    file_path: &str,
) -> Result<(Vec<Entity>, Vec<Reference>), String> {
    let value: serde_json::Value = serde_json::from_str(source)
        .map_err(|e| format!("JSON parse error: {}", e))?;

    let mut entities = Vec::new();
    let line_positions = build_line_index(source);

    if let serde_json::Value::Object(map) = &value {
        extract_object_entities(source, file_path, map, None, &line_positions, &mut entities);
    }

    entities.sort_by(|a, b| a.line_start.cmp(&b.line_start));
    Ok((entities, Vec::new()))
}
```

Key helper functions needed:

1. **`build_line_index(source) -> Vec<usize>`** — returns byte offset for the start of each line (index 0 = line 1).

2. **`find_key_line(source, key, search_start_byte, line_positions) -> (u32, usize)`** — scans source from `search_start_byte` to find `"key":`, returns (1-indexed line number, byte offset after the colon). Must handle escaped quotes in keys correctly.

3. **`find_value_end_line(source, value_start_byte, value, line_positions) -> (u32, usize)`** — determines the line where the value ends. For objects/arrays, finds the matching closing `}` or `]` tracking brace depth. For primitives, the end line is the same as the key line.

4. **`extract_object_entities(source, file_path, map, parent, line_positions, entities)`** — iterates the `serde_json::Map`, for each key:
   - Finds line position using `find_key_line`
   - Determines value type string (`"string"`, `"number"`, `"boolean"`, `"null"`, `"object"`, `"array"`)
   - Computes `line_end` using `find_value_end_line`
   - Extracts raw text for `struct_hash`
   - Computes `sig` as `"key": <type>`
   - Computes `sig_hash` and `body_hash`
   - For object values, recurses into `extract_object_entities` with current key as parent
   - Entity kind: `"object"` | `"array"` | `"property"`
   - Always set `meta: None` (JSON has no decorators/annotations)

5. **`json_type_name(value) -> &str`** — returns type string for a `serde_json::Value`.

- [ ] **Step 4: Run unit tests to verify they pass**

Run: `cargo test --lib json_index`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml src/json_index.rs
git commit -m "feat: add JSON parser module for structural entity extraction"
```

---

### Task 2: Register Module and Route JSON Files in Index Pipeline

**Files:**
- Modify: `src/main.rs:1` (add `mod json_index;`)
- Modify: `src/index.rs:16-23` (`parse_single_file` — add JSON branch before codeix)
- Modify: `src/index.rs:231-247` (`discover_source_files` — include `.json` files)

- [ ] **Step 1: Add `mod json_index` to main.rs**

Add after `mod index;` (line 10 in main.rs):
```rust
mod json_index;
```

- [ ] **Step 2: Route JSON in `parse_single_file`**

At the top of `parse_single_file` (before the codeix call), add an early return for JSON:

```rust
if language == "json" {
    return crate::json_index::parse_json_file(source, file_path);
}
```

- [ ] **Step 3: Route JSON in `discover_source_files`**

Modify the filter closure to also accept `.json` files:

```rust
.filter(|entry| {
    entry.path().extension()
        .and_then(|e| e.to_str())
        .map(|ext| {
            ext == "json" || codeix::parser::languages::detect_language(ext).is_some()
        })
        .unwrap_or(false)
})
```

- [ ] **Step 4: Route JSON in `build_index` language detection**

In the `build_index` loop (around line 130-139), change the language detection to handle JSON:

```rust
let lang = if ext == "json" {
    "json"
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

Note: `lang` type changes from `&str` (returned by `detect_language`) to `&str` (string literal), so no type issues.

- [ ] **Step 5: Run existing tests to verify nothing is broken**

Run: `cargo test`
Expected: All existing tests PASS

- [ ] **Step 6: Commit**

```bash
git add src/main.rs src/index.rs
git commit -m "feat: route JSON files to custom parser in index pipeline"
```

---

### Task 3: Route JSON Files in Diff Pipeline

**Files:**
- Modify: `src/diff.rs:43-47` (language detection in diff loop)

- [ ] **Step 1: Add JSON routing in diff**

In `compute_diff`, modify the language detection (around line 43-47):

```rust
let lang: &str = if ext == "json" {
    "json"
} else {
    match codeix::parser::languages::detect_language(ext) {
        Some(l) => l,
        None => continue,
    }
};
```

- [ ] **Step 2: Run tests**

Run: `cargo test`
Expected: All tests PASS

- [ ] **Step 3: Commit**

```bash
git add src/diff.rs
git commit -m "feat: route JSON files to custom parser in diff pipeline"
```

---

### Task 4: Integration Tests

**Files:**
- Create: `tests/fixtures/sample.json`
- Modify: `tests/integration.rs`

- [ ] **Step 1: Create the JSON test fixture**

```json
{
  "name": "sigil-test",
  "version": "1.0.0",
  "settings": {
    "theme": {
      "color": "dark",
      "font_size": 14
    },
    "debug": false,
    "tags": ["fast", "structural"]
  },
  "dependencies": {
    "serde": "1.0",
    "blake3": "1.0"
  }
}
```

- [ ] **Step 2: Write the integration test**

Add to `tests/integration.rs`:

```rust
#[test]
fn indexes_json_fixture() {
    let output = run_sigil_index(
        &fixture_path(),
        &["--files", &format!("{}/sample.json", fixture_path())],
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

Run: `cargo test --test integration indexes_json_fixture`
Expected: PASS

- [ ] **Step 4: Run full test suite**

Run: `cargo test`
Expected: All tests PASS (including existing deterministic_output, entities_sorted_by_file_then_line, struct_hash_always_present which now also cover JSON entities)

- [ ] **Step 5: Commit**

```bash
git add tests/fixtures/sample.json tests/integration.rs
git commit -m "test: add JSON indexing integration tests and fixture"
```

---

### Task 5: Update Documentation

**Files:**
- Modify: `README.md` (supported languages table)
- Modify: `CLAUDE.md` (architecture listing)

- [ ] **Step 1: Add JSON to the supported languages table in README.md**

In the "Supported Languages" section (around line 287), add a row after the Markdown row:

```markdown
| JSON | `.json` (built-in parser) |
```

- [ ] **Step 2: Add json_index.rs to CLAUDE.md architecture listing**

In the `Architecture` section of `CLAUDE.md`, add after the `index.rs` entry:

```
  json_index.rs    — JSON file parsing (custom parser, not tree-sitter)
```

- [ ] **Step 3: Commit**

```bash
git add README.md CLAUDE.md
git commit -m "docs: add JSON to supported languages and architecture listing"
```

---

### Task 6: Manual Smoke Test

- [ ] **Step 1: Run sigil on its own codebase to verify JSON indexing**

```bash
cargo run -- index -v
```

Check that `.sigil/entities.jsonl` contains JSON entities (e.g., from any `.json` files in the project).

- [ ] **Step 2: Test with a standalone JSON file**

```bash
echo '{"name": "test", "nested": {"key": "val"}}' > /tmp/test.json
cargo run -- index --root /tmp --files /tmp/test.json --stdout --full
```

Expected: 3 entities (name, nested, key) with correct kinds, parents, and hashes.
