mod cache;
mod change_detail;
mod classifier;
mod diff;
mod diff_json;
mod entity;
mod formatter;
mod git;
mod hasher;
mod index;
mod json_index;
mod toml_index;
mod yaml_index;
mod inline_diff;
mod matcher;
mod meta;
mod query;
mod signature;
mod writer;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "sigil", about = "Structural code fingerprinting")]
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
    /// Structural diff between two git refs
    Diff {
        /// Ref spec: HEAD~1, main..HEAD, abc123..def456
        #[arg()]
        ref_spec: String,

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
        Cli::Diff { ref_spec, root, json, pretty, verbose } => {
            let (base_ref, head_ref) = git::parse_ref_spec(&ref_spec)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });

            let opts = diff::DiffOptions { include_unchanged: false, verbose };
            let result = diff::compute_diff(&root, &base_ref, &head_ref, &opts)
                .unwrap_or_else(|e| { eprintln!("error: {}", e); std::process::exit(1); });

            if json {
                let out = std::io::stdout();
                let mut out = out.lock();
                if pretty {
                    serde_json::to_writer_pretty(&mut out, &result)
                } else {
                    serde_json::to_writer(&mut out, &result)
                }.expect("Failed to write JSON");
                println!();
            } else {
                print!("{}", formatter::format_terminal(&result));
            }
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
    }
}
