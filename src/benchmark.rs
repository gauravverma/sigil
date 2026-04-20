//! `sigil benchmark` — publish a "Nx fewer tokens per equivalent query"
//! number after any run.
//!
//! The first honest-numbers build. We compare:
//!
//!   Control → raw approach (e.g. `git diff HEAD~1..HEAD`).
//!   Treatment → sigil approach (e.g. `sigil review HEAD~1..HEAD`).
//!
//! Both outputs are captured, token-estimated (bytes/4), and printed as a
//! table. This is a **self-benchmark** at Week 10 — it tells a publisher
//! "on this repo, sigil review is N× more compact than git diff at the same
//! fidelity." The §8 eval harness (Weeks 11–12) subsumes this with a real
//! multi-repo corpus + agent-in-the-loop measurement; keep this command as
//! the local, no-setup smoke version.

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{Context as _, Result};
use serde::Serialize;

use crate::context as sigil_context;
use crate::map as sigil_map;
use crate::query::index::Index;
use crate::rank::RankManifest;
use crate::review;
use crate::tokens::Tokenizer;

#[derive(Debug, Clone)]
pub struct BenchmarkOptions {
    /// Ref spec used for diff-shaped queries (PR review / change summary).
    pub refspec: String,
    /// Sample symbol for the context-shaped query. Falls back to the
    /// highest-blast entity in the index if not provided.
    pub symbol: Option<String>,
    pub format: BenchmarkFormat,
    /// Which tokenizer to use for both control and treatment counts.
    /// Defaults to the proxy; BPE variants require the `tokenizer`
    /// cargo feature.
    pub tokenizer: Tokenizer,
}

