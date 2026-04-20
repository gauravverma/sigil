//! Phase 0 day 3 live-index smoke — deletes itself by day 6.

use std::path::PathBuf;

#[test]
fn live_index_roundtrip_over_self() {
    // Requires `sigil index` to have been run; our pre-commit hook ensures that.
    let root: PathBuf = std::env::current_dir().unwrap();
    if !root.join(".sigil/entities.jsonl").exists() {
        eprintln!("skipping: no .sigil/entities.jsonl present");
        return;
    }
    let idx = sigil::query::index::Index::load(&root).expect("load");
    let (ne, nr) = idx.len();
    assert!(ne > 500, "expected >500 entities in sigil's own index, got {}", ne);
    assert!(nr > 500, "expected >500 references in sigil's own index, got {}", nr);

    // Index is round-trippable via its maps.
    let any_entity = idx.entities.first().expect("at least one entity");
    let found: Vec<_> = idx.entities_by_name(&any_entity.name).collect();
    assert!(found.iter().any(|e| e.file == any_entity.file));
}
