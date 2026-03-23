mod cache;
mod change_detail;
mod classifier;
mod diff;
mod diff_json;
mod entity;
mod formatter;
mod git;
mod grouping;
mod hasher;
mod index;
mod json_index;
mod toml_index;
mod yaml_index;
mod inline_diff;
mod markdown_formatter;
mod matcher;
mod meta;
mod output;
mod query;
mod signature;
mod writer;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "sigil", about = "Structural code fingerprinting", version)]
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

        /// Include code context in output (optionally specify lines of context, default 3)
        #[arg(long, default_missing_value = "3", num_args = 0..=1)]
        context: Option<usize>,

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
    /// Update sigil to the latest release
    Update,
}

fn main() {
    let cli = Cli::parse();

    match cli {
        Cli::Index { root, files, stdout, pretty, full, no_refs, verbose } => {
            let files_arg = if files.is_empty() { None } else { Some(files.as_slice()) };
            let result = index::build_index(&root, files_arg, full, !no_refs, verbose);

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
            } else {
                writer::write_to_files(&result.entities, &result.refs, &root, pretty)
                    .expect("Failed to write index");
                if verbose {
                    eprintln!(
                        "Wrote {} entities and {} refs to .sigil/",
                        result.entities.len(),
                        result.refs.len()
                    );
                }
            }
        }
        Cli::Diff { ref_spec, files, root, json, pretty, verbose, lines, context, markdown, no_emoji, no_color, no_callers, summary, group } => {
            // Handle --no-color
            if no_color {
                colored::control::set_override(false);
            }

            // Compute diff result
            let include_context = context.is_some();
            let context_lines = context.unwrap_or(3);
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
                match query::load_index(&root) {
                    Ok((_mt, db)) => {
                        let db = db.lock().unwrap();
                        let callers_fn = |name: &str| -> Vec<(String, u32, String)> {
                            db.get_callers(name, None, None, None, 100, 0)
                                .unwrap_or_default()
                                .into_iter()
                                .map(|r| (r.file.clone(), r.line.first().copied().unwrap_or(0) as u32, r.caller.clone().unwrap_or_default()))
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

            // Exit codes (ONLY for Diff command)
            let s = &output.summary;
            let exit_code = if s.has_breaking { 2 }
                else if s.added + s.removed + s.modified + s.moves + s.renamed > 0 { 1 }
                else { 0 };
            std::process::exit(exit_code);
        }
        Cli::Explore { root, path, max_entries, json } => {
            let (_mt, db) = query::load_index(&root)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            let db = db.lock().unwrap();
            let result = query::explore(&db, None, path.as_deref(), max_entries)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            if json {
                // For JSON, output the raw file listing
                let files = db.explore_files_capped("", path.as_deref(), None, max_entries)
                    .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
                serde_json::to_writer_pretty(std::io::stdout(), &files.iter().map(|(dir, path, lang)| {
                    serde_json::json!({"directory": dir, "path": path, "language": lang})
                }).collect::<Vec<_>>()).ok();
                println!();
            } else {
                print!("{}", result);
            }
        }
        Cli::Search { query: q, root, scope, kind, path, limit, json } => {
            let (_mt, db) = query::load_index(&root)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            let db = db.lock().unwrap();
            let results = db.search(&q, &scope, &kind, path.as_deref(), None, None, limit, 0)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            if json {
                serde_json::to_writer_pretty(std::io::stdout(), &results).ok();
                println!();
            } else {
                print!("{}", query::format_search_results(&results));
            }
        }
        Cli::Symbols { file, root, limit, json } => {
            let (_mt, db) = query::load_index(&root)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            let db = db.lock().unwrap();
            let symbols = db.get_file_symbols(&file, None, limit, 0)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            if json {
                serde_json::to_writer_pretty(std::io::stdout(), &symbols).ok();
                println!();
            } else {
                print!("{}", query::format_symbols(&symbols));
            }
        }
        Cli::Children { file, parent, root, limit, json } => {
            let (_mt, db) = query::load_index(&root)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            let db = db.lock().unwrap();
            let children = db.get_children(&file, &parent, None, limit, 0)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            if json {
                serde_json::to_writer_pretty(std::io::stdout(), &children).ok();
                println!();
            } else {
                print!("{}", query::format_symbols(&children));
            }
        }
        Cli::Callers { name, root, kind, limit, json } => {
            let (_mt, db) = query::load_index(&root)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            let db = db.lock().unwrap();
            let refs = db.get_callers(&name, kind.as_deref(), None, None, limit, 0)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            if json {
                serde_json::to_writer_pretty(std::io::stdout(), &refs).ok();
                println!();
            } else {
                print!("{}", query::format_references(&refs));
            }
        }
        Cli::Callees { caller, root, kind, limit, json } => {
            let (_mt, db) = query::load_index(&root)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            let db = db.lock().unwrap();
            let refs = db.get_callees(&caller, kind.as_deref(), None, None, limit, 0)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });
            if json {
                serde_json::to_writer_pretty(std::io::stdout(), &refs).ok();
                println!();
            } else {
                print!("{}", query::format_references(&refs));
            }
        }
        Cli::Update => {
            eprintln!("Checking for updates...");
            let mut updater = axoupdater::AxoUpdater::new_for("sigil");
            let version: axoupdater::Version = env!("CARGO_PKG_VERSION").parse()
                .unwrap_or_else(|e| { eprintln!("error parsing version: {}", e); std::process::exit(1); });
            if let Err(e) = updater.set_current_version(version) {
                eprintln!("Update failed: {}", e);
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
