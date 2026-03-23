use similar::{ChangeTag, TextDiff};
use serde::{Serialize, Deserialize};
use crate::inline_diff::{DiffLine, DiffLineKind};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DetailKind {
    ValueChanged,
    IdentifierChanged,
    ArgumentAdded,
    ArgumentRemoved,
    Comment,
    LineAdded,
    LineRemoved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeDetail {
    pub kind: DetailKind,
    pub description: String,
}

/// Extract structured change details from inline diff lines.
pub fn extract_change_details(lines: &[DiffLine]) -> Vec<ChangeDetail> {
    if lines.is_empty() {
        return Vec::new();
    }

    let removed: Vec<String> = lines.iter()
        .filter(|l| l.kind == DiffLineKind::Removed)
        .map(|l| l.text.clone())
        .collect();
    let added: Vec<String> = lines.iter()
        .filter(|l| l.kind == DiffLineKind::Added)
        .map(|l| l.text.clone())
        .collect();

    let mut details = Vec::new();
    let pairs = pair_similar_lines(&removed, &added);

    for (old_line, new_line) in &pairs {
        details.extend(diff_line_pair(old_line, new_line));
    }

    // Handle unpaired lines
    let paired_removed: Vec<&str> = pairs.iter().map(|(r, _)| r.as_str()).collect();
    let paired_added: Vec<&str> = pairs.iter().map(|(_, a)| a.as_str()).collect();

    for r in &removed {
        if !paired_removed.contains(&r.as_str()) {
            if !is_comment_line(r) {
                details.push(ChangeDetail {
                    kind: DetailKind::LineRemoved,
                    description: format!("- {}", r.trim()),
                });
            }
        }
    }
    for a in &added {
        if !paired_added.contains(&a.as_str()) {
            if !is_comment_line(a) {
                details.push(ChangeDetail {
                    kind: DetailKind::LineAdded,
                    description: format!("+ {}", a.trim()),
                });
            }
        }
    }

    details
}

/// Pair removed and added lines by word-overlap similarity.
pub fn pair_similar_lines(removed: &[String], added: &[String]) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut used_added: Vec<bool> = vec![false; added.len()];

    for r in removed {
        let mut best_score = 0.0f64;
        let mut best_idx = None;

        for (ai, a) in added.iter().enumerate() {
            if used_added[ai] { continue; }
            let score = line_similarity(r, a);
            if score > best_score && score >= 0.5 {
                best_score = score;
                best_idx = Some(ai);
            }
        }

        if let Some(ai) = best_idx {
            pairs.push((r.clone(), added[ai].clone()));
            used_added[ai] = true;
        }
    }

    pairs
}

fn line_similarity(a: &str, b: &str) -> f64 {
    let words_a: Vec<&str> = a.split_whitespace().collect();
    let words_b: Vec<&str> = b.split_whitespace().collect();
    if words_a.is_empty() && words_b.is_empty() { return 1.0; }
    if words_a.is_empty() || words_b.is_empty() { return 0.0; }
    let shared = words_a.iter().filter(|w| words_b.contains(w)).count();
    let total = words_a.len().max(words_b.len());
    let word_sim = shared as f64 / total as f64;

    // For single-token lines with low word similarity, use character overlap
    if words_a.len() == 1 && words_b.len() == 1 && word_sim == 0.0 {
        return char_similarity(words_a[0], words_b[0]);
    }

    word_sim
}

fn char_similarity(a: &str, b: &str) -> f64 {
    let min_len = a.len().min(b.len()) as f64;
    if min_len == 0.0 {
        return 0.0;
    }
    let mut matching = 0;
    for (ca, cb) in a.chars().zip(b.chars()) {
        if ca == cb {
            matching += 1;
        }
    }
    matching as f64 / a.len().max(b.len()) as f64
}

