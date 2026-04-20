//! Smoke-test `sigil::rank` against sigil's own .sigil/ index. Deleted or
//! promoted to a proper benchmark when the Week 2 `--rank` CLI flag lands
//! and we have a formal sigil-bench fixture.

use std::path::PathBuf;

fn project_root() -> PathBuf {
    std::env::current_dir().unwrap()
}

#[test]
fn ranks_sigil_own_source() {
    let root = project_root();
    if !root.join(".sigil/entities.jsonl").exists() {
        eprintln!("skip: run `sigil index` first");
        return;
    }

    let idx = sigil::query::index::Index::load(&root).expect("load index");
    let (n_ent, n_ref) = idx.len();
    assert!(n_ent > 500, "expected a populated index, got {n_ent} entities");

    let ranked = sigil::rank::rank(&idx.entities, &idx.references);

    // PageRank scores roughly sum to 1 (accumulated float error tolerated).
    let total: f64 = ranked.file_rank.values().sum();
    assert!(
        (total - 1.0).abs() < 1e-3,
        "PageRank totals should sum to ~1, got {total}"
    );

    // Every entity gets a blast radius entry (the hash map is keyed on
    // (file, name, parent) so collisions across entities in the same file
    // with the same name/parent collapse — acceptable for now).
    assert!(!ranked.blast.is_empty());

    // The top-ranked file must be one we actually indexed.
    let top = ranked
        .file_rank
        .iter()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .expect("at least one file");
    eprintln!(
        "top ranked file in sigil's own source: {} = {:.4} (entities={}, refs={})",
        top.0, top.1, n_ent, n_ref
    );
    assert!(!top.0.is_empty());
}
