use std::path::{Path, PathBuf};

use crate::cache::{self, Cache};
use crate::entity::{Entity, Reference};
use crate::hasher;
use crate::meta;
use crate::signature;

pub struct IndexResult {
    pub entities: Vec<Entity>,
    pub refs: Vec<Reference>,
}

/// Parse a single file's source code and return entities and references.
/// Used by the diff engine to parse file content fetched from git refs.
pub fn parse_single_file(
    source: &str,
    file_path: &str,
    language: &str,
) -> Result<(Vec<Entity>, Vec<Reference>), String> {
    if language == "json" {
        return crate::json_index::parse_json_file(source, file_path);
    }
    if language == "yaml" {
        return crate::yaml_index::parse_yaml_file(source, file_path);
    }
    if language == "toml" {
        return crate::toml_index::parse_toml_file(source, file_path);
    }
    if language == "markdown" {
        return crate::markdown_index::parse_markdown_file(source, file_path);
    }

    let (symbols, _texts, references) = crate::parser::treesitter::parse_file(
        source.as_bytes(), language, file_path
    ).map_err(|e| format!("parse error: {}", e))?;

    let mut entities = Vec::new();
    let mut refs = Vec::new();

    for sym in &symbols {
        let line_start = sym.line[0] as usize;
        let line_end = sym.line[1] as usize;

        let raw_text = hasher::extract_raw_bytes(source, line_start, line_end);
        let (sig, body_start) = signature::extract_signature(
            source, line_start, line_end, language
        );
        let meta_start = find_decorator_start(source, line_start, language);
        let markers = meta::extract_markers(source, meta_start, line_end, language);

        let (sig, sig_hash) = if is_import_kind(&sym.kind) {
            (None, None)
        } else {
            let sh = hasher::sig_hash(sig.as_deref());
            (sig, sh)
        };

        entities.push(Entity {
            file: file_path.to_string(),
            name: sym.name.clone(),
            kind: normalize_kind(&sym.kind),
            line_start: sym.line[0],
            line_end: sym.line[1],
            parent: sym.parent.clone(),
            sig,
            meta: markers,
            body_hash: hasher::body_hash(source, body_start, line_end),
            sig_hash,
            struct_hash: hasher::struct_hash(raw_text.as_bytes()),
        });
    }

    for refentry in &references {
        refs.push(Reference {
            file: file_path.to_string(),
            caller: refentry.caller.clone(),
            name: refentry.name.clone(),
            ref_kind: refentry.kind.clone(),
            line: refentry.line[0],
        });
    }

    Ok((entities, refs))
}

pub fn build_index(
    root: &Path,
    files: Option<&[PathBuf]>,
    full: bool,
    include_refs: bool,
    verbose: bool,
) -> IndexResult {
    let files_to_index = match files {
        Some(f) => f.to_vec(),
        None => discover_source_files(root),
    };

    let sigil_dir = root.join(".sigil");
    let prev_cache = if full { None } else { Cache::load(&sigil_dir) };
    let prev_entities = if full { Vec::new() } else { load_previous_entities(&sigil_dir) };
    let prev_refs = if full || !include_refs { Vec::new() } else { load_previous_refs(&sigil_dir) };

    let mut all_entities: Vec<Entity> = Vec::new();
    let mut all_refs: Vec<Reference> = Vec::new();
    let mut new_cache = Cache::new();
    let mut parsed_count = 0usize;
    let mut cached_count = 0usize;

    for filepath in &files_to_index {
        let relative_path = filepath.strip_prefix(root).unwrap_or(filepath);
        let relative_str = relative_path.to_string_lossy().replace('\\', "/");

        let source_bytes = match std::fs::read(filepath) {
            Ok(b) => b,
            Err(e) => {
                if verbose {
                    eprintln!("skip (read error): {}: {}", relative_str, e);
                }
                continue;
            }
        };

        let file_hash = cache::hash_file_contents(&source_bytes);

        // Check cache
        if let Some(ref cache) = prev_cache {
            if !cache.file_changed(&relative_str, &file_hash) {
                all_entities.extend(
                    prev_entities.iter().filter(|e| e.file == relative_str).cloned()
                );
                if include_refs {
                    all_refs.extend(
                        prev_refs.iter().filter(|r| r.file == relative_str).cloned()
                    );
                }
                new_cache.files.insert(relative_str, file_hash);
                cached_count += 1;
                continue;
            }
        }

        let ext = filepath.extension().and_then(|e| e.to_str()).unwrap_or("");
        let lang = if ext == "json" {
            "json"
        } else if ext == "yaml" || ext == "yml" {
            "yaml"
        } else if ext == "toml" {
            "toml"
        } else if ext == "md" || ext == "markdown" || ext == "mdx" {
            "markdown"
        } else {
            match crate::parser::languages::detect_language(ext) {
                Some(l) => l,
                None => {
                    if verbose {
                        eprintln!("skip (unsupported): {}", relative_str);
                    }
                    continue;
                }
            }
        };

        let source = match String::from_utf8(source_bytes) {
            Ok(s) => s,
            Err(_) => {
                if verbose {
                    eprintln!("skip (not UTF-8): {}", relative_str);
                }
                continue;
            }
        };

        match parse_single_file(&source, &relative_str, lang) {
            Ok((file_entities, file_refs)) => {
                let file_entity_count = file_entities.len();
                all_entities.extend(file_entities);
                if include_refs {
                    all_refs.extend(file_refs);
                }
                new_cache.files.insert(relative_str.clone(), file_hash);
                parsed_count += 1;
                if verbose {
                    eprintln!("indexed: {} ({} entities)", relative_str, file_entity_count);
                }
            }
            Err(e) => {
                if verbose {
                    eprintln!("skip (parse error): {}: {}", relative_str, e);
                }
                continue;
            }
        }
    }

    // Sort deterministically
    all_entities.sort_by(|a, b| {
        a.file.cmp(&b.file).then(a.line_start.cmp(&b.line_start))
    });
    all_refs.sort_by(|a, b| {
        a.file.cmp(&b.file).then(a.line.cmp(&b.line))
    });

    // Save cache
    std::fs::create_dir_all(&sigil_dir).ok();
    new_cache.save(&sigil_dir).ok();

    if verbose {
        eprintln!(
            "done: {} files parsed, {} cached, {} entities total",
            parsed_count, cached_count, all_entities.len()
        );
    }

    IndexResult { entities: all_entities, refs: all_refs }
}

