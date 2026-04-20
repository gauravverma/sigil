# Review — `HEAD~3..HEAD`

315 entities changed · 306a / 5m / 1r / 3mv / 0rn / 0fmt · ⚠ breaking changes present

## Most impactful (5)

- `RankManifest` — modified in `src/rank.rs` · rank 0.0292 · blast 4f/7c/26t · ⚠ breaking
  _sig_changed_
- `RankManifest` — moved in `src/rank.rs` · rank 0.0292 · blast 4f/7c/26t · ⚠ breaking
  _moved_
- `EntityKey` — modified in `src/rank.rs` · rank 0.0292 · blast 1f/2c/21t · ⚠ breaking
  _sig_changed_
- `RankConfig` — modified in `src/rank.rs` · rank 0.0292 · blast 1f/3c/18t · ⚠ breaking
  _sig_changed_
- `EntityKey` — moved in `src/rank.rs` · rank 0.0292 · blast 1f/2c/21t · ⚠ breaking
  _moved_

## Structural deltas

### `src/rank.rs` (rank 0.0292)

- _modified_ `struct` `RankManifest` ⚠ **breaking** · rank 0.0292 · blast 4f/7c/26t
- _moved_ `impl` `RankManifest` ⚠ **breaking** · rank 0.0292 · blast 4f/7c/26t
- _modified_ `struct` `EntityKey` ⚠ **breaking** · rank 0.0292 · blast 1f/2c/21t
- _modified_ `struct` `RankConfig` ⚠ **breaking** · rank 0.0292 · blast 1f/3c/18t
- _moved_ `impl` `EntityKey` ⚠ **breaking** · rank 0.0292 · blast 1f/2c/21t
- _moved_ `impl` `RankConfig` ⚠ **breaking** · rank 0.0292 · blast 1f/3c/18t

### `src/lib.rs` (rank 0.0205)

- _added_ `module` `install` · rank 0.0205 · blast 8f/39c/28t
- _added_ `module` `benchmark` · rank 0.0205 · blast 0f/0c/0t
- _added_ `module` `blast` · rank 0.0205 · blast 0f/0c/0t
- _added_ `module` `duplicates` · rank 0.0205 · blast 0f/0c/0t

### `src/install/claude.rs` (rank 0.0171)

- _added_ `import` `super::*` · rank 0.0171 · blast 51f/51c/0t
- _added_ `import` `std::path::Path` · rank 0.0171 · blast 19f/24c/59t
- _added_ `import` `anyhow::{Context as _, Result}` · rank 0.0171 · blast 10f/10c/0t
- _added_ `function` `install` · rank 0.0171 · blast 8f/39c/28t
- _added_ `function` `uninstall` · rank 0.0171 · blast 6f/14c/12t
- _added_ `function` `tmpdir` · rank 0.0171 · blast 6f/27c/23t
- _added_ `import` `super::{capability_block, remove_marker_block, upsert_marker_block, UpsertResult}` · rank 0.0171 · blast 5f/5c/0t
- _added_ `import` `serde_json::{json, Value}` · rank 0.0171 · blast 4f/4c/0t
- _added_ `enum` `InstallStep` · rank 0.0171 · blast 4f/4c/29t
- _added_ `enum` `UninstallStep` · rank 0.0171 · blast 4f/4c/13t
- _added_ `function` `is_sigil_hook` · rank 0.0171 · blast 3f/6c/37t
- _added_ `function` `upsert_claude_hook` · rank 0.0171 · blast 1f/1c/29t
- _added_ `function` `remove_claude_hook` · rank 0.0171 · blast 1f/1c/13t
- _added_ `constant` `HOOK_ID` · rank 0.0171 · blast 0f/0c/0t
- _added_ `constant` `HINT_LINE` · rank 0.0171 · blast 0f/0c/0t
- _added_ `module` `tests` · rank 0.0171 · blast 0f/0c/0t
- _added_ `function` `install_creates_claude_md_and_settings` · rank 0.0171 · blast 0f/0c/0t
- _added_ `function` `install_preserves_existing_settings` · rank 0.0171 · blast 0f/0c/0t
- _added_ `function` `install_is_idempotent` · rank 0.0171 · blast 0f/0c/0t
- _added_ `function` `uninstall_removes_sigil_without_touching_other_hooks` · rank 0.0171 · blast 0f/0c/0t
- _added_ `function` `uninstall_without_install_is_noop` · rank 0.0171 · blast 0f/0c/0t
- _added_ `function` `uninstall_deletes_settings_if_emptied` · rank 0.0171 · blast 0f/0c/0t
- _added_ `function` `escape_single_quotes_handles_shell_safety` · rank 0.0171 · blast 0f/0c/0t

