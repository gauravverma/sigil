//! `sigil where <symbol>` — single-shot locator.
//!
//! Consolidates the common SWE-bench-Lite phase-1 flow of "find the
//! definition(s) of this name" into one command with a bundled answer.
//! The same question today takes an agent: `sigil search foo` (list
//! many hits) → `read_file` the indicated line range (find the class) →
//! optionally another grep/search to verify siblings. `sigil where` does
//! all of that at index-time and returns one record per definition.
//!
//! Matching rule: the last `::` / `.`-separated segment of an entity
//! name equals the queried symbol. This lets `sigil where get_default`
//! match `Parameter.get_default` and `Option.get_default` but NOT
//! `CliRunner.get_default_prog_name`. A substring search is too noisy
//! for a "locator"-shaped command.

use serde::Serialize;

use crate::entity::Entity;
use crate::query::index::Index;

/// Kinds that count as "a place where something is defined." Variables,
/// imports, and constants are excluded — they're not what a `sigil where`
/// consumer is usually looking for.
const DEFINITION_KINDS: &[&str] = &[
    "class",
    "struct",
    "enum",
    "trait",
    "interface",
    "function",
    "fn",
    "method",
    "type_alias",
    "module",
];

/// One definition surfaced by `sigil where`. Compact shape that maps
/// directly to the JSON row emitted on `--json`.
///
/// The `name` field is the entity's **tail segment only** (e.g.
/// `get_default`, not `Parameter.get_default`). The qualifying class
/// lives in `parent`; duplicating it in `name` just wastes bytes on
/// every row. Consumers that want the full qualified name can join
/// `parent` + `.` + `name` when parent is non-null.
#[derive(Debug, Clone, Serialize)]
pub struct Definition {
    pub name: String,
    pub file: String,
    pub line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_end: Option<u32>,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sig: Option<String>,
    /// Number of same-(name, parent, file, kind) entities — captures
    /// Python `@overload` stubs where the signature repeats.
    #[serde(skip_serializing_if = "is_one")]
    pub overloads: u32,
    /// Whether the defining file lives under a typical test path.
    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    pub in_test: bool,
}

fn is_one(n: &u32) -> bool {
    *n == 1
}

/// Full locator report — the symbol asked for + each matching definition.
#[derive(Debug, Clone, Serialize)]
pub struct WhereReport {
    pub symbol: String,
    pub definitions: Vec<Definition>,
}

/// Last `::`- or `.`-separated segment of an entity name.
fn tail_segment(name: &str) -> &str {
    name.rsplit(|c| c == ':' || c == '.').next().unwrap_or(name)
}

/// Heuristic for "this file is test code" — mirrors `is_test_path` from
/// `entity.rs` but at the path level. Used for the `in_test` flag and
/// optional test-file filtering.
fn is_test_file(file: &str) -> bool {
    crate::entity::is_test_path(file)
}

pub fn find_definitions(idx: &Index, symbol: &str, include_tests: bool) -> WhereReport {
    // Collect every entity whose tail segment matches and whose kind is
    // a definition-kind. `Index::entities` is sorted by (file, line_start)
    // already, so iteration preserves a stable on-disk order.
    let mut matches: Vec<&Entity> = idx
        .entities
        .iter()
        .filter(|e| tail_segment(&e.name) == symbol)
        .filter(|e| DEFINITION_KINDS.contains(&e.kind.as_str()))
        .collect();

    if !include_tests {
        matches.retain(|e| !is_test_file(&e.file));
    }

    // Dedupe by (file, parent, kind) — overloads collapse into one
    // `Definition` with `overloads: N`. Keep the earliest line (first
    // seen) as the canonical line for the record.
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<
        (String, Option<String>, String),
        (u32, u32, Option<String>, u32, bool, String),
    > = BTreeMap::new();
    let mut order: Vec<(String, Option<String>, String)> = Vec::new();

    for e in matches {
        let key = (e.file.clone(), e.parent.clone(), e.kind.clone());
        groups
            .entry(key.clone())
            .and_modify(|(_, _, _, n, _, _)| *n += 1)
            .or_insert_with(|| {
                order.push(key);
                (
                    e.line_start,
                    e.line_end,
                    e.sig.clone(),
                    1,
                    is_test_file(&e.file),
                    e.name.clone(),
                )
            });
    }

    let mut definitions = Vec::with_capacity(order.len());
    for key in order {
        let (line_start, line_end, sig, overloads, in_test, full_name) = groups[&key].clone();
        let (file, parent, kind) = key;
        let line_end = if line_end != line_start {
            Some(line_end)
        } else {
            None
        };
        // Tail-only name — parent already carries the qualifying class.
        let name = tail_segment(&full_name).to_string();
        definitions.push(Definition {
            name,
            file,
            line: line_start,
            line_end,
            kind,
            parent,
            sig,
            overloads,
            in_test,
        });
    }

    WhereReport {
        symbol: symbol.to_string(),
        definitions,
    }
}

