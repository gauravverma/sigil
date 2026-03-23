use crate::entity::{Entity, Reference};
use crate::hasher;

/// Parse a JSON file and extract nested keys as entities.
pub fn parse_json_file(
    source: &str,
    file_path: &str,
) -> Result<(Vec<Entity>, Vec<Reference>), String> {
    let value: serde_json::Value =
        serde_json::from_str(source).map_err(|e| format!("JSON parse error: {}", e))?;

    let mut entities = Vec::new();
    let line_positions = build_line_index(source);

    if let serde_json::Value::Object(map) = &value {
        let mut search_start = 0usize;
        extract_object_entities(
            source,
            file_path,
            map,
            None,
            &line_positions,
            &mut search_start,
            &mut entities,
        );
    }

    entities.sort_by(|a, b| a.line_start.cmp(&b.line_start));
    Ok((entities, Vec::new()))
}

/// Build an index of byte offsets for the start of each line.
/// Returns a Vec where index i holds the byte offset of line (i+1).
fn build_line_index(source: &str) -> Vec<usize> {
    let mut positions = vec![0usize]; // line 1 starts at byte 0
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            positions.push(i + 1);
        }
    }
    positions
}

/// Convert a byte offset to a 1-indexed line number using the line_positions index.
fn byte_offset_to_line(line_positions: &[usize], offset: usize) -> u32 {
    // Binary search for the last line whose start <= offset
    match line_positions.binary_search(&offset) {
        Ok(idx) => (idx + 1) as u32,
        Err(idx) => idx as u32, // idx is the insertion point; idx-1 is the line, so line number = idx
    }
}

/// Find the position of a key in the source text, scanning forward from search_start.
/// Returns (1-indexed line number, byte offset just past the colon after the key).
fn find_key_line(
    source: &str,
    key: &str,
    search_start: usize,
    line_positions: &[usize],
) -> (u32, usize) {
    // We need to find `"key"` followed by `:` in the source.
    // Build the needle: `"key"`
    let needle = serde_json::to_string(key).unwrap_or_else(|_| format!("\"{}\"", key));
    let bytes = source.as_bytes();
    let needle_bytes = needle.as_bytes();

    let mut pos = search_start;
    while pos + needle_bytes.len() <= bytes.len() {
        if let Some(rel) = source[pos..].find(&needle) {
            let abs = pos + rel;
            // Check that the character after the closing quote is eventually a colon
            let after_key = abs + needle_bytes.len();
            // Skip whitespace to find the colon
            let mut colon_pos = after_key;
            while colon_pos < bytes.len() && bytes[colon_pos].is_ascii_whitespace() {
                colon_pos += 1;
            }
            if colon_pos < bytes.len() && bytes[colon_pos] == b':' {
                let line = byte_offset_to_line(line_positions, abs);
                return (line, colon_pos + 1);
            }
            // Not a key usage, keep searching
            pos = abs + 1;
        } else {
            break;
        }
    }

    // Fallback: return line 1 if not found (should not happen with valid JSON)
    (1, search_start)
}

/// Find the end line of a value in the source. For objects/arrays, track brace/bracket depth.
/// For primitives, the end line equals the key line.
/// Returns (1-indexed end line, byte offset past the end of the value).
fn find_value_end_line(
    source: &str,
    value_start_byte: usize,
    value: &serde_json::Value,
    line_positions: &[usize],
) -> (u32, usize) {
    let bytes = source.as_bytes();

    // Skip whitespace to find the actual value start
    let mut pos = value_start_byte;
    while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
        pos += 1;
    }

    match value {
        serde_json::Value::Object(_) => {
            // Find the matching closing brace
            if pos < bytes.len() && bytes[pos] == b'{' {
                let end = find_matching_close(source, pos, b'{', b'}');
                let line = byte_offset_to_line(line_positions, end);
                (line, end + 1)
            } else {
                let line = byte_offset_to_line(line_positions, pos);
                (line, pos)
            }
        }
        serde_json::Value::Array(_) => {
            // Find the matching closing bracket
            if pos < bytes.len() && bytes[pos] == b'[' {
                let end = find_matching_close(source, pos, b'[', b']');
                let line = byte_offset_to_line(line_positions, end);
                (line, end + 1)
            } else {
                let line = byte_offset_to_line(line_positions, pos);
                (line, pos)
            }
        }
        _ => {
            // Primitive value: scan to the end of the value
            let end = find_primitive_end(source, pos);
            let line = byte_offset_to_line(line_positions, pos);
            (line, end)
        }
    }
}

