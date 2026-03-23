use serde::{Serialize, Deserialize};
use crate::entity::Entity;
use crate::inline_diff::DiffLine;
use crate::change_detail::ChangeDetail;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Removed,
    Modified,
    Moved,
    Renamed,
    FormattingOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityDiff {
    pub change: ChangeKind,
    pub name: String,
    pub kind: String,
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sig_changed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_changed: Option<bool>,
    pub breaking: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub breaking_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<Entity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new: Option<Entity>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_diff: Option<Vec<DiffLine>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_details: Option<Vec<ChangeDetail>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossFilePattern {
    pub description: String,
    pub entity_names: Vec<String>,
    pub files: Vec<String>,
    pub change: ChangeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffSummary {
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
    pub moved: usize,
    pub renamed: usize,
    pub formatting_only: usize,
    pub has_breaking_change: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffResult {
    pub base_ref: String,
    pub head_ref: String,
    pub entities: Vec<EntityDiff>,
    pub patterns: Vec<CrossFilePattern>,
    pub summary: DiffSummary,
}

impl DiffResult {
    pub fn compute_summary(entities: &[EntityDiff]) -> DiffSummary {
        let mut s = DiffSummary {
            added: 0, removed: 0, modified: 0,
            moved: 0, renamed: 0, formatting_only: 0,
            has_breaking_change: false,
        };
        for e in entities {
            match e.change {
                ChangeKind::Added => s.added += 1,
                ChangeKind::Removed => s.removed += 1,
                ChangeKind::Modified => s.modified += 1,
                ChangeKind::Moved => s.moved += 1,
                ChangeKind::Renamed => s.renamed += 1,
                ChangeKind::FormattingOnly => s.formatting_only += 1,
            }
            if e.breaking {
                s.has_breaking_change = true;
            }
        }
        s
    }

    pub fn detect_patterns(entities: &[EntityDiff]) -> Vec<CrossFilePattern> {
        use std::collections::HashMap;

        let mut groups: HashMap<(String, String), Vec<&EntityDiff>> = HashMap::new();
        for e in entities {
            if matches!(e.change, ChangeKind::FormattingOnly) { continue; }
            let base_name = e.name.rsplit('.').next().unwrap_or(&e.name).to_string();
            let key = (format!("{:?}", e.change), base_name);
            groups.entry(key).or_default().push(e);
        }

        let mut patterns = Vec::new();
        for ((change_str, base_name), group) in &groups {
            let mut files: Vec<String> = group.iter().map(|e| e.file.clone()).collect();
            files.sort();
            files.dedup();
            if files.len() < 2 { continue; }

            patterns.push(CrossFilePattern {
                description: format!(
                    "same {} applied to {} across {} files",
                    change_str.to_lowercase(), base_name, files.len()
                ),
                entity_names: group.iter().map(|e| e.name.clone()).collect(),
                files,
                change: group[0].change.clone(),
            });
        }

        patterns.sort_by(|a, b| b.files.len().cmp(&a.files.len()));
        patterns
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_summary_counts() {
        let entities = vec![
            EntityDiff {
                change: ChangeKind::Added, name: "foo".into(), kind: "function".into(),
                file: "a.py".into(), old_file: None, old_name: None,
                sig_changed: None, body_changed: None, breaking: false, breaking_reason: None,
                old: None, new: None, inline_diff: None, change_details: None,
            },
            EntityDiff {
                change: ChangeKind::Modified, name: "bar".into(), kind: "function".into(),
                file: "a.py".into(), old_file: None, old_name: None,
                sig_changed: Some(true), body_changed: Some(false), breaking: true,
                breaking_reason: Some("sig_changed".into()),
                old: None, new: None, inline_diff: None, change_details: None,
            },
            EntityDiff {
                change: ChangeKind::FormattingOnly, name: "baz".into(), kind: "function".into(),
                file: "a.py".into(), old_file: None, old_name: None,
                sig_changed: None, body_changed: None, breaking: false, breaking_reason: None,
                old: None, new: None, inline_diff: None, change_details: None,
            },
        ];
        let s = DiffResult::compute_summary(&entities);
        assert_eq!(s.added, 1);
        assert_eq!(s.modified, 1);
        assert_eq!(s.formatting_only, 1);
        assert!(s.has_breaking_change);
    }

    #[test]
    fn change_kind_serializes_snake_case() {
        let json = serde_json::to_string(&ChangeKind::FormattingOnly).unwrap();
        assert_eq!(json, "\"formatting_only\"");
    }

    #[test]
    fn diff_result_roundtrips_json() {
        let result = DiffResult {
            base_ref: "abc123".into(),
            head_ref: "def456".into(),
            entities: vec![],
            patterns: vec![],
            summary: DiffSummary {
                added: 1, removed: 0, modified: 0,
                moved: 0, renamed: 0, formatting_only: 0,
                has_breaking_change: false,
            },
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: DiffResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.base_ref, "abc123");
        assert_eq!(parsed.summary.added, 1);
    }

    #[test]
    fn detect_cross_file_pattern() {
        let diffs = vec![
            EntityDiff {
                change: ChangeKind::Modified, name: "init".into(), kind: "function".into(),
                file: "a.py".into(), old_file: None, old_name: None,
                sig_changed: Some(true), body_changed: Some(false), breaking: false, breaking_reason: None,
                old: None, new: None, inline_diff: None, change_details: None,
            },
            EntityDiff {
                change: ChangeKind::Modified, name: "init".into(), kind: "function".into(),
                file: "b.py".into(), old_file: None, old_name: None,
                sig_changed: Some(true), body_changed: Some(false), breaking: false, breaking_reason: None,
                old: None, new: None, inline_diff: None, change_details: None,
            },
        ];
        let patterns = DiffResult::detect_patterns(&diffs);
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].files.len(), 2);
        assert!(patterns[0].description.contains("init"));
    }
}