### `src/install/gemini.rs` (rank 0.0170)

- _added_ `import` `super::*` · rank 0.0170 · blast 51f/51c/0t
- _added_ `import` `std::path::Path` · rank 0.0170 · blast 19f/24c/59t
- _added_ `import` `anyhow::{Context as _, Result}` · rank 0.0170 · blast 10f/10c/0t
- _added_ `function` `install` · rank 0.0170 · blast 8f/39c/28t
- _added_ `function` `uninstall` · rank 0.0170 · blast 6f/14c/12t
- _added_ `function` `tmpdir` · rank 0.0170 · blast 6f/27c/23t
- _added_ `import` `super::{capability_block, remove_marker_block, upsert_marker_block, UpsertResult}` · rank 0.0170 · blast 5f/5c/0t
- _added_ `import` `serde_json::{json, Value}` · rank 0.0170 · blast 4f/4c/0t
- _added_ `enum` `InstallStep` · rank 0.0170 · blast 4f/4c/29t
- _added_ `enum` `UninstallStep` · rank 0.0170 · blast 4f/4c/13t
- _added_ `function` `is_sigil_hook` · rank 0.0170 · blast 3f/6c/37t
- _added_ `function` `upsert_gemini_hook` · rank 0.0170 · blast 1f/1c/29t
- _added_ `function` `remove_gemini_hook` · rank 0.0170 · blast 1f/1c/13t
- _added_ `constant` `HOOK_ID` · rank 0.0170 · blast 0f/0c/0t
- _added_ `constant` `HINT_LINE` · rank 0.0170 · blast 0f/0c/0t
- _added_ `module` `tests` · rank 0.0170 · blast 0f/0c/0t
- _added_ `function` `install_creates_gemini_md_and_hook` · rank 0.0170 · blast 0f/0c/0t
- _added_ `function` `install_idempotent_and_uninstall_clean` · rank 0.0170 · blast 0f/0c/0t
- _added_ `function` `uninstall_preserves_sibling_user_hooks` · rank 0.0170 · blast 0f/0c/0t

### `src/install/codex.rs` (rank 0.0170)

- _added_ `import` `super::*` · rank 0.0170 · blast 51f/51c/0t
- _added_ `import` `std::path::Path` · rank 0.0170 · blast 19f/24c/59t
- _added_ `import` `anyhow::{Context as _, Result}` · rank 0.0170 · blast 10f/10c/0t
- _added_ `function` `install` · rank 0.0170 · blast 8f/39c/28t
- _added_ `function` `uninstall` · rank 0.0170 · blast 6f/14c/12t
- _added_ `function` `tmpdir` · rank 0.0170 · blast 6f/27c/23t
- _added_ `import` `super::{capability_block, remove_marker_block, upsert_marker_block, UpsertResult}` · rank 0.0170 · blast 5f/5c/0t
- _added_ `import` `serde_json::{json, Value}` · rank 0.0170 · blast 4f/4c/0t
- _added_ `enum` `InstallStep` · rank 0.0170 · blast 4f/4c/29t
- _added_ `enum` `UninstallStep` · rank 0.0170 · blast 4f/4c/13t
- _added_ `function` `is_sigil_hook` · rank 0.0170 · blast 3f/6c/37t
- _added_ `function` `upsert_codex_hook` · rank 0.0170 · blast 1f/1c/29t
- _added_ `function` `remove_codex_hook` · rank 0.0170 · blast 1f/1c/13t
- _added_ `constant` `HOOK_ID` · rank 0.0170 · blast 0f/0c/0t
- _added_ `constant` `HINT_LINE` · rank 0.0170 · blast 0f/0c/0t
- _added_ `module` `tests` · rank 0.0170 · blast 0f/0c/0t
- _added_ `function` `install_creates_agents_md_and_hooks` · rank 0.0170 · blast 0f/0c/0t
- _added_ `function` `install_preserves_existing_agents_md_content` · rank 0.0170 · blast 0f/0c/0t
- _added_ `function` `install_is_idempotent` · rank 0.0170 · blast 0f/0c/0t
- _added_ `function` `uninstall_restores_prior_state` · rank 0.0170 · blast 0f/0c/0t
- _added_ `function` `uninstall_without_install_is_noop` · rank 0.0170 · blast 0f/0c/0t

### `src/install/opencode.rs` (rank 0.0159)

