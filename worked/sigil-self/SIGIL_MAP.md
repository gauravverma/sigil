# Sigil Map

88 files, 2635 entities, 12887 refs · sigil 0.2.4
token budget: 3000 · estimated: 2837

## Top files by impact

### src/entity.rs — rank 0.0676 (rust)
- struct **Entity** [public] — blast 18f/50c/244t
  `pub struct Entity`
- struct **Reference** [public] — blast 11f/28c/155t
  `pub struct Reference`
- struct **BlastRadius** [public] — blast 5f/8c/222t
  `pub struct BlastRadius`

### src/diff_json.rs — rank 0.0430 (rust)
- struct **EntityDiff** — blast 5f/13c/58t
  `pub struct EntityDiff`
- struct **DiffResult** — blast 3f/9c/38t
  `pub struct DiffResult`
- impl **DiffResult** — blast 3f/9c/38t
  `impl DiffResult`
- enum **ChangeKind** — blast 3f/5c/52t
  `pub enum ChangeKind`
- struct **CrossFilePattern** — blast 2f/3c/39t
  `pub struct CrossFilePattern`

### src/output.rs — rank 0.0384 (rust)
- struct **DiffOutput** [public] — blast 3f/11c/45t
  `pub struct DiffOutput`
- struct **OutputEntity** [public] — blast 3f/8c/44t
  `pub struct OutputEntity`
- struct **FileSection** [public] — blast 3f/6c/50t
  `pub struct FileSection`
- struct **OutputPattern** [public] — blast 3f/5c/49t
  `pub struct OutputPattern`
- function **make_entity** [private] — blast 3f/13c/10t
  `fn make_entity(file: &str, name: &str, kind: &str, line_start: u32, line_end: u32) -> Entity`

### src/parser/helpers.rs — rank 0.0377 (rust)
- function **find_child_by_field** — blast 10f/236c/50t
  `pub fn find_child_by_field<'a>(node: Node<'a>, field: &str) -> Option<Node<'a>>`
- function **node_text** — blast 14f/211c/218t
  `pub fn node_text(node: Node, source: &[u8]) -> String`
- function **node_line_range** — blast 13f/118c/190t
  `pub fn node_line_range(node: Node) -> [u32; 2]`
- function **push_symbol** — blast 11f/96c/43t
  `pub fn push_symbol(symbols: &mut Vec<SymbolEntry>, file_path: &str, name: String, kind: &str, line: [u32; 2], parent: Option<&str>, tokens: Option<String>, alias: Option<String>, visibility: Option<String>)`
- function **extract_tokens** — blast 10f/26c/16t
  `pub fn extract_tokens(node: Node, source: &[u8]) -> Option<String>`

### src/parser/format.rs — rank 0.0352 (rust)
- struct **SymbolEntry** — blast 13f/118c/188t
  `pub struct SymbolEntry`
- struct **ReferenceEntry** — blast 11f/92c/163t
  `pub struct ReferenceEntry`
- struct **TextEntry** — blast 13f/64c/174t
  `pub struct TextEntry`

### src/inline_diff.rs — rank 0.0330 (rust)
- struct **DiffLine** — blast 3f/4c/67t
  `pub struct DiffLine`
- function **compute_inline_diff** — blast 1f/3c/3t
  `pub fn compute_inline_diff(old_text: &str, new_text: &str) -> Option<Vec<DiffLine>>`
- function **compute_inline_diff_hunked** — blast 1f/3c/3t
  `pub fn compute_inline_diff_hunked(old_text: &str, new_text: &str, context_lines: usize) -> Option<Vec<DiffLine>>`
- enum **DiffLineKind** — blast 1f/1c/33t
  `pub enum DiffLineKind`
- function **extract_entity_text** — blast 0f/0c/0t
  `pub fn extract_entity_text(source: &str, line_start: u32, line_end: u32) -> String`

### src/change_detail.rs — rank 0.0301 (rust)
- function **extract_change_details** — blast 1f/9c/9t
  `pub fn extract_change_details(lines: &[DiffLine]) -> Vec<ChangeDetail>`
- struct **ChangeDetail** — blast 2f/3c/60t
  `pub struct ChangeDetail`
- function **is_comment_line** — blast 1f/4c/11t
  `fn is_comment_line(line: &str) -> bool`
- enum **DetailKind** — blast 1f/2c/27t
  `pub enum DetailKind`
- function **pair_similar_lines** — blast 1f/2c/11t
  `pub fn pair_similar_lines(removed: &[String], added: &[String]) -> Vec<(String, String)>`

### src/rank.rs — rank 0.0292 (rust)
- struct **RankManifest** [public] — blast 4f/7c/26t
  `pub struct RankManifest`
