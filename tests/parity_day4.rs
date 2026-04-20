//! Phase 0 day 4 — Index self-consistency against `.sigil/` on disk.
//!
//! Rationale: we started day 4 with "parity vs codeix SearchDb" tests, but
//! running them exposed several legitimate divergences that aren't bugs:
//!
//!   1. codeix treats sub-`Cargo.toml` dirs (e.g. `python/`) as separate
//!      project roots and excludes them from root-project queries. Sigil's
//!      walker unifies them — Index sees more files, intentionally.
//!   2. codeix's markdown parser produces a different section hierarchy
//!      than sigil's custom `markdown_index.rs`.
//!   3. codeix's SearchDb tokenizes ref names for FTS, so
//!      `get_callers("Entity")` misses refs sigil stored as the plain
//!      leaf name. Sigil's storage is directly queryable by exact name.
//!   4. codeix's `get_file_symbols` filters out imports; sigil keeps them
//!      (they are load-bearing for rank in Phase 1).
//!
//! These are features of the unified in-house index, not regressions. The
//! useful contract for day 4 is therefore: `Index::load(root)` returns the
//! same data that's on disk in `.sigil/`, in shape-correct form, with the
//! right rows flowing to the right query methods. That's what this file
//! tests. Day 6 adds a CLI-level smoke comparison via `sigil callers` +
//! friends to catch user-visible regressions end-to-end.
//!
//! This file is deleted on day 6 along with the codeix dependency.

use std::path::{Path, PathBuf};

fn project_root() -> PathBuf {
    std::env::current_dir().unwrap()
}

fn has_index(root: &Path) -> bool {
    root.join(".sigil/entities.jsonl").exists() && root.join(".sigil/refs.jsonl").exists()
}

fn load_sigil(root: &Path) -> sigil::query::index::Index {
    sigil::query::index::Index::load(root).expect("load in-house index")
}

#[test]
fn index_counts_match_disk() {
    let root = project_root();
    if !has_index(&root) {
        return;
    }
    let idx = load_sigil(&root);
    let (ne, nr) = idx.len();

    let ent_lines = std::fs::read_to_string(root.join(".sigil/entities.jsonl"))
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count();
    let ref_lines = std::fs::read_to_string(root.join(".sigil/refs.jsonl"))
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count();

    assert_eq!(ne, ent_lines, "entity count must match entities.jsonl line count");
    assert_eq!(nr, ref_lines, "ref count must match refs.jsonl line count");
}

#[test]
fn get_file_symbols_returns_expected_entities_from_entity_rs() {
    // src/entity.rs is stable and small — known-good fixture for parity.
    let root = project_root();
    if !has_index(&root) {
        return;
    }
    let idx = load_sigil(&root);

    let syms = idx.get_file_symbols("src/entity.rs", None, 0);
    let names: Vec<&str> = syms.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"Entity"), "expected struct Entity in src/entity.rs, got {names:?}");
    assert!(names.contains(&"Reference"), "expected struct Reference in src/entity.rs, got {names:?}");
}

#[test]
fn get_file_symbols_kind_filter_works_on_live_data() {
    let root = project_root();
    if !has_index(&root) {
        return;
    }
    let idx = load_sigil(&root);

    let all = idx.get_file_symbols("src/entity.rs", None, 0);
    let structs = idx.get_file_symbols("src/entity.rs", Some("struct"), 0);
    let imports = idx.get_file_symbols("src/entity.rs", Some("import"), 0);
    assert!(structs.len() >= 2, "expected ≥ 2 structs, got {}", structs.len());
    assert!(imports.len() >= 1, "expected ≥ 1 import in src/entity.rs, got {}", imports.len());
    assert!(
        all.len() >= structs.len() + imports.len(),
        "filtered sums must not exceed unfiltered"
    );
}

#[test]
fn get_callers_finds_real_callers_of_Entity() {
    // sigil's own source references Entity heavily. Validate the lookup
    // returns non-zero + well-shaped results.
    let root = project_root();
    if !has_index(&root) {
        return;
    }
    let idx = load_sigil(&root);

    let callers = idx.get_callers("Entity", None, 0);
    assert!(!callers.is_empty(), "expected some callers of `Entity`");
    for r in &callers {
        assert_eq!(r.name, "Entity");
        assert!(!r.file.is_empty());
        assert!(r.line > 0);
    }
}

#[test]
fn get_callees_for_real_caller_returns_matching_refs() {
    let root = project_root();
    if !has_index(&root) {
        return;
    }
    let idx = load_sigil(&root);

    // Any caller with at least one callee — resilient to source churn.
    let caller = idx
        .references
        .iter()
        .filter_map(|r| r.caller.clone())
        .find(|c| !idx.get_callees(c, None, 1).is_empty())
        .expect("some caller exists in sigil's own source");

    let callees = idx.get_callees(&caller, None, 0);
    assert!(!callees.is_empty());
    for r in &callees {
        assert_eq!(r.caller.as_deref(), Some(caller.as_str()));
    }
}

#[test]
fn get_children_returns_only_matching_parent() {
    let root = project_root();
    if !has_index(&root) {
        return;
    }
    let idx = load_sigil(&root);

    // Find any (file, parent) pair with children, then verify all returned
    // entities have exactly that parent.
    let pair = idx
        .entities
        .iter()
        .find_map(|e| {
            let parent = e.parent.as_ref()?;
            if idx.get_children(&e.file, parent, None, 1).is_empty() {
                None
            } else {
                Some((e.file.clone(), parent.clone()))
            }
        })
        .expect("some (file, parent) with children exists");

    let children = idx.get_children(&pair.0, &pair.1, None, 0);
    assert!(!children.is_empty());
    for c in &children {
        assert_eq!(c.file, pair.0);
        assert_eq!(c.parent.as_deref(), Some(pair.1.as_str()));
    }
}

#[test]
fn limit_never_exceeds_requested() {
    let root = project_root();
    if !has_index(&root) {
        return;
    }
    let idx = load_sigil(&root);

    // Pick the most-referenced name in the repo and cap it.
    let most_refd = idx
        .references
        .iter()
        .fold(std::collections::HashMap::new(), |mut acc, r| {
            *acc.entry(&r.name).or_insert(0usize) += 1;
            acc
        })
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .map(|(k, _)| k.clone())
        .expect("at least one ref");

    assert!(idx.get_callers(&most_refd, None, 5).len() <= 5);
    assert!(idx.get_callers(&most_refd, None, 1).len() <= 1);
    // limit=0 means unlimited — should equal total count.
    let total = idx.get_callers(&most_refd, None, 0).len();
    let unbounded = idx.refs_to(&most_refd).count();
    assert_eq!(total, unbounded);
}
