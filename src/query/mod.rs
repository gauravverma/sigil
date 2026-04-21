//! Query helpers used by `main.rs`.
//!
//! Two entry points:
//!
//! - `Backend::load(root)` — picks between the in-memory `Index`
//!   (see `src/query/index.rs`) and the DuckDB backend
//!   (`duckdb_backend.rs`, gated on the `db` feature). Used by the
//!   routed commands: `sigil symbols` / `children` / `callers` / `callees`.
//!
//! - `load(root)` — legacy direct-to-Index path. Still used by `sigil
//!   diff`'s caller enrichment, `sigil explore`, and `sigil search`
//!   until their DuckDB equivalents + router wiring land.
//!
//! codeix's SearchDb was removed on Phase 0 day 6; nothing here depends
//! on it anymore.

use std::path::Path;

use anyhow::{Context, Result};

use crate::entity::{Entity, Reference};
use crate::query::index::{DirSummary, FileHit, Index, Scope, SearchHit};

/// Owned sibling of [`index::SearchHit`]. Used by the DuckDB router path
/// since cross-backend uniformity is easier with owned data — the
/// in-memory backend clones its borrows into owned form on return, and
/// the DuckDB path already produces owned rows.
#[derive(Debug, Clone)]
pub enum SearchHitOwned {
    Symbol(Entity),
    File(FileHit),
}

pub mod index;

/// DuckDB-backed scale path. Empty namespace when the `db` feature is
/// disabled; consumers never need to conditionally import.
#[cfg(feature = "db")]
pub mod duckdb_backend;

/// Query engine selector.
///
/// Wraps either the in-memory `Index` (always available) or the DuckDB
/// backend (built-in only when `--features db`). `Backend::load(root)`
/// picks one transparently:
///
/// 1. `SIGIL_BACKEND=memory` env var → force in-memory.
/// 2. `SIGIL_BACKEND=db` env var     → force DuckDB (errors if feature
///    is off — fail-loud beats silent fallback for reproducibility).
/// 3. Otherwise, engage DuckDB if JSONL size exceeds
///    `DEFAULT_AUTO_UPGRADE_THRESHOLD_BYTES` (50 MB by default, see
///    §14.9 of the plan).
/// 4. Fall back to in-memory.
///
/// Only covers the query methods wired into main.rs routing today:
/// get_callers / get_callees / get_file_symbols / get_children. The
/// heavier analytical commands (map / context / review / blast /
/// benchmark) stay on in-memory Index unconditionally until the
/// corresponding DuckDB methods + parity tests land.
pub enum Backend {
    InMemory(index::Index),
    #[cfg(feature = "db")]
    DuckDb(Box<duckdb_backend::DuckDbBackend>),
}

impl Backend {
    /// Pick a backend per the rules in the type doc. Returns an owned
    /// engine the caller can query directly.
    pub fn load(root: &Path) -> Result<Self> {
        let root = root
            .canonicalize()
            .with_context(|| format!("cannot resolve path: {}", root.display()))?;
        let forced = std::env::var("SIGIL_BACKEND").ok();
        match forced.as_deref() {
            Some("memory") => load_in_memory(&root),
            Some("db") => load_db(&root),
            Some(other) if !other.is_empty() => anyhow::bail!(
                "SIGIL_BACKEND={other:?} is not recognized — expected `memory` or `db`"
            ),
            _ => auto_load(&root),
        }
    }

    /// Convenience for callers that just want the counts. Matches the
    /// semantics of `Index::len` and `DuckDbBackend::len`.
    pub fn len(&self) -> (usize, usize) {
        match self {
            Self::InMemory(idx) => idx.len(),
            #[cfg(feature = "db")]
            Self::DuckDb(db) => db.len().unwrap_or((0, 0)),
        }
    }

    /// All refs whose target is `name`. Returns owned rows so the API is
    /// uniform across backends. In-memory walkers are cheap even with
    /// the clone; callers needing zero-copy can still use
    /// `Index::refs_to` directly.
    pub fn get_callers(
        &self,
        name: &str,
        kind_filter: Option<&str>,
        limit: usize,
    ) -> Vec<Reference> {
        match self {
            Self::InMemory(idx) => idx
                .get_callers(name, kind_filter, limit)
                .into_iter()
                .cloned()
                .collect(),
            #[cfg(feature = "db")]
            Self::DuckDb(db) => db
                .get_callers(name, kind_filter, limit)
                .unwrap_or_else(|e| {
                    eprintln!("warning: DuckDB get_callers failed: {e}");
                    Vec::new()
                }),
        }
    }

