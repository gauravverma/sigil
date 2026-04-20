use pyo3::prelude::*;
use pyo3::exceptions::PyValueError;
use std::path::Path;

/// Diff two JSON strings and return a structured result as a Python dict.
///
/// Args:
///     old: The old JSON string
///     new: The new JSON string
///
/// Returns:
///     A dict with the diff result (same structure as `sigil diff --json`)
#[pyfunction]
fn diff_json(old: &str, new: &str) -> PyResult<Py<PyAny>> {
    // Normalize minified JSON
    let old_source = sigil_core::json_index::normalize_json_source(old)
        .unwrap_or_else(|| old.to_string());
    let new_source = sigil_core::json_index::normalize_json_source(new)
        .unwrap_or_else(|| new.to_string());

    // Parse both
    let (old_entities, _) = sigil_core::index::parse_single_file(&old_source, "old.json", "json")
        .map_err(|e| PyValueError::new_err(format!("failed to parse old JSON: {}", e)))?;
    let (new_entities, _) = sigil_core::index::parse_single_file(&new_source, "new.json", "json")
        .map_err(|e| PyValueError::new_err(format!("failed to parse new JSON: {}", e)))?;

    // Match, classify, enrich
    let mut old_sources = std::collections::HashMap::new();
    let mut new_sources = std::collections::HashMap::new();
    // Use same canonical path so entities match by name
    old_sources.insert("doc.json".to_string(), old_source);
    new_sources.insert("doc.json".to_string(), new_source);

    let (old_remapped, new_remapped): (Vec<_>, Vec<_>) = {
        let remap = |entities: Vec<sigil_core::entity::Entity>| -> Vec<sigil_core::entity::Entity> {
            entities.into_iter().map(|mut e| {
                e.file = "doc.json".to_string();
                e
            }).collect()
        };
        (remap(old_entities), remap(new_entities))
    };

    let matches = sigil_core::matcher::match_entities(&old_remapped, &new_remapped);
    let mut entity_diffs: Vec<sigil_core::diff_json::EntityDiff> = matches.iter()
        .map(|m| sigil_core::classifier::classify(m))
        .collect();

    // Enrich with inline diffs
    for diff in &mut entity_diffs {
        if let (Some(old_e), Some(new_e)) = (&diff.old, &diff.new) {
            if let (Some(old_src), Some(new_src)) = (old_sources.get(&old_e.file), new_sources.get(&new_e.file)) {
                let old_text = sigil_core::inline_diff::extract_entity_text(old_src, old_e.line_start, old_e.line_end);
                let new_text = sigil_core::inline_diff::extract_entity_text(new_src, new_e.line_start, new_e.line_end);
                diff.inline_diff = sigil_core::inline_diff::compute_inline_diff(&old_text, &new_text);
                if let Some(ref il) = diff.inline_diff {
                    let details = sigil_core::change_detail::extract_change_details(il);
                    if !details.is_empty() {
                        diff.change_details = Some(details);
                    }
                }
            }
        }
    }

    let patterns = sigil_core::diff_json::DiffResult::detect_patterns(&entity_diffs);
    let summary = sigil_core::diff_json::DiffResult::compute_summary(&entity_diffs);

    let result = sigil_core::diff_json::DiffResult {
        base_ref: "old".to_string(),
        head_ref: "new".to_string(),
        base_sha: None,
        head_sha: None,
        entities: entity_diffs,
        patterns,
        summary,
        old_sources: Some(old_sources),
        new_sources: Some(new_sources),
    };

    // Convert to DiffOutput (applies derived filtering, qualified names, etc.)
    let output = sigil_core::output::DiffOutput::from_result(&result, true, 3);

    // Serialize to JSON, then parse as Python dict
    let json_str = serde_json::to_string(&output)
        .map_err(|e| PyValueError::new_err(format!("serialization error: {}", e)))?;

    Python::attach(|py| {
        let json_mod = py.import("json")?;
        let dict = json_mod.call_method1("loads", (json_str,))?;
        Ok(dict.unbind())
    })
}

/// Diff two files and return a structured result as a Python dict.
///
/// Args:
///     old_path: Path to the old file
///     new_path: Path to the new file
///
/// Returns:
///     A dict with the diff result (same structure as `sigil diff --json`)
#[pyfunction]
fn diff_files(old_path: &str, new_path: &str) -> PyResult<Py<PyAny>> {
    let opts = sigil_core::diff::DiffOptions {
        include_unchanged: false,
        verbose: false,
        include_context: true,
        context_lines: 3,
    };

    let result = sigil_core::diff::compute_file_diff(
        Path::new(old_path),
        Path::new(new_path),
        &opts,
    ).map_err(|e| PyValueError::new_err(e))?;

    let output = sigil_core::output::DiffOutput::from_result(&result, true, 3);

    let json_str = serde_json::to_string(&output)
        .map_err(|e| PyValueError::new_err(format!("serialization error: {}", e)))?;

    Python::attach(|py| {
        let json_mod = py.import("json")?;
        let dict = json_mod.call_method1("loads", (json_str,))?;
        Ok(dict.unbind())
    })
}

/// Index a JSON string and return entities as a list of dicts.
///
/// Args:
///     source: The JSON string to index
///
/// Returns:
///     A list of entity dicts with name, kind, parent, line_start, line_end, etc.
#[pyfunction]
fn index_json(source: &str) -> PyResult<Py<PyAny>> {
    let effective = sigil_core::json_index::normalize_json_source(source)
        .unwrap_or_else(|| source.to_string());

    let (entities, _) = sigil_core::json_index::parse_json_file(&effective, "input.json")
        .map_err(|e| PyValueError::new_err(e))?;

    let json_str = serde_json::to_string(&entities)
        .map_err(|e| PyValueError::new_err(format!("serialization error: {}", e)))?;

    Python::attach(|py| {
        let json_mod = py.import("json")?;
        let list = json_mod.call_method1("loads", (json_str,))?;
        Ok(list.unbind())
    })
}

/// Diff two files using git refs and return a structured result.
///
/// Args:
///     repo_path: Path to the git repository root
///     base_ref: Base git ref (e.g., "HEAD~1", "main")
///     head_ref: Head git ref (e.g., "HEAD", "feature-branch")
///
/// Returns:
///     A dict with the diff result
#[pyfunction]
fn diff_refs(repo_path: &str, base_ref: &str, head_ref: &str) -> PyResult<Py<PyAny>> {
    let opts = sigil_core::diff::DiffOptions {
        include_unchanged: false,
        verbose: false,
        include_context: true,
        context_lines: 3,
    };

    let result = sigil_core::diff::compute_diff(
        Path::new(repo_path),
        base_ref,
        head_ref,
        &opts,
    ).map_err(|e| PyValueError::new_err(e))?;

    let output = sigil_core::output::DiffOutput::from_result(&result, true, 3);

    let json_str = serde_json::to_string(&output)
        .map_err(|e| PyValueError::new_err(format!("serialization error: {}", e)))?;

    Python::attach(|py| {
        let json_mod = py.import("json")?;
        let dict = json_mod.call_method1("loads", (json_str,))?;
        Ok(dict.unbind())
    })
}

/// sigil — Structural code fingerprinting and diffing
#[pymodule]
fn sigil(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(diff_json, m)?)?;
    m.add_function(wrap_pyfunction!(diff_files, m)?)?;
    m.add_function(wrap_pyfunction!(index_json, m)?)?;
    m.add_function(wrap_pyfunction!(diff_refs, m)?)?;
    Ok(())
}
