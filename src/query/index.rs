//! In-house query index for sigil — replaces codeix's SearchDb on Phase 0 day 6.
//!
//! The index is built from `.sigil/entities.jsonl` + `.sigil/refs.jsonl` (sigil's
//! on-disk source of truth). It lives in memory and exposes the lookups that
//! `sigil callers / callees / symbols / children / search / explore` need.
//!
//! Scale-wise this is the Phase 0 backend — fine up to ~500k entities. The
//! DuckDB backend from Phase 0.5 (§14.9 of agent-adoption-plan.md) will slot in
//! behind the same public API once JSONL size crosses the auto-upgrade
//! threshold.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};

use crate::entity::{Entity, Reference};

/// In-memory index over sigil's entities and references.
///
/// Lookup complexity: O(1) for exact-name/exact-file lookups via the maps;
/// O(n) for substring search over entity names (still fast at <1M entities).
#[derive(Debug, Default)]
pub struct Index {
    pub entities: Vec<Entity>,
    pub references: Vec<Reference>,

    // Precomputed maps built during `build()`. Indices point into the vecs
    // above. Using `Vec<usize>` rather than `SmallVec` for now — easy to swap
    // later if a profile shows it matters.
    entities_by_name: HashMap<String, Vec<usize>>,
    entities_by_file: HashMap<String, Vec<usize>>,
    refs_by_name: HashMap<String, Vec<usize>>,     // target name → ref idxs (callers)
    refs_by_caller: HashMap<String, Vec<usize>>,   // caller → ref idxs (callees)
    refs_by_file: HashMap<String, Vec<usize>>,
}

impl Index {
    /// Build an index from already-in-memory entities + references. Takes
    /// ownership so we can move the vecs in rather than copying ~100 MB of
    /// data at large scale.
    pub fn build(entities: Vec<Entity>, references: Vec<Reference>) -> Self {
        let mut entities_by_name: HashMap<String, Vec<usize>> = HashMap::new();
        let mut entities_by_file: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, e) in entities.iter().enumerate() {
            entities_by_name.entry(e.name.clone()).or_default().push(i);
            entities_by_file.entry(e.file.clone()).or_default().push(i);
        }