- _added_ `import` `super::*` · rank 0.0159 · blast 51f/51c/0t
- _added_ `import` `std::path::Path` · rank 0.0159 · blast 19f/24c/59t
- _added_ `import` `anyhow::{Context as _, Result}` · rank 0.0159 · blast 10f/10c/0t
- _added_ `function` `install` · rank 0.0159 · blast 8f/39c/28t
- _added_ `function` `uninstall` · rank 0.0159 · blast 6f/14c/12t
- _added_ `function` `tmpdir` · rank 0.0159 · blast 6f/27c/23t
- _added_ `import` `super::{capability_block, remove_marker_block, upsert_marker_block, UpsertResult}` · rank 0.0159 · blast 5f/5c/0t
- _added_ `import` `serde_json::{json, Value}` · rank 0.0159 · blast 4f/4c/0t
- _added_ `enum` `InstallStep` · rank 0.0159 · blast 4f/4c/29t
- _added_ `enum` `UninstallStep` · rank 0.0159 · blast 4f/4c/13t
- _added_ `function` `plugin_js` · rank 0.0159 · blast 1f/1c/29t
- _added_ `function` `upsert_plugin_registration` · rank 0.0159 · blast 1f/1c/29t
- _added_ `function` `remove_plugin_registration` · rank 0.0159 · blast 1f/1c/13t
- _added_ `constant` `PLUGIN_REL` · rank 0.0159 · blast 0f/0c/0t
- _added_ `constant` `HOOK_ID` · rank 0.0159 · blast 0f/0c/0t
- _added_ `constant` `HINT_LINE` · rank 0.0159 · blast 0f/0c/0t
- _added_ `module` `tests` · rank 0.0159 · blast 0f/0c/0t
- _added_ `function` `install_creates_all_three_artifacts` · rank 0.0159 · blast 0f/0c/0t
- _added_ `function` `install_idempotent` · rank 0.0159 · blast 0f/0c/0t
- _added_ `function` `install_preserves_existing_opencode_json` · rank 0.0159 · blast 0f/0c/0t
- _added_ `function` `uninstall_cleans_all_artifacts` · rank 0.0159 · blast 0f/0c/0t
- _added_ `function` `uninstall_preserves_user_plugin_entries` · rank 0.0159 · blast 0f/0c/0t

### `src/install/mod.rs` (rank 0.0154)

- _added_ `import` `super::*` · rank 0.0154 · blast 51f/51c/0t
- _added_ `function` `capability_block` · rank 0.0154 · blast 6f/7c/31t
- _added_ `function` `upsert_marker_block` · rank 0.0154 · blast 6f/10c/33t
- _added_ `function` `remove_marker_block` · rank 0.0154 · blast 6f/8c/16t
- _added_ `enum` `UpsertResult` · rank 0.0154 · blast 4f/4c/34t
- _added_ `function` `replace_marker_block` · rank 0.0154 · blast 1f/3c/36t
- _added_ `function` `strip_marker_block` · rank 0.0154 · blast 1f/3c/19t
- _added_ `function` `find_marker_bounds` · rank 0.0154 · blast 1f/2c/16t
- _added_ `module` `aider` · rank 0.0154 · blast 0f/0c/0t
- _added_ `module` `claude` · rank 0.0154 · blast 0f/0c/0t
- _added_ `module` `codex` · rank 0.0154 · blast 0f/0c/0t
- _added_ `module` `copilot` · rank 0.0154 · blast 0f/0c/0t
- _added_ `module` `cursor` · rank 0.0154 · blast 0f/0c/0t
- _added_ `module` `gemini` · rank 0.0154 · blast 0f/0c/0t
- _added_ `module` `githook` · rank 0.0154 · blast 0f/0c/0t
- _added_ `module` `opencode` · rank 0.0154 · blast 0f/0c/0t
- _added_ `function` `shell_escape_single_quoted` · rank 0.0154 · blast 0f/0c/0t
- _added_ `constant` `MARKER_BEGIN` · rank 0.0154 · blast 0f/0c/0t
- _added_ `constant` `MARKER_END` · rank 0.0154 · blast 0f/0c/0t
- _added_ `constant` `JSON_MARKER` · rank 0.0154 · blast 0f/0c/0t
- _added_ `function` `capability_block_with_markers` · rank 0.0154 · blast 0f/0c/0t
- _added_ `module` `tests` · rank 0.0154 · blast 0f/0c/0t
- _added_ `function` `capability_block_lists_all_shipped_commands` · rank 0.0154 · blast 0f/0c/0t
- _added_ `function` `capability_block_avoids_preference_language` · rank 0.0154 · blast 0f/0c/0t
- _added_ `function` `replace_marker_block_roundtrips_without_changing_user_content` · rank 0.0154 · blast 0f/0c/0t
- _added_ `function` `replace_marker_block_appends_when_markers_absent` · rank 0.0154 · blast 0f/0c/0t
- _added_ `function` `strip_marker_block_removes_cleanly` · rank 0.0154 · blast 0f/0c/0t
- _added_ `function` `strip_marker_block_idempotent_when_absent` · rank 0.0154 · blast 0f/0c/0t
- _added_ `function` `upsert_marker_block_creates_new_file` · rank 0.0154 · blast 0f/0c/0t
- _added_ `function` `upsert_marker_block_upgrade_in_place` · rank 0.0154 · blast 0f/0c/0t
- _added_ `function` `upsert_marker_block_is_noop_when_content_matches` · rank 0.0154 · blast 0f/0c/0t
- _added_ `function` `remove_marker_block_deletes_empty_file_after_removal` · rank 0.0154 · blast 0f/0c/0t
- _added_ `function` `remove_marker_block_preserves_user_content` · rank 0.0154 · blast 0f/0c/0t
- _added_ `function` `remove_marker_block_missing_file_returns_false` · rank 0.0154 · blast 0f/0c/0t

