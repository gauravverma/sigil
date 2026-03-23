use std::path::Path;
use std::sync::{Arc, Mutex};
use anyhow::{Context, Result};
use codeix::server::db::SearchDb;
use codeix::mount::MountTable;

/// Load the codeix index for a project. Returns (MountTable, SearchDb).
/// Loads from .codeindex/ cache if available, builds from source otherwise.
pub fn load_index(root: &Path) -> Result<(Arc<Mutex<MountTable>>, Arc<Mutex<SearchDb>>)> {
    let root = root.canonicalize()
        .with_context(|| format!("cannot resolve path: {}", root.display()))?;
    codeix::cli::build::build_index_to_db(&root, true, true, None)
        .context("failed to build/load index")
}

/// Explore project structure.
pub fn explore(
    db: &SearchDb,
    project: Option<&str>,
    path_prefix: Option<&str>,
    max_entries: usize,
) -> Result<String> {
    let projects = db.list_projects()?;
    let proj = project.unwrap_or("");

    let overview = db.explore_dir_overview(proj, path_prefix)?;
    if overview.is_empty() {
        return Ok("No files found.".to_string());
    }

    // Calculate cap per group
    let visible_groups = overview.len().max(1);
    let cap = (max_entries / visible_groups).max(1);

    let files = db.explore_files_capped(proj, path_prefix, None, cap)?;

    let mut output = String::new();

    // Show subprojects if at root level
    if path_prefix.is_none() && projects.len() > 1 {
        output.push_str("Subprojects:\n");
        for p in &projects {
            if !p.is_empty() {
                output.push_str(&format!("  {}\n", p));
            }
        }
        output.push('\n');
    }

    // Group files by directory
    let mut by_dir: std::collections::BTreeMap<String, Vec<String>> = std::collections::BTreeMap::new();
    for (dir, file_path, _lang) in &files {
        by_dir.entry(dir.clone()).or_default().push(file_path.clone());
    }

    // Show total counts from overview
    let total_map: std::collections::HashMap<&str, usize> = overview.iter()
        .map(|(dir, _lang, _vis, count)| (dir.as_str(), *count))
        .fold(std::collections::HashMap::new(), |mut acc, (dir, count)| {
            *acc.entry(dir).or_insert(0) += count;
            acc
        });

    for (dir, shown_files) in &by_dir {
        let dir_display = if dir.is_empty() { "." } else { dir.as_str() };
        let total = total_map.get(dir.as_str()).copied().unwrap_or(shown_files.len());
        output.push_str(&format!("{}/ ({} files)\n", dir_display, total));
        for f in shown_files {
            output.push_str(&format!("  {}\n", f));
        }
        let remaining = total.saturating_sub(shown_files.len());
        if remaining > 0 {
            output.push_str(&format!("  ... +{} more\n", remaining));
        }
    }

    Ok(output)
}

/// Format search results.
pub fn format_search_results(results: &[codeix::server::db::SearchResult]) -> String {
    use codeix::server::db::SearchResult;

    if results.is_empty() {
        return "No results found.".to_string();
    }

    let mut output = String::new();
    for result in results {
        match result {
            SearchResult::Symbol(s) => {
                let vis = s.visibility.as_deref().unwrap_or("");
                let parent = s.parent.as_deref().map(|p| format!(" (in {})", p)).unwrap_or_default();
                output.push_str(&format!(
                    "[symbol] {} {} {}:{}-{}{}\n",
                    s.kind, s.name, s.file, s.line[0], s.line[1], parent
                ));
                if !vis.is_empty() {
                    output.push_str(&format!("         visibility: {}\n", vis));
                }
            }
            SearchResult::File(f) => {
                let lang = f.lang.as_deref().unwrap_or("unknown");
                let title = f.title.as_deref().unwrap_or("");
                output.push_str(&format!("[file]   {} ({}, {} lines)", f.path, lang, f.lines));
                if !title.is_empty() {
                    output.push_str(&format!(" -- {}", title));
                }
                output.push('\n');
            }
            SearchResult::Text(t) => {
                let parent = t.parent.as_deref().unwrap_or("");
                let text_preview: String = t.text.chars().take(100).collect();
                output.push_str(&format!(
                    "[text]   {} {}:{}-{} {}\n",
                    t.kind, t.file, t.line[0], t.line[1],
                    if !parent.is_empty() { format!("(in {}) ", parent) } else { String::new() }
                ));
                output.push_str(&format!("         {}\n", text_preview.replace('\n', " ")));
            }
        }
    }
    output
}

/// Format symbol entries.
pub fn format_symbols(symbols: &[codeix::index::format::SymbolEntry]) -> String {
    if symbols.is_empty() {
        return "No symbols found.".to_string();
    }
    let mut output = String::new();
    for s in symbols {
        let parent = s.parent.as_deref().map(|p| format!(" (in {})", p)).unwrap_or_default();
        let vis = s.visibility.as_deref().unwrap_or("");
        output.push_str(&format!(
            "{:12} {:40} {}:{}-{}{}\n",
            s.kind, s.name, s.file, s.line[0], s.line[1], parent
        ));
        if !vis.is_empty() && vis != "public" {
            output.push_str(&format!("{:12} visibility: {}\n", "", vis));
        }
    }
    output
}

/// Format reference entries.
pub fn format_references(refs: &[codeix::index::format::ReferenceEntry]) -> String {
    if refs.is_empty() {
        return "No references found.".to_string();
    }
    let mut output = String::new();
    for r in refs {
        let caller = r.caller.as_deref().unwrap_or("<top-level>");
        output.push_str(&format!(
            "{:12} {} -> {} at {}:{}\n",
            r.kind, caller, r.name, r.file, r.line[0]
        ));
    }
    output
}
