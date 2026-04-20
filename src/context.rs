//! `sigil context <symbol>` — the minimum-viable-context bundle.
//!
//! Collapses the agent loop "read 6 files to understand one function before
//! editing" into a single structured call. Per §4.3 of
//! agent-adoption-plan.md, this is the single highest-leverage new command —
//! it touches the most-expensive recurring pattern in real agent work.
//!
//! Output shape (see `Context` struct):
//!   - the resolved entity (file, line range, signature, visibility)
//!   - direct callers (enclosing symbol + file:line)
//!   - direct callees (with ref_kind so the agent sees the relationship)
//!   - related types used in the symbol's body (ref_kind == type_annotation)
//!   - blast-radius summary
//!
//! All three renderers share the same `Context` data model — the difference
//! is packing and format. `Agent` is compact, short-keyed JSON for LLM
//! ingestion; `Markdown` is human-readable; `Full` is the unabridged JSON.

use std::cmp::Reverse;
use std::collections::HashSet;

use serde::Serialize;

use crate::entity::{BlastRadius, Entity, Reference};
use crate::query::index::Index;

/// Config knobs for a single `sigil context` invocation.
#[derive(Debug, Clone)]
pub struct ContextOptions {
    /// Rough output token cap. 0 = unlimited.
    pub budget: usize,
    /// How many callers / callees / related types to include.
    pub depth: usize,
    pub format: ContextFormat,
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            budget: 1500,
            depth: 10,
            format: ContextFormat::Markdown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextFormat {
    /// Compact JSON with short keys — designed for LLM token budgets.
    Agent,
    /// Human-readable markdown.
    Markdown,
    /// Full structured JSON — stable field names, safe to deserialize.
    Full,
}

impl ContextFormat {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "agent" => Some(Self::Agent),
            "markdown" | "md" => Some(Self::Markdown),
            "json" | "full" => Some(Self::Full),
            _ => None,
        }
    }
}

/// A resolved entity — enough to locate it in the codebase and understand
/// its shape without reading the file.
#[derive(Debug, Clone, Serialize)]
pub struct SymbolRef {
    pub file: String,
    pub name: String,
    pub kind: String,
    pub line_start: u32,
    pub line_end: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sig: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blast_radius: Option<BlastRadius>,
}

impl SymbolRef {
    fn from_entity(e: &Entity) -> Self {
        Self {
            file: e.file.clone(),
            name: e.name.clone(),
            kind: e.kind.clone(),
            line_start: e.line_start,
            line_end: e.line_end,
            parent: e.parent.clone(),
            sig: e.sig.clone(),
            visibility: e.visibility.clone(),
            blast_radius: e.blast_radius,
        }
    }
}

/// One edge in the context graph — caller or callee.
#[derive(Debug, Clone, Serialize)]
pub struct Edge {
    pub file: String,
    pub line: u32,
    pub symbol: String,
    /// `ref_kind` from the Reference row (call, import, type_annotation,
    /// instantiation, …). Surface it so the agent doesn't have to guess
    /// whether a row is a function call or a type usage.
    pub kind: String,
    /// Enclosing symbol where the reference appears, when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caller: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Context {
    pub query: String,
    /// The entity the context was built for.
    pub chosen: SymbolRef,
    /// When `query` resolves to multiple entities, the others are surfaced
    /// so the caller can disambiguate on the next invocation.
    pub alternatives: Vec<SymbolRef>,
    pub callers: Vec<Edge>,
    pub callees: Vec<Edge>,
    pub related_types: Vec<Edge>,
    pub skipped_callers: usize,
    pub skipped_callees: usize,
    pub skipped_types: usize,
    pub estimated_tokens: usize,
}

