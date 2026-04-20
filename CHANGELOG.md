# Changelog

All notable changes to sigil are documented here. Format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versions follow
[SemVer](https://semver.org/spec/v2.0.0.html).

## [Unreleased] — Phase 0: in-house parsing + code intelligence

Sigil's last external runtime dependency on `codeix` is gone. Parsing and
all code-intelligence queries (`search`, `symbols`, `children`, `callers`,
`callees`, `explore`) now run against sigil-owned code. No behavior
changes visible to end users — the CLI output format is preserved.

### Added

- `src/parser/` — vendored tree-sitter extractors for 11 languages
  (C, C++, C#, Go, Java, JavaScript, Python, Ruby, Rust, TypeScript,
  Markdown) plus Vue/Svelte SFC support. Originally forked from codeix
  v0.5.0 under Apache-2.0; see `src/parser/NOTICE` for attribution.
  Feature-gated per language via `lang-<name>` flags.
- `src/query/index.rs` — in-house `Index` struct: loads
  `.sigil/entities.jsonl` + `refs.jsonl`, precomputes five lookup maps,
  exposes `get_callers`, `get_callees`, `get_file_symbols`,
  `get_children`, `search`, `explore_dir_overview`,
  `explore_files_capped`, `list_projects`.
- `Scope` enum (`All`, `Symbols`, `Files`) for `sigil search`; parses
  codeix-compatible scope strings.
- 23 unit tests for the query layer covering filter/limit semantics,
  substring/case matching, directory grouping, and parser fallbacks.

### Changed

- **Breaking (internal; no CLI change): `src/query.rs` replaced by a
  `src/query/` module.** `load_index()` → `load()`, returning an owned
  `Index` instead of `Arc<Mutex<SearchDb>>`. The mutex dance in
  `main.rs` is gone.
- `sigil search` result format: `SearchHit::Symbol(&Entity)` /
  `SearchHit::File(FileHit)` instead of codeix's three-variant
  `SearchResult`. JSON output now uses a `type` discriminator
  (`"symbol"` / `"file"`). Text-block hits dropped — sigil doesn't
  index docstring/comment bodies today; deferred until a clear
  consumer surfaces.
- `sigil explore` queries run against the in-house Index; output shape
  unchanged.
- Module reference in `CLAUDE.md` updated to reflect in-house ownership.

### Removed

- `codeix` git dependency (`github.com/montanetech/codeix`). Removed
  from `Cargo.toml` and no longer appears in `Cargo.lock`.
- `.codeindex/` directory — sigil no longer generates it. Added to
  `.gitignore` for repos that still have it around from an older
  install.
- Transitive deps pulled in by codeix (rusqlite, tokio, notify,
  tracing, rmcp, walkdir, …). Binary size drops by several MB on
  release builds.

### Fixed

- `.gitignore` had a concatenated typo (`.codeindexpython/.venv/`)
  that ignored neither `.codeindex/` nor `python/.venv/`. Split into
  two correct entries and added Phase 0.5 DuckDB reservations.
- `src/output.rs`: internal comment referenced "the codeix index";
  now correctly says "the sigil index".

### Python bindings (`sigil-diff` on PyPI)

- `python/pyproject.toml`: add `readme = "README.md"`, author,
  project URLs (Homepage / Repository / Issues), 14 trove
  classifiers (Python 3.8–3.13, Rust, MIT, OS support, topic
  taxonomy), plus `tree-sitter` and `ast` keywords. The next release
  will publish a complete PyPI project page instead of a blank one.
- All four bindings (`diff_json`, `diff_files`, `diff_refs`,
  `index_json`) verified end-to-end against the in-house code path.
  No Python-side code changes needed — the crate depends on
  `sigil_core` by path, which picked up the decodeix work
  transparently.

## [0.2.4] — 2026-04-16

- CI: use `--find-interpreter` for Linux manylinux builds in the
  Python publish workflow.

## [0.2.3] — 2026-04-16

- ci: add GitHub Actions workflow for publishing Python wheels to
  PyPI.
- docs: add Python SDK documentation, rename package to `sigil-diff`.
- feat: add Python bindings via PyO3 — `import sigil;
  sigil.diff_json(old, new)`.

---

For versions 0.2.2 and earlier see `git log`.