- function **rank** [public] — blast 1f/13c/12t
  `pub fn rank(entities: &[Entity], references: &[Reference]) -> RankedIndex`
- function **ent** [private] — blast 3f/11c/8t
  `fn ent(file: &str, name: &str, kind: &str) -> Entity`
- impl **RankManifest** [private] — blast 4f/7c/26t
  `impl RankManifest`
- struct **RankedIndex** [public] — blast 1f/4c/19t
  `pub struct RankedIndex`

### src/cochange.rs — rank 0.0256 (rust)
- struct **CochangeManifest** [public] — blast 2f/6c/15t
  `pub struct CochangeManifest`
- struct **CochangeConfig** [public] — blast 1f/2c/4t
  `pub struct CochangeConfig`
- function **load** [public] — blast 1f/2c/2t
  `pub fn load(root: &Path) -> Result<CochangeManifest>`
- struct **Pair** [public] — blast 1f/1c/16t
  `pub struct Pair`
- function **save** [public] — blast 1f/1c/1t
  `pub fn save(manifest: &CochangeManifest, root: &Path, pretty: bool) -> Result<()>`

### src/query/index.rs — rank 0.0253 (rust)
- struct **Index** [public] — blast 7f/14c/54t
  `pub struct Index`
- impl **Index** [private] — blast 7f/14c/54t
  `impl Index`
- function **ent** [private] — blast 3f/11c/8t
  `fn ent(file: &str, name: &str, kind: &str) -> Entity`
- enum **SearchHit** [public] — blast 2f/2c/2t
  `pub enum SearchHit<'a>`
- function **lang_for** [private] — blast 2f/4c/14t
  `fn lang_for(file: &str) -> Option<&'static str>`

### src/grouping.rs — rank 0.0232 (rust)
- function **make_output** — blast 2f/21c/21t
  `fn make_output() -> DiffOutput`
- function **compute_groups** — blast 1f/2c/2t
  `pub fn compute_groups(output: &DiffOutput) -> Vec<ChangeGroup>`
- struct **GroupedEntity** — blast 1f/1c/4t
  `pub struct GroupedEntity`
- struct **ChangeGroup** — blast 1f/1c/3t
  `pub struct ChangeGroup`
- module **tests** — blast 0f/0c/0t
  `mod tests`

### src/map.rs — rank 0.0217 (rust)
- function **render_markdown** [public] — blast 6f/16c/16t
  `pub fn render_markdown(m: &Map) -> String`
- function **build_map** [public] — blast 1f/11c/10t
  `pub fn build_map(idx: &Index, rank: &RankManifest, opts: &MapOptions) -> Map`
- function **ent** [private] — blast 3f/11c/8t
  `fn ent(file: &str, name: &str, kind: &str, blast_files: u32, blast_callers: u32) -> Entity`
- function **estimate_tokens** [private] — blast 2f/7c/19t
  `fn estimate_tokens(s: &str) -> usize`
- function **manifest** [private] — blast 1f/11c/11t
  `fn manifest(file_rank: &[(&str, f64)]) -> RankManifest`

### src/cache.rs — rank 0.0215 (rust)
- function **hash_file_contents** — blast 1f/4c/2t
  `pub fn hash_file_contents(contents: &[u8]) -> String`
- struct **Cache** — blast 0f/0c/0t
  `pub struct Cache`
- impl **Cache** — blast 0f/0c/0t
  `impl Cache`
- method **Cache.new** (in Cache) — blast 0f/0c/0t
  `pub fn new() -> Self`
- method **Cache.load** (in Cache) — blast 0f/0c/0t
  `pub fn load(sigil_dir: &Path) -> Option<Self>`

### src/lib.rs — rank 0.0205 (rust)
- module **install** [public] — blast 8f/39c/28t
  `pub mod install;`
- module **entity** [public] — blast 2f/32c/16t
  `pub mod entity;`
- module **rank** [public] — blast 1f/13c/12t
  `pub mod rank;`
- module **benchmark** [public] — blast 0f/0c/0t
  `pub mod benchmark;`
- module **blast** [public] — blast 0f/0c/0t
  `pub mod blast;`

### tests/markdown_integration.rs — rank 0.0177 (rust)
- function **run_sigil_index** — blast 2f/11c/10t
  `fn run_sigil_index(fixture_dir: &str, extra_args: &[&str]) -> String`
- function **fixture_path** — blast 2f/11c/10t
  `fn fixture_path() -> String`
- function **indexes_markdown_fixture** — blast 0f/0c/0t
  `fn indexes_markdown_fixture()`