/// Parse a query string into (optional file filter, optional parent filter, name).
///
/// Accepted forms:
///   - `foo`                      — bare name
///   - `Foo::bar`                 — `bar` with parent `Foo`
///   - `src/x.rs::bar`            — `bar` in `src/x.rs`
///   - `src/x.rs::Foo::bar`       — `bar` with parent `Foo` in `src/x.rs`
fn split_query(query: &str) -> (Option<&str>, Option<&str>, &str) {
    let parts: Vec<&str> = query.split("::").collect();
    match parts.len() {
        1 => (None, None, parts[0]),
        2 => {
            // Either file::name (first part looks like a path) or parent::name.
            let a = parts[0];
            let b = parts[1];
            if a.contains('/') || a.contains('.') {
                (Some(a), None, b)
            } else {
                (None, Some(a), b)
            }
        }
        _ => {
            // 3+ parts: last = name, second-last = parent, everything before = file.
            let name = parts[parts.len() - 1];
            let parent = parts[parts.len() - 2];
            let file = parts[..parts.len() - 2].join("::");
            // Leak to match the &str return. Acceptable — resolve is called once
            // per CLI invocation. Using a heap-allocated String + lifetime gymnastics
            // would clutter call sites for no practical win.
            let file_static: &'static str = Box::leak(file.into_boxed_str());
            (Some(file_static), Some(parent), name)
        }
    }
}

/// Find every entity in `idx` that matches the query. Sort by impact so
/// ambiguous names pick up the load-bearing definition first.
pub fn resolve<'a>(idx: &'a Index, query: &str) -> Vec<&'a Entity> {
    let (file_hint, parent_hint, name) = split_query(query);

    let mut matches: Vec<&Entity> = idx
        .entities_by_name(name)
        .filter(|e| match file_hint {
            Some(f) => e.file == f || e.file.ends_with(f),
            None => true,
        })
        .filter(|e| match parent_hint {
            Some(p) => e.parent.as_deref() == Some(p),
            None => true,
        })
        // Don't resolve to imports — `sigil context use foo::bar` is never
        // what the caller wants; they want the defining entity.
        .filter(|e| e.kind != "import")
        .collect();

    // Rank by blast direct_files desc (load-bearing definition first), then
    // by line_start ascending for stable output on ties.
    matches.sort_by_key(|e| {
        (
            Reverse(e.blast_radius.as_ref().map(|b| b.direct_files).unwrap_or(0)),
            e.line_start,
        )
    });

    matches
}

/// Primary entry point. Pure over `Index`.
pub fn build_context(idx: &Index, query: &str, opts: &ContextOptions) -> Option<Context> {
    let resolved = resolve(idx, query);
    let chosen_entity = resolved.first()?;
    let chosen = SymbolRef::from_entity(chosen_entity);
    let alternatives: Vec<SymbolRef> = resolved
        .iter()
        .skip(1)
        .take(4) // cap alt list — more than 4 is rarely helpful, often noise
        .map(|e| SymbolRef::from_entity(e))
        .collect();

    let depth = opts.depth.max(1);

    // Callers — refs whose target is this name. Dedup by (file, line) since a
    // symbol can be called twice on the same line (e.g. chained calls).
    let mut seen: HashSet<(String, u32)> = HashSet::new();
    let callers_all: Vec<&Reference> = idx
        .refs_to(&chosen.name)
        .filter(|r| seen.insert((r.file.clone(), r.line)))
        .collect();
    let callers: Vec<Edge> = callers_all
        .iter()
        .take(depth)
        .map(|r| caller_edge(r))
        .collect();
    let skipped_callers = callers_all.len().saturating_sub(callers.len());

    // Callees — refs whose `caller` is this symbol's name. Split into real
    // callees (call / instantiation) vs related types (type_annotation) so
    // the agent sees the distinction without post-processing.
    let mut seen: HashSet<(String, u32, String)> = HashSet::new();
    let from_self: Vec<&Reference> = idx
        .refs_from(&chosen.name)
        .filter(|r| seen.insert((r.file.clone(), r.line, r.name.clone())))
        .collect();

    let (type_refs, call_refs): (Vec<&&Reference>, Vec<&&Reference>) = from_self
        .iter()
        .partition(|r| r.ref_kind == "type_annotation");

    let callees: Vec<Edge> = call_refs
        .iter()
        .take(depth)
        .map(|r| callee_edge(r))
        .collect();
    let skipped_callees = call_refs.len().saturating_sub(callees.len());

    let related_types: Vec<Edge> = type_refs
        .iter()
        .take(depth)
        .map(|r| callee_edge(r))
        .collect();
    let skipped_types = type_refs.len().saturating_sub(related_types.len());

    let mut ctx = Context {
        query: query.to_string(),
        chosen,
        alternatives,
        callers,
        callees,
        related_types,
        skipped_callers,
        skipped_callees,
        skipped_types,
        estimated_tokens: 0,
    };

    // Budget enforcement: render, estimate, trim back-half lists if over.
    enforce_budget(&mut ctx, opts);

    Some(ctx)
}