### `src/blast.rs` (rank 0.0147)

- _added_ `import` `super::*` · rank 0.0147 · blast 51f/51c/0t
- _added_ `import` `std::collections::HashMap` · rank 0.0147 · blast 8f/11c/0t
- _added_ `import` `std::collections::HashMap` · rank 0.0147 · blast 8f/11c/0t
- _added_ `import` `crate::query::index::Index` · rank 0.0147 · blast 6f/10c/0t
- _added_ `function` `render_markdown` · rank 0.0147 · blast 6f/16c/16t
- _added_ `import` `crate::query::index::Index` · rank 0.0147 · blast 6f/10c/0t
- _added_ `import` `serde::Serialize` · rank 0.0147 · blast 5f/5c/0t
- _added_ `import` `crate::rank::RankManifest` · rank 0.0147 · blast 4f/4c/0t
- _added_ `import` `crate::entity::{BlastRadius, Entity, Reference}` · rank 0.0147 · blast 3f/5c/0t
- _added_ `import` `crate::entity::{BlastRadius, Entity, Reference}` · rank 0.0147 · blast 3f/5c/0t
- _added_ `function` `ent` · rank 0.0147 · blast 3f/11c/8t
- _added_ `struct` `AgentEdge` · rank 0.0147 · blast 2f/5c/2t
- _added_ `function` `refr` · rank 0.0147 · blast 2f/2c/2t
- _added_ `struct` `BlastOptions` · rank 0.0147 · blast 1f/1c/7t
- _added_ `impl` `BlastOptions` · rank 0.0147 · blast 1f/1c/7t
- _added_ `enum` `BlastFormat` · rank 0.0147 · blast 1f/1c/8t
- _added_ `impl` `BlastFormat` · rank 0.0147 · blast 1f/1c/8t
- _added_ `struct` `BlastReport` · rank 0.0147 · blast 1f/4c/21t
- _added_ `struct` `CallerRow` · rank 0.0147 · blast 1f/2c/22t
- _added_ `struct` `BlastAlt` · rank 0.0147 · blast 1f/1c/21t
- _added_ `function` `run_blast` · rank 0.0147 · blast 1f/6c/6t
- _added_ `function` `rank_sorted_callers` · rank 0.0147 · blast 1f/1c/7t
- _added_ `function` `render_json` · rank 0.0147 · blast 1f/1c/1t
- _added_ `function` `render_agent` · rank 0.0147 · blast 1f/1c/1t
- _added_ `function` `rank_of` · rank 0.0147 · blast 1f/6c/6t
- _added_ `method` `BlastOptions.default` · rank 0.0147 · blast 0f/0c/0t
- _added_ `method` `BlastFormat.parse` · rank 0.0147 · blast 0f/0c/0t
- _added_ `struct` `AgentView` · rank 0.0147 · blast 0f/0c/0t
- _added_ `method` `render_agent.is_zero` · rank 0.0147 · blast 0f/0c/0t
- _added_ `module` `tests` · rank 0.0147 · blast 0f/0c/0t
- _added_ `function` `missing_symbol_returns_none` · rank 0.0147 · blast 0f/0c/0t
- _added_ `function` `chooses_highest_blast_match` · rank 0.0147 · blast 0f/0c/0t
- _added_ `function` `top_callers_sorted_by_file_rank` · rank 0.0147 · blast 0f/0c/0t
- _added_ `function` `depth_caps_callers_and_sets_skipped_count` · rank 0.0147 · blast 0f/0c/0t
- _added_ `function` `same_line_callers_deduped` · rank 0.0147 · blast 0f/0c/0t
- _added_ `function` `markdown_renderer_has_expected_headings` · rank 0.0147 · blast 0f/0c/0t
- _added_ `function` `agent_form_is_single_line_short_keys` · rank 0.0147 · blast 0f/0c/0t
- _added_ `function` `format_parse_covers_known_values` · rank 0.0147 · blast 0f/0c/0t