    pub fn get_callees(
        &self,
        caller: &str,
        kind_filter: Option<&str>,
        limit: usize,
    ) -> Vec<Reference> {
        match self {
            Self::InMemory(idx) => idx
                .get_callees(caller, kind_filter, limit)
                .into_iter()
                .cloned()
                .collect(),
            #[cfg(feature = "db")]
            Self::DuckDb(db) => db
                .get_callees(caller, kind_filter, limit)
                .unwrap_or_else(|e| {
                    eprintln!("warning: DuckDB get_callees failed: {e}");
                    Vec::new()
                }),
        }
    }

    pub fn get_file_symbols(
        &self,
        file: &str,
        kind_filter: Option<&str>,
        limit: usize,
    ) -> Vec<Entity> {
        match self {
            Self::InMemory(idx) => idx
                .get_file_symbols(file, kind_filter, limit)
                .into_iter()
                .cloned()
                .collect(),
            #[cfg(feature = "db")]
            Self::DuckDb(db) => db
                .get_file_symbols(file, kind_filter, limit)
                .unwrap_or_else(|e| {
                    eprintln!("warning: DuckDB get_file_symbols failed: {e}");
                    Vec::new()
                }),
        }
    }

    pub fn get_children(
        &self,
        file: &str,
        parent: &str,
        kind_filter: Option<&str>,
        limit: usize,
    ) -> Vec<Entity> {
        match self {
            Self::InMemory(idx) => idx
                .get_children(file, parent, kind_filter, limit)
                .into_iter()
                .cloned()
                .collect(),
            #[cfg(feature = "db")]
            Self::DuckDb(db) => db
                .get_children(file, parent, kind_filter, limit)
                .unwrap_or_else(|e| {
                    eprintln!("warning: DuckDB get_children failed: {e}");
                    Vec::new()
                }),
        }
    }

    /// Full search matching `Index::search` semantics. Returns owned
    /// hits so both backends feed the same downstream formatter.
    pub fn search(
        &self,
        query: &str,
        scope: Scope,
        kind_filter: Option<&str>,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Vec<SearchHitOwned> {
        match self {
            Self::InMemory(idx) => idx
                .search(query, scope, kind_filter, path_prefix, limit)
                .into_iter()
                .map(|h| match h {
                    SearchHit::Symbol(e) => SearchHitOwned::Symbol(e.clone()),
                    SearchHit::File(f) => SearchHitOwned::File(f),
                })
                .collect(),
            #[cfg(feature = "db")]
            Self::DuckDb(db) => db
                .search(query, scope, kind_filter, path_prefix, limit)
                .unwrap_or_else(|e| {
                    eprintln!("warning: DuckDB search failed: {e}");
                    Vec::new()
                }),
        }
    }

    pub fn explore_dir_overview(&self, path_prefix: Option<&str>) -> Vec<DirSummary> {
        match self {
            Self::InMemory(idx) => idx.explore_dir_overview(path_prefix),
            #[cfg(feature = "db")]
            Self::DuckDb(db) => db
                .explore_dir_overview(path_prefix)
                .unwrap_or_else(|e| {
                    eprintln!("warning: DuckDB explore_dir_overview failed: {e}");
                    Vec::new()
                }),
        }
    }

    pub fn explore_files_capped(
        &self,
        path_prefix: Option<&str>,
        cap_per_dir: usize,
    ) -> Vec<(String, String, Option<String>)> {
        match self {
            Self::InMemory(idx) => idx.explore_files_capped(path_prefix, cap_per_dir),
            #[cfg(feature = "db")]
            Self::DuckDb(db) => db
                .explore_files_capped(path_prefix, cap_per_dir)
                .unwrap_or_else(|e| {
                    eprintln!("warning: DuckDB explore_files_capped failed: {e}");
                    Vec::new()
                }),
        }
    }

    pub fn list_projects(&self) -> Vec<String> {
        match self {
            Self::InMemory(idx) => idx.list_projects(),
            #[cfg(feature = "db")]
            Self::DuckDb(db) => db.list_projects().unwrap_or_default(),
        }
    }

    /// Short label describing which backend is in play — useful for
    /// verbose output so users can confirm routing without guessing.
    pub fn label(&self) -> &'static str {
        match self {
            Self::InMemory(_) => "in-memory",
            #[cfg(feature = "db")]
            Self::DuckDb(_) => "duckdb",
        }
    }
}