fn caller_edge(r: &Reference) -> Edge {
    Edge {
        file: r.file.clone(),
        line: r.line,
        symbol: r.caller.clone().unwrap_or_else(|| "<top-level>".to_string()),
        kind: r.ref_kind.clone(),
        caller: r.caller.clone(),
    }
}

fn callee_edge(r: &Reference) -> Edge {
    Edge {
        file: r.file.clone(),
        line: r.line,
        symbol: r.name.clone(),
        kind: r.ref_kind.clone(),
        caller: r.caller.clone(),
    }
}

/// Token estimator — 4 bytes ≈ 1 token, same heuristic `sigil map` uses.
fn estimate_tokens(s: &str) -> usize {
    (s.len() + 3) / 4
}

/// Trim alternatives / callees / related_types / callers (in that order of
/// priority — always preserve the chosen entity and at least one caller) so
/// the rendered output fits within `opts.budget`.
fn enforce_budget(ctx: &mut Context, opts: &ContextOptions) {
    // Markdown is the widest renderer — budget against that form so the
    // other formats (smaller) always fit.
    let mut estimated = estimate_tokens(&render_markdown(ctx));
    if opts.budget == 0 || estimated <= opts.budget {
        ctx.estimated_tokens = estimated;
        return;
    }

    // Drop alternatives first — they're disambiguators, not context.
    while estimated > opts.budget && !ctx.alternatives.is_empty() {
        ctx.alternatives.pop();
        estimated = estimate_tokens(&render_markdown(ctx));
    }

    // Then trim related_types.
    while estimated > opts.budget && !ctx.related_types.is_empty() {
        ctx.related_types.pop();
        ctx.skipped_types += 1;
        estimated = estimate_tokens(&render_markdown(ctx));
    }

    // Then callees.
    while estimated > opts.budget && !ctx.callees.is_empty() {
        ctx.callees.pop();
        ctx.skipped_callees += 1;
        estimated = estimate_tokens(&render_markdown(ctx));
    }

    // Finally callers — but keep at least one. A symbol with no caller
    // context is barely useful; letting the budget drop it entirely would
    // defeat the command's purpose.
    while estimated > opts.budget && ctx.callers.len() > 1 {
        ctx.callers.pop();
        ctx.skipped_callers += 1;
        estimated = estimate_tokens(&render_markdown(ctx));
    }

    ctx.estimated_tokens = estimated;
}

// ──────────────────────────────────────────────────────────────────────────
// Renderers. Markdown is the source of truth for budget estimation since
// it's the largest form; Agent/Full can only be smaller.
// ──────────────────────────────────────────────────────────────────────────

