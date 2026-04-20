//! Query helpers used by `main.rs`. All queries run against the in-house
//! `Index` (see `src/query/index.rs`). codeix's SearchDb was removed on
//! Phase 0 day 6.

use std::path::Path;

use anyhow::{Context, Result};

use crate::entity::{Entity, Reference};
use crate::query::index::{FileHit, Index, SearchHit};

pub mod index;

/// Load the sigil index from `.sigil/` under `root`. Thin wrapper over
/// `Index::load` for call-site symmetry with the old `load_index`.
pub fn load(root: &Path) -> Result<Index> {
    let root = root
        .canonicalize()
        .with_context(|| format!("cannot resolve path: {}", root.display()))?;
    let idx = Index::load(&root).context("failed to load .sigil/ index")?;
    if idx.is_empty() {
        anyhow::bail!(
            "no sigil index found under {} — run `sigil index` first",
            root.display()
        );
    }
    Ok(idx)
}

// ──────────────────────────────────────────────────────────────────────────
// Human-readable formatters. Shapes mirror the pre-Phase-0 output so the
// CLI looks the same before/after the swap (modulo legitimate divergences
// documented in tests/parity_day4.rs — now deleted along with that file).
// ──────────────────────────────────────────────────────────────────────────

/// Directory overview for `sigil explore`. Driven by the Index directly.
pub fn explore_text(idx: &Index, path_prefix: Option<&str>, max_entries: usize) -> String {
    let overview = idx.explore_dir_overview(path_prefix);
    if overview.is_empty() {
        return "No files found.".to_string();
    }

    let visible_groups = overview.len().max(1);
    let cap = (max_entries / visible_groups).max(1);
    let files = idx.explore_files_capped(path_prefix, cap);

    let mut by_dir: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for (dir, file, _lang) in &files {
        by_dir.entry(dir.clone()).or_default().push(file.clone());
    }

    let total_map: std::collections::HashMap<&str, usize> = overview
        .iter()
        .map(|d| (d.path.as_str(), d.file_count))
        .collect();

    let mut out = String::new();
    for (dir, shown) in &by_dir {
        let dir_display = if dir.is_empty() { "." } else { dir.as_str() };
        let total = total_map.get(dir.as_str()).copied().unwrap_or(shown.len());
        out.push_str(&format!("{}/ ({} files)\n", dir_display, total));
        for f in shown {
            out.push_str(&format!("  {}\n", f));
        }
        let remaining = total.saturating_sub(shown.len());
        if remaining > 0 {
            out.push_str(&format!("  ... +{} more\n", remaining));
        }
    }
    out
}

pub fn format_search_hits(hits: &[SearchHit<'_>]) -> String {
    if hits.is_empty() {
        return "No results found.".to_string();
    }
    let mut out = String::new();
    for hit in hits {
        match hit {
            SearchHit::Symbol(e) => {
                let parent = e
                    .parent
                    .as_deref()
                    .map(|p| format!(" (in {})", p))
                    .unwrap_or_default();
                out.push_str(&format!(
                    "[symbol] {} {} {}:{}-{}{}\n",
                    e.kind, e.name, e.file, e.line_start, e.line_end, parent
                ));
            }
            SearchHit::File(FileHit {
                path,
                lang,
                entity_count,
            }) => {
                let lang = lang.as_deref().unwrap_or("unknown");
                out.push_str(&format!(
                    "[file]   {} ({}, {} symbols)\n",
                    path, lang, entity_count
                ));
            }
        }
    }
    out
}

pub fn format_entities(entities: &[&Entity]) -> String {
    if entities.is_empty() {
        return "No symbols found.".to_string();
    }
    let mut out = String::new();
    for e in entities {
        let parent = e
            .parent
            .as_deref()
            .map(|p| format!(" (in {})", p))
            .unwrap_or_default();
        out.push_str(&format!(
            "{:12} {:40} {}:{}-{}{}\n",
            e.kind, e.name, e.file, e.line_start, e.line_end, parent
        ));
    }
    out
}

pub fn format_refs(refs: &[&Reference]) -> String {
    if refs.is_empty() {
        return "No references found.".to_string();
    }
    let mut out = String::new();
    for r in refs {
        let caller = r.caller.as_deref().unwrap_or("<top-level>");
        out.push_str(&format!(
            "{:12} {} -> {} at {}:{}\n",
            r.ref_kind, caller, r.name, r.file, r.line
        ));
    }
    out
}
