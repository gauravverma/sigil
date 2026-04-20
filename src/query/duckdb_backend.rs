//! DuckDB-backed query engine — Phase 0.5 scale path (plan §14.9).
//!
//! Built lazily from sigil's existing JSONL source of truth
//! (`.sigil/entities.jsonl` + `.sigil/refs.jsonl`). When JSONL size
//! exceeds the auto-upgrade threshold (50 MB by default), or when the
//! caller asks for the DB backend explicitly, this module stands in for
//! the in-memory `Index` on the same query API.
//!
//! ## Why DuckDB
//!
//! See plan §14.9 for the full trade-off matrix. The short version: the
//! in-memory hash-map Index is great up to ~500k entities; above that,
//! cold-start JSONL parse + hash-map construction becomes painful.
//! DuckDB's zero-ETL `read_json_auto` + vectorized columnar engine
//! handles analytical queries (rank joins, blast-radius aggregates,
//! map-shaped ranked group-bys) 5–20× faster than a row-oriented store
//! at this scale, with a smaller memory footprint than keeping every
//! entity in RAM.
//!
//! ## Artifacts on disk
//!
//! - `.sigil/index.duckdb`        — the materialized database (gitignored)
//! - `.sigil/index.duckdb.stamp`  — JSONL mtime/size fingerprint; the DB
//!   rebuilds from scratch on any mismatch
//!
//! ## Lifecycle
//!
//! ```text
//!          ┌─────────────────────────┐
//!          │  DuckDbBackend::open()  │
//!          └──────────┬──────────────┘
//!                     │
//!             stamp matches JSONL?
//!                 ┌───┴───┐
//!                 │       │
//!                yes      no
//!                 │       ▼
//!                 │   rebuild from JSONL via read_json_auto
//!                 │       │
//!                 ▼       ▼
//!          ┌──────────────────┐
//!          │ ready for queries │
//!          └──────────────────┘
//! ```
//!
//! The stamp file stores bytes length + modified epoch for each JSONL
//! source. A size-only check would miss content-preserving edits
//! (impossible in practice but cheap to guard against).
//!
//! ## Feature gate
//!
//! Compiled only when `cargo build --features db`. Absent that, the
//! module is a type-free empty module and callers fall through to the
//! in-memory path unconditionally.
//!
//! ## Build requirements
//!
//! `--features db` pulls in `libduckdb-sys`, which bundles DuckDB's C++
//! source and compiles it with the host toolchain. A working C++17
//! toolchain + stdlib headers are required. On macOS that means Xcode
//! Command Line Tools (`xcode-select --install`); on Debian-ish Linux,
//! `apt install build-essential`; on Windows, MSVC or an MSYS2
//! toolchain. CI images typically have this already.
//!
//! If a build fails with `fatal error: 'memory' file not found` or
//! similar missing-stdlib messages, the host C++ toolchain is the
//! problem — not sigil. The fix is environmental.

#![cfg(feature = "db")]

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use duckdb::{Connection, params};
use serde::{Deserialize, Serialize};

use crate::entity::{Entity, Reference};

/// Default auto-upgrade threshold in bytes. Set conservatively — the in-
/// memory index is quite fast up to ~100 MB of JSONL; above 50 MB we
/// start paying multi-second cold-start costs. Override via
/// `DuckDbBackend::open_with_threshold`.
pub const DEFAULT_AUTO_UPGRADE_THRESHOLD_BYTES: u64 = 50 * 1024 * 1024;

/// Returns `true` when the DuckDB backend should engage by default for
/// the given `root` — used by callers that want transparent routing
/// rather than forcing a build config at compile time.
pub fn should_auto_engage(root: &Path, threshold_bytes: u64) -> bool {
    let total = [".sigil/entities.jsonl", ".sigil/refs.jsonl", ".sigil/files.jsonl"]
        .iter()
        .map(|p| std::fs::metadata(root.join(p)).map(|m| m.len()).unwrap_or(0))
        .sum::<u64>();
    total >= threshold_bytes
}