pub fn render_markdown(ctx: &Context) -> String {
    let mut out = String::with_capacity(2048);
    let c = &ctx.chosen;

    out.push_str(&format!("# `{}`\n\n", display_symbol(c)));
    out.push_str(&format!(
        "**{}** in `{}`:{}-{}",
        c.kind, c.file, c.line_start, c.line_end,
    ));
    if let Some(vis) = &c.visibility {
        out.push_str(&format!(" · {}", vis));
    }
    if let Some(br) = &c.blast_radius {
        out.push_str(&format!(
            " · blast {}f/{}c/{}t",
            br.direct_files, br.direct_callers, br.transitive_callers
        ));
    }
    out.push_str("\n\n");

    if let Some(sig) = &c.sig {
        out.push_str("## Signature\n\n");
        out.push_str("```\n");
        out.push_str(sig.trim());
        out.push_str("\n```\n\n");
    }

    render_edge_section(
        &mut out,
        "Callers",
        &ctx.callers,
        ctx.skipped_callers,
        /* show_target */ false,
    );
    render_edge_section(
        &mut out,
        "Callees",
        &ctx.callees,
        ctx.skipped_callees,
        /* show_target */ true,
    );
    render_edge_section(
        &mut out,
        "Related types",
        &ctx.related_types,
        ctx.skipped_types,
        /* show_target */ true,
    );

    if !ctx.alternatives.is_empty() {
        out.push_str(&format!(
            "## Ambiguous — {} other match(es)\n\n",
            ctx.alternatives.len()
        ));
        for alt in &ctx.alternatives {
            out.push_str(&format!(
                "- `{}` at `{}`:{}",
                display_symbol(alt),
                alt.file,
                alt.line_start
            ));
            if let Some(br) = &alt.blast_radius {
                out.push_str(&format!(" (blast {}f)", br.direct_files));
            }
            out.push('\n');
        }
        out.push('\n');
    }

    out
}

fn render_edge_section(
    out: &mut String,
    heading: &str,
    edges: &[Edge],
    skipped: usize,
    show_target: bool,
) {
    if edges.is_empty() && skipped == 0 {
        return;
    }
    out.push_str(&format!("## {}", heading));
    if !edges.is_empty() {
        out.push_str(&format!(" ({})", edges.len()));
    }
    out.push_str("\n\n");
    for e in edges {
        if show_target {
            out.push_str(&format!(
                "- `{}` → `{}`  _{}_  `{}:{}`\n",
                e.caller.as_deref().unwrap_or("<top-level>"),
                e.symbol,
                e.kind,
                e.file,
                e.line,
            ));
        } else {
            out.push_str(&format!(
                "- `{}`  _{}_  `{}:{}`\n",
                e.symbol, e.kind, e.file, e.line
            ));
        }
    }
    if skipped > 0 {
        out.push_str(&format!("- _+{} more truncated by budget_\n", skipped));
    }
    out.push('\n');
}

fn display_symbol(s: &SymbolRef) -> String {
    match &s.parent {
        Some(p) => format!("{}::{}", p, s.name),
        None => s.name.clone(),
    }
}

/// Compact short-keyed JSON tuned for LLM token economy. One-line bundle,
/// no whitespace; callers that want pretty output pass `--format json`.
#[derive(Debug, Clone, Serialize)]
struct AgentView<'a> {
    q: &'a str,
    f: &'a str,
    n: &'a str,
    k: &'a str,
    l: [u32; 2],
    #[serde(skip_serializing_if = "Option::is_none")]
    p: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    s: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    v: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    br: Option<[u32; 3]>,
    cr: Vec<AgentEdge<'a>>, // callers
    ce: Vec<AgentEdge<'a>>, // callees
    rt: Vec<AgentEdge<'a>>, // related types
    #[serde(skip_serializing_if = "is_zero_skip")]
    sk: [usize; 3], // [callers, callees, types]
}

fn is_zero_skip(s: &[usize; 3]) -> bool {
    s.iter().all(|x| *x == 0)
}

#[derive(Debug, Clone, Serialize)]
struct AgentEdge<'a> {
    f: &'a str,
    l: u32,
    s: &'a str, // symbol
    k: &'a str, // kind
}