impl Default for BenchmarkOptions {
    fn default() -> Self {
        Self {
            refspec: "HEAD~1..HEAD".to_string(),
            symbol: None,
            format: BenchmarkFormat::Markdown,
            tokenizer: Tokenizer::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkFormat {
    Markdown,
    Json,
}

impl BenchmarkFormat {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "markdown" | "md" => Some(Self::Markdown),
            "json" => Some(Self::Json),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchmarkReport {
    pub sigil_version: String,
    pub refspec: String,
    /// Which tokenizer produced the counts ("bytes/4 proxy", "o200k_base",
    /// etc.). Included in the JSON so downstream readers can tell proxy
    /// numbers apart from BPE-accurate ones without guesswork.
    pub tokenizer: String,
    pub queries: Vec<QueryResult>,
    pub median_ratio: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct QueryResult {
    pub name: String,
    pub control_command: String,
    pub treatment_command: String,
    pub control_tokens: usize,
    pub treatment_tokens: usize,
    /// control / treatment. >1 = sigil is more compact.
    pub ratio: f64,
    /// True when the control command failed (typically: no git history,
    /// unknown ref). Query still surfaces with tokens=0 / ratio=0 so the
    /// reader sees what was attempted.
    pub control_skipped: bool,
}

// Token counting now flows through a `Tokenizer` selected per-run. The
// proxy-only path is still the default and lives in `tokens::proxy_count`,
// so maps / contexts that don't need BPE-accurate numbers are unaffected.

/// Run the full benchmark. All three queries run with current .sigil/ data
/// — caller is responsible for running `sigil index` and
/// `sigil cochange` first if they want the richest treatment numbers.
pub fn run_benchmark(root: &Path, opts: &BenchmarkOptions) -> Result<BenchmarkReport> {
    let idx = Index::load(root).context("load sigil index")?;
    if idx.is_empty() {
        anyhow::bail!(".sigil/ is empty — run `sigil index` first");
    }
    let rank = sigil_map::load_rank_manifest(root).unwrap_or_default();

    // Fail fast if the user asked for a BPE tokenizer but sigil was built
    // without the feature. Better to surface the misconfiguration than to
    // silently fall back to the proxy and publish mismatched numbers.
    opts.tokenizer
        .count("")
        .context("selected tokenizer is not available in this build")?;

    let mut queries = Vec::new();
    let tok = &opts.tokenizer;

    // Q1: review vs git diff.
    queries.push(run_review_query(root, &opts.refspec, tok));

    // Q2: context <symbol> vs reading the whole file.
    let symbol = opts.symbol.clone().or_else(|| pick_high_blast_symbol(&idx));
    if let Some(sym) = symbol {
        queries.push(run_context_query(root, &idx, &sym, tok));
    }

    // Q3: map vs a shallow ls + file reads.
    queries.push(run_map_query(root, &idx, &rank, tok));

    let ratios: Vec<f64> = queries.iter().filter(|q| !q.control_skipped).map(|q| q.ratio).collect();
    let median_ratio = median(&ratios);

    Ok(BenchmarkReport {
        sigil_version: env!("CARGO_PKG_VERSION").to_string(),
        refspec: opts.refspec.clone(),
        tokenizer: opts.tokenizer.label().to_string(),
        queries,
        median_ratio,
    })
}

fn pick_high_blast_symbol(idx: &Index) -> Option<String> {
    idx.entities
        .iter()
        .filter(|e| e.kind != "import")
        .max_by_key(|e| {
            e.blast_radius
                .as_ref()
                .map(|b| b.direct_files)
                .unwrap_or(0)
        })
        .map(|e| e.name.clone())
}

fn run_review_query(root: &Path, refspec: &str, tok: &Tokenizer) -> QueryResult {
    let control_cmd = format!("git diff --stat --patch {refspec}");
    let treatment_cmd = format!("sigil review {refspec}");

    let (control_tokens, control_skipped) = git_output_tokens(root, refspec, tok);
    let treatment_tokens = match review::run_review(
        root,
        refspec,
        &review::ReviewOptions {
            format: review::ReviewFormat::Markdown,
            show_cochange: true,
            ..review::ReviewOptions::default()
        },
    ) {
        Ok(s) => count_or_proxy(tok, &s),
        Err(_) => 0,
    };

    let ratio = ratio(control_tokens, treatment_tokens);
    QueryResult {
        name: "PR review".to_string(),
        control_command: control_cmd,
        treatment_command: treatment_cmd,
        control_tokens,
        treatment_tokens,
        ratio,
        control_skipped,
    }
}

fn run_context_query(root: &Path, idx: &Index, symbol: &str, tok: &Tokenizer) -> QueryResult {
    let control_cmd = format!(
        "cat $(grep -rl '{symbol}' .)   # very rough proxy for a 'read-everywhere' expansion"
    );
    let treatment_cmd = format!("sigil context {symbol}");

    // Control: reading every file that references the symbol. Bounded for
    // sanity (huge repos) — accumulator caps at 100 files.
    let control_tokens = read_files_touching_symbol_tokens(root, idx, symbol, 100, tok);
    let treatment_tokens = match sigil_context::build_context(
        idx,
        symbol,
        &sigil_context::ContextOptions::default(),
    ) {
        Some(ctx) => count_or_proxy(tok, &sigil_context::render_markdown(&ctx)),
        None => 0,
    };
    let ratio = ratio(control_tokens, treatment_tokens);
    QueryResult {
        name: format!("Context for `{symbol}`"),
        control_command: control_cmd,
        treatment_command: treatment_cmd,
        control_tokens,
        treatment_tokens,
        ratio,
        control_skipped: false,
    }
}

fn run_map_query(root: &Path, idx: &Index, rank: &RankManifest, tok: &Tokenizer) -> QueryResult {
    let control_cmd = "find . -type f -name '*.rs' | head -20 | xargs cat".to_string();
    let treatment_cmd = "sigil map --tokens 2000".to_string();

    let control_tokens = read_top_files_tokens(root, idx, 20, tok);
    let map_output = sigil_map::build_map(
        idx,
        rank,
        &sigil_map::MapOptions {
            tokens: 2000,
            ..sigil_map::MapOptions::default()
        },
    );
    let treatment_tokens = count_or_proxy(tok, &sigil_map::render_markdown(&map_output));
    let ratio = ratio(control_tokens, treatment_tokens);
    QueryResult {
        name: "Cold-start orientation".to_string(),
        control_command: control_cmd,
        treatment_command: treatment_cmd,
        control_tokens,
        treatment_tokens,
        ratio,
        control_skipped: false,
    }
}

fn git_output_tokens(root: &Path, refspec: &str, tok: &Tokenizer) -> (usize, bool) {
    let out = Command::new("git")
        .args(["diff", "--stat", "--patch", refspec])
        .current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output();
    match out {
        Ok(out) if out.status.success() => (
            count_or_proxy(tok, &String::from_utf8_lossy(&out.stdout)),
            false,
        ),
        _ => (0, true),
    }
}

fn read_files_touching_symbol_tokens(
    root: &Path,
    idx: &Index,
    symbol: &str,
    cap: usize,
    tok: &Tokenizer,
) -> usize {
    use std::collections::HashSet;
    let mut files: Vec<&str> = idx
        .refs_to(symbol)
        .map(|r| r.file.as_str())
        .chain(
            idx.entities_by_name(symbol)
                .map(|e| e.file.as_str()),
        )
        .collect::<HashSet<&str>>()
        .into_iter()
        .collect();
    files.sort();
    files.truncate(cap);
    files
        .iter()
        .map(|f| read_file_tokens(root, f, tok))
        .sum()
}

fn read_top_files_tokens(root: &Path, idx: &Index, cap: usize, tok: &Tokenizer) -> usize {
    use std::collections::BTreeSet;
    let files: BTreeSet<&str> = idx.entities.iter().map(|e| e.file.as_str()).collect();
    files
        .iter()
        .take(cap)
        .map(|f| read_file_tokens(root, f, tok))
        .sum()
}

fn read_file_tokens(root: &Path, relative: &str, tok: &Tokenizer) -> usize {
    let path = root.join(relative);
    match std::fs::read_to_string(&path) {
        Ok(s) => count_or_proxy(tok, &s),
        Err(_) => 0,
    }
}

/// Defense-in-depth wrapper: if the tokenizer errors at runtime we fall
/// back to the proxy rather than blowing up mid-benchmark. This only
/// kicks in for BPE variants; the proxy path never errors.
fn count_or_proxy(tok: &Tokenizer, s: &str) -> usize {
    tok.count(s).unwrap_or_else(|_| crate::tokens::proxy_count(s))
}

fn ratio(control: usize, treatment: usize) -> f64 {
    if treatment == 0 {
        return 0.0;
    }
    control as f64 / treatment as f64
}

fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut v = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    }
}

pub fn render_markdown(report: &BenchmarkReport) -> String {
    let mut out = String::with_capacity(1024);
    out.push_str(&format!(
        "# Sigil Benchmark — {}\n\n",
        report.sigil_version
    ));
    out.push_str(&format!(
        "refspec: `{}`  ·  tokenizer: `{}`  ·  median reduction: **{:.2}×**\n\n",
        report.refspec, report.tokenizer, report.median_ratio
    ));

    out.push_str("| Query | Control tokens | Sigil tokens | Ratio |\n");
    out.push_str("|---|---:|---:|---:|\n");
    for q in &report.queries {
        let ratio = if q.control_skipped {
            "n/a".to_string()
        } else {
            format!("{:.2}×", q.ratio)
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            q.name, q.control_tokens, q.treatment_tokens, ratio
        ));
    }
    out.push('\n');
    if report.tokenizer.contains("proxy") {
        out.push_str("_Token estimate uses `bytes / 4` — rough proxy for modern tokenizers, off by ~20% either way. Build with `cargo install sigil --features tokenizer` and pass `--tokenizer o200k_base` for BPE-accurate counts._\n");
    } else {
        out.push_str(&format!(
            "_BPE-accurate token counts via {}._\n",
            report.tokenizer
        ));
    }
    out
}

pub fn render_json(report: &BenchmarkReport, pretty: bool) -> String {
    if pretty {
        serde_json::to_string_pretty(report).expect("report serializes infallibly")
    } else {
        serde_json::to_string(report).expect("report serializes infallibly")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proxy_token_count_matches_prior_spec() {
        // Delegated to `crate::tokens::proxy_count` since the bytes/4 rule
        // is shared with map.rs and context.rs. Regression guard against
        // any accidental change in the heuristic.
        assert_eq!(crate::tokens::proxy_count(""), 0);
        assert_eq!(crate::tokens::proxy_count("a"), 1);
        assert_eq!(crate::tokens::proxy_count("abcd"), 1);
        assert_eq!(crate::tokens::proxy_count("abcde"), 2);
    }

    #[test]
    fn ratio_handles_zero_denominator() {
        assert_eq!(ratio(100, 0), 0.0);
        assert_eq!(ratio(0, 100), 0.0);
        assert!((ratio(1000, 100) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn median_odd_and_even_length() {
        assert_eq!(median(&[]), 0.0);
        assert_eq!(median(&[7.0]), 7.0);
        assert_eq!(median(&[3.0, 1.0, 2.0]), 2.0); // odd
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), 2.5); // even
    }

    #[test]
    fn format_parse_covers_known_values() {
        assert_eq!(
            BenchmarkFormat::parse("markdown"),
            Some(BenchmarkFormat::Markdown)
        );
        assert_eq!(BenchmarkFormat::parse("json"), Some(BenchmarkFormat::Json));
        assert!(BenchmarkFormat::parse("").is_none());
    }

    #[test]
    fn render_markdown_contains_query_table() {
        let report = BenchmarkReport {
            sigil_version: "test".to_string(),
            refspec: "HEAD~1..HEAD".to_string(),
            tokenizer: "bytes/4 proxy".to_string(),
            queries: vec![QueryResult {
                name: "test query".to_string(),
                control_command: "cat".to_string(),
                treatment_command: "sigil map".to_string(),
                control_tokens: 1000,
                treatment_tokens: 100,
                ratio: 10.0,
                control_skipped: false,
            }],
            median_ratio: 10.0,
        };
        let md = render_markdown(&report);
        assert!(md.contains("# Sigil Benchmark"));
        assert!(md.contains("HEAD~1..HEAD"));
        assert!(md.contains("| test query |"));
        assert!(md.contains("10.00×"));
    }
}