        let mut refs_by_name: HashMap<String, Vec<usize>> = HashMap::new();
        let mut refs_by_caller: HashMap<String, Vec<usize>> = HashMap::new();
        let mut refs_by_file: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, r) in references.iter().enumerate() {
            refs_by_name.entry(r.name.clone()).or_default().push(i);
            if let Some(caller) = &r.caller {
                refs_by_caller.entry(caller.clone()).or_default().push(i);
            }
            refs_by_file.entry(r.file.clone()).or_default().push(i);
        }

        Index {
            entities,
            references,
            entities_by_name,
            entities_by_file,
            refs_by_name,
            refs_by_caller,
            refs_by_file,
        }
    }

    /// Load from `.sigil/entities.jsonl` + `.sigil/refs.jsonl` under the given
    /// project root. Missing files are treated as empty.
    pub fn load(root: &Path) -> Result<Self> {
        let sigil_dir = root.join(".sigil");
        let entities = read_jsonl::<Entity>(&sigil_dir.join("entities.jsonl"))
            .context("failed to load entities.jsonl")?;
        let references = read_jsonl::<Reference>(&sigil_dir.join("refs.jsonl"))
            .context("failed to load refs.jsonl")?;
        Ok(Self::build(entities, references))
    }

    /// Total counts — useful for stats output and for the Phase 0.5 DuckDB
    /// auto-upgrade heuristic.
    pub fn len(&self) -> (usize, usize) {
        (self.entities.len(), self.references.len())
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty() && self.references.is_empty()
    }

    /// Entities defined with this exact name. Multiple hits for ambiguous
    /// symbols (e.g., two modules each defining `Config`).
    pub fn entities_by_name(&self, name: &str) -> impl Iterator<Item = &Entity> {
        self.entities_by_name
            .get(name)
            .map(|idxs| idxs.as_slice())
            .unwrap_or(&[])
            .iter()
            .map(move |&i| &self.entities[i])
    }

    /// All entities in a file.
    pub fn entities_by_file(&self, file: &str) -> impl Iterator<Item = &Entity> {
        self.entities_by_file
            .get(file)
            .map(|idxs| idxs.as_slice())
            .unwrap_or(&[])
            .iter()
            .map(move |&i| &self.entities[i])
    }

    /// References whose *target* is `name` — i.e., callers of `name`.
    pub fn refs_to(&self, name: &str) -> impl Iterator<Item = &Reference> {
        self.refs_by_name
            .get(name)
            .map(|idxs| idxs.as_slice())
            .unwrap_or(&[])
            .iter()
            .map(move |&i| &self.references[i])
    }

    /// References whose *caller* is `caller` — i.e., what `caller` calls.
    pub fn refs_from(&self, caller: &str) -> impl Iterator<Item = &Reference> {
        self.refs_by_caller
            .get(caller)
            .map(|idxs| idxs.as_slice())
            .unwrap_or(&[])
            .iter()
            .map(move |&i| &self.references[i])
    }

    /// References defined in a file.
    pub fn refs_in_file(&self, file: &str) -> impl Iterator<Item = &Reference> {
        self.refs_by_file
            .get(file)
            .map(|idxs| idxs.as_slice())
            .unwrap_or(&[])
            .iter()
            .map(move |&i| &self.references[i])
    }

    // ──────────────────────────────────────────────────────────────────────
    // Public query API — mirrors codeix::SearchDb methods used by main.rs.
    // Return sigil's own `Entity` / `Reference` types; the day-6 switch in
    // main.rs swaps these in and deletes the codeix-backed functions in
    // src/query/mod.rs.
    //
    // `kind_filter`: exact-match filter on ref_kind (for refs) or kind (for
    // entities). None = no filter. Matches codeix's behavior.
    //
    // `limit`: 0 = unlimited. Positive = take at most `limit` results.
    // Results are returned in insertion order (which, for sigil's index, is
    // sorted by (file, line_start) per the project convention — so this
    // ordering is stable across runs).
    // ──────────────────────────────────────────────────────────────────────

    /// All references targeting `name`, optionally filtered by ref kind.
    pub fn get_callers(&self, name: &str, kind_filter: Option<&str>, limit: usize) -> Vec<&Reference> {
        let iter = self.refs_to(name).filter(|r| match kind_filter {
            Some(k) => r.ref_kind == k,
            None => true,
        });
        apply_limit(iter, limit)
    }

    /// All references made by `caller`, optionally filtered by ref kind.
    pub fn get_callees(&self, caller: &str, kind_filter: Option<&str>, limit: usize) -> Vec<&Reference> {
        let iter = self.refs_from(caller).filter(|r| match kind_filter {
            Some(k) => r.ref_kind == k,
            None => true,
        });
        apply_limit(iter, limit)
    }

    /// All entities defined in `file`, optionally filtered by entity kind.
    pub fn get_file_symbols(&self, file: &str, kind_filter: Option<&str>, limit: usize) -> Vec<&Entity> {
        let iter = self.entities_by_file(file).filter(|e| match kind_filter {
            Some(k) => e.kind == k,
            None => true,
        });
        apply_limit(iter, limit)
    }

    /// All entities in `file` whose `parent` matches `parent`.
    pub fn get_children(
        &self,
        file: &str,
        parent: &str,
        kind_filter: Option<&str>,
        limit: usize,
    ) -> Vec<&Entity> {
        let iter = self.entities_by_file(file).filter(|e| {
            e.parent.as_deref() == Some(parent)
                && match kind_filter {
                    Some(k) => e.kind == k,
                    None => true,
                }
        });
        apply_limit(iter, limit)
    }
}