### `src/install/aider.rs` (rank 0.0143)

- _added_ `import` `super::*` · rank 0.0143 · blast 51f/51c/0t
- _added_ `import` `std::path::Path` · rank 0.0143 · blast 19f/24c/59t
- _added_ `import` `anyhow::{Context as _, Result}` · rank 0.0143 · blast 10f/10c/0t
- _added_ `function` `install` · rank 0.0143 · blast 8f/39c/28t
- _added_ `function` `uninstall` · rank 0.0143 · blast 6f/14c/12t
- _added_ `function` `tmpdir` · rank 0.0143 · blast 6f/27c/23t
- _added_ `import` `super::{capability_block, remove_marker_block, upsert_marker_block, UpsertResult}` · rank 0.0143 · blast 5f/5c/0t
- _added_ `module` `tests` · rank 0.0143 · blast 0f/0c/0t
- _added_ `function` `install_writes_agents_md` · rank 0.0143 · blast 0f/0c/0t
- _added_ `function` `install_preserves_existing_agents_md` · rank 0.0143 · blast 0f/0c/0t
- _added_ `function` `uninstall_roundtrips_cleanly` · rank 0.0143 · blast 0f/0c/0t

### `src/install/cursor.rs` (rank 0.0142)

- _added_ `import` `super::*` · rank 0.0142 · blast 51f/51c/0t
- _added_ `import` `std::path::Path` · rank 0.0142 · blast 19f/24c/59t
- _added_ `import` `anyhow::{Context as _, Result}` · rank 0.0142 · blast 10f/10c/0t
- _added_ `function` `install` · rank 0.0142 · blast 8f/39c/28t
- _added_ `function` `uninstall` · rank 0.0142 · blast 6f/14c/12t
- _added_ `function` `tmpdir` · rank 0.0142 · blast 6f/27c/23t
- _added_ `import` `super::{capability_block, UpsertResult}` · rank 0.0142 · blast 2f/2c/0t
- _added_ `constant` `RULE_PATH` · rank 0.0142 · blast 0f/0c/0t
- _added_ `module` `tests` · rank 0.0142 · blast 0f/0c/0t
- _added_ `function` `install_creates_rule_file_with_frontmatter` · rank 0.0142 · blast 0f/0c/0t
- _added_ `function` `install_is_idempotent` · rank 0.0142 · blast 0f/0c/0t
- _added_ `function` `uninstall_removes_file_and_empty_rules_dir` · rank 0.0142 · blast 0f/0c/0t
- _added_ `function` `uninstall_without_install_is_noop` · rank 0.0142 · blast 0f/0c/0t
- _added_ `function` `uninstall_preserves_sibling_rule_files` · rank 0.0142 · blast 0f/0c/0t

### `src/benchmark.rs` (rank 0.0133)

