use serde::{Serialize, Deserialize};

// Eq is dropped because `rank: Option<f64>` can't implement Eq. Callers that
// need equality still have `PartialEq`; callers that want hashability on
// rank-less Entities can compare the struct_hash field directly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entity {
    pub file: String,
    pub name: String,
    pub kind: String,
    pub line_start: u32,
    pub line_end: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sig: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sig_hash: Option<String>,
    pub struct_hash: String,

    // Phase 1 additions — all Option<T> + skip_serializing_if so old v1 JSONL
    // round-trips as None and newer writes add the fields only when populated.
    // Populated after a rank pass (src/rank.rs); None when the caller opted
    // out via `--no-rank` or when the parser didn't emit visibility info.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blast_radius: Option<BlastRadius>,
}

/// Downstream impact summary for a single entity. Used by `sigil review`,
/// `sigil map`, `sigil blast`, and the Phase 1 ranking pipeline.
///
/// `direct_callers`   — number of reference rows targeting this entity's name.
/// `direct_files`     — distinct files those references live in.
/// `transitive_callers` — BFS over the reverse-call graph, capped at depth
/// 3 to avoid cycles and runaway cost on highly-connected symbols.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlastRadius {
    pub direct_callers: u32,
    pub direct_files: u32,
    pub transitive_callers: u32,
}

/// Path-based heuristic for "is this entity part of test code?" Covers the
/// common conventions across languages sigil parses today:
///
///   Rust   — `tests/*`, `**/tests/*`, `*_test.rs`
///   Python — `test_*.py`, `*_test.py`, `tests/**/*.py`
///   JS/TS  — `*.test.{js,ts}`, `*.spec.{js,ts}`, `__tests__/**`
///   Go     — `*_test.go`
///   Java   — `*Test.java`, `src/test/**`
///
/// Deliberately pragmatic — we don't look inside the AST for `#[cfg(test)]`
/// or `@pytest.fixture`; those would need a parser-side change. This covers
/// the 90% case for `--exclude-tests` filtering on `map`/`context`/`blast`.
pub fn is_test_path(file: &str) -> bool {
    let file = file.replace('\\', "/");
    let fname = file.rsplit('/').next().unwrap_or(&file);

    // Directory-based signals
    if file.starts_with("tests/")
        || file.starts_with("test/")
        || file.contains("/tests/")
        || file.contains("/test/")
        || file.contains("/__tests__/")
        || file.contains("/src/test/")
    {
        return true;
    }

    // Suffix-based signals
    let test_suffixes = [
        "_test.rs",
        "_test.go",
        "_test.py",
        "_test.ts",
        "_test.js",
        ".test.ts",
        ".test.tsx",
        ".test.js",
        ".test.jsx",
        ".spec.ts",
        ".spec.tsx",
        ".spec.js",
        ".spec.jsx",
        "Test.java",
        "Tests.java",
        "_spec.rb",
    ];
    if test_suffixes.iter().any(|s| fname.ends_with(s)) {
        return true;
    }

    // Prefix-based signals (Python pytest, Ruby)
    if (fname.starts_with("test_") && (fname.ends_with(".py") || fname.ends_with(".rs")))
        || fname.starts_with("Test") && fname.ends_with(".java")
    {
        return true;
    }

    false
}

#[cfg(test)]
mod is_test_path_tests {
    use super::is_test_path;

    #[test]
    fn detects_rust_test_conventions() {
        assert!(is_test_path("tests/integration.rs"));
        assert!(is_test_path("src/foo/tests/fixture.rs"));
        assert!(is_test_path("src/parser_test.rs"));
        assert!(is_test_path("src/test_utils.rs"));
        assert!(!is_test_path("src/parser.rs"));
        assert!(!is_test_path("src/entity.rs"));
    }

    #[test]
    fn detects_python_test_conventions() {
        assert!(is_test_path("tests/test_core.py"));
        assert!(is_test_path("tests/core_test.py"));
        assert!(is_test_path("src/test_utils.py"));
        assert!(!is_test_path("src/core.py"));
    }

    #[test]
    fn detects_js_ts_test_conventions() {
        assert!(is_test_path("src/foo.test.ts"));
        assert!(is_test_path("src/foo.spec.js"));
        assert!(is_test_path("src/__tests__/foo.ts"));
        assert!(is_test_path("packages/api/tests/handler.test.tsx"));
        assert!(!is_test_path("src/foo.ts"));
    }

    #[test]
    fn detects_go_test_convention() {
        assert!(is_test_path("pkg/handler_test.go"));
        assert!(!is_test_path("pkg/handler.go"));
    }

    #[test]
    fn detects_java_test_conventions() {
        assert!(is_test_path("src/main/java/FooTest.java"));
        assert!(is_test_path("src/test/java/FooTest.java"));
        assert!(is_test_path("com/example/TestFoo.java"));
        assert!(!is_test_path("src/main/java/Foo.java"));
    }

    #[test]
    fn does_not_false_positive_on_words_containing_test() {
        // "test" as a substring inside a non-test-conventional path.
        assert!(!is_test_path("src/attestation.rs"));
        assert!(!is_test_path("src/latest.py"));
        assert!(!is_test_path("contest/engine.ts"));
    }

    #[test]
    fn handles_windows_style_paths() {
        assert!(is_test_path("tests\\integration.rs"));
        assert!(is_test_path("src\\foo\\__tests__\\bar.ts"));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Reference {
    pub file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller: Option<String>,
    pub name: String,
    pub ref_kind: String,
    pub line: u32,
}