fn apply_limit<'a, T, I>(iter: I, limit: usize) -> Vec<&'a T>
where
    I: Iterator<Item = &'a T>,
{
    if limit == 0 {
        iter.collect()
    } else {
        iter.take(limit).collect()
    }
}

fn read_jsonl<T: for<'de> serde::Deserialize<'de>>(path: &Path) -> Result<Vec<T>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    let mut out = Vec::new();
    for (lineno, line) in content.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let item: T = serde_json::from_str(line)
            .with_context(|| format!("{}:{}: parse JSON", path.display(), lineno + 1))?;
        out.push(item);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Entity, Reference};

    fn ent(file: &str, name: &str, kind: &str) -> Entity {
        Entity {
            file: file.to_string(),
            name: name.to_string(),
            kind: kind.to_string(),
            line_start: 1,
            line_end: 2,
            parent: None,
            sig: None,
            meta: None,
            body_hash: None,
            sig_hash: None,
            struct_hash: "deadbeef".to_string(),
        }
    }

    fn refr(file: &str, caller: Option<&str>, name: &str, kind: &str) -> Reference {
        Reference {
            file: file.to_string(),
            caller: caller.map(|c| c.to_string()),
            name: name.to_string(),
            ref_kind: kind.to_string(),
            line: 1,
        }
    }

    #[test]
    fn empty_index_has_zero_counts() {
        let idx = Index::build(vec![], vec![]);
        assert_eq!(idx.len(), (0, 0));
        assert!(idx.is_empty());
    }

    #[test]
    fn entities_by_name_finds_all_matches() {
        let idx = Index::build(
            vec![
                ent("a.rs", "foo", "function"),
                ent("b.rs", "foo", "function"), // ambiguous — two files define foo
                ent("c.rs", "bar", "function"),
            ],
            vec![],
        );
        let foos: Vec<_> = idx.entities_by_name("foo").collect();
        assert_eq!(foos.len(), 2);
        let bars: Vec<_> = idx.entities_by_name("bar").collect();
        assert_eq!(bars.len(), 1);
        let missing: Vec<_> = idx.entities_by_name("nope").collect();
        assert_eq!(missing.len(), 0);
    }

    #[test]
    fn entities_by_file_groups_correctly() {
        let idx = Index::build(
            vec![
                ent("a.rs", "foo", "function"),
                ent("a.rs", "bar", "function"),
                ent("b.rs", "baz", "function"),
            ],
            vec![],
        );
        let in_a: Vec<_> = idx.entities_by_file("a.rs").collect();
        assert_eq!(in_a.len(), 2);
        let in_b: Vec<_> = idx.entities_by_file("b.rs").collect();
        assert_eq!(in_b.len(), 1);
    }

    #[test]
    fn refs_to_returns_callers() {
        let idx = Index::build(
            vec![ent("a.rs", "foo", "function")],
            vec![
                refr("b.rs", Some("main"), "foo", "call"),
                refr("c.rs", Some("helper"), "foo", "call"),
                refr("d.rs", Some("main"), "other", "call"), // should not match
            ],
        );
        let callers: Vec<_> = idx.refs_to("foo").collect();
        assert_eq!(callers.len(), 2);
        let callers_other: Vec<_> = idx.refs_to("other").collect();
        assert_eq!(callers_other.len(), 1);
    }

    #[test]
    fn refs_from_returns_callees() {
        let idx = Index::build(
            vec![],
            vec![
                refr("a.rs", Some("main"), "foo", "call"),
                refr("a.rs", Some("main"), "bar", "call"),
                refr("a.rs", Some("helper"), "foo", "call"),
            ],
        );
        let from_main: Vec<_> = idx.refs_from("main").collect();
        assert_eq!(from_main.len(), 2);
        let from_helper: Vec<_> = idx.refs_from("helper").collect();
        assert_eq!(from_helper.len(), 1);
    }

    #[test]
    fn refs_with_no_caller_skipped_in_refs_from() {
        // Top-level refs (no enclosing caller) must not appear in refs_from.
        let idx = Index::build(
            vec![],
            vec![
                refr("a.rs", None, "foo", "import"),
                refr("a.rs", Some("main"), "foo", "call"),
            ],
        );
        let from_main: Vec<_> = idx.refs_from("main").collect();
        assert_eq!(from_main.len(), 1);
        // Top-level ref is still findable via refs_to
        let to_foo: Vec<_> = idx.refs_to("foo").collect();
        assert_eq!(to_foo.len(), 2);
    }

    #[test]
    fn refs_in_file_groups_by_file() {
        let idx = Index::build(
            vec![],
            vec![
                refr("a.rs", Some("m"), "x", "call"),
                refr("a.rs", Some("m"), "y", "call"),
                refr("b.rs", Some("m"), "z", "call"),
            ],
        );
        let in_a: Vec<_> = idx.refs_in_file("a.rs").collect();
        assert_eq!(in_a.len(), 2);
    }

    #[test]
    fn load_missing_dir_returns_empty_index() {
        let tmp = std::env::temp_dir().join(format!("sigil_query_empty_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).unwrap();
        let idx = Index::load(&tmp).expect("missing jsonl should load as empty");
        assert!(idx.is_empty());
        std::fs::remove_dir_all(&tmp).ok();
    }

    // ──────────────────────────────────────────────────────────────────
    // Day-4 public API: get_callers / get_callees / get_file_symbols /
    // get_children — kind filter + limit semantics.
    // ──────────────────────────────────────────────────────────────────

    #[test]
    fn get_callers_filters_by_kind() {
        let idx = Index::build(
            vec![],
            vec![
                refr("a.rs", Some("m"), "foo", "call"),
                refr("b.rs", Some("m"), "foo", "import"),
                refr("c.rs", Some("m"), "foo", "call"),
            ],
        );
        assert_eq!(idx.get_callers("foo", None, 0).len(), 3);
        assert_eq!(idx.get_callers("foo", Some("call"), 0).len(), 2);
        assert_eq!(idx.get_callers("foo", Some("import"), 0).len(), 1);
        assert_eq!(idx.get_callers("foo", Some("nope"), 0).len(), 0);
    }

    #[test]
    fn get_callers_respects_limit() {
        let idx = Index::build(
            vec![],
            (0..10)
                .map(|i| refr(&format!("f{i}.rs"), Some("m"), "foo", "call"))
                .collect(),
        );
        assert_eq!(idx.get_callers("foo", None, 0).len(), 10, "limit 0 = unlimited");
        assert_eq!(idx.get_callers("foo", None, 3).len(), 3);
        assert_eq!(idx.get_callers("foo", None, 100).len(), 10, "limit > total returns all");
    }

    #[test]
    fn get_callees_filters_and_limits() {
        let idx = Index::build(
            vec![],
            vec![
                refr("a.rs", Some("main"), "foo", "call"),
                refr("a.rs", Some("main"), "Bar", "instantiation"),
                refr("a.rs", Some("main"), "baz", "call"),
                refr("a.rs", Some("helper"), "foo", "call"),
            ],
        );
        assert_eq!(idx.get_callees("main", None, 0).len(), 3);
        assert_eq!(idx.get_callees("main", Some("call"), 0).len(), 2);
        assert_eq!(idx.get_callees("main", None, 1).len(), 1);
        assert_eq!(idx.get_callees("unknown", None, 0).len(), 0);
    }

    #[test]
    fn get_file_symbols_filters_by_kind() {
        let idx = Index::build(
            vec![
                ent("a.rs", "Foo", "struct"),
                ent("a.rs", "foo", "function"),
                ent("a.rs", "bar", "function"),
                ent("b.rs", "Baz", "struct"),
            ],
            vec![],
        );
        assert_eq!(idx.get_file_symbols("a.rs", None, 0).len(), 3);
        assert_eq!(idx.get_file_symbols("a.rs", Some("function"), 0).len(), 2);
        assert_eq!(idx.get_file_symbols("a.rs", Some("struct"), 0).len(), 1);
        assert_eq!(idx.get_file_symbols("missing.rs", None, 0).len(), 0);
    }

    #[test]
    fn get_children_filters_by_parent() {
        let mut parent_foo = ent("a.rs", "method1", "method");
        parent_foo.parent = Some("Foo".to_string());
        let mut parent_foo_2 = ent("a.rs", "method2", "method");
        parent_foo_2.parent = Some("Foo".to_string());
        let mut parent_bar = ent("a.rs", "other", "method");
        parent_bar.parent = Some("Bar".to_string());

        let idx = Index::build(
            vec![
                ent("a.rs", "Foo", "struct"),
                parent_foo,
                parent_foo_2,
                parent_bar,
            ],
            vec![],
        );
        assert_eq!(idx.get_children("a.rs", "Foo", None, 0).len(), 2);
        assert_eq!(idx.get_children("a.rs", "Bar", None, 0).len(), 1);
        assert_eq!(idx.get_children("a.rs", "Nobody", None, 0).len(), 0);
        // Top-level entities (parent: None) are not children of anything.
        assert_eq!(idx.get_children("a.rs", "", None, 0).len(), 0);
    }

    #[test]
    fn get_children_respects_kind_filter_and_limit() {
        let mk = |name: &str, kind: &str, parent: &str| {
            let mut e = ent("a.rs", name, kind);
            e.parent = Some(parent.to_string());
            e
        };
        let idx = Index::build(
            vec![
                mk("m1", "method", "C"),
                mk("m2", "method", "C"),
                mk("f", "field", "C"),
                mk("m3", "method", "C"),
            ],
            vec![],
        );
        assert_eq!(idx.get_children("a.rs", "C", None, 0).len(), 4);
        assert_eq!(idx.get_children("a.rs", "C", Some("method"), 0).len(), 3);
        assert_eq!(idx.get_children("a.rs", "C", Some("method"), 2).len(), 2);
    }

    #[test]
    fn get_returns_results_in_insertion_order() {
        // Callers listed in the order they appear in refs.jsonl — sigil writes
        // refs sorted by (file, line) so this matters for stable CLI output.
        let idx = Index::build(
            vec![],
            vec![
                refr("a.rs", Some("m"), "foo", "call"),
                refr("b.rs", Some("m"), "foo", "call"),
                refr("c.rs", Some("m"), "foo", "call"),
            ],
        );
        let callers: Vec<&str> = idx.get_callers("foo", None, 0).iter().map(|r| r.file.as_str()).collect();
        assert_eq!(callers, vec!["a.rs", "b.rs", "c.rs"]);
    }

    #[test]
    fn load_roundtrips_jsonl() {
        let tmp = std::env::temp_dir().join(format!("sigil_query_rt_{}", std::process::id()));
        let sigil = tmp.join(".sigil");
        std::fs::create_dir_all(&sigil).unwrap();

        let entities = vec![ent("a.rs", "foo", "function"), ent("a.rs", "bar", "function")];
        let refs = vec![refr("a.rs", Some("foo"), "bar", "call")];

        // Reuse sigil's own writer so the format on disk matches production.
        crate::writer::write_to_files(&entities, &refs, &tmp, false).unwrap();

        let idx = Index::load(&tmp).expect("load should succeed");
        assert_eq!(idx.len(), (2, 1));
        assert_eq!(idx.entities_by_name("foo").count(), 1);
        assert_eq!(idx.refs_to("bar").count(), 1);
        assert_eq!(idx.refs_from("foo").count(), 1);

        std::fs::remove_dir_all(&tmp).ok();
    }
}
