/// Compute struct_hash: BLAKE3 of raw entity text, truncated to 16 hex chars.
pub fn struct_hash(raw_bytes: &[u8]) -> String {
    let hash = blake3::hash(raw_bytes);
    hash.to_hex()[..16].to_string()
}

/// Extract the raw text of an entity from source (1-indexed, inclusive).
/// Includes newlines between lines, matching "raw entity text exactly as in source."
pub fn extract_raw_bytes(source: &str, line_start: usize, line_end: usize) -> &str {
    let mut byte_offset = 0;
    let mut start_byte = 0;
    let mut end_byte = source.len();
    for (i, line) in source.split('\n').enumerate() {
        let line_num = i + 1;
        if line_num == line_start {
            start_byte = byte_offset;
        }
        byte_offset += line.len() + 1; // +1 for the '\n'
        if line_num == line_end {
            // Include the line content but not the trailing newline after the last line
            end_byte = (byte_offset - 1).min(source.len());
            break;
        }
    }
    &source[start_byte..end_byte]
}

/// Extract lines from source (1-indexed, inclusive).
fn get_lines(source: &str, start: usize, end: usize) -> Vec<&str> {
    source.lines()
        .skip(start.saturating_sub(1))
        .take(end.saturating_sub(start.saturating_sub(1)))
        .collect()
}

/// Check if a line is only a comment.
fn is_comment_only(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("//")
        || trimmed.starts_with('#')
        || trimmed.starts_with("/*")
        || trimmed.starts_with('*')
        || trimmed.starts_with("*/")
        || trimmed == "\"\"\""
        || trimmed == "'''"
}

/// Normalize body text for hashing.
/// Tracks `"""` / `'''` toggle state to strip multi-line docstring content.
fn normalize_body(lines: &[&str]) -> String {
    let mut in_docstring = false;
    let mut result = Vec::new();

    for raw_line in lines {
        let trimmed = raw_line.trim_start();

        // Toggle docstring state on triple-quote lines
        if trimmed.starts_with("\"\"\"") || trimmed.starts_with("'''") {
            let quote = &trimmed[..3];
            let count = trimmed.matches(quote).count();
            if count == 1 {
                in_docstring = !in_docstring;
            }
            // Either way, this line is part of a docstring — skip it
            continue;
        }

        if in_docstring || trimmed.is_empty() || is_comment_only(trimmed) {
            continue;
        }

        let collapsed = trimmed.split_whitespace().collect::<Vec<_>>().join(" ");
        result.push(collapsed);
    }

    result.join("\n")
}

/// Compute body_hash: BLAKE3 of normalized body.
/// Returns None for single-line entities or when body_start > line_end.
pub fn body_hash(source: &str, body_start_line: usize, line_end: usize) -> Option<String> {
    if body_start_line > line_end || body_start_line == 0 {
        return None;
    }
    let lines = get_lines(source, body_start_line, line_end);
    let normalized = normalize_body(&lines);
    if normalized.is_empty() {
        return None;
    }
    let hash = blake3::hash(normalized.as_bytes());
    Some(hash.to_hex()[..16].to_string())
}

/// Compute body_hash for config files: normalized raw text (strip indent, collapse whitespace).
/// Unlike code body_hash, does not strip comments or docstrings.
pub fn body_hash_raw(source: &str, line_start: usize, line_end: usize) -> Option<String> {
    if line_start > line_end || line_start == 0 {
        return None;
    }
    let lines = get_lines(source, line_start, line_end);
    let normalized: Vec<String> = lines.iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    if normalized.is_empty() {
        return None;
    }
    let joined = normalized.join("\n");
    let hash = blake3::hash(joined.as_bytes());
    Some(hash.to_hex()[..16].to_string())
}

/// Compute sig_hash: BLAKE3 of normalized signature.
pub fn sig_hash(sig: Option<&str>) -> Option<String> {
    let sig = sig?;
    let normalized = sig.trim().split_whitespace().collect::<Vec<_>>().join(" ");
    let hash = blake3::hash(normalized.as_bytes());
    Some(hash.to_hex()[..16].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_hash_deterministic() {
        let input = b"def foo():\n    return 1\n";
        assert_eq!(struct_hash(input), struct_hash(input));
    }

    #[test]
    fn struct_hash_16_hex_chars() {
        let h = struct_hash(b"anything");
        assert_eq!(h.len(), 16);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn struct_hash_changes_on_whitespace() {
        let a = struct_hash(b"def foo():\n    return 1\n");
        let b = struct_hash(b"def foo():\n        return 1\n");
        assert_ne!(a, b);
    }

    #[test]
    fn body_hash_strips_indentation() {
        let src_a = "def foo():\n    return 1\n";
        let src_b = "def foo():\n        return 1\n";
        assert_eq!(body_hash(src_a, 2, 2), body_hash(src_b, 2, 2));
    }

    #[test]
    fn body_hash_strips_comments() {
        let src_a = "def foo():\n    return 1\n";
        let src_b = "def foo():\n    # a comment\n    return 1\n";
        assert_eq!(body_hash(src_a, 2, 2), body_hash(src_b, 2, 3));
    }

    #[test]
    fn body_hash_detects_logic_change() {
        let src_a = "def foo():\n    return 1\n";
        let src_b = "def foo():\n    return 2\n";
        assert_ne!(body_hash(src_a, 2, 2), body_hash(src_b, 2, 2));
    }

    #[test]
    fn body_hash_none_for_single_line() {
        // For single-line entities, signature extraction returns body_start past line_end
        let src = "import os\n";
        assert!(body_hash(src, 2, 1).is_none());
    }

    #[test]
    fn body_hash_none_when_body_start_exceeds_end() {
        let src = "def foo(): pass\n";
        assert!(body_hash(src, 3, 1).is_none());
    }

    #[test]
    fn sig_hash_none_for_none() {
        assert!(sig_hash(None).is_none());
    }

    #[test]
    fn sig_hash_normalizes_whitespace() {
        let a = sig_hash(Some("def foo(x:  int) -> bool:"));
        let b = sig_hash(Some("def foo(x: int) -> bool:"));
        assert_eq!(a, b);
    }

    #[test]
    fn sig_hash_detects_param_change() {
        let a = sig_hash(Some("def foo(x: int) -> bool:"));
        let b = sig_hash(Some("def foo(x: int, y: int) -> bool:"));
        assert_ne!(a, b);
    }
}
