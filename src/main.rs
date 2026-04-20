use sigil::diff;
use sigil::formatter;
use sigil::git;
use sigil::grouping;
use sigil::index;
use sigil::markdown_formatter;
use sigil::output;
use sigil::query;
use sigil::writer;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "sigil",
    about = "Deterministic structural code intelligence for AI coding agents",
    long_about = "\
Deterministic structural code intelligence for AI coding agents.

sigil groups commands into two tiers (see `agent-adoption-plan.md` §15):

  AGENT-FACING (narrated, budget-aware, markdown-first):
    map         Ranked codebase digest for cold-start orientation
    context     Signature + callers + callees + related types, budget-capped
    review      PR review: structural diff + rank + blast + co-change misses
    blast       Impact summary — callers, files, transitive reach
    benchmark   Publishes median token reduction vs raw alternatives

  SCRIPT-FACING (raw, unbounded, JSON-friendly):
    search      Substring search over symbols + file paths
    symbols     All entities in a file
    children    Entities under a parent
    callers     All refs targeting a symbol (unbounded)
    callees     What a symbol calls
    explore     Directory overview
    duplicates  Clone report across the codebase
    cochange    Git-history file-pair co-change miner

  INSTALLERS (platform integrations, all idempotent):
    claude · cursor · codex · gemini · opencode · aider · copilot · hook