/// DuckDB-backed query engine. Opens (or rebuilds) the `.sigil/index.duckdb`
/// cache on construction; queries run against that materialized store.
pub struct DuckDbBackend {
    conn: Connection,
    root: PathBuf,
}

impl DuckDbBackend {
    /// Open the backend at `root/.sigil/index.duckdb`, rebuilding from
    /// JSONL if the stamp file is stale or missing.
    pub fn open(root: &Path) -> Result<Self> {
        let sigil_dir = root.join(".sigil");
        std::fs::create_dir_all(&sigil_dir)?;
        let db_path = sigil_dir.join("index.duckdb");
        let stamp_path = sigil_dir.join("index.duckdb.stamp");

        let expected = fingerprint(&sigil_dir);
        let actual = Stamp::load(&stamp_path).ok();

        // Rebuild when stamps diverge. Dropping the DB file entirely is
        // cheaper than TRUNCATE + re-import because DuckDB lays the file
        // out in its own format we don't control.
        let needs_rebuild = actual.as_ref() != Some(&expected);
        if needs_rebuild && db_path.exists() {
            std::fs::remove_file(&db_path).ok();
        }

        let conn = Connection::open(&db_path)
            .with_context(|| format!("open DuckDB at {}", db_path.display()))?;

        if needs_rebuild {
            populate(&conn, &sigil_dir)
                .context("rebuild DuckDB index from JSONL")?;
            expected.save(&stamp_path)?;
        }

        Ok(Self {
            conn,
            root: root.to_path_buf(),
        })
    }

