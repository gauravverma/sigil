use crate::output::DiffOutput;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupedEntity {
    pub change: String,
    pub name: String,
    pub kind: String,
    pub file: String,
    pub line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeGroup {
    pub label: String,
    pub file_count: usize,
    pub entities: Vec<GroupedEntity>,
}

/// Group entities by co-location and pattern membership.
/// Returns groups sorted by size (largest first).
pub fn compute_groups(output: &DiffOutput) -> Vec<ChangeGroup> {
    use std::collections::{HashMap, HashSet};

    // Build pattern membership: entity name → pattern id
    let mut pattern_groups: HashMap<String, Vec<GroupedEntity>> = HashMap::new();
    let mut in_pattern: HashSet<String> = HashSet::new();

    for pat in &output.patterns {
        let key = pat.id.clone();
        for section in &output.files {
            for entity in &section.entities {
                if pat.entities.contains(&entity.name) {
                    in_pattern.insert(format!("{}:{}", section.file, entity.name));
                    pattern_groups.entry(key.clone()).or_default().push(GroupedEntity {
                        change: entity.change.clone(),
                        name: entity.name.clone(),
                        kind: entity.kind.clone(),
                        file: section.file.clone(),
                        line: entity.line,
                    });
                }
            }
        }
    }

    // Group remaining entities by file (co-location)
    let mut file_groups: HashMap<String, Vec<GroupedEntity>> = HashMap::new();
    for section in &output.files {
        for entity in &section.entities {
            let key = format!("{}:{}", section.file, entity.name);
            if in_pattern.contains(&key) { continue; }
            if entity.change == "formatting_only" { continue; }
            file_groups.entry(section.file.clone()).or_default().push(GroupedEntity {
                change: entity.change.clone(),
                name: entity.name.clone(),
                kind: entity.kind.clone(),
                file: section.file.clone(),
                line: entity.line,
            });
        }
    }

    let mut groups = Vec::new();

    // Pattern-based groups
    for (pat_id, entities) in pattern_groups {
        if entities.is_empty() { continue; }
        let files: HashSet<&str> = entities.iter().map(|e| e.file.as_str()).collect();
        let label = output.patterns.iter()
            .find(|p| p.id == pat_id)
            .and_then(|p| p.entity_name.clone())
            .unwrap_or_else(|| entities[0].name.clone());
        groups.push(ChangeGroup {
            label: format!("{} (pattern across {} files)", label, files.len()),
            file_count: files.len(),
            entities,
        });
    }

    // File-based groups
    for (file, entities) in file_groups {
        if entities.is_empty() { continue; }
        let label = if entities.len() == 1 {
            format!("{} in {}", entities[0].name, file)
        } else {
            let names: Vec<&str> = entities.iter().take(3).map(|e| e.name.as_str()).collect();
            let suffix = if entities.len() > 3 { format!(" +{} more", entities.len() - 3) } else { String::new() };
            format!("{}{} in {}", names.join(", "), suffix, file)
        };
        groups.push(ChangeGroup {
            label,
            file_count: 1,
            entities,
        });
    }

    // Sort by number of entities descending
    groups.sort_by(|a, b| b.entities.len().cmp(&a.entities.len()));
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::*;

    fn make_output() -> DiffOutput {
        DiffOutput {
            meta: Meta {
                base_ref: "HEAD~1".to_string(),
                head_ref: "HEAD".to_string(),
                base_sha: None,
                head_sha: None,
                generated_at: String::new(),
                sigil_version: "0.2.0".to_string(),
            },
            summary: OutputSummary {
                files_changed: 2,
                patterns: 1,
                moves: 0,
                added: 1,
                removed: 0,
                modified: 2,
                renamed: 0,
                formatting_only: 0,
                has_breaking: false,
                natural_language: String::new(),
                summary_line: None,
            },
            breaking: vec![],
            patterns: vec![OutputPattern {
                id: "pat_1".to_string(),
                pattern_type: "body_identical".to_string(),
                entity_kind: "function".to_string(),
                from_glob: None,
                to_glob: None,
                entity_name: Some("init".to_string()),
                file_count: 2,
                files: vec!["a.py".to_string(), "b.py".to_string()],
                entities: vec!["init".to_string()],
            }],
            moves: vec![],
            files: vec![
                FileSection {
                    file: "a.py".to_string(),
                    summary: FileSummary { added: 0, modified: 1, removed: 0, renamed: 0, formatting_only: 0 },
                    entities: vec![OutputEntity {
                        change: "modified".to_string(),
                        name: "init".to_string(),
                        kind: "function".to_string(),
                        line: 10, line_end: 20,
                        sig_changed: None, body_changed: Some(true),
                        breaking: false, breaking_reason: None,
                        pattern_ref: Some("pat_1".to_string()),
                        token_changes: vec![], old_name: None, context: None,
                    }],
                },
                FileSection {
                    file: "b.py".to_string(),
                    summary: FileSummary { added: 1, modified: 1, removed: 0, renamed: 0, formatting_only: 0 },
                    entities: vec![
                        OutputEntity {
                            change: "modified".to_string(),
                            name: "init".to_string(),
                            kind: "function".to_string(),
                            line: 5, line_end: 15,
                            sig_changed: None, body_changed: Some(true),
                            breaking: false, breaking_reason: None,
                            pattern_ref: Some("pat_1".to_string()),
                            token_changes: vec![], old_name: None, context: None,
                        },
                        OutputEntity {
                            change: "added".to_string(),
                            name: "helper".to_string(),
                            kind: "function".to_string(),
                            line: 20, line_end: 30,
                            sig_changed: None, body_changed: None,
                            breaking: false, breaking_reason: None,
                            pattern_ref: None,
                            token_changes: vec![], old_name: None, context: None,
                        },
                    ],
                },
            ],
            groups: None,
        }
    }

    #[test]
    fn groups_pattern_entities_together() {
        let output = make_output();
        let groups = compute_groups(&output);
        let pat_group = groups.iter().find(|g| g.label.contains("pattern")).unwrap();
        assert_eq!(pat_group.entities.len(), 2);
        assert_eq!(pat_group.file_count, 2);
    }

    #[test]
    fn non_pattern_entities_grouped_by_file() {
        let output = make_output();
        let groups = compute_groups(&output);
        let file_group = groups.iter().find(|g| g.label.contains("helper"));
        assert!(file_group.is_some(), "non-pattern entity should be in a file group");
    }

    #[test]
    fn empty_output_produces_no_groups() {
        let output = DiffOutput {
            meta: Meta {
                base_ref: "HEAD~1".to_string(), head_ref: "HEAD".to_string(),
                base_sha: None, head_sha: None,
                generated_at: String::new(), sigil_version: "0.2.0".to_string(),
            },
            summary: OutputSummary {
                files_changed: 0, patterns: 0, moves: 0,
                added: 0, removed: 0, modified: 0, renamed: 0,
                formatting_only: 0, has_breaking: false,
                natural_language: String::new(), summary_line: None,
            },
            breaking: vec![], patterns: vec![], moves: vec![], files: vec![],
            groups: None,
        };
        assert!(compute_groups(&output).is_empty());
    }
}