- _added_ `import` `super::*` · rank 0.0133 · blast 51f/51c/0t
- _added_ `import` `std::path::Path` · rank 0.0133 · blast 19f/24c/59t
- _added_ `import` `anyhow::{Context as _, Result}` · rank 0.0133 · blast 10f/10c/0t
- _added_ `import` `crate::query::index::Index` · rank 0.0133 · blast 6f/10c/0t
- _added_ `function` `render_markdown` · rank 0.0133 · blast 6f/16c/16t
- _added_ `import` `serde::Serialize` · rank 0.0133 · blast 5f/5c/0t
- _added_ `import` `std::collections::HashSet` · rank 0.0133 · blast 5f/5c/1t
- _added_ `import` `crate::rank::RankManifest` · rank 0.0133 · blast 4f/4c/0t
- _added_ `import` `std::process::{Command, Stdio}` · rank 0.0133 · blast 1f/1c/0t
- _added_ `import` `crate::context` · rank 0.0133 · blast 1f/1c/0t
- _added_ `import` `crate::map` · rank 0.0133 · blast 1f/1c/0t
- _added_ `import` `crate::review` · rank 0.0133 · blast 1f/1c/0t
- _added_ `struct` `BenchmarkOptions` · rank 0.0133 · blast 1f/1c/1t
- _added_ `impl` `BenchmarkOptions` · rank 0.0133 · blast 1f/1c/1t
- _added_ `enum` `BenchmarkFormat` · rank 0.0133 · blast 1f/1c/2t
- _added_ `impl` `BenchmarkFormat` · rank 0.0133 · blast 1f/1c/2t
- _added_ `struct` `BenchmarkReport` · rank 0.0133 · blast 1f/3c/15t
- _added_ `struct` `QueryResult` · rank 0.0133 · blast 1f/4c/18t
- _added_ `function` `tokens` · rank 0.0133 · blast 1f/5c/8t
- _added_ `function` `pick_high_blast_symbol` · rank 0.0133 · blast 1f/1c/1t
- _added_ `function` `run_review_query` · rank 0.0133 · blast 1f/1c/1t
- _added_ `function` `run_context_query` · rank 0.0133 · blast 1f/1c/1t
- _added_ `function` `run_map_query` · rank 0.0133 · blast 1f/1c/1t
- _added_ `function` `git_output_tokens` · rank 0.0133 · blast 1f/1c/2t
- _added_ `function` `read_files_touching_symbol_tokens` · rank 0.0133 · blast 1f/1c/2t
- _added_ `function` `read_top_files_tokens` · rank 0.0133 · blast 1f/1c/2t
- _added_ `import` `std::collections::BTreeSet` · rank 0.0133 · blast 1f/1c/0t
- _added_ `function` `read_file_tokens` · rank 0.0133 · blast 1f/2c/5t
- _added_ `function` `ratio` · rank 0.0133 · blast 1f/3c/4t
- _added_ `function` `median` · rank 0.0133 · blast 1f/1c/1t
- _added_ `function` `render_json` · rank 0.0133 · blast 1f/1c/1t
- _added_ `method` `BenchmarkOptions.default` · rank 0.0133 · blast 0f/0c/0t
- _added_ `method` `BenchmarkFormat.parse` · rank 0.0133 · blast 0f/0c/0t
- _added_ `function` `run_benchmark` · rank 0.0133 · blast 0f/0c/0t
- _added_ `module` `tests` · rank 0.0133 · blast 0f/0c/0t
- _added_ `function` `tokens_estimate_matches_spec` · rank 0.0133 · blast 0f/0c/0t
- _added_ `function` `ratio_handles_zero_denominator` · rank 0.0133 · blast 0f/0c/0t
- _added_ `function` `median_odd_and_even_length` · rank 0.0133 · blast 0f/0c/0t
- _added_ `function` `format_parse_covers_known_values` · rank 0.0133 · blast 0f/0c/0t
- _added_ `function` `render_markdown_contains_query_table` · rank 0.0133 · blast 0f/0c/0t

### `src/duplicates.rs` (rank 0.0124)

- _added_ `import` `super::*` · rank 0.0124 · blast 51f/51c/0t
- _added_ `import` `std::collections::HashMap` · rank 0.0124 · blast 8f/11c/0t
- _added_ `import` `crate::entity::Entity` · rank 0.0124 · blast 7f/10c/0t
- _added_ `import` `crate::entity::Entity` · rank 0.0124 · blast 7f/10c/0t
- _added_ `import` `crate::query::index::Index` · rank 0.0124 · blast 6f/10c/0t
- _added_ `function` `render_markdown` · rank 0.0124 · blast 6f/16c/16t
- _added_ `import` `crate::query::index::Index` · rank 0.0124 · blast 6f/10c/0t
- _added_ `import` `serde::Serialize` · rank 0.0124 · blast 5f/5c/0t
- _added_ `function` `ent` · rank 0.0124 · blast 3f/11c/8t
- _added_ `struct` `DuplicatesOptions` · rank 0.0124 · blast 1f/1c/11t
- _added_ `impl` `DuplicatesOptions` · rank 0.0124 · blast 1f/1c/11t
- _added_ `enum` `DuplicatesFormat` · rank 0.0124 · blast 1f/1c/12t
- _added_ `impl` `DuplicatesFormat` · rank 0.0124 · blast 1f/1c/12t
- _added_ `struct` `CloneGroup` · rank 0.0124 · blast 1f/1c/23t
- _added_ `struct` `CloneMember` · rank 0.0124 · blast 1f/1c/5t
- _added_ `struct` `DuplicatesReport` · rank 0.0124 · blast 1f/3c/23t
- _added_ `function` `find_duplicates` · rank 0.0124 · blast 1f/10c/10t
- _added_ `function` `render_json` · rank 0.0124 · blast 1f/1c/1t
- _added_ `method` `DuplicatesOptions.default` · rank 0.0124 · blast 0f/0c/0t
- _added_ `method` `DuplicatesFormat.parse` · rank 0.0124 · blast 0f/0c/0t
- _added_ `module` `tests` · rank 0.0124 · blast 0f/0c/0t
- _added_ `function` `empty_index_returns_zero_groups` · rank 0.0124 · blast 0f/0c/0t
- _added_ `function` `finds_basic_clone_pair` · rank 0.0124 · blast 0f/0c/0t
- _added_ `function` `excludes_short_bodies` · rank 0.0124 · blast 0f/0c/0t
- _added_ `function` `excludes_import_kind_by_default` · rank 0.0124 · blast 0f/0c/0t
- _added_ `function` `skips_entities_without_body_hash` · rank 0.0124 · blast 0f/0c/0t
- _added_ `function` `groups_sorted_by_size_desc` · rank 0.0124 · blast 0f/0c/0t
- _added_ `function` `max_group_size_drops_huge_clusters` · rank 0.0124 · blast 0f/0c/0t
- _added_ `function` `members_sorted_by_file_then_line` · rank 0.0124 · blast 0f/0c/0t
- _added_ `function` `markdown_renderer_outputs_header_and_groups` · rank 0.0124 · blast 0f/0c/0t
- _added_ `function` `markdown_handles_empty_report` · rank 0.0124 · blast 0f/0c/0t
- _added_ `function` `format_parse_covers_known_values` · rank 0.0124 · blast 0f/0c/0t