fn load_in_memory(root: &Path) -> Result<Backend> {
    let idx = index::Index::load(root).context("failed to load .sigil/ index")?;
    if idx.is_empty() {
        anyhow::bail!(
            "no sigil index found under {} — run `sigil index` first",
            root.display()
        );
    }
    Ok(Backend::InMemory(idx))
}

#[cfg(feature = "db")]
fn load_db(root: &Path) -> Result<Backend> {
    let db = duckdb_backend::DuckDbBackend::open(root)?;
    if db.len().map(|(e, _)| e).unwrap_or(0) == 0 {
        anyhow::bail!(
            "DuckDB index is empty under {} — run `sigil index` first",
            root.display()
        );
    }
    Ok(Backend::DuckDb(Box::new(db)))
}

#[cfg(not(feature = "db"))]
fn load_db(_: &Path) -> Result<Backend> {
    anyhow::bail!(
        "SIGIL_BACKEND=db requested but this sigil was built without the `db` feature; \
         rebuild with `cargo install sigil --features db`"
    )
}

fn auto_load(root: &Path) -> Result<Backend> {
    #[cfg(feature = "db")]
    {
        let threshold = auto_engage_threshold_bytes();
        if duckdb_backend::should_auto_engage(root, threshold) {
            return load_db(root);
        }
    }
    load_in_memory(root)
}

/// Compute the DuckDB auto-engage threshold. `SIGIL_AUTO_ENGAGE_THRESHOLD_MB`
/// wins when set to a non-negative integer; falls back to the compiled-in
/// default (see `DEFAULT_AUTO_UPGRADE_THRESHOLD_BYTES`). Parse failures
/// fall back silently — a bogus env value shouldn't block query routing.
#[cfg(feature = "db")]
fn auto_engage_threshold_bytes() -> u64 {
    if let Ok(v) = std::env::var("SIGIL_AUTO_ENGAGE_THRESHOLD_MB")
        && let Ok(mb) = v.trim().parse::<u64>()
    {
        return mb.saturating_mul(1024 * 1024);
    }
    duckdb_backend::DEFAULT_AUTO_UPGRADE_THRESHOLD_BYTES
}

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

