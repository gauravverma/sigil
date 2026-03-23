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

#[allow(dead_code)]
pub struct DiffOptions {
    pub include_unchanged: bool,
    pub verbose: bool,
    pub include_context: bool,
    pub context_lines: usize,
}

impl Default for DiffOptions {
    fn default() -> Self {
        DiffOptions { include_unchanged: false, verbose: false, include_context: false, context_lines: 3 }
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
        // Skip .sigil/ internal cache files
        if change.path.starts_with(".sigil/") || change.path.starts_with(".sigil\\") {
            continue;
        }

        let ext = change.path.rsplit('.').next().unwrap_or("");
        let lang: &str = if ext == "json" {
            "json"
        } else if ext == "yaml" || ext == "yml" {
            "yaml"
        } else if ext == "toml" {
            "toml"
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

    let entity_diffs = match_classify_enrich(
        &old_entities, &new_entities, &old_sources, &new_sources, opts.verbose,
    );

    let patterns = DiffResult::detect_patterns(&entity_diffs);
    let summary = DiffResult::compute_summary(&entity_diffs);

    let base_sha = git::resolve_ref(root, base_ref).ok();
    let head_sha = git::resolve_ref(root, head_ref).ok();

    Ok(DiffResult {
        base_ref: base_ref.to_string(),
        head_ref: head_ref.to_string(),
        base_sha,
        head_sha,
        entities: entity_diffs,
        patterns,
        summary,
        old_sources: if opts.include_context { Some(old_sources) } else { None },
        new_sources: if opts.include_context { Some(new_sources) } else { None },
    })
}

/// Compare two files directly without git.
pub fn compute_file_diff(
    old_path: &Path,
    new_path: &Path,
    opts: &DiffOptions,
) -> Result<DiffResult, String> {
    let old_ext = old_path.extension().and_then(|e| e.to_str()).unwrap_or("");
    let new_ext = new_path.extension().and_then(|e| e.to_str()).unwrap_or("");

    let old_lang = detect_lang(old_ext)
        .ok_or_else(|| format!("unsupported file type: {}", old_path.display()))?;
    let new_lang = detect_lang(new_ext)
        .ok_or_else(|| format!("unsupported file type: {}", new_path.display()))?;

    if old_lang != new_lang {
        return Err(format!(
            "language mismatch: {} ({}) vs {} ({})",
            old_path.display(), old_lang, new_path.display(), new_lang
        ));
    }

    let old_source = std::fs::read_to_string(old_path)
        .map_err(|e| format!("cannot read {}: {}", old_path.display(), e))?;
    let new_source = std::fs::read_to_string(new_path)
        .map_err(|e| format!("cannot read {}: {}", new_path.display(), e))?;

    let old_path_str = old_path.to_string_lossy().to_string();
    let new_path_str = new_path.to_string_lossy().to_string();

    // Use the same file path for both so entities match by name (not treated as moves)
    let canonical_path = new_path_str.clone();

    let (old_entities, _) = index::parse_single_file(&old_source, &canonical_path, old_lang)?;
    let (new_entities, _) = index::parse_single_file(&new_source, &canonical_path, new_lang)?;

    if opts.verbose {
        eprintln!("parsed {} old entities, {} new entities", old_entities.len(), new_entities.len());
    }

    let mut old_sources: HashMap<String, String> = HashMap::new();
    let mut new_sources: HashMap<String, String> = HashMap::new();
    old_sources.insert(canonical_path.clone(), old_source);
    new_sources.insert(canonical_path, new_source);

    let entity_diffs = match_classify_enrich(
        &old_entities, &new_entities, &old_sources, &new_sources, opts.verbose,
    );

    let patterns = DiffResult::detect_patterns(&entity_diffs);
    let summary = DiffResult::compute_summary(&entity_diffs);

    Ok(DiffResult {
        base_ref: old_path_str,
        head_ref: new_path_str,
        base_sha: None,
        head_sha: None,
        entities: entity_diffs,
        patterns,
        summary,
        old_sources: if opts.include_context { Some(old_sources) } else { None },
        new_sources: if opts.include_context { Some(new_sources) } else { None },
    })
}

/// Detect language from file extension.
fn detect_lang(ext: &str) -> Option<&str> {
    match ext {
        "json" => Some("json"),
        "yaml" | "yml" => Some("yaml"),
        "toml" => Some("toml"),
        _ => codeix::parser::languages::detect_language(ext),
    }
}

/// Shared pipeline: match entities, classify changes, enrich with inline diffs.
fn match_classify_enrich(
    old_entities: &[Entity],
    new_entities: &[Entity],
    old_sources: &HashMap<String, String>,
    new_sources: &HashMap<String, String>,
    verbose: bool,
) -> Vec<EntityDiff> {
    let matches = matcher::match_entities(old_entities, new_entities);

    let mut entity_diffs: Vec<EntityDiff> = matches.iter()
        .map(|m| classifier::classify(m))
        .collect();

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

    if verbose {
        eprintln!("{} entity diffs produced", entity_diffs.len());
    }

    entity_diffs
}