Plus `index` (build the .sigil/ index), `diff` (the 0.2.x structural diff
engine), `update` (self-update via axoupdater).",
    version
)]
enum Cli {
    /// Build the entity index for a project
    Index {
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        root: PathBuf,

        /// Only index specific files
        #[arg(long)]
        files: Vec<PathBuf>,

        /// Write to stdout instead of .sigil/ files
        #[arg(long)]
        stdout: bool,

        /// Pretty-print JSON output
        #[arg(long)]
        pretty: bool,

        /// Force full re-index, ignore cache
        #[arg(long)]
        full: bool,

        /// Skip reference extraction
        #[arg(long)]
        no_refs: bool,

        /// Skip the rank + blast-radius pass (Phase 1). Rank is on by
        /// default; this flag is a one-off opt-out for CI/speed cases.
        #[arg(long)]
        no_rank: bool,

        /// Print progress information
        #[arg(short, long)]
        verbose: bool,
    },
    /// Structural diff between two git refs or two files
    Diff {
        /// Ref spec: HEAD~1, main..HEAD, abc123..def456
        #[arg(required_unless_present = "files")]
        ref_spec: Option<String>,

        /// Compare two files directly instead of git refs
        #[arg(long, num_args = 2, value_names = ["OLD", "NEW"])]
        files: Vec<PathBuf>,

        /// Project root directory
        #[arg(short, long, default_value = ".")]
        root: PathBuf,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Pretty-print JSON output
        #[arg(long)]
        pretty: bool,

        /// Print progress information
        #[arg(short, long)]
        verbose: bool,

        /// Show line numbers next to entity names
        #[arg(long)]
        lines: bool,

        /// Lines of context around changes (default 3, use --no-context to disable)
        #[arg(long, default_value = "3")]
        context: usize,

        /// Disable code context in output
        #[arg(long)]
        no_context: bool,

        /// Output as GitHub-flavored Markdown
        #[arg(long)]
        markdown: bool,

        /// Use ASCII glyphs instead of emoji (with --markdown)
        #[arg(long)]
        no_emoji: bool,

        /// Disable ANSI color output
        #[arg(long)]
        no_color: bool,

        /// Skip caller analysis for breaking changes
        #[arg(long)]
        no_callers: bool,

        /// Show one-line summary of changes
        #[arg(long)]
        summary: bool,

        /// Group related changes together
        #[arg(long)]
        group: bool,
    },
    /// Explore project structure: files grouped by directory
    Explore {
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
        /// Filter to a subdirectory
        #[arg(long)]
        path: Option<String>,
        /// Max entries to show
        #[arg(long, default_value = "200")]
        max_entries: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Search across symbols, files, and texts
    Search {
        /// Search query (FTS5 syntax, supports * wildcards)
        query: String,
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
        /// Filter by scope: symbol, file, text
        #[arg(long)]
        scope: Vec<String>,
        /// Filter by kind (e.g., function, class, method)
        #[arg(long)]
        kind: Vec<String>,
        /// Filter by file path (GLOB pattern)
        #[arg(long)]
        path: Option<String>,
        /// Max results
        #[arg(long, default_value = "20")]
        limit: u32,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// List all symbols in a file
    Symbols {
        /// File path (supports GLOB patterns)
        file: String,
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
        /// Max results
        #[arg(long, default_value = "100")]
        limit: u32,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Get children of a class or module
    Children {
        /// File containing the parent symbol
        file: String,
        /// Parent symbol name
        parent: String,
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
        /// Max results
        #[arg(long, default_value = "100")]
        limit: u32,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Find all callers/references to a symbol
    Callers {
        /// Symbol name to find callers of
        name: String,
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
        /// Filter by reference kind (call, import, type_annotation, instantiation)
        #[arg(long)]
        kind: Option<String>,
        /// Max results
        #[arg(long, default_value = "100")]
        limit: u32,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Find all symbols that a function calls
    Callees {
        /// Caller symbol name
        caller: String,
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
        /// Filter by reference kind
        #[arg(long)]
        kind: Option<String>,
        /// Max results
        #[arg(long, default_value = "100")]
        limit: u32,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Impact summary for a symbol — blast counts + top callers by file rank.
    Blast {
        /// Symbol name.
        symbol: String,
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
        /// How many top callers to surface. 0 = all.
        #[arg(long, default_value = "10")]
        depth: usize,
        /// Output format: markdown (default), json, or agent (compact JSON).
        #[arg(long, default_value = "markdown")]
        format: String,
        /// Pretty-print when --format=json.
        #[arg(long)]
        pretty: bool,
        /// Drop test-file callers and test-file candidates.
        #[arg(long)]
        exclude_tests: bool,
    },
    /// Clone report — groups entities by body_hash to surface duplicated code.
    Duplicates {
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
        /// Ignore entities whose body is fewer than this many lines.
        #[arg(long, default_value = "3")]
        min_lines: u32,
        /// Drop groups larger than this (likely auto-generated). 0 = no cap.
        #[arg(long, default_value = "0")]
        max_group_size: usize,
        /// Output format: markdown (default) or json.
        #[arg(long, default_value = "markdown")]
        format: String,
        /// Pretty-print when --format=json.
        #[arg(long)]
        pretty: bool,
    },
    /// Token-reduction benchmark: sigil commands vs raw alternatives.
    Benchmark {
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
        /// Git refspec for the PR-review query.
        #[arg(long, default_value = "HEAD~1..HEAD")]
        refspec: String,
        /// Symbol for the context query. Defaults to the highest-blast entity.
        #[arg(long)]
        symbol: Option<String>,
        /// Output format: markdown (default) or json.
        #[arg(long, default_value = "markdown")]
        format: String,
        /// Pretty-print when --format=json.
        #[arg(long)]
        pretty: bool,
        /// Token counter. `proxy` (default) is the zero-dep bytes/4
        /// heuristic. `cl100k_base`, `o200k_base`, `p50k_base` require
        /// the `tokenizer` cargo feature and give BPE-accurate counts.
        #[arg(long, default_value = "proxy")]
        tokenizer: String,
    },
    /// PR review artifact — structural diff enriched with rank, blast
    /// radius, and co-change misses. Reviewer reads this instead of
    /// `git diff`.
    Review {
        /// Ref spec: HEAD~1, main..HEAD, abc123..def456
        ref_spec: String,
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
        /// Output format: markdown (default) or json.
        #[arg(long, default_value = "markdown")]
        format: String,
        /// Max entries in the "Most impactful" section.
        #[arg(long, default_value = "5")]
        top_k: usize,
        /// Skip co-change miss detection.
        #[arg(long)]
        no_cochange: bool,
    },
    /// Build / refresh the co-change cache (`.sigil/cochange.json`).
    /// Reads `git log --name-only` and weights file pairs that move together.
    Cochange {
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
        /// Number of historical commits to walk.
        #[arg(long, default_value = "500")]
        commits: u32,
        /// Drop pairs with fewer than this many co-occurrences.
        #[arg(long, default_value = "2")]
        min_support: u32,
        /// Ignore commits that touch more than this many files.
        #[arg(long, default_value = "30")]
        max_files_per_commit: u32,
        /// Pretty-print the JSON output.
        #[arg(long)]
        pretty: bool,
    },
    /// Minimum-viable context for a symbol — signature, callers, callees,
    /// related types. One call replaces the read-6-files orientation loop.
    Context {
        /// Symbol name, or qualified form like `file::name`,
        /// `Parent::name`, `file::Parent::name`.
        query: String,
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
        /// Output token cap. 0 = unlimited.
        #[arg(long, default_value = "1500")]
        budget: usize,
        /// How many callers / callees / related types to show per section.
        #[arg(long, default_value = "10")]
        depth: usize,
        /// Output format: `markdown` (default), `agent` (compact JSON for
        /// LLM ingestion), `json` / `full` (structured JSON).
        #[arg(long, default_value = "markdown")]
        format: String,
        /// Pretty-print when --format=json. Ignored otherwise.
        #[arg(long)]
        pretty: bool,
        /// Drop test-file candidates and test-file callers from the bundle.
        #[arg(long)]
        exclude_tests: bool,
    },
    /// Budget-aware ranked digest of the codebase — drop into an agent's
    /// context for cold-start orientation.
    Map {
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
        /// Token budget. 0 = unlimited.
        #[arg(long, default_value = "4000")]
        tokens: usize,
        /// Boost entities under this path prefix so the digest centers on
        /// that subtree (useful for subsystem-focused runs).
        #[arg(long)]
        focus: Option<String>,
        /// Max entities surfaced per file.
        #[arg(long, default_value = "5")]
        depth: usize,
        /// Output format: markdown (default) or json.
        #[arg(long, default_value = "markdown")]
        format: String,
        /// Also write the Markdown form to .sigil/SIGIL_MAP.md for the
        /// agent-platform hook installers to point at.
        #[arg(long)]
        write: bool,
        /// Drop test-file entities (matching `tests/`, `*_test.rs`,
        /// `*.spec.ts`, etc.) from the map output.
        #[arg(long)]
        exclude_tests: bool,
        /// Skip the community-detection pass. Default is to include a
        /// `## Subsystems` section in the markdown output.
        #[arg(long)]
        no_clusters: bool,
    },
    /// Install or uninstall the Claude Code integration
    /// (CLAUDE.md capability block + PreToolUse hint hook).
    Claude {
        #[command(subcommand)]
        action: InstallAction,
    },
    /// Install or uninstall the Cursor integration
    /// (`.cursor/rules/sigil.mdc` with `alwaysApply: true`).
    Cursor {
        #[command(subcommand)]
        action: InstallAction,
    },
    /// Install or uninstall the Codex integration
    /// (`AGENTS.md` capability block + `.codex/hooks.json` Bash hint hook).
    Codex {
        #[command(subcommand)]
        action: InstallAction,
    },
    /// Install or uninstall the Gemini CLI integration
    /// (`GEMINI.md` capability block + `.gemini/settings.json` BeforeTool hint hook).
    Gemini {
        #[command(subcommand)]
        action: InstallAction,
    },
    /// Install or uninstall the OpenCode integration
    /// (`AGENTS.md` + `.opencode/plugins/sigil.js` + `opencode.json`).
    Opencode {
        #[command(subcommand)]
        action: InstallAction,
    },
    /// Install or uninstall the Aider integration (`AGENTS.md` block).
    Aider {
        #[command(subcommand)]
        action: InstallAction,
    },
    /// Install or uninstall the GitHub Copilot CLI skill
    /// (`~/.copilot/skills/sigil/SKILL.md`).
    Copilot {
        #[command(subcommand)]
        action: InstallAction,
    },
    /// Install git hooks (post-commit + post-checkout) that auto-rebuild
    /// the sigil index in the background.
    Hook {
        #[command(subcommand)]
        action: InstallAction,
    },
    /// Update sigil to the latest release
    Update,
}

#[derive(Subcommand)]
enum InstallAction {
    Install {
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
    },
    Uninstall {
        /// Project root directory
        #[arg(short, long, default_value = ".")]
        root: PathBuf,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli {
        Cli::Index { root, files, stdout, pretty, full, no_refs, no_rank, verbose } => {
            let files_arg = if files.is_empty() { None } else { Some(files.as_slice()) };
            let mut result = index::build_index(&root, files_arg, full, !no_refs, verbose);

            // Phase 1 rank pass. On by default; `--no-rank` skips it (useful
            // in CI or when refs are also skipped). Rank is a whole-repo
            // computation — a changed subset of files still re-ranks globally
            // because cross-file references affect the graph.
            let rank_manifest = if !no_rank && !result.refs.is_empty() {
                let cfg = sigil::rank::RankConfig::default();
                let ranked = sigil::rank::rank_with_config(&result.entities, &result.refs, &cfg);
                sigil::rank::apply_blast_radius(&mut result.entities, &ranked);
                Some(sigil::rank::RankManifest::from_ranked(&ranked, &cfg))
            } else {
                // If the user opted out, also wipe any stale rank/blast_radius
                // that cached entities carried over from a previous run — the
                // on-disk output should reflect the requested mode.
                for e in &mut result.entities {
                    e.rank = None;
                    e.blast_radius = None;
                }
                None
            };

            if stdout {
                let out = std::io::stdout();
                let mut out = out.lock();
                writer::write_entities_jsonl(&result.entities, &mut out, pretty)
                    .expect("Failed to write to stdout");
                // Write refs to stderr in stdout mode to avoid mixing
                if !result.refs.is_empty() {
                    let err = std::io::stderr();
                    let mut err = err.lock();
                    writer::write_refs_jsonl(&result.refs, &mut err, pretty)
                        .expect("Failed to write refs to stderr");
                }
                // rank.json is a project-level artifact; we don't emit it on
                // stdout. blast_radius is already on each entity above.
            } else {
                writer::write_to_files(&result.entities, &result.refs, &root, pretty)
                    .expect("Failed to write index");
                match &rank_manifest {
                    Some(m) => writer::write_rank_json(m, &root, pretty)
                        .expect("Failed to write rank.json"),
                    None => {
                        // Clean up any stale rank.json from a prior run when
                        // the user explicitly disables ranking.
                        let _ = writer::remove_rank_json(&root);
                    }
                }
                if verbose {
                    let rank_note = match &rank_manifest {
                        Some(m) => format!(", {} files ranked", m.file_count),
                        None => " (rank skipped)".to_string(),
                    };
                    eprintln!(
                        "Wrote {} entities and {} refs to .sigil/{}",
                        result.entities.len(),
                        result.refs.len(),
                        rank_note
                    );
                }
            }
        }
        Cli::Diff { ref_spec, files, root, json, pretty, verbose, lines, context, no_context, markdown, no_emoji, no_color, no_callers, summary, group } => {
            // Handle --no-color
            if no_color {
                colored::control::set_override(false);
            }

            // Compute diff result
            let include_context = !no_context;
            let context_lines = context;
            let result = if files.len() == 2 {
                let opts = diff::DiffOptions { include_unchanged: false, verbose, include_context, context_lines };
                diff::compute_file_diff(&files[0], &files[1], &opts)
                    .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(3); })
            } else {
                let ref_spec = ref_spec.unwrap();
                let (base_ref, head_ref) = git::parse_ref_spec(&ref_spec)
                    .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(3); });
                let opts = diff::DiffOptions { include_unchanged: false, verbose, include_context, context_lines };
                diff::compute_diff(&root, &base_ref, &head_ref, &opts)
                    .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(3); })
            };

            // Build DiffOutput
            let mut output = output::DiffOutput::from_result(&result, include_context, context_lines);
            if !summary {
                output.summary.summary_line = None;
            }

            // Caller analysis for breaking changes
            if !no_callers && output.summary.has_breaking {
                // Collect files touched by the diff
                let diff_files: std::collections::HashSet<String> = output.files.iter()
                    .map(|f| f.file.clone())
                    .collect();

                // Try to load index for caller queries
                match query::load(&root) {
                    Ok(idx) => {
                        let callers_fn = |name: &str| -> Vec<(String, u32, String)> {
                            idx.get_callers(name, None, 100)
                                .into_iter()
                                .map(|r| (r.file.clone(), r.line, r.caller.clone().unwrap_or_default()))
                                .collect()
                        };
                        output::enrich_breaking_with_callers(&mut output.breaking, &callers_fn, &diff_files);
                    }
                    Err(_) => {
                        // Index not available — skip caller analysis silently
                        if verbose {
                            eprintln!("note: run `sigil index` to enable caller impact analysis");
                        }
                    }
                }
            }

            // Compute groups if --group flag is set
            if group {
                output.groups = Some(grouping::compute_groups(&output));
            }

            // Dispatch to formatter
            if json {
                let out = std::io::stdout();
                let mut out = out.lock();
                if pretty {
                    serde_json::to_writer_pretty(&mut out, &output)
                } else {
                    serde_json::to_writer(&mut out, &output)
                }.expect("Failed to write JSON");
                println!();
            } else if markdown {
                let opts = markdown_formatter::MarkdownOptions {
                    use_emoji: !no_emoji,
                    show_context: include_context,
                };
                print!("{}", markdown_formatter::format_markdown(&output, &opts));
            } else {
                let opts = formatter::FormatOptions {
                    show_lines: lines,
                    show_context: include_context,
                    use_color: !no_color,
                };
                print!("{}", formatter::format_terminal_v2(&output, &opts));
            }

        }
        Cli::Explore { root, path, max_entries, json } => {
            let idx = query::load(&root)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            if json {
                let files = idx.explore_files_capped(path.as_deref(), max_entries);
                serde_json::to_writer_pretty(std::io::stdout(), &files.iter().map(|(dir, path, lang)| {
                    serde_json::json!({"directory": dir, "path": path, "language": lang})
                }).collect::<Vec<_>>()).ok();
                println!();
            } else {
                print!("{}", query::explore_text(&idx, path.as_deref(), max_entries));
            }
        }
        Cli::Search { query: q, root, scope, kind, path, limit, json } => {
            let idx = query::load(&root)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            // `scope` / `kind` are Vec<String> for CLI multi-arg compatibility;
            // first value wins for filtering (matches codeix's prior behavior).
            let scope_enum = scope
                .first()
                .map(|s| query::index::Scope::parse(s))
                .unwrap_or(query::index::Scope::All);
            let kind_filter = kind.first().map(|s| s.as_str());
            let results = idx.search(&q, scope_enum, kind_filter, path.as_deref(), limit as usize);
            if json {
                // Serialize as a flat list with a `type` discriminator so the
                // shape is stable for scripts/agents consuming the output.
                let json_hits: Vec<serde_json::Value> = results.iter().map(|h| match h {
                    query::index::SearchHit::Symbol(e) => serde_json::json!({
                        "type": "symbol",
                        "file": e.file,
                        "name": e.name,
                        "kind": e.kind,
                        "line": [e.line_start, e.line_end],
                        "parent": e.parent,
                    }),
                    query::index::SearchHit::File(f) => serde_json::json!({
                        "type": "file",
                        "path": f.path,
                        "lang": f.lang,
                        "entity_count": f.entity_count,
                    }),
                }).collect();
                serde_json::to_writer_pretty(std::io::stdout(), &json_hits).ok();
                println!();
            } else {
                print!("{}", query::format_search_hits(&results));
            }
        }
        Cli::Symbols { file, root, limit, json } => {
            let backend = sigil::query::Backend::load(&root)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            let symbols = backend.get_file_symbols(&file, None, limit as usize);
            let refs: Vec<&sigil::entity::Entity> = symbols.iter().collect();
            if json {
                serde_json::to_writer_pretty(std::io::stdout(), &symbols).ok();
                println!();
            } else {
                print!("{}", query::format_entities(&refs));
            }
        }
        Cli::Children { file, parent, root, limit, json } => {
            let backend = sigil::query::Backend::load(&root)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            let children = backend.get_children(&file, &parent, None, limit as usize);
            let refs: Vec<&sigil::entity::Entity> = children.iter().collect();
            if json {
                serde_json::to_writer_pretty(std::io::stdout(), &children).ok();
                println!();
            } else {
                print!("{}", query::format_entities(&refs));
            }
        }
        Cli::Callers { name, root, kind, limit, json } => {
            let backend = sigil::query::Backend::load(&root)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            let refs = backend.get_callers(&name, kind.as_deref(), limit as usize);
            let borrowed: Vec<&sigil::entity::Reference> = refs.iter().collect();
            if json {
                serde_json::to_writer_pretty(std::io::stdout(), &refs).ok();
                println!();
            } else {
                print!("{}", query::format_refs(&borrowed));
            }
        }
        Cli::Callees { caller, root, kind, limit, json } => {
            let backend = sigil::query::Backend::load(&root)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            let refs = backend.get_callees(&caller, kind.as_deref(), limit as usize);
            let borrowed: Vec<&sigil::entity::Reference> = refs.iter().collect();
            if json {
                serde_json::to_writer_pretty(std::io::stdout(), &refs).ok();
                println!();
            } else {
                print!("{}", query::format_refs(&borrowed));
            }
        }
        Cli::Blast { symbol, root, depth, format, pretty, exclude_tests } => {
            let idx = query::load(&root)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            let rank = sigil::map::load_rank_manifest(&root).unwrap_or_default();
            let Some(fmt) = sigil::blast::BlastFormat::parse(&format) else {
                eprintln!("error: unknown --format {}. expected markdown|json|agent", format);
                std::process::exit(1);
            };
            let opts = sigil::blast::BlastOptions {
                depth,
                format: fmt,
                exclude_tests,
            };
            let Some(report) = sigil::blast::run_blast(&idx, &rank, &symbol, &opts) else {
                eprintln!("no entity named `{}` (skipping imports)", symbol);
                eprintln!("hint: try `sigil search {}` to find similar symbols", symbol);
                std::process::exit(2);
            };
            match fmt {
                sigil::blast::BlastFormat::Markdown => print!("{}", sigil::blast::render_markdown(&report)),
                sigil::blast::BlastFormat::Json => println!("{}", sigil::blast::render_json(&report, pretty)),
                sigil::blast::BlastFormat::Agent => println!("{}", sigil::blast::render_agent(&report)),
            }
        }
        Cli::Duplicates { root, min_lines, max_group_size, format, pretty } => {
            let idx = query::load(&root)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            let Some(fmt) = sigil::duplicates::DuplicatesFormat::parse(&format) else {
                eprintln!("error: unknown --format {}. expected markdown|json", format);
                std::process::exit(1);
            };
            let opts = sigil::duplicates::DuplicatesOptions {
                min_lines,
                max_group_size,
                format: fmt,
                ..sigil::duplicates::DuplicatesOptions::default()
            };
            let report = sigil::duplicates::find_duplicates(&idx, &opts);
            match fmt {
                sigil::duplicates::DuplicatesFormat::Markdown => {
                    print!("{}", sigil::duplicates::render_markdown(&report));
                }
                sigil::duplicates::DuplicatesFormat::Json => {
                    println!("{}", sigil::duplicates::render_json(&report, pretty));
                }
            }
        }
        Cli::Benchmark { root, refspec, symbol, format, pretty, tokenizer } => {
            let Some(fmt) = sigil::benchmark::BenchmarkFormat::parse(&format) else {
                eprintln!("error: unknown --format {}. expected markdown|json", format);
                std::process::exit(1);
            };
            let Some(tok) = sigil::tokens::Tokenizer::parse(&tokenizer) else {
                eprintln!("error: unknown --tokenizer {}. expected proxy|cl100k_base|o200k_base|p50k_base", tokenizer);
                std::process::exit(1);
            };
            let opts = sigil::benchmark::BenchmarkOptions {
                refspec,
                symbol,
                format: fmt,
                tokenizer: tok,
            };
            match sigil::benchmark::run_benchmark(&root, &opts) {
                Ok(report) => match fmt {
                    sigil::benchmark::BenchmarkFormat::Markdown => {
                        print!("{}", sigil::benchmark::render_markdown(&report));
                    }
                    sigil::benchmark::BenchmarkFormat::Json => {
                        println!("{}", sigil::benchmark::render_json(&report, pretty));
                    }
                },
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Cli::Review { ref_spec, root, format, top_k, no_cochange } => {
            let Some(fmt) = sigil::review::ReviewFormat::parse(&format) else {
                eprintln!("error: unknown --format {}. expected `markdown` or `json`", format);
                std::process::exit(1);
            };
            let opts = sigil::review::ReviewOptions {
                format: fmt,
                top_k,
                show_cochange: !no_cochange,
                ..sigil::review::ReviewOptions::default()
            };
            match sigil::review::run_review(&root, &ref_spec, &opts) {
                Ok(rendered) => {
                    if matches!(fmt, sigil::review::ReviewFormat::Json) {
                        println!("{}", rendered);
                    } else {
                        print!("{}", rendered);
                    }
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Cli::Cochange { root, commits, min_support, max_files_per_commit, pretty } => {
            let cfg = sigil::cochange::CochangeConfig { commits, min_support, max_files_per_commit };
            match sigil::cochange::mine(&root, &cfg) {
                Ok(manifest) => {
                    if let Err(e) = sigil::cochange::save(&manifest, &root, pretty) {
                        eprintln!("error writing .sigil/cochange.json: {}", e);
                        std::process::exit(1);
                    }
                    eprintln!(
                        "scanned {} commits, {} files, {} pairs (min_support={})",
                        manifest.commits_scanned,
                        manifest.file_count,
                        manifest.pairs.len(),
                        manifest.min_support,
                    );
                }
                Err(e) => {
                    eprintln!("error: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Cli::Context { query: q, root, budget, depth, format, pretty, exclude_tests } => {
            let idx = query::load(&root)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            let Some(fmt) = sigil::context::ContextFormat::parse(&format) else {
                eprintln!("error: unknown --format {}. expected `markdown`, `agent`, or `json`", format);
                std::process::exit(1);
            };
            let opts = sigil::context::ContextOptions {
                budget,
                depth,
                format: fmt,
                exclude_tests,
            };
            let Some(ctx) = sigil::context::build_context(&idx, &q, &opts) else {
                eprintln!("no entity matches `{}`", q);
                eprintln!("hint: try `sigil search {}` to find similar symbols", q);
                std::process::exit(2);
            };
            match fmt {
                sigil::context::ContextFormat::Markdown => {
                    print!("{}", sigil::context::render_markdown(&ctx));
                }
                sigil::context::ContextFormat::Agent => {
                    println!("{}", sigil::context::render_agent_json(&ctx));
                }
                sigil::context::ContextFormat::Full => {
                    println!("{}", sigil::context::render_full_json(&ctx, pretty));
                }
            }
        }
        Cli::Map { root, tokens, focus, depth, format, write, exclude_tests, no_clusters } => {
            let idx = query::load(&root)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            let rank_manifest = sigil::map::load_rank_manifest(&root)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            if rank_manifest.file_rank.is_empty() {
                eprintln!("note: .sigil/rank.json not found — files will be listed without rank ordering");
                eprintln!("      run `sigil index` (rank is on by default) to populate it");
            }

            let opts = sigil::map::MapOptions {
                tokens,
                focus,
                depth,
                exclude_tests,
                clusters: !no_clusters,
                ..sigil::map::MapOptions::default()
            };
            let map = sigil::map::build_map(&idx, &rank_manifest, &opts);

            match format.as_str() {
                "json" => {
                    serde_json::to_writer_pretty(std::io::stdout(), &map).ok();
                    println!();
                }
                "markdown" | "md" => {
                    print!("{}", sigil::map::render_markdown(&map));
                }
                other => {
                    eprintln!("error: unknown --format {}. expected `markdown` or `json`", other);
                    std::process::exit(1);
                }
            }

            if write {
                sigil::map::write_sigil_map(&map, &root)
                    .unwrap_or_else(|e| { eprintln!("error writing .sigil/SIGIL_MAP.md: {}", e); std::process::exit(1); });
            }
        }
        Cli::Claude { action } => match action {
            InstallAction::Install { root } => {
                match sigil::install::claude::install(&root) {
                    Ok(steps) => {
                        for s in &steps {
                            eprintln!("claude: {:?}", s);
                        }
                        eprintln!("sigil Claude Code integration installed at {}", root.display());
                    }
                    Err(e) => {
                        eprintln!("error installing Claude integration: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            InstallAction::Uninstall { root } => {
                match sigil::install::claude::uninstall(&root) {
                    Ok(steps) => {
                        for s in &steps {
                            eprintln!("claude: {:?}", s);
                        }
                    }
                    Err(e) => {
                        eprintln!("error uninstalling Claude integration: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        },
        Cli::Cursor { action } => match action {
            InstallAction::Install { root } => match sigil::install::cursor::install(&root) {
                Ok(r) => eprintln!("cursor: {:?}", r),
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            },
            InstallAction::Uninstall { root } => match sigil::install::cursor::uninstall(&root) {
                Ok(true) => eprintln!("cursor: removed"),
                Ok(false) => eprintln!("cursor: nothing to remove"),
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            },
        },
        Cli::Codex { action } => match action {
            InstallAction::Install { root } => match sigil::install::codex::install(&root) {
                Ok(steps) => {
                    for s in &steps {
                        eprintln!("codex: {:?}", s);
                    }
                }
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            },
            InstallAction::Uninstall { root } => match sigil::install::codex::uninstall(&root) {
                Ok(steps) => {
                    for s in &steps {
                        eprintln!("codex: {:?}", s);
                    }
                }
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            },
        },
        Cli::Gemini { action } => match action {
            InstallAction::Install { root } => match sigil::install::gemini::install(&root) {
                Ok(steps) => {
                    for s in &steps {
                        eprintln!("gemini: {:?}", s);
                    }
                }
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            },
            InstallAction::Uninstall { root } => match sigil::install::gemini::uninstall(&root) {
                Ok(steps) => {
                    for s in &steps {
                        eprintln!("gemini: {:?}", s);
                    }
                }
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            },
        },
        Cli::Opencode { action } => match action {
            InstallAction::Install { root } => match sigil::install::opencode::install(&root) {
                Ok(steps) => {
                    for s in &steps {
                        eprintln!("opencode: {:?}", s);
                    }
                }
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            },
            InstallAction::Uninstall { root } => match sigil::install::opencode::uninstall(&root) {
                Ok(steps) => {
                    for s in &steps {
                        eprintln!("opencode: {:?}", s);
                    }
                }
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            },
        },
        Cli::Aider { action } => match action {
            InstallAction::Install { root } => match sigil::install::aider::install(&root) {
                Ok(r) => eprintln!("aider: {:?}", r),
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            },
            InstallAction::Uninstall { root } => match sigil::install::aider::uninstall(&root) {
                Ok(true) => eprintln!("aider: removed"),
                Ok(false) => eprintln!("aider: nothing to remove"),
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            },
        },
        Cli::Copilot { action } => match action {
            InstallAction::Install { root } => match sigil::install::copilot::install(&root) {
                Ok(r) => eprintln!("copilot: {:?}", r),
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            },
            InstallAction::Uninstall { root } => match sigil::install::copilot::uninstall(&root) {
                Ok(true) => eprintln!("copilot: removed"),
                Ok(false) => eprintln!("copilot: nothing to remove"),
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            },
        },
        Cli::Hook { action } => match action {
            InstallAction::Install { root } => match sigil::install::githook::install(&root) {
                Ok(steps) => {
                    for s in &steps {
                        eprintln!("hook {}: {:?}", s.name, s.result);
                    }
                }
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            },
            InstallAction::Uninstall { root } => match sigil::install::githook::uninstall(&root) {
                Ok(steps) => {
                    for s in &steps {
                        eprintln!("hook {}: {:?}", s.name, s.result);
                    }
                }
                Err(e) => { eprintln!("error: {}", e); std::process::exit(1); }
            },
        },
        Cli::Update => {
            eprintln!("Checking for updates...");
            let mut updater = axoupdater::AxoUpdater::new_for("sigil");
            let version: axoupdater::Version = env!("CARGO_PKG_VERSION").parse()
                .unwrap_or_else(|e| { eprintln!("error parsing version: {}", e); std::process::exit(1); });
            if let Err(e) = updater.set_current_version(version) {
                eprintln!("Update failed: {}", e);
                std::process::exit(1);
            }
            if let Err(e) = updater.load_receipt() {
                eprintln!("Update failed: {}", e);
                eprintln!("hint: self-update only works when sigil was installed via the official installer.");
                eprintln!("      Reinstall with: curl --proto '=https' --tlsv1.2 -LsSf https://github.com/gauravverma/sigil/releases/latest/download/sigil-installer.sh | sh");
                std::process::exit(1);
            }
            match updater.run_sync() {
                Ok(Some(result)) => {
                    eprintln!("Updated sigil to {}", result.new_version);
                }
                Ok(None) => {
                    eprintln!("Already on the latest version ({})", env!("CARGO_PKG_VERSION"));
                }
                Err(e) => {
                    eprintln!("Update failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }
}