pub fn render_markdown(report: &WhereReport) -> String {
    if report.definitions.is_empty() {
        return format!("No definition of `{}` found.\n", report.symbol);
    }
    let mut out = format!("{}\n", report.symbol);
    for d in &report.definitions {
        let class = d.parent.as_deref().unwrap_or("<top-level>");
        let range = match d.line_end {
            Some(e) => format!("{}:{}-{}", d.file, d.line, e),
            None => format!("{}:{}", d.file, d.line),
        };
        let overload_note = if d.overloads > 1 {
            format!(", {} overloads", d.overloads)
        } else {
            String::new()
        };
        let test_note = if d.in_test { ", test" } else { "" };
        out.push_str(&format!(
            "  {class}.{name}  {range}  ({kind}{overload_note}{test_note})\n",
            name = d.name,
            kind = d.kind,
        ));
        if let Some(sig) = d.sig.as_deref() {
            out.push_str(&format!("    {sig}\n"));
        }
    }
    out
}

pub fn render_json(report: &WhereReport, pretty: bool) -> String {
    if pretty {
        serde_json::to_string_pretty(report).expect("WhereReport serializes infallibly")
    } else {
        serde_json::to_string(report).expect("WhereReport serializes infallibly")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{BlastRadius, Entity, Reference};
    use crate::query::index::Index;

    fn ent(file: &str, name: &str, kind: &str, parent: Option<&str>, line: u32) -> Entity {
        Entity {
            file: file.into(),
            name: name.into(),
            kind: kind.into(),
            line_start: line,
            line_end: line + 2,
            parent: parent.map(String::from),
            sig: Some(format!("sig of {name}")),
            meta: None,
            body_hash: None,
            sig_hash: None,
            struct_hash: "h".into(),
            visibility: Some("public".into()),
            rank: None,
            blast_radius: Some(BlastRadius::default()),
        }
    }

    #[test]
    fn where_matches_tail_segment_across_parents() {
        let idx = Index::build(
            vec![
                ent("a.py", "Parameter.get_default", "method", Some("Parameter"), 10),
                ent("a.py", "Option.get_default", "method", Some("Option"), 50),
                ent("a.py", "CliRunner.get_default_prog_name", "method", Some("CliRunner"), 100),
            ],
            vec![],
        );
        let report = find_definitions(&idx, "get_default", false);
        assert_eq!(report.definitions.len(), 2, "only exact tail match, not prefix");
        assert_eq!(report.definitions[0].parent.as_deref(), Some("Parameter"));
        assert_eq!(report.definitions[0].name, "get_default", "name is tail-only, not qualified");
        assert_eq!(report.definitions[1].parent.as_deref(), Some("Option"));
        assert_eq!(report.definitions[1].name, "get_default");
    }

    #[test]
    fn where_collapses_python_overloads() {
        let idx = Index::build(
            vec![
                ent("a.py", "P.get_default", "method", Some("P"), 10),
                ent("a.py", "P.get_default", "method", Some("P"), 15),
                ent("a.py", "P.get_default", "method", Some("P"), 20),
            ],
            vec![],
        );
        let report = find_definitions(&idx, "get_default", false);
        assert_eq!(report.definitions.len(), 1);
        assert_eq!(report.definitions[0].overloads, 3);
        assert_eq!(report.definitions[0].line, 10, "earliest line wins");
    }

    #[test]
    fn where_filters_tests_by_default() {
        let idx = Index::build(
            vec![
                ent("src/core.py", "P.get_default", "method", Some("P"), 10),
                ent("tests/test_core.py", "FakeP.get_default", "method", Some("FakeP"), 30),
            ],
            vec![],
        );
        let default = find_definitions(&idx, "get_default", false);
        assert_eq!(default.definitions.len(), 1, "test file filtered out by default");
        let with_tests = find_definitions(&idx, "get_default", true);
        assert_eq!(with_tests.definitions.len(), 2);
        assert!(with_tests.definitions[1].in_test);
    }

    #[test]
    fn where_skips_variables_and_imports() {
        let idx = Index::build(
            vec![
                ent("a.py", "foo", "variable", None, 10),
                ent("a.py", "foo", "import", None, 20),
                ent("a.py", "foo", "function", None, 30),
            ],
            vec![],
        );
        let report = find_definitions(&idx, "foo", false);
        assert_eq!(report.definitions.len(), 1);
        assert_eq!(report.definitions[0].kind, "function");
    }

    // Silence "unused" warning on Reference — kept here for future
    // call-tracing on `sigil where` (e.g. an "also calls this" block).
    #[allow(dead_code)]
    fn _ref_shape() -> Reference {
        Reference {
            file: "a.py".into(),
            caller: Some("main".into()),
            name: "foo".into(),
            ref_kind: "call".into(),
            line: 1,
        }
    }
}