### `src/main.rs` (rank 0.0096)

- _added_ `import` `clap::{Parser, Subcommand}` · rank 0.0096 · blast 1f/1c/0t
- _modified_ `enum` `Cli` · rank 0.0096 · blast 0f/0c/0t
- _modified_ `function` `main` · rank 0.0096 · blast 0f/0c/0t
- _removed_ `import` `clap::Parser` · rank 0.0096
- _added_ `enum` `InstallAction` · rank 0.0096 · blast 0f/0c/0t

### `src/install/copilot.rs` (rank 0.0094)

- _added_ `import` `super::*` · rank 0.0094 · blast 51f/51c/0t
- _added_ `import` `anyhow::{Context as _, Result}` · rank 0.0094 · blast 10f/10c/0t
- _added_ `function` `install` · rank 0.0094 · blast 8f/39c/28t
- _added_ `function` `uninstall` · rank 0.0094 · blast 6f/14c/12t
- _added_ `import` `std::path::{Path, PathBuf}` · rank 0.0094 · blast 3f/3c/0t
- _added_ `import` `super::{capability_block, UpsertResult}` · rank 0.0094 · blast 2f/2c/0t
- _added_ `function` `skill_home` · rank 0.0094 · blast 1f/2c/31t
- _added_ `import` `std::sync::{Mutex, MutexGuard, OnceLock}` · rank 0.0094 · blast 1f/1c/0t
- _added_ `function` `env_guard` · rank 0.0094 · blast 1f/4c/4t
- _added_ `function` `fresh_home` · rank 0.0094 · blast 1f/4c/4t
- _added_ `function` `cleanup` · rank 0.0094 · blast 1f/4c/4t
- _added_ `constant` `SKILL_DIR` · rank 0.0094 · blast 0f/0c/0t
- _added_ `module` `tests` · rank 0.0094 · blast 0f/0c/0t
- _added_ `constant` `LOCK` · rank 0.0094 · blast 0f/0c/0t
- _added_ `function` `install_creates_skill_file` · rank 0.0094 · blast 0f/0c/0t
- _added_ `function` `install_is_idempotent` · rank 0.0094 · blast 0f/0c/0t
- _added_ `function` `uninstall_removes_only_sigil_skill` · rank 0.0094 · blast 0f/0c/0t
- _added_ `function` `uninstall_without_install_is_noop` · rank 0.0094 · blast 0f/0c/0t

### `src/install/githook.rs` (rank 0.0087)

