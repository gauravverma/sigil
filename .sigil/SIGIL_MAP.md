# Sigil Map

72 files, 2152 entities, 10169 refs · sigil 0.2.4
token budget: 2000 · estimated: 1909

## Top files by impact

### src/entity.rs — rank 0.0798 (rust)
- struct **Entity** [public] — blast 14f/43c/180t
  `pub struct Entity`
- struct **Reference** [public] — blast 9f/24c/101t
  `pub struct Reference`
- struct **BlastRadius** [public] — blast 2f/3c/168t
  `pub struct BlastRadius`

### src/parser/helpers.rs — rank 0.0556 (rust)
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

### src/rank.rs — rank 0.0545 (rust)
- function **rank** [public] — blast 1f/13c/12t
  `pub fn rank(entities: &[Entity], references: &[Reference]) -> RankedIndex`
- struct **RankedIndex** [public] — blast 1f/4c/19t
  `pub struct RankedIndex`
- function **rank_with_config** [public] — blast 1f/4c/15t
  `pub fn rank_with_config(entities: &[Entity], references: &[Reference], cfg: &RankConfig) -> RankedIndex`
- struct **RankConfig** [public] — blast 1f/3c/18t
  `pub struct RankConfig`
- function **ent** [private] — blast 1f/7c/4t
  `fn ent(file: &str, name: &str, kind: &str) -> Entity`

### src/diff_json.rs — rank 0.0512 (rust)
- struct **EntityDiff** — blast 4f/10c/38t
  `pub struct EntityDiff`
- struct **DiffResult** — blast 2f/5c/20t
  `pub struct DiffResult`
- impl **DiffResult** — blast 2f/5c/20t
  `impl DiffResult`
- enum **ChangeKind** — blast 2f/3c/41t
  `pub enum ChangeKind`
- struct **CrossFilePattern** — blast 2f/3c/22t
  `pub struct CrossFilePattern`

### src/parser/format.rs — rank 0.0504 (rust)
- struct **SymbolEntry** — blast 13f/118c/188t
  `pub struct SymbolEntry`
- struct **ReferenceEntry** — blast 11f/92c/163t
  `pub struct ReferenceEntry`
- struct **TextEntry** — blast 13f/64c/174t
  `pub struct TextEntry`

### src/output.rs — rank 0.0503 (rust)
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

### src/inline_diff.rs — rank 0.0405 (rust)
- struct **DiffLine** — blast 3f/4c/57t
  `pub struct DiffLine`
- function **compute_inline_diff** — blast 1f/3c/3t
  `pub fn compute_inline_diff(old_text: &str, new_text: &str) -> Option<Vec<DiffLine>>`
- function **compute_inline_diff_hunked** — blast 1f/3c/3t
  `pub fn compute_inline_diff_hunked(old_text: &str, new_text: &str, context_lines: usize) -> Option<Vec<DiffLine>>`
- enum **DiffLineKind** — blast 1f/1c/30t
  `pub enum DiffLineKind`
- function **extract_entity_text** — blast 0f/0c/0t
  `pub fn extract_entity_text(source: &str, line_start: u32, line_end: u32) -> String`

### src/change_detail.rs — rank 0.0369 (rust)
- function **extract_change_details** — blast 1f/9c/9t
  `pub fn extract_change_details(lines: &[DiffLine]) -> Vec<ChangeDetail>`
- struct **ChangeDetail** — blast 2f/3c/50t
  `pub struct ChangeDetail`
- function **is_comment_line** — blast 1f/4c/11t
  `fn is_comment_line(line: &str) -> bool`
- enum **DetailKind** — blast 1f/2c/24t
  `pub enum DetailKind`
- function **pair_similar_lines** — blast 1f/2c/11t
  `pub fn pair_similar_lines(removed: &[String], added: &[String]) -> Vec<(String, String)>`

### src/lib.rs — rank 0.0299 (rust)
- module **entity** [public] — blast 2f/32c/16t
  `pub mod entity;`
- module **rank** [public] — blast 1f/13c/12t
  `pub mod rank;`
- module **cache** [public] — blast 0f/0c/0t
  `pub mod cache;`
- module **change_detail** [public] — blast 0f/0c/0t
  `pub mod change_detail;`
- module **classifier** [public] — blast 0f/0c/0t
  `pub mod classifier;`

### src/grouping.rs — rank 0.0291 (rust)
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

### src/cache.rs — rank 0.0281 (rust)
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

### src/parser/markdown.rs — rank 0.0232 (rust)
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

### tests/markdown_integration.rs — rank 0.0229 (rust)
- function **run_sigil_index** — blast 2f/11c/10t
  `fn run_sigil_index(fixture_dir: &str, extra_args: &[&str]) -> String`
- function **fixture_path** — blast 2f/11c/10t
  `fn fixture_path() -> String`
- function **indexes_markdown_fixture** — blast 0f/0c/0t
  `fn indexes_markdown_fixture()`
- function **diff_markdown_files** — blast 0f/0c/0t
  `fn diff_markdown_files()`

### tests/integration.rs — rank 0.0209 (rust)
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


_Truncated: 58 more file(s) below budget. Increase `--tokens` or scope with `--focus` to see more._