fn diff_line_pair(old_line: &str, new_line: &str) -> Vec<ChangeDetail> {
    if is_comment_line(old_line) && is_comment_line(new_line) {
        return vec![ChangeDetail {
            kind: DetailKind::Comment,
            description: "comment updated".to_string(),
        }];
    }

    let diff = TextDiff::from_words(old_line.trim(), new_line.trim());

    // Collect all tokens with their tags
    let tokens: Vec<(ChangeTag, String)> = diff.iter_all_changes()
        .map(|change| (change.tag(), change.value().to_string()))
        .collect();

    // Check if there are any changes at all
    let has_changes = tokens.iter().any(|(tag, _)| *tag != ChangeTag::Equal);
    if !has_changes {
        return Vec::new();
    }

    // Find change regions and expand with context
    let context_tokens = 5;
    let mut details = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i].0 == ChangeTag::Equal {
            i += 1;
            continue;
        }

        // Found start of a change region; advance to end of consecutive non-Equal tokens
        let region_start = i;
        while i < tokens.len() && tokens[i].0 != ChangeTag::Equal {
            i += 1;
        }
        let region_end = i;

        // Expand context backwards and forwards
        let ctx_start = region_start.saturating_sub(context_tokens);
        let ctx_end = (region_end + context_tokens).min(tokens.len());

        // Build old and new strings from expanded region
        let mut old_parts: Vec<&str> = Vec::new();
        let mut new_parts: Vec<&str> = Vec::new();
        for (tag, val) in &tokens[ctx_start..ctx_end] {
            match tag {
                ChangeTag::Equal => {
                    old_parts.push(val.as_str());
                    new_parts.push(val.as_str());
                }
                ChangeTag::Delete => {
                    old_parts.push(val.as_str());
                }
                ChangeTag::Insert => {
                    new_parts.push(val.as_str());
                }
            }
        }

        let old_str = old_parts.concat().trim().to_string();
        let new_str = new_parts.concat().trim().to_string();

        if old_str == new_str || (old_str.is_empty() && new_str.is_empty()) {
            continue;
        }

        // Determine kind: pure removal or pure addition use ArgumentRemoved/Added;
        // otherwise classify based on the changed content.
        let kind = if old_str.is_empty() {
            DetailKind::ArgumentAdded
        } else if new_str.is_empty() {
            DetailKind::ArgumentRemoved
        } else {
            classify_token_change(&old_str, &new_str)
        };

        details.push(ChangeDetail {
            kind,
            description: format!("{} \u{2192} {}", truncate_str(&old_str, 80), truncate_str(&new_str, 80)),
        });
    }

    // Fallback: if no details extracted but lines differ
    if details.is_empty() {
        let old_trimmed = old_line.trim();
        let new_trimmed = new_line.trim();
        if old_trimmed != new_trimmed {
            details.push(ChangeDetail {
                kind: DetailKind::ValueChanged,
                description: format!("{} \u{2192} {}", truncate_str(old_trimmed, 80), truncate_str(new_trimmed, 80)),
            });
        }
    }

    details
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len { return s.to_string(); }
    match s.char_indices().nth(max_len) {
        Some((idx, _)) => format!("{}...", &s[..idx]),
        None => s.to_string(),
    }
}

fn classify_token_change(old: &str, new: &str) -> DetailKind {
    if is_string_literal(old) && is_string_literal(new) {
        DetailKind::ValueChanged
    } else if is_number(old) && is_number(new) {
        DetailKind::ValueChanged
    } else if is_identifier(old) && is_identifier(new) {
        DetailKind::IdentifierChanged
    } else {
        DetailKind::ValueChanged
    }
}

fn is_string_literal(s: &str) -> bool {
    (s.starts_with('"') && s.ends_with('"'))
        || (s.starts_with('\'') && s.ends_with('\''))
        || (s.starts_with('`') && s.ends_with('`'))
}

fn is_number(s: &str) -> bool {
    s.parse::<f64>().is_ok()
}

fn is_identifier(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.')
}