/// Find the byte offset of the matching closing delimiter, handling nesting and strings.
fn find_matching_close(source: &str, start: usize, open: u8, close: u8) -> usize {
    let bytes = source.as_bytes();
    let mut depth = 0i32;
    let mut pos = start;
    let mut in_string = false;

    while pos < bytes.len() {
        let b = bytes[pos];
        if in_string {
            if b == b'\\' {
                pos += 1; // skip escaped character
            } else if b == b'"' {
                in_string = false;
            }
        } else {
            if b == b'"' {
                in_string = true;
            } else if b == open {
                depth += 1;
            } else if b == close {
                depth -= 1;
                if depth == 0 {
                    return pos;
                }
            }
        }
        pos += 1;
    }
    // Fallback: end of source
    source.len().saturating_sub(1)
}

/// Find the end of a primitive JSON value (string, number, boolean, null).
fn find_primitive_end(source: &str, start: usize) -> usize {
    let bytes = source.as_bytes();
    if start >= bytes.len() {
        return start;
    }

    if bytes[start] == b'"' {
        // String: find the closing quote
        let mut pos = start + 1;
        while pos < bytes.len() {
            if bytes[pos] == b'\\' {
                pos += 1; // skip escaped char
            } else if bytes[pos] == b'"' {
                return pos + 1;
            }
            pos += 1;
        }
        source.len()
    } else {
        // Number, boolean, null: scan until delimiter
        let mut pos = start;
        while pos < bytes.len() {
            match bytes[pos] {
                b',' | b'}' | b']' | b'\n' | b'\r' | b' ' | b'\t' => return pos,
                _ => pos += 1,
            }
        }
        source.len()
    }
}

/// Return the JSON type name for a value.
fn json_type_name(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

/// Return the entity kind for a value.
fn entity_kind(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Object(_) => "object",
        serde_json::Value::Array(_) => "array",
        _ => "property",
    }
}

/// Recursively extract entities from a JSON object.
fn extract_object_entities(
    source: &str,
    file_path: &str,
    map: &serde_json::Map<String, serde_json::Value>,
    parent: Option<&str>,
    line_positions: &[usize],
    search_start: &mut usize,
    entities: &mut Vec<Entity>,
) {
    for (key, value) in map {
        let (key_line, after_colon) = find_key_line(source, key, *search_start, line_positions);
        let (end_line, after_value) =
            find_value_end_line(source, after_colon, value, line_positions);

        // Update search_start so next key search starts after this value
        *search_start = after_value;

        let kind = entity_kind(value);
        let type_name = json_type_name(value);
        let sig = format!("\"{}\": {}", key, type_name);

        // Extract raw text for struct_hash
        let raw = hasher::extract_raw_bytes(source, key_line as usize, end_line as usize);
        let struct_hash = hasher::struct_hash(raw.as_bytes());

        // Compute body_hash from raw source lines
        let body_hash = hasher::body_hash_raw(source, key_line as usize, end_line as usize);

        // Compute sig_hash
        let sig_hash = hasher::sig_hash(Some(&sig));

        entities.push(Entity {
            file: file_path.to_string(),
            name: key.clone(),
            kind: kind.to_string(),
            line_start: key_line,
            line_end: end_line,
            parent: parent.map(|s| s.to_string()),
            sig: Some(sig),
            meta: None,
            body_hash,
            sig_hash,
            struct_hash,
        });

        // Recurse into nested objects
        if let serde_json::Value::Object(nested_map) = value {
            // For recursion, we need a new search_start that begins inside this object
            // Find the opening brace of this object
            let bytes = source.as_bytes();
            let mut obj_start = after_colon;
            while obj_start < bytes.len() && bytes[obj_start] != b'{' {
                obj_start += 1;
            }
            let mut child_search_start = obj_start + 1; // start after the opening brace
            extract_object_entities(
                source,
                file_path,
                nested_map,
                Some(key),
                line_positions,
                &mut child_search_start,
                entities,
            );
        }
    }
}

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

        let sigs: Vec<(&str, &str)> = entities
            .iter()
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

    #[test]
    fn keys_with_escaped_quotes() {
        let source = r#"{
  "say \"hello\"": 1
}"#;
        let (entities, _) = parse_json_file(source, "test.json").unwrap();
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, r#"say "hello""#);
        assert_eq!(entities[0].line_start, 2);
    }
}