/// Check if a codeix entity kind represents an import.
fn is_import_kind(kind: &str) -> bool {
    kind == "import" || kind == "use" || kind == "package"
}

/// Scan backwards from `line_start` to find all consecutive decorator/attribute lines.
fn find_decorator_start(source: &str, line_start: usize, lang: &str) -> usize {
    let all_lines: Vec<&str> = source.lines().collect();
    let mut start = line_start;
    while start > 1 {
        let prev_line = all_lines[start - 2].trim();
        let is_decorator = match lang {
            "python" => prev_line.starts_with('@'),
            "rust" => prev_line.starts_with("#[") || prev_line.starts_with("#!["),
            "java" | "csharp" => prev_line.starts_with('@'),
            "typescript" | "javascript" | "tsx" => prev_line.starts_with('@'),
            _ => false,
        };
        if is_decorator {
            start -= 1;
        } else {
            break;
        }
    }
    start
}

fn normalize_kind(kind: &str) -> String {
    match kind {
        "trait_impl" => "impl".to_string(),
        "field" => "property".to_string(),
        "procedure" => "function".to_string(),
        other => other.to_string(),
    }
}

fn discover_source_files(root: &Path) -> Vec<PathBuf> {
    ignore::WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .build()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
        .filter(|entry| {
            entry.path().extension()
                .and_then(|e| e.to_str())
                .map(|ext| {
                    ext == "md" || ext == "markdown" || ext == "mdx"
                        || ext == "json" || ext == "yaml" || ext == "yml" || ext == "toml"
                        || crate::parser::languages::detect_language(ext).is_some()
                })
                .unwrap_or(false)
        })
        .map(|entry| entry.into_path())
        .collect()
}

fn load_previous_entities(sigil_dir: &Path) -> Vec<Entity> {
    let path = sigil_dir.join("entities.jsonl");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    content.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

fn load_previous_refs(sigil_dir: &Path) -> Vec<Reference> {
    let path = sigil_dir.join("refs.jsonl");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    content.lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_kind_mappings() {
        assert_eq!(normalize_kind("trait_impl"), "impl");
        assert_eq!(normalize_kind("field"), "property");
        assert_eq!(normalize_kind("procedure"), "function");
        assert_eq!(normalize_kind("function"), "function");
        assert_eq!(normalize_kind("class"), "class");
    }

    #[test]
    fn load_previous_entities_missing_file() {
        let entities = load_previous_entities(Path::new("/nonexistent"));
        assert!(entities.is_empty());
    }

    #[test]
    fn load_previous_entities_roundtrip() {
        let dir = std::env::temp_dir().join("sigil_index_test");
        std::fs::create_dir_all(&dir).unwrap();
        let entity_json = r#"{"file":"a.py","name":"foo","kind":"function","line_start":1,"line_end":2,"parent":null,"sig":"def foo():","meta":null,"body_hash":"abc","sig_hash":"def","struct_hash":"ghi"}"#;
        std::fs::write(dir.join("entities.jsonl"), format!("{}\n", entity_json)).unwrap();
        let entities = load_previous_entities(&dir);
        assert_eq!(entities.len(), 1);
        assert_eq!(entities[0].name, "foo");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_previous_refs_missing_file() {
        let refs = load_previous_refs(Path::new("/nonexistent"));
        assert!(refs.is_empty());
    }

    #[test]
    fn parse_single_file_python() {
        let source = "def foo(x: int) -> bool:\n    return True\n";
        let (entities, _refs) = parse_single_file(source, "test.py", "python").unwrap();
        assert!(!entities.is_empty());
        let names: Vec<&str> = entities.iter().map(|e| e.name.as_str()).collect();
        assert!(names.contains(&"foo"));
    }

    #[test]
    fn parse_single_file_rust() {
        let source = "pub fn bar(x: i32) -> bool {\n    true\n}\n";
        let (entities, _refs) = parse_single_file(source, "test.rs", "rust").unwrap();
        assert!(!entities.is_empty());
    }

    #[test]
    fn parse_single_file_empty_source() {
        let (entities, refs) = parse_single_file("", "test.py", "python").unwrap();
        assert!(entities.is_empty());
        assert!(refs.is_empty());
    }
}