- _added_ `import` `super::*` · rank 0.0087 · blast 51f/51c/0t
- _added_ `import` `anyhow::{Context as _, Result}` · rank 0.0087 · blast 10f/10c/0t
- _added_ `function` `install` · rank 0.0087 · blast 8f/39c/28t
- _added_ `function` `uninstall` · rank 0.0087 · blast 6f/14c/12t
- _added_ `import` `std::path::{Path, PathBuf}` · rank 0.0087 · blast 3f/3c/0t
- _added_ `struct` `HookStep` · rank 0.0087 · blast 1f/2c/31t
- _added_ `enum` `HookResult` · rank 0.0087 · blast 1f/2c/33t
- _added_ `function` `sentinel_for` · rank 0.0087 · blast 1f/2c/31t
- _added_ `function` `resolve_hooks_dir` · rank 0.0087 · blast 1f/2c/31t
- _added_ `function` `sigil_block` · rank 0.0087 · blast 1f/1c/30t
- _added_ `function` `upsert_hook_script` · rank 0.0087 · blast 1f/1c/29t
- _added_ `function` `remove_hook_script` · rank 0.0087 · blast 1f/1c/13t
- _added_ `function` `split_existing_block` · rank 0.0087 · blast 1f/2c/33t
- _added_ `function` `make_executable` · rank 0.0087 · blast 1f/3c/30t
- _added_ `import` `std::os::unix::fs::PermissionsExt` · rank 0.0087 · blast 1f/2c/0t
- _added_ `function` `make_executable` · rank 0.0087 · blast 1f/3c/30t
- _added_ `function` `setup_repo` · rank 0.0087 · blast 1f/6c/6t
- _added_ `import` `std::os::unix::fs::PermissionsExt` · rank 0.0087 · blast 1f/2c/0t
- _added_ `constant` `SENTINEL` · rank 0.0087 · blast 0f/0c/0t
- _added_ `constant` `CHECKOUT_SENTINEL` · rank 0.0087 · blast 0f/0c/0t
- _added_ `constant` `HOOKS` · rank 0.0087 · blast 0f/0c/0t
- _added_ `module` `tests` · rank 0.0087 · blast 0f/0c/0t
- _added_ `function` `install_creates_both_hooks_and_marks_executable` · rank 0.0087 · blast 0f/0c/0t
- _added_ `function` `install_preserves_existing_user_hook` · rank 0.0087 · blast 0f/0c/0t
- _added_ `function` `install_is_idempotent` · rank 0.0087 · blast 0f/0c/0t
- _added_ `function` `uninstall_leaves_user_hook_intact` · rank 0.0087 · blast 0f/0c/0t
- _added_ `function` `uninstall_deletes_hook_files_created_solely_by_sigil` · rank 0.0087 · blast 0f/0c/0t
- _added_ `function` `uninstall_without_install_is_noop` · rank 0.0087 · blast 0f/0c/0t

## Co-change misses (6)

_These files historically change with the files in this PR but did not this time._

- `src/rank.rs` changed; expected companion `src/writer.rs` (weight 1.00, 2 historical co-changes)
- `src/main.rs` changed; expected companion `.sigil/refs.jsonl` (weight 0.56, 19 historical co-changes)
- `src/main.rs` changed; expected companion `.sigil/entities.jsonl` (weight 0.53, 20 historical co-changes)
- `src/lib.rs` changed; expected companion `.sigil/rank.json` (weight 0.52, 2 historical co-changes)
- `src/main.rs` changed; expected companion `.sigil/cache.json` (weight 0.50, 20 historical co-changes)
- `src/lib.rs` changed; expected companion `.sigil/refs.jsonl` (weight 0.31, 5 historical co-changes)

## Patterns (28)

- added across 12 file(s): _same added applied to tests across 12 files_
- added across 12 file(s): _same added applied to super::* across 12 files_
- added across 9 file(s): _same added applied to anyhow::{Context as _, Result} across 9 files_
- added across 9 file(s): _same added applied to install across 9 files_
- added across 8 file(s): _same added applied to uninstall across 8 files_
- added across 7 file(s): _same added applied to std::path::Path across 7 files_
- added across 6 file(s): _same added applied to tmpdir across 6 files_
- added across 5 file(s): _same added applied to uninstall_without_install_is_noop across 5 files_
- added across 5 file(s): _same added applied to install_is_idempotent across 5 files_
- added across 5 file(s): _same added applied to super::{capability_block, remove_marker_block, upsert_marker_block, UpsertResult} across 5 files_
- added across 4 file(s): _same added applied to InstallStep across 4 files_
- added across 4 file(s): _same added applied to serde_json::{json, Value} across 4 files_
- added across 4 file(s): _same added applied to UninstallStep across 4 files_
- added across 4 file(s): _same added applied to HINT_LINE across 4 files_
- added across 4 file(s): _same added applied to HOOK_ID across 4 files_
- added across 3 file(s): _same added applied to format_parse_covers_known_values across 3 files_
- added across 3 file(s): _same added applied to is_sigil_hook across 3 files_
- added across 3 file(s): _same added applied to parse across 3 files_
- added across 3 file(s): _same added applied to render_json across 3 files_
- added across 3 file(s): _same added applied to serde::Serialize across 3 files_
- added across 3 file(s): _same added applied to default across 3 files_
- added across 3 file(s): _same added applied to crate::query::index::Index across 3 files_
- added across 3 file(s): _same added applied to render_markdown across 3 files_
- added across 2 file(s): _same added applied to std::path::{Path, PathBuf} across 2 files_
- added across 2 file(s): _same added applied to crate::rank::RankManifest across 2 files_
- added across 2 file(s): _same added applied to super::{capability_block, UpsertResult} across 2 files_
- added across 2 file(s): _same added applied to std::collections::HashMap across 2 files_
- added across 2 file(s): _same added applied to ent across 2 files_