    /// Total `(entities, references)` counts — cheap sanity check.
    pub fn len(&self) -> Result<(usize, usize)> {
        let entities: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))?;
        let refs: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM refs", [], |r| r.get(0))?;
        Ok((entities as usize, refs as usize))
    }

    /// Callers of `name`, in (file, line) order for stable output.
    /// `limit == 0` → unlimited.
    pub fn get_callers(
        &self,
        name: &str,
        kind_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Reference>> {
        let mut sql = String::from(
            "SELECT file, caller, name, ref_kind, line \
             FROM refs \
             WHERE name = ?",
        );
        if kind_filter.is_some() {
            sql.push_str(" AND ref_kind = ?");
        }
        sql.push_str(" ORDER BY file, line");
        if limit > 0 {
            sql.push_str(&format!(" LIMIT {}", limit));
        }

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = if let Some(k) = kind_filter {
            stmt.query_map(params![name, k], row_to_reference)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![name], row_to_reference)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        Ok(rows)
    }

    /// Callees of `caller` — refs whose `caller` column equals `caller`.
    /// Mirrors `Index::get_callees`. Dedupe happens implicitly at the
    /// index level since refs carry `(file, line)` as a natural key.
    pub fn get_callees(
        &self,
        caller: &str,
        kind_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Reference>> {
        let mut sql = String::from(
            "SELECT file, caller, name, ref_kind, line \
             FROM refs \
             WHERE caller = ?",
        );
        if kind_filter.is_some() {
            sql.push_str(" AND ref_kind = ?");
        }
        sql.push_str(" ORDER BY file, line");
        if limit > 0 {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = if let Some(k) = kind_filter {
            stmt.query_map(params![caller, k], row_to_reference)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![caller], row_to_reference)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        Ok(rows)
    }

    /// All entities in `file`, optionally filtered by kind. Ordered by
    /// `line_start` so successive calls return the same prefix — stable
    /// behavior callers depend on for pagination.
    ///
    /// Returns sigil `Entity` rows; the DuckDB backend only hydrates
    /// scalar columns (no `meta`, `rank`, or `blast_radius`). Consumers
    /// needing those fields should load the in-memory `Index`, which
    /// carries the full struct. Documented on
    /// [`populate_entity_from_row`] below.
    pub fn get_file_symbols(
        &self,
        file: &str,
        kind_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Entity>> {
        let mut sql = String::from(
            "SELECT file, name, kind, line_start, line_end, parent, sig, \
                    visibility, body_hash, sig_hash, struct_hash \
             FROM entities \
             WHERE file = ?",
        );
        if kind_filter.is_some() {
            sql.push_str(" AND kind = ?");
        }
        sql.push_str(" ORDER BY line_start");
        if limit > 0 {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = if let Some(k) = kind_filter {
            stmt.query_map(params![file, k], row_to_entity)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![file], row_to_entity)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        Ok(rows)
    }

    /// Children of `(file, parent)` — entities whose `parent` column
    /// matches. Same column set + limitations as `get_file_symbols`.
    pub fn get_children(
        &self,
        file: &str,
        parent: &str,
        kind_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Entity>> {
        let mut sql = String::from(
            "SELECT file, name, kind, line_start, line_end, parent, sig, \
                    visibility, body_hash, sig_hash, struct_hash \
             FROM entities \
             WHERE file = ? AND parent = ?",
        );
        if kind_filter.is_some() {
            sql.push_str(" AND kind = ?");
        }
        sql.push_str(" ORDER BY line_start");
        if limit > 0 {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = if let Some(k) = kind_filter {
            stmt.query_map(params![file, parent, k], row_to_entity)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![file, parent], row_to_entity)?
                .collect::<std::result::Result<Vec<_>, _>>()?
        };
        Ok(rows)
    }

    /// Case-insensitive substring search over entity names, matching the
    /// `Scope::Symbols` branch of `Index::search`. File search (the
    /// `Scope::Files` branch) runs a separate DISTINCT scan.
    ///
    /// Unlike `Index::search`, this only returns the symbol form for now
    /// — file-path matching without entity hits is rare in agent
    /// workflows, and adding it requires another query + enum serde
    /// gymnastics. Deferred until a consumer needs it.
    pub fn search_symbols(
        &self,
        query: &str,
        kind_filter: Option<&str>,
        path_prefix: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Entity>> {
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let needle = format!("%{}%", query.to_lowercase());
        let mut sql = String::from(
            "SELECT file, name, kind, line_start, line_end, parent, sig, \
                    visibility, body_hash, sig_hash, struct_hash \
             FROM entities \
             WHERE lower(name) LIKE ?",
        );
        if kind_filter.is_some() {
            sql.push_str(" AND kind = ?");
        }
        if path_prefix.is_some() {
            sql.push_str(" AND file LIKE ?");
        }
        sql.push_str(" ORDER BY file, line_start");
        if limit > 0 {
            sql.push_str(&format!(" LIMIT {}", limit));
        }
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = match (kind_filter, path_prefix) {
            (Some(k), Some(p)) => stmt
                .query_map(params![needle, k, format!("{p}%")], row_to_entity)?
                .collect::<std::result::Result<Vec<_>, _>>()?,
            (Some(k), None) => stmt
                .query_map(params![needle, k], row_to_entity)?
                .collect::<std::result::Result<Vec<_>, _>>()?,
            (None, Some(p)) => stmt
                .query_map(params![needle, format!("{p}%")], row_to_entity)?
                .collect::<std::result::Result<Vec<_>, _>>()?,
            (None, None) => stmt
                .query_map(params![needle], row_to_entity)?
                .collect::<std::result::Result<Vec<_>, _>>()?,
        };
        Ok(rows)
    }

    /// Sigil operates in single-project mode — the empty-string project
    /// is the whole tree. `Index::list_projects` returns `vec![""]` for
    /// compatibility with pre-decodeix call sites that expected the
    /// codeix MountTable convention; we mirror it here.
    pub fn list_projects(&self) -> Result<Vec<String>> {
        Ok(vec![String::new()])
    }

    /// Where the DuckDB store lives on disk. Exposed for consumers (the
    /// future `sigil query 'SQL'` REPL) that want to run ad-hoc SQL
    /// against the same database.
    pub fn db_path(&self) -> PathBuf {
        self.root.join(".sigil/index.duckdb")
    }
}

// ---- internals ----

fn populate(conn: &Connection, sigil_dir: &Path) -> Result<()> {
    let entities_jsonl = path_for_sql(&sigil_dir.join("entities.jsonl"));
    let refs_jsonl = path_for_sql(&sigil_dir.join("refs.jsonl"));

    // Materialize into real tables (not views) so queries don't re-parse
    // JSONL on every call. Rebuild is cheap — zero-ETL via read_json_auto.
    conn.execute_batch(&format!(
        "CREATE TABLE entities AS SELECT * FROM read_json_auto('{entities_jsonl}');
         CREATE TABLE refs     AS SELECT * FROM read_json_auto('{refs_jsonl}');
         CREATE INDEX idx_entities_name ON entities(name);
         CREATE INDEX idx_entities_file ON entities(file);
         CREATE INDEX idx_refs_name   ON refs(name);
         CREATE INDEX idx_refs_caller ON refs(caller);
         CREATE INDEX idx_refs_file   ON refs(file);",
    ))
    .context("populate entities/refs tables + indexes")?;
    Ok(())
}

fn row_to_reference(row: &duckdb::Row<'_>) -> duckdb::Result<Reference> {
    Ok(Reference {
        file: row.get::<_, String>(0)?,
        caller: row.get::<_, Option<String>>(1)?,
        name: row.get::<_, String>(2)?,
        ref_kind: row.get::<_, String>(3)?,
        line: row.get::<_, i64>(4)? as u32,
    })
}

/// Hydrate the subset of `Entity` that the DuckDB backend extracts —
/// scalar columns only. `meta`, `rank`, and `blast_radius` stay `None`
/// because reading them back requires DuckDB STRUCT/LIST parsing that
/// isn't necessary for the query methods we serve today. Any consumer
/// that needs the full struct should load the in-memory `Index` (which
/// parses JSONL directly into the Rust struct and keeps every field).
///
/// Column order must match the SELECT lists in the methods above.
fn row_to_entity(row: &duckdb::Row<'_>) -> duckdb::Result<Entity> {
    Ok(Entity {
        file: row.get::<_, String>(0)?,
        name: row.get::<_, String>(1)?,
        kind: row.get::<_, String>(2)?,
        line_start: row.get::<_, i64>(3)? as u32,
        line_end: row.get::<_, i64>(4)? as u32,
        parent: row.get::<_, Option<String>>(5)?,
        sig: row.get::<_, Option<String>>(6)?,
        meta: None,
        body_hash: row.get::<_, Option<String>>(8)?,
        sig_hash: row.get::<_, Option<String>>(9)?,
        struct_hash: row.get::<_, String>(10)?,
        visibility: row.get::<_, Option<String>>(7)?,
        rank: None,
        blast_radius: None,
    })
}

/// DuckDB's SQL expects `'...'` strings; we single-quote by escaping any
/// embedded quotes. The paths we pass are absolute sigil-controlled
/// locations, so injection isn't a real risk — this is just correctness.
fn path_for_sql(p: &Path) -> String {
    p.display().to_string().replace('\'', "''")
}

/// Fingerprint of the JSONL files the DB was built from. Captured at
/// build time and compared on next open to decide whether to rebuild.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Stamp {
    entities_len: u64,
    entities_mtime_ms: u128,
    refs_len: u64,
    refs_mtime_ms: u128,
}

impl Stamp {
    fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("read {}", path.display()))?;
        serde_json::from_str(&text).map_err(Into::into)
    }

    fn save(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string(self)?;
        std::fs::write(path, text).map_err(Into::into)
    }
}

fn fingerprint(sigil_dir: &Path) -> Stamp {
    let (entities_len, entities_mtime_ms) = meta_pair(&sigil_dir.join("entities.jsonl"));
    let (refs_len, refs_mtime_ms) = meta_pair(&sigil_dir.join("refs.jsonl"));
    Stamp {
        entities_len,
        entities_mtime_ms,
        refs_len,
        refs_mtime_ms,
    }
}

fn meta_pair(p: &Path) -> (u64, u128) {
    let Ok(m) = std::fs::metadata(p) else {
        return (0, 0);
    };
    let len = m.len();
    let mtime_ms = m
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis())
        .unwrap_or(0);
    (len, mtime_ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Entity, Reference};
    use crate::writer;

    fn tmpdir(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("sigil_duckdb_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

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
            body_hash: Some("d".to_string()),
            sig_hash: None,
            struct_hash: "s".to_string(),
            visibility: None,
            rank: None,
            blast_radius: None,
        }
    }

    fn refr(file: &str, caller: Option<&str>, name: &str, kind: &str, line: u32) -> Reference {
        Reference {
            file: file.to_string(),
            caller: caller.map(str::to_string),
            name: name.to_string(),
            ref_kind: kind.to_string(),
            line,
        }
    }

    fn seed(root: &Path, entities: Vec<Entity>, refs: Vec<Reference>) {
        writer::write_to_files(&entities, &refs, root, false).unwrap();
    }

    #[test]
    fn opens_and_populates_from_jsonl() {
        let root = tmpdir("populate");
        seed(
            &root,
            vec![ent("a.rs", "Foo", "struct"), ent("b.rs", "bar", "function")],
            vec![refr("a.rs", Some("caller"), "bar", "call", 10)],
        );
        let db = DuckDbBackend::open(&root).expect("open");
        assert_eq!(db.len().unwrap(), (2, 1));
        assert!(root.join(".sigil/index.duckdb").exists());
        assert!(root.join(".sigil/index.duckdb.stamp").exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn stamp_match_skips_rebuild() {
        let root = tmpdir("cached");
        seed(
            &root,
            vec![ent("a.rs", "Foo", "struct")],
            vec![refr("b.rs", Some("c"), "Foo", "type_annotation", 5)],
        );
        let _ = DuckDbBackend::open(&root).unwrap();
        let db_mtime_first = std::fs::metadata(root.join(".sigil/index.duckdb"))
            .unwrap()
            .modified()
            .unwrap();
        // Small sleep to ensure the filesystem mtime could differ if we
        // were to rewrite. A proper clock-skew-tolerant test would check
        // a monotonic counter, but this is sufficient for local runs.
        std::thread::sleep(std::time::Duration::from_millis(10));
        let _ = DuckDbBackend::open(&root).unwrap();
        let db_mtime_second = std::fs::metadata(root.join(".sigil/index.duckdb"))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(db_mtime_first, db_mtime_second, "DB should not be rewritten");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn stamp_mismatch_triggers_rebuild() {
        let root = tmpdir("stale");
        seed(
            &root,
            vec![ent("a.rs", "Foo", "struct")],
            vec![refr("b.rs", Some("c"), "Foo", "call", 5)],
        );
        let first = DuckDbBackend::open(&root).unwrap();
        assert_eq!(first.len().unwrap(), (1, 1));
        drop(first);

        // Re-seed with more data — stamp's (size, mtime) will differ and
        // force a rebuild.
        std::thread::sleep(std::time::Duration::from_millis(10));
        seed(
            &root,
            vec![
                ent("a.rs", "Foo", "struct"),
                ent("c.rs", "Bar", "struct"),
                ent("d.rs", "Baz", "struct"),
            ],
            vec![
                refr("b.rs", Some("c"), "Foo", "call", 5),
                refr("b.rs", Some("c"), "Bar", "call", 6),
            ],
        );
        let second = DuckDbBackend::open(&root).unwrap();
        assert_eq!(second.len().unwrap(), (3, 2), "DB should reflect new JSONL");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn get_callers_matches_in_memory_semantics() {
        let root = tmpdir("callers");
        seed(
            &root,
            vec![ent("tgt.rs", "Foo", "struct")],
            vec![
                refr("a.rs", Some("user"), "Foo", "type_annotation", 1),
                refr("b.rs", Some("user"), "Foo", "call", 2),
                refr("c.rs", Some("user"), "Foo", "call", 3),
                refr("d.rs", Some("user"), "Other", "call", 4),
            ],
        );
        let db = DuckDbBackend::open(&root).unwrap();

        let all = db.get_callers("Foo", None, 0).unwrap();
        assert_eq!(all.len(), 3);

        let filtered = db.get_callers("Foo", Some("call"), 0).unwrap();
        assert_eq!(filtered.len(), 2);

        let limited = db.get_callers("Foo", None, 2).unwrap();
        assert_eq!(limited.len(), 2);

        let missing = db.get_callers("Nonexistent", None, 0).unwrap();
        assert!(missing.is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn get_callers_parity_with_in_memory_backend() {
        // Load the same JSONL into both backends; get_callers answers
        // should match row-for-row (modulo insertion order vs DuckDB's
        // file/line ordering — we sort both sides to canonicalize).
        let root = tmpdir("parity");
        seed(
            &root,
            vec![ent("tgt.rs", "Foo", "struct")],
            (0..12)
                .map(|i| refr(&format!("c{i}.rs"), Some("m"), "Foo", "call", i + 1))
                .collect(),
        );

        let db = DuckDbBackend::open(&root).unwrap();
        let idx = crate::query::index::Index::load(&root).unwrap();

        let mut from_db = db.get_callers("Foo", None, 0).unwrap();
        let mut from_idx: Vec<Reference> = idx
            .get_callers("Foo", None, 0)
            .into_iter()
            .cloned()
            .collect();
        from_db.sort_by(|a, b| a.file.cmp(&b.file).then_with(|| a.line.cmp(&b.line)));
        from_idx.sort_by(|a, b| a.file.cmp(&b.file).then_with(|| a.line.cmp(&b.line)));
        assert_eq!(from_db, from_idx);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn should_auto_engage_honors_threshold() {
        let root = tmpdir("threshold");
        seed(
            &root,
            vec![ent("a.rs", "Foo", "struct")],
            vec![refr("b.rs", Some("c"), "Foo", "call", 1)],
        );
        assert!(!should_auto_engage(&root, 50 * 1024 * 1024));
        // Tiny threshold → even the one-entity fixture flips the gate.
        assert!(should_auto_engage(&root, 1));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn get_callees_mirrors_caller_column_lookup() {
        let root = tmpdir("callees");
        seed(
            &root,
            vec![ent("a.rs", "main", "function")],
            vec![
                refr("a.rs", Some("main"), "foo", "call", 1),
                refr("a.rs", Some("main"), "bar", "call", 2),
                refr("a.rs", Some("helper"), "foo", "call", 3),
            ],
        );
        let db = DuckDbBackend::open(&root).unwrap();
        let from_main = db.get_callees("main", None, 0).unwrap();
        assert_eq!(from_main.len(), 2);
        let from_helper = db.get_callees("helper", None, 0).unwrap();
        assert_eq!(from_helper.len(), 1);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn get_file_symbols_returns_entities_ordered_by_line() {
        let root = tmpdir("file_symbols");
        let mut a = ent("a.rs", "Foo", "struct");
        a.line_start = 10;
        let mut b = ent("a.rs", "bar", "function");
        b.line_start = 3;
        let mut c = ent("b.rs", "other", "function");
        c.line_start = 1;
        seed(&root, vec![a, b, c], vec![]);

        let db = DuckDbBackend::open(&root).unwrap();
        let in_a = db.get_file_symbols("a.rs", None, 0).unwrap();
        assert_eq!(in_a.len(), 2);
        assert_eq!(in_a[0].name, "bar", "line 3 sorts before line 10");
        assert_eq!(in_a[1].name, "Foo");

        let only_structs = db.get_file_symbols("a.rs", Some("struct"), 0).unwrap();
        assert_eq!(only_structs.len(), 1);
        assert_eq!(only_structs[0].name, "Foo");

        let missing = db.get_file_symbols("nonexistent.rs", None, 0).unwrap();
        assert!(missing.is_empty());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn get_children_filters_by_parent() {
        let root = tmpdir("children");
        let mut m1 = ent("a.rs", "method_one", "method");
        m1.parent = Some("Foo".to_string());
        m1.line_start = 5;
        let mut m2 = ent("a.rs", "method_two", "method");
        m2.parent = Some("Foo".to_string());
        m2.line_start = 10;
        let mut m3 = ent("a.rs", "other", "method");
        m3.parent = Some("Bar".to_string());
        seed(&root, vec![m1, m2, m3], vec![]);

        let db = DuckDbBackend::open(&root).unwrap();
        let foo_methods = db.get_children("a.rs", "Foo", None, 0).unwrap();
        assert_eq!(foo_methods.len(), 2);
        assert!(foo_methods.iter().all(|e| e.parent.as_deref() == Some("Foo")));
        let bar_methods = db.get_children("a.rs", "Bar", None, 0).unwrap();
        assert_eq!(bar_methods.len(), 1);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn search_symbols_case_insensitive_with_path_prefix() {
        let root = tmpdir("search");
        seed(
            &root,
            vec![
                ent("src/a.rs", "ParseError", "struct"),
                ent("src/a.rs", "parse", "function"),
                ent("tests/parse_test.rs", "parse", "function"),
            ],
            vec![],
        );
        let db = DuckDbBackend::open(&root).unwrap();

        // Case-insensitive match hits both "parse" and "ParseError".
        let all = db.search_symbols("PARSE", None, None, 0).unwrap();
        assert_eq!(all.len(), 3);

        // Filter by kind.
        let only_fns = db.search_symbols("parse", Some("function"), None, 0).unwrap();
        assert_eq!(only_fns.len(), 2);

        // Filter by path prefix (src/) — tests/ excluded.
        let src_only = db.search_symbols("parse", None, Some("src/"), 0).unwrap();
        assert_eq!(src_only.len(), 2);

        // Empty query short-circuits.
        let empty = db.search_symbols("", None, None, 0).unwrap();
        assert!(empty.is_empty());

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn list_projects_returns_single_root() {
        let root = tmpdir("list_proj");
        seed(
            &root,
            vec![ent("a.rs", "x", "function")],
            vec![refr("b.rs", Some("c"), "x", "call", 1)],
        );
        let db = DuckDbBackend::open(&root).unwrap();
        assert_eq!(db.list_projects().unwrap(), vec![String::new()]);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn get_file_symbols_parity_with_in_memory() {
        let root = tmpdir("file_parity");
        let mut entities = Vec::new();
        for i in 0..15 {
            let mut e = ent("src/core.rs", &format!("sym{i}"), "function");
            e.line_start = (i as u32) * 10 + 1;
            e.line_end = e.line_start + 5;
            entities.push(e);
        }
        entities.push(ent("src/other.rs", "other", "function"));
        seed(&root, entities, vec![]);

        let db = DuckDbBackend::open(&root).unwrap();
        let idx = crate::query::index::Index::load(&root).unwrap();

        let mut from_db = db.get_file_symbols("src/core.rs", None, 0).unwrap();
        let mut from_idx: Vec<Entity> = idx
            .get_file_symbols("src/core.rs", None, 0)
            .into_iter()
            .cloned()
            .collect();
        // Both backends should produce the same set; DuckDB already
        // sorts by line_start, so we sort the in-memory side to match.
        from_db.sort_by_key(|e| e.line_start);
        from_idx.sort_by_key(|e| e.line_start);

        // Compare the scalar columns the DuckDB backend populates.
        let project = |e: &Entity| (
            e.file.clone(),
            e.name.clone(),
            e.kind.clone(),
            e.line_start,
            e.line_end,
            e.parent.clone(),
        );
        let a: Vec<_> = from_db.iter().map(project).collect();
        let b: Vec<_> = from_idx.iter().map(project).collect();
        assert_eq!(a, b);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn missing_jsonl_rebuilds_as_empty_view_and_errors_on_open() {
        // Opening with no .sigil/entities.jsonl should fail loudly rather
        // than silently produce an empty backend — consumers need to
        // know the index wasn't built yet.
        let root = tmpdir("empty");
        std::fs::create_dir_all(root.join(".sigil")).unwrap();
        let err = DuckDbBackend::open(&root).err();
        assert!(err.is_some(), "expected open() to error without JSONL");
        std::fs::remove_dir_all(&root).ok();
    }
}