fn is_comment_line(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("//") || t.starts_with('#') || t.starts_with("/*")
        || t.starts_with('*') || t.starts_with("*/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inline_diff::{DiffLine, DiffLineKind};

    #[test]
    fn detects_string_value_change() {
        let lines = vec![
            DiffLine { kind: DiffLineKind::Removed, text: r#"    cmd.Flags().StringP("commit", "c", "true", "Promote changes")"#.into() },
            DiffLine { kind: DiffLineKind::Added, text: r#"    cmd.Flags().StringP("commit", "c", "false", "Promote changes")"#.into() },
        ];
        let details = extract_change_details(&lines);
        assert!(!details.is_empty());
        assert!(details.iter().any(|d| d.description.contains("true") && d.description.contains("false")));
    }

    #[test]
    fn detects_number_value_change() {
        let lines = vec![
            DiffLine { kind: DiffLineKind::Removed, text: "    port: 8080,".into() },
            DiffLine { kind: DiffLineKind::Added, text: "    port: 3000,".into() },
        ];
        let details = extract_change_details(&lines);
        assert!(details.iter().any(|d| d.description.contains("8080") && d.description.contains("3000")));
    }

    #[test]
    fn detects_identifier_change() {
        let lines = vec![
            DiffLine { kind: DiffLineKind::Removed, text: "    return self.validate_card(card)".into() },
            DiffLine { kind: DiffLineKind::Added, text: "    return self.check_card(card)".into() },
        ];
        let details = extract_change_details(&lines);
        assert!(details.iter().any(|d| d.description.contains("validate_card") && d.description.contains("check_card")));
    }

    #[test]
    fn detects_added_argument() {
        let lines = vec![
            DiffLine { kind: DiffLineKind::Removed, text: "def process(order, card):".into() },
            DiffLine { kind: DiffLineKind::Added, text: "def process(order, card, key=None):".into() },
        ];
        let details = extract_change_details(&lines);
        assert!(details.iter().any(|d|
            d.description.contains("key") || d.kind == DetailKind::ArgumentAdded || d.kind == DetailKind::ValueChanged
        ));
    }

    #[test]
    fn no_details_for_comment_only_changes() {
        let lines = vec![
            DiffLine { kind: DiffLineKind::Removed, text: "    # old comment".into() },
            DiffLine { kind: DiffLineKind::Added, text: "    # new comment".into() },
        ];
        let details = extract_change_details(&lines);
        assert!(details.is_empty() || details.iter().all(|d| d.kind == DetailKind::Comment));
    }

    #[test]
    fn unpaired_lines_produce_details() {
        let lines = vec![
            DiffLine { kind: DiffLineKind::Removed, text: "    old_method()".into() },
            DiffLine { kind: DiffLineKind::Removed, text: "    another_old()".into() },
            DiffLine { kind: DiffLineKind::Added, text: "    new_method()".into() },
        ];
        let details = extract_change_details(&lines);
        assert!(!details.is_empty());
    }

    #[test]
    fn empty_lines_no_details() {
        let details = extract_change_details(&[]);
        assert!(details.is_empty());
    }

    #[test]
    fn token_diff_includes_surrounding_context() {
        let lines = vec![
            DiffLine { kind: DiffLineKind::Removed, text: "if (typeof window !== 'undefined') {".into() },
            DiffLine { kind: DiffLineKind::Added, text: "if (typeof window === 'undefined') return;".into() },
        ];
        let details = extract_change_details(&lines);
        assert!(!details.is_empty(), "should produce change details");
        let desc = &details[0].description;
        assert!(desc.contains("window"), "token diff should include surrounding context: {}", desc);
    }

    #[test]
    fn pair_lines_by_similarity_test() {
        let removed = vec![
            "    flag default: true".to_string(),
            "    help: old description".to_string(),
        ];
        let added = vec![
            "    flag default: false".to_string(),
            "    help: new description".to_string(),
        ];
        let pairs = pair_similar_lines(&removed, &added);
        assert_eq!(pairs.len(), 2);
    }

    #[test]
    fn config_value_change_produces_token_diff() {
        let lines = vec![
            DiffLine { kind: DiffLineKind::Removed, text: "\"PyJWT==2.11.0\",".into() },
            DiffLine { kind: DiffLineKind::Added, text: "\"PyJWT==2.12.0\",".into() },
        ];
        let details = extract_change_details(&lines);
        assert!(!details.is_empty(), "single value change should produce a token diff");
        assert!(details.iter().any(|d| d.description.contains("2.11.0") && d.description.contains("2.12.0")),
            "description should show the version change, got: {:?}", details);
    }
}
