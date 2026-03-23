use std::collections::HashMap;
use std::path::Path;

use crate::diff_json::{DiffResult, EntityDiff};
use crate::entity::Entity;
use crate::git::{self, FileStatus};
use crate::index;
use crate::matcher;
use crate::classifier;
use crate::inline_diff;
use crate::change_detail;

pub struct DiffOptions {
    pub include_unchanged: bool,
    pub verbose: bool,
}

impl Default for DiffOptions {
    fn default() -> Self {
        DiffOptions { include_unchanged: false, verbose: false }
    }
}

pub fn compute_diff(
    root: &Path,
    base_ref: &str,
    head_ref: &str,
    opts: &DiffOptions,
) -> Result<DiffResult, String> {
    let changes = git::changed_files(root, base_ref, head_ref)?;

    if opts.verbose {
        eprintln!("{} files changed between {} and {}", changes.len(), base_ref, head_ref);
    }

    let mut old_entities: Vec<Entity> = Vec::new();
    let mut new_entities: Vec<Entity> = Vec::new();
    // Keep source texts for inline diffs
    let mut old_sources: HashMap<String, String> = HashMap::new();
    let mut new_sources: HashMap<String, String> = HashMap::new();

    for change in &changes {
        let ext = change.path.rsplit('.').next().unwrap_or("");
        let lang: &str = if ext == "json" {
            "json"
        } else {
            match codeix::parser::languages::detect_language(ext) {
                Some(l) => l,
                None => continue,
            }
        };

        if change.status != FileStatus::Added {
            match git::file_at_ref(root, base_ref, &change.path) {
                Ok(bytes) => {
                    if let Ok(source) = String::from_utf8(bytes) {
                        if let Ok((entities, _)) = index::parse_single_file(&source, &change.path, lang) {
                            old_entities.extend(entities);
                        }
                        old_sources.insert(change.path.clone(), source);
                    }
                }
                Err(e) => {
                    if opts.verbose { eprintln!("skip old {}: {}", change.path, e); }
                }
            }
        }

        if change.status != FileStatus::Deleted {
            match git::file_at_ref(root, head_ref, &change.path) {
                Ok(bytes) => {
                    if let Ok(source) = String::from_utf8(bytes) {
                        if let Ok((entities, _)) = index::parse_single_file(&source, &change.path, lang) {
                            new_entities.extend(entities);
                        }
                        new_sources.insert(change.path.clone(), source);
                    }
                }
                Err(e) => {
                    if opts.verbose { eprintln!("skip new {}: {}", change.path, e); }
                }
            }
        }
    }

    if opts.verbose {
        eprintln!("parsed {} old entities, {} new entities", old_entities.len(), new_entities.len());
    }

    let matches = matcher::match_entities(&old_entities, &new_entities);

    let mut entity_diffs: Vec<EntityDiff> = matches.iter()
        .map(|m| classifier::classify(m))
        .collect();

    // Enrich with inline diffs
    for diff in &mut entity_diffs {
        if let (Some(old_e), Some(new_e)) = (&diff.old, &diff.new) {
            if let (Some(old_src), Some(new_src)) = (old_sources.get(&old_e.file), new_sources.get(&new_e.file)) {
                let old_text = inline_diff::extract_entity_text(old_src, old_e.line_start, old_e.line_end);
                let new_text = inline_diff::extract_entity_text(new_src, new_e.line_start, new_e.line_end);
                diff.inline_diff = inline_diff::compute_inline_diff(&old_text, &new_text);
                if let Some(ref il) = diff.inline_diff {
                    let details = change_detail::extract_change_details(il);
                    if !details.is_empty() {
                        diff.change_details = Some(details);
                    }
                }
            }
        }
    }

    let patterns = DiffResult::detect_patterns(&entity_diffs);
    let summary = DiffResult::compute_summary(&entity_diffs);

    Ok(DiffResult {
        base_ref: base_ref.to_string(),
        head_ref: head_ref.to_string(),
        entities: entity_diffs,
        patterns,
        summary,
    })
}