- function **diff_markdown_files** — blast 0f/0c/0t
  `fn diff_markdown_files()`

### src/install/claude.rs — rank 0.0171 (rust)
- function **install** [public] — blast 8f/39c/28t
  `pub fn install(root: &Path) -> Result<Vec<InstallStep>>`
- function **uninstall** [public] — blast 6f/14c/12t
  `pub fn uninstall(root: &Path) -> Result<Vec<UninstallStep>>`
- function **tmpdir** [private] — blast 6f/27c/23t
  `fn tmpdir(name: &str) -> std::path::PathBuf`
- enum **InstallStep** [public] — blast 4f/4c/29t
  `pub enum InstallStep`
- enum **UninstallStep** [public] — blast 4f/4c/13t
  `pub enum UninstallStep`

### src/install/gemini.rs — rank 0.0170 (rust)
- function **install** [public] — blast 8f/39c/28t
  `pub fn install(root: &Path) -> Result<Vec<InstallStep>>`
- function **uninstall** [public] — blast 6f/14c/12t
  `pub fn uninstall(root: &Path) -> Result<Vec<UninstallStep>>`
- function **tmpdir** [private] — blast 6f/27c/23t
  `fn tmpdir(name: &str) -> std::path::PathBuf`
- enum **InstallStep** [public] — blast 4f/4c/29t
  `pub enum InstallStep`
- enum **UninstallStep** [public] — blast 4f/4c/13t
  `pub enum UninstallStep`

### src/install/codex.rs — rank 0.0170 (rust)
- function **install** [public] — blast 8f/39c/28t
  `pub fn install(root: &Path) -> Result<Vec<InstallStep>>`
- function **uninstall** [public] — blast 6f/14c/12t
  `pub fn uninstall(root: &Path) -> Result<Vec<UninstallStep>>`
- function **tmpdir** [private] — blast 6f/27c/23t
  `fn tmpdir(name: &str) -> std::path::PathBuf`
- enum **InstallStep** [public] — blast 4f/4c/29t
  `pub enum InstallStep`
- enum **UninstallStep** [public] — blast 4f/4c/13t
  `pub enum UninstallStep`

### src/parser/markdown.rs — rank 0.0161 (rust)
- function **parse_and_extract** — blast 1f/14c/14t
  `pub fn parse_and_extract(source: &[u8], file_path: &str) -> Result<(Vec<SymbolEntry>, Vec<TextEntry>)>`
- function **walk_blocks** — blast 1f/2c/15t
  `fn walk_blocks(node: Node, source: &[u8], file_path: &str, heading_stack: &mut Vec<(u32, String)>, symbols: &mut Vec<SymbolEntry>, texts: &mut Vec<TextEntry>)`
- function **get_heading_text** — blast 1f/2c/4t
  `fn get_heading_text(node: Node, source: &[u8]) -> String`
- function **compute_qualified_name** — blast 1f/2c/4t
  `fn compute_qualified_name(heading_stack: &mut Vec<(u32, String)>, level: u32, name: &str) -> (String, Option<String>)`
- function **extract_atx_heading** — blast 1f/1c/16t
  `fn extract_atx_heading(node: Node, source: &[u8], file_path: &str, heading_stack: &mut Vec<(u32, String)>, symbols: &mut Vec<SymbolEntry>)`

### tests/integration.rs — rank 0.0159 (rust)
- function **run_sigil_index** — blast 2f/11c/10t
  `fn run_sigil_index(fixture_dir: &str, extra_args: &[&str]) -> String`
- function **fixture_path** — blast 2f/11c/10t
  `fn fixture_path() -> String`
- function **indexes_python_fixture** — blast 0f/0c/0t
  `fn indexes_python_fixture()`
- function **indexes_rust_fixture** — blast 0f/0c/0t
  `fn indexes_rust_fixture()`
- function **indexes_go_fixture** — blast 0f/0c/0t
  `fn indexes_go_fixture()`

### src/install/opencode.rs — rank 0.0159 (rust)
- function **install** [public] — blast 8f/39c/28t
  `pub fn install(root: &Path) -> Result<Vec<InstallStep>>`
- function **uninstall** [public] — blast 6f/14c/12t
  `pub fn uninstall(root: &Path) -> Result<Vec<UninstallStep>>`
- function **tmpdir** [private] — blast 6f/27c/23t
  `fn tmpdir(name: &str) -> std::path::PathBuf`
- enum **InstallStep** [public] — blast 4f/4c/29t
  `pub enum InstallStep`
- enum **UninstallStep** [public] — blast 4f/4c/13t
  `pub enum UninstallStep`


_Truncated: 67 more file(s) below budget. Increase `--tokens` or scope with `--focus` to see more._
