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
    shared as f64 / total as f64
}

fn diff_line_pair(old_line: &str, new_line: &str) -> Vec<ChangeDetail> {
    if is_comment_line(old_line) && is_comment_line(new_line) {
        return vec![ChangeDetail {
            kind: DetailKind::Comment,
            description: "comment updated".to_string(),
        }];
    }

    let diff = TextDiff::from_words(old_line.trim(), new_line.trim());
    let mut removed_tokens = Vec::new();
    let mut added_tokens = Vec::new();

    for change in diff.iter_all_changes() {
        let val = change.value().trim();
        if val.is_empty() { continue; }
        match change.tag() {
            ChangeTag::Delete => removed_tokens.push(val.to_string()),
            ChangeTag::Insert => added_tokens.push(val.to_string()),
            ChangeTag::Equal => {}
        }
    }

    if removed_tokens.is_empty() && added_tokens.is_empty() {
        return Vec::new();
    }

    let mut details = Vec::new();

    if removed_tokens.len() == added_tokens.len() {
        for (old_tok, new_tok) in removed_tokens.iter().zip(added_tokens.iter()) {
            let kind = classify_token_change(old_tok, new_tok);
            details.push(ChangeDetail {
                kind,
                description: format!("{} → {}", old_tok, new_tok),
            });
        }
    } else if removed_tokens.is_empty() {
        let joined = added_tokens.join(" ");
        details.push(ChangeDetail {
            kind: DetailKind::ArgumentAdded,
            description: format!("+ {}", joined),
        });
    } else if added_tokens.is_empty() {
        let joined = removed_tokens.join(" ");
        details.push(ChangeDetail {
            kind: DetailKind::ArgumentRemoved,
            description: format!("- {}", joined),
        });
    } else {
        let old_summary = removed_tokens.join(" ");
        let new_summary = added_tokens.join(" ");
        details.push(ChangeDetail {
            kind: DetailKind::ValueChanged,
            description: format!("{} → {}", old_summary, new_summary),
        });
    }

    details
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
}