/// Owned-hit variant of [`format_search_hits`]. Called from the router
/// path where the Backend returns owned rows.
pub fn format_search_hits_owned(hits: &[SearchHitOwned]) -> String {
    if hits.is_empty() {
        return "No results found.".to_string();
    }
    let mut out = String::new();
    for hit in hits {
        match hit {
            SearchHitOwned::Symbol(e) => {
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
            SearchHitOwned::File(FileHit {
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

/// Render a tree-style explore overview from pre-computed
/// `(dirs, files)` pulled off any `Backend`. Matches the layout
/// `explore_text` produces on an Index.
pub fn render_explore(
    dirs: &[DirSummary],
    files: &[(String, String, Option<String>)],
) -> String {
    if dirs.is_empty() {
        return "No files found.".to_string();
    }
    let total_map: std::collections::HashMap<&str, usize> = dirs
        .iter()
        .map(|d| (d.path.as_str(), d.file_count))
        .collect();
    let mut by_dir: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    for (dir, file, _lang) in files {
        by_dir.entry(dir.clone()).or_default().push(file.clone());
    }
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

/// Emit a slice of entities as JSON on `w`. Default is minified; pass
/// `pretty=true` for indented output. When `with_hashes=false` the internal
/// BLAKE3 columns (`struct_hash`, `body_hash`, `sig_hash`) are stripped —
/// they rarely help downstream consumers (agents, grep pipelines) and
/// usually just inflate the payload. Set `with_hashes=true` for scripts
/// that want the raw JSONL view.
pub fn emit_entities_json<W: std::io::Write>(
    mut w: W,
    entities: &[&Entity],
    pretty: bool,
    with_hashes: bool,
) -> std::io::Result<()> {
    let mut values: Vec<serde_json::Value> = entities
        .iter()
        .map(|e| serde_json::to_value(e).expect("Entity serializes infallibly"))
        .collect();
    if !with_hashes {
        strip_hashes_in_place(&mut values);
    }
    if pretty {
        serde_json::to_writer_pretty(&mut w, &values)?;
    } else {
        serde_json::to_writer(&mut w, &values)?;
    }
    writeln!(w)
}

/// Emit a slice of references as JSON on `w`. References carry no hash
/// columns, so there's no `with_hashes` knob — the compact schema is the
/// only form. `pretty=true` gives indented output.
pub fn emit_references_json<W: std::io::Write>(
    mut w: W,
    refs: &[&Reference],
    pretty: bool,
) -> std::io::Result<()> {
    if pretty {
        serde_json::to_writer_pretty(&mut w, refs)?;
    } else {
        serde_json::to_writer(&mut w, refs)?;
    }
    writeln!(w)
}

fn strip_hashes_in_place(values: &mut [serde_json::Value]) {
    for v in values {
        if let Some(obj) = v.as_object_mut() {
            obj.remove("struct_hash");
            obj.remove("body_hash");
            obj.remove("sig_hash");
        }
    }
}

#[cfg(test)]
mod json_emit_tests {
    use super::*;
    use crate::entity::{BlastRadius, Entity};

    fn sample_struct() -> Entity {
        Entity {
            file: "src/x.rs".into(),
            name: "Foo".into(),
            kind: "struct".into(),
            line_start: 10,
            line_end: 20,
            parent: None,
            sig: Some("pub struct Foo".into()),
            meta: Some(vec!["Debug".into(), "Clone".into()]),
            body_hash: Some("abc".into()),
            sig_hash: Some("def".into()),
            struct_hash: "ghi".into(),
            visibility: Some("public".into()),
            rank: None,
            blast_radius: Some(BlastRadius {
                direct_callers: 3,
                direct_files: 1,
                transitive_callers: 7,
            }),
        }
    }

    fn sample_import() -> Entity {
        Entity {
            file: "src/x.rs".into(),
            name: "std::collections::HashMap".into(),
            kind: "import".into(),
            line_start: 1,
            line_end: 1,
            parent: None,
            sig: None,
            meta: Some(vec![]), // parser emits empty vec often
            body_hash: None,
            sig_hash: None,
            struct_hash: "h".into(),
            visibility: Some("private".into()),
            rank: None,
            blast_radius: Some(BlastRadius::default()), // all zeros
        }
    }

    #[test]
    fn compact_entity_drops_hashes_by_default() {
        let e = sample_struct();
        let es = vec![&e];
        let mut buf = Vec::new();
        emit_entities_json(&mut buf, &es, false, false).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(!s.contains("struct_hash"));
        assert!(!s.contains("body_hash"));
        assert!(!s.contains("sig_hash"));
        assert!(s.contains("\"name\":\"Foo\""));
        assert!(s.contains("\"blast_radius\""));
    }

    #[test]
    fn compact_entity_keeps_hashes_when_requested() {
        let e = sample_struct();
        let es = vec![&e];
        let mut buf = Vec::new();
        emit_entities_json(&mut buf, &es, false, true).unwrap();
        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("\"struct_hash\":\"ghi\""));
        assert!(s.contains("\"body_hash\":\"abc\""));
    }

    #[test]
    fn compact_entity_drops_noise_on_import() {
        let e = sample_import();
        let es = vec![&e];
        let mut buf = Vec::new();
        emit_entities_json(&mut buf, &es, false, false).unwrap();
        let s = String::from_utf8(buf).unwrap();
        // visibility "private" elided; zero blast_radius elided; empty meta
        // elided; hashes elided. Only the positive identity fields remain.
        assert!(!s.contains("visibility"));
        assert!(!s.contains("blast_radius"));
        assert!(!s.contains("meta"));
        assert!(!s.contains("struct_hash"));
        assert!(s.contains("\"kind\":\"import\""));
    }

    #[test]
    fn compact_output_is_minified_by_default() {
        let e = sample_struct();
        let es = vec![&e];
        let mut buf = Vec::new();
        emit_entities_json(&mut buf, &es, false, false).unwrap();
        let s = String::from_utf8(buf).unwrap();
        // Minified: no indented whitespace after commas, no newlines inside
        // the JSON payload.
        assert!(!s.contains(",\n  "));
        assert!(!s.contains(": "));
    }

    #[test]
    fn reference_serializes_kind_not_ref_kind() {
        use crate::entity::Reference;
        let r = Reference {
            file: "a.rs".into(),
            caller: Some("m".into()),
            name: "foo".into(),
            ref_kind: "call".into(),
            line: 7,
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"kind\":\"call\""));
        assert!(!s.contains("ref_kind"));

        // And the alias lets us read old-format refs.jsonl.
        let old = r#"{"file":"a.rs","caller":"m","name":"foo","ref_kind":"call","line":7}"#;
        let parsed: Reference = serde_json::from_str(old).unwrap();
        assert_eq!(parsed.ref_kind, "call");
    }
}