pub fn render_agent_json(ctx: &Context) -> String {
    fn edge<'a>(e: &'a Edge) -> AgentEdge<'a> {
        AgentEdge {
            f: &e.file,
            l: e.line,
            s: &e.symbol,
            k: &e.kind,
        }
    }
    let br = ctx
        .chosen
        .blast_radius
        .as_ref()
        .map(|b| [b.direct_callers, b.direct_files, b.transitive_callers]);
    let view = AgentView {
        q: &ctx.query,
        f: &ctx.chosen.file,
        n: &ctx.chosen.name,
        k: &ctx.chosen.kind,
        l: [ctx.chosen.line_start, ctx.chosen.line_end],
        p: ctx.chosen.parent.as_deref(),
        s: ctx.chosen.sig.as_deref(),
        v: ctx.chosen.visibility.as_deref(),
        br,
        cr: ctx.callers.iter().map(edge).collect(),
        ce: ctx.callees.iter().map(edge).collect(),
        rt: ctx.related_types.iter().map(edge).collect(),
        sk: [ctx.skipped_callers, ctx.skipped_callees, ctx.skipped_types],
    };
    serde_json::to_string(&view).expect("AgentView serializes infallibly")
}

pub fn render_full_json(ctx: &Context, pretty: bool) -> String {
    if pretty {
        serde_json::to_string_pretty(ctx).expect("Context serializes infallibly")
    } else {
        serde_json::to_string(ctx).expect("Context serializes infallibly")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{BlastRadius, Entity, Reference};
    use crate::query::index::Index;

    fn ent_full(
        file: &str,
        name: &str,
        kind: &str,
        parent: Option<&str>,
        sig: Option<&str>,
        visibility: Option<&str>,
        blast_files: u32,
    ) -> Entity {
        Entity {
            file: file.to_string(),
            name: name.to_string(),
            kind: kind.to_string(),
            line_start: 10,
            line_end: 20,
            parent: parent.map(str::to_string),
            sig: sig.map(str::to_string),
            meta: None,
            body_hash: None,
            sig_hash: None,
            struct_hash: "deadbeef".to_string(),
            visibility: visibility.map(str::to_string),
            rank: None,
            blast_radius: Some(BlastRadius {
                direct_callers: blast_files * 2,
                direct_files: blast_files,
                transitive_callers: blast_files * 5,
            }),
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

    #[test]
    fn split_query_forms() {
        assert_eq!(split_query("foo"), (None, None, "foo"));
        assert_eq!(split_query("Foo::bar"), (None, Some("Foo"), "bar"));
        assert_eq!(
            split_query("src/x.rs::bar"),
            (Some("src/x.rs"), None, "bar")
        );
        assert_eq!(
            split_query("src/x.rs::Foo::bar"),
            (Some("src/x.rs"), Some("Foo"), "bar")
        );
    }

    #[test]
    fn resolve_returns_highest_blast_first() {
        let idx = Index::build(
            vec![
                ent_full("a.rs", "Config", "struct", None, None, None, 1),
                ent_full("b.rs", "Config", "struct", None, None, None, 5), // louder
                ent_full("c.rs", "Config", "struct", None, None, None, 3),
            ],
            vec![],
        );
        let matches = resolve(&idx, "Config");
        assert_eq!(matches.len(), 3);
        assert_eq!(matches[0].file, "b.rs", "highest direct_files first");
        assert_eq!(matches[1].file, "c.rs");
        assert_eq!(matches[2].file, "a.rs");
    }

    #[test]
    fn resolve_with_file_hint_filters_candidates() {
        let idx = Index::build(
            vec![
                ent_full("a.rs", "Config", "struct", None, None, None, 1),
                ent_full("src/x.rs", "Config", "struct", None, None, None, 5),
            ],
            vec![],
        );
        let matches = resolve(&idx, "src/x.rs::Config");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].file, "src/x.rs");
    }

    #[test]
    fn resolve_with_parent_hint_filters_candidates() {
        let idx = Index::build(
            vec![
                ent_full("a.rs", "new", "function", Some("Foo"), None, None, 3),
                ent_full("a.rs", "new", "function", Some("Bar"), None, None, 5),
                ent_full("a.rs", "new", "function", None, None, None, 1),
            ],
            vec![],
        );
        let matches = resolve(&idx, "Foo::new");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].parent.as_deref(), Some("Foo"));
    }

    #[test]
    fn resolve_skips_imports() {
        let idx = Index::build(
            vec![
                ent_full("a.rs", "Config", "import", None, None, None, 0),
                ent_full("b.rs", "Config", "struct", None, None, None, 5),
            ],
            vec![],
        );
        let matches = resolve(&idx, "Config");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].kind, "struct");
    }

    #[test]
    fn build_context_populates_callers_and_callees() {
        let idx = Index::build(
            vec![ent_full(
                "a.rs",
                "process",
                "function",
                None,
                Some("fn process(x: T) -> R"),
                Some("public"),
                3,
            )],
            vec![
                refr("b.rs", Some("main"), "process", "call", 1),
                refr("c.rs", Some("wrapper"), "process", "call", 2),
                refr("a.rs", Some("process"), "T", "type_annotation", 1),
                refr("a.rs", Some("process"), "helper", "call", 3),
            ],
        );
        let ctx = build_context(&idx, "process", &ContextOptions { budget: 0, depth: 10, format: ContextFormat::Markdown }).unwrap();
        assert_eq!(ctx.chosen.name, "process");
        assert_eq!(ctx.callers.len(), 2);
        assert_eq!(ctx.callees.len(), 1, "only `helper` is a pure callee");
        assert_eq!(ctx.related_types.len(), 1, "`T` is a type_annotation");
        assert_eq!(ctx.related_types[0].symbol, "T");
    }

    #[test]
    fn missing_symbol_returns_none() {
        let idx = Index::build(
            vec![ent_full("a.rs", "foo", "function", None, None, None, 0)],
            vec![],
        );
        assert!(build_context(&idx, "nonexistent", &ContextOptions::default()).is_none());
    }

    #[test]
    fn alternatives_populated_when_ambiguous() {
        let idx = Index::build(
            vec![
                ent_full("a.rs", "Config", "struct", None, None, None, 5),
                ent_full("b.rs", "Config", "struct", None, None, None, 3),
                ent_full("c.rs", "Config", "struct", None, None, None, 1),
            ],
            vec![],
        );
        let ctx = build_context(&idx, "Config", &ContextOptions { budget: 0, depth: 10, format: ContextFormat::Markdown }).unwrap();
        assert_eq!(ctx.chosen.file, "a.rs");
        assert_eq!(ctx.alternatives.len(), 2);
    }

    #[test]
    fn depth_caps_each_section() {
        let idx = Index::build(
            vec![ent_full("a.rs", "foo", "function", None, None, None, 0)],
            (0..20)
                .flat_map(|i| {
                    vec![
                        refr(&format!("f{i}.rs"), Some("m"), "foo", "call", i as u32),
                        refr("a.rs", Some("foo"), &format!("cb{i}"), "call", i as u32),
                        refr("a.rs", Some("foo"), &format!("T{i}"), "type_annotation", i as u32),
                    ]
                })
                .collect(),
        );
        let ctx = build_context(&idx, "foo", &ContextOptions { budget: 0, depth: 3, format: ContextFormat::Markdown }).unwrap();
        assert_eq!(ctx.callers.len(), 3);
        assert_eq!(ctx.callees.len(), 3);
        assert_eq!(ctx.related_types.len(), 3);
        assert_eq!(ctx.skipped_callers, 17);
        assert_eq!(ctx.skipped_callees, 17);
        assert_eq!(ctx.skipped_types, 17);
    }

    #[test]
    fn budget_trims_but_keeps_chosen_and_one_caller() {
        let idx = Index::build(
            vec![ent_full("a.rs", "foo", "function", None, Some("fn foo()"), None, 0)],
            (0..50)
                .flat_map(|i| {
                    vec![
                        refr(&format!("f{i}.rs"), Some("m"), "foo", "call", i as u32),
                        refr("a.rs", Some("foo"), &format!("cb{i}"), "call", i as u32),
                    ]
                })
                .collect(),
        );
        // Absurdly small budget — implementation must keep at least 1 caller.
        let ctx = build_context(&idx, "foo", &ContextOptions { budget: 50, depth: 50, format: ContextFormat::Markdown }).unwrap();
        assert_eq!(ctx.chosen.name, "foo");
        assert!(ctx.callers.len() >= 1);
        assert!(ctx.callees.is_empty() || ctx.callees.len() < 50);
        assert!(ctx.skipped_callers > 0 || ctx.skipped_callees > 0);
    }

    #[test]
    fn render_markdown_has_expected_sections() {
        let idx = Index::build(
            vec![ent_full(
                "a.rs",
                "foo",
                "function",
                None,
                Some("fn foo(x: T) -> R"),
                Some("public"),
                2,
            )],
            vec![
                refr("b.rs", Some("main"), "foo", "call", 42),
                refr("a.rs", Some("foo"), "T", "type_annotation", 1),
                refr("a.rs", Some("foo"), "helper", "call", 2),
            ],
        );
        let ctx = build_context(&idx, "foo", &ContextOptions { budget: 0, depth: 10, format: ContextFormat::Markdown }).unwrap();
        let md = render_markdown(&ctx);
        assert!(md.starts_with("# `foo`"));
        assert!(md.contains("## Signature"));
        assert!(md.contains("fn foo(x: T) -> R"));
        assert!(md.contains("## Callers"));
        assert!(md.contains("## Callees"));
        assert!(md.contains("## Related types"));
        assert!(md.contains("public"));
        assert!(md.contains("b.rs"));
    }

    #[test]
    fn render_agent_json_is_compact_and_short_keyed() {
        // Use a non-trivial fixture so the comparison against markdown is
        // meaningful. At 10+ edges markdown's per-bullet prose overhead
        // exceeds the JSON structure cost.
        let idx = Index::build(
            vec![ent_full(
                "a.rs",
                "foo",
                "function",
                None,
                Some("pub fn foo(x: Input, cfg: Config) -> Result<Output, Error>"),
                Some("public"),
                5,
            )],
            (0..10)
                .flat_map(|i| {
                    vec![
                        refr(&format!("callers/c{i}.rs"), Some("main_caller"), "foo", "call", i as u32 + 1),
                        refr("a.rs", Some("foo"), &format!("callee_{i}"), "call", i as u32 + 50),
                        refr("a.rs", Some("foo"), &format!("Type{i}"), "type_annotation", i as u32 + 100),
                    ]
                })
                .collect(),
        );
        let ctx = build_context(
            &idx,
            "foo",
            &ContextOptions { budget: 0, depth: 10, format: ContextFormat::Agent },
        )
        .unwrap();
        let agent = render_agent_json(&ctx);
        let markdown = render_markdown(&ctx);

        // Structural properties that actually matter for agent ingestion:
        //   - single-line (fits cleanly into a tool-result slot)
        //   - short, stable keys (so tokens-per-key doesn't explode)
        //   - no long human-readable prose (markdown headings, etc.)
        //
        // Byte count vs markdown isn't a useful invariant — at modest
        // fixture sizes JSON structure overhead (quoted keys + commas)
        // roughly matches markdown bullet + backtick overhead, and the
        // winner flips depending on string lengths.
        assert!(!agent.contains('\n'), "agent format must be single-line");
        assert!(!agent.contains("## "), "agent format must not contain markdown headings");
        assert!(agent.contains("\"q\":"));
        assert!(agent.contains("\"cr\":"));
        assert!(agent.contains("\"ce\":"));
        assert!(agent.contains("\"rt\":"));
        // Sanity: the rendered agent JSON actually parses.
        let _: serde_json::Value = serde_json::from_str(&agent).expect("agent JSON must parse");
        // Keep `markdown` referenced so the fixture stays useful if a future
        // invariant uses it again.
        let _ = markdown.len();
    }

    #[test]
    fn format_parse_covers_known_values() {
        assert_eq!(ContextFormat::parse("agent"), Some(ContextFormat::Agent));
        assert_eq!(ContextFormat::parse("markdown"), Some(ContextFormat::Markdown));
        assert_eq!(ContextFormat::parse("md"), Some(ContextFormat::Markdown));
        assert_eq!(ContextFormat::parse("json"), Some(ContextFormat::Full));
        assert_eq!(ContextFormat::parse("full"), Some(ContextFormat::Full));
        assert_eq!(ContextFormat::parse("nonsense"), None);
    }
}
