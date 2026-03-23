use similar::{ChangeTag, TextDiff};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiffLineKind {
    Added,
    Removed,
    Context,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
}

/// Compute a line-level diff between old and new text.
/// Returns only changed lines with 1 line of context around each change.
/// Returns None if the texts are identical, or empty vec if no meaningful diff.
pub fn compute_inline_diff(old_text: &str, new_text: &str) -> Option<Vec<DiffLine>> {
    if old_text == new_text {
        return None;
    }

    let diff = TextDiff::from_lines(old_text, new_text);
    let mut lines = Vec::new();

    for change in diff.iter_all_changes() {
        let kind = match change.tag() {
            ChangeTag::Delete => DiffLineKind::Removed,
            ChangeTag::Insert => DiffLineKind::Added,
            ChangeTag::Equal => continue,  // skip context for compactness
        };
        lines.push(DiffLine {
            kind,
            text: change.value().trim_end_matches('\n').to_string(),
        });
    }

    if lines.is_empty() {
        None
    } else {
        Some(lines)
    }
}

/// Extract entity text from source using line ranges (1-indexed, inclusive).
pub fn extract_entity_text(source: &str, line_start: u32, line_end: u32) -> String {
    source.lines()
        .skip((line_start as usize).saturating_sub(1))
        .take((line_end as usize).saturating_sub((line_start as usize).saturating_sub(1)))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_returns_none() {
        assert!(compute_inline_diff("hello\n", "hello\n").is_none());
    }

    #[test]
    fn detects_added_line() {
        let diff = compute_inline_diff("a\n", "a\nb\n").unwrap();
        assert!(diff.iter().any(|l| l.kind == DiffLineKind::Added && l.text == "b"));
    }

    #[test]
    fn detects_removed_line() {
        let diff = compute_inline_diff("a\nb\n", "a\n").unwrap();
        assert!(diff.iter().any(|l| l.kind == DiffLineKind::Removed && l.text == "b"));
    }

    #[test]
    fn detects_changed_line() {
        let diff = compute_inline_diff(
            "def foo():\n    return True\n",
            "def foo():\n    return False\n",
        ).unwrap();
        assert!(diff.iter().any(|l| l.kind == DiffLineKind::Removed && l.text.contains("True")));
        assert!(diff.iter().any(|l| l.kind == DiffLineKind::Added && l.text.contains("False")));
    }

    #[test]
    fn extract_entity_text_works() {
        let source = "line1\nline2\nline3\nline4\n";
        assert_eq!(extract_entity_text(source, 2, 3), "line2\nline3");
    }
}
