//! Codex installer.
//!
//! Codex reads `AGENTS.md` every session and supports PreToolUse hooks
//! in `.codex/hooks.json`. Same shape as Claude's installer, different
//! files:
//!   1. `AGENTS.md` — capability block with markers.
//!   2. `.codex/hooks.json` — PreToolUse entry that fires before Bash
//!      and echoes the sigil hint.
//!
//! The hint text mirrors Claude's — sigil is the same tool either way.

use std::path::Path;

use anyhow::{Context as _, Result};
use serde_json::{json, Value};

use super::{capability_block, remove_marker_block, upsert_marker_block, UpsertResult};

const HOOK_ID: &str = "sigil-hint";
const HINT_LINE: &str = "sigil available in this repo: use `sigil map` for orientation, `sigil context <symbol>` for a focused bundle, `sigil callers/callees` for exact lookups. See .sigil/SIGIL_MAP.md if present.";

pub fn install(root: &Path) -> Result<Vec<InstallStep>> {
    let mut steps = Vec::new();

    let agents_md = root.join("AGENTS.md");
    let md_result =
        upsert_marker_block(&agents_md, &capability_block()).with_context(|| format!("upsert {}", agents_md.display()))?;
    steps.push(InstallStep::AgentsMd(md_result));

    let hooks_path = root.join(".codex").join("hooks.json");
    let changed = upsert_codex_hook(&hooks_path).with_context(|| format!("update {}", hooks_path.display()))?;
    steps.push(InstallStep::Hooks(if changed {
        UpsertResult::Updated
    } else {
        UpsertResult::Unchanged
    }));

    Ok(steps)
}

pub fn uninstall(root: &Path) -> Result<Vec<UninstallStep>> {
    let mut steps = Vec::new();
    let agents_md = root.join("AGENTS.md");
    let removed_md = remove_marker_block(&agents_md)?;
    steps.push(UninstallStep::AgentsMd(removed_md));

    let hooks_path = root.join(".codex").join("hooks.json");
    let removed_hook = remove_codex_hook(&hooks_path)?;
    steps.push(UninstallStep::Hooks(removed_hook));

    Ok(steps)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStep {
    AgentsMd(UpsertResult),
    Hooks(UpsertResult),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallStep {
    AgentsMd(bool),
    Hooks(bool),
}

fn upsert_codex_hook(path: &Path) -> Result<bool> {
    let mut hooks: Value = if path.exists() {
        let text = std::fs::read_to_string(path)?;
        if text.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?
        }
    } else {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        json!({})
    };

    let hooks_obj = hooks
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("hooks.json root must be an object"))?;
    let pre = hooks_obj
        .entry("PreToolUse".to_string())
        .or_insert_with(|| json!([]));
    let arr = pre
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("PreToolUse must be an array"))?;

    arr.retain(|e| !is_sigil_hook(e));

    arr.push(json!({
        "matcher": "Bash",
        "command": format!("echo '{}' # {}", super::shell_escape_single_quoted(HINT_LINE), HOOK_ID),
    }));

    let mut text = serde_json::to_string_pretty(&hooks)?;
    text.push('\n');

    if path.exists() {
        let existing = std::fs::read_to_string(path)?;
        if existing == text {
            return Ok(false);
        }
    }
    std::fs::write(path, text)?;
    Ok(true)
}

fn remove_codex_hook(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let text = std::fs::read_to_string(path)?;
    if text.trim().is_empty() {
        return Ok(false);
    }
    let mut hooks: Value = serde_json::from_str(&text)?;
    let Some(pre) = hooks.get_mut("PreToolUse") else {
        return Ok(false);
    };
    let Some(arr) = pre.as_array_mut() else {
        return Ok(false);
    };
    let before = arr.len();
    arr.retain(|e| !is_sigil_hook(e));
    if arr.len() == before {
        return Ok(false);
    }
    if arr.is_empty() {
        hooks
            .as_object_mut()
            .expect("root is object")
            .remove("PreToolUse");
    }
    let empty = hooks.as_object().map(|o| o.is_empty()).unwrap_or(false);
    if empty {
        std::fs::remove_file(path)?;
    } else {
        let mut out = serde_json::to_string_pretty(&hooks)?;
        out.push('\n');
        std::fs::write(path, out)?;
    }
    Ok(true)
}

fn is_sigil_hook(entry: &Value) -> bool {
    entry
        .get("command")
        .and_then(|c| c.as_str())
        .map(|s| s.contains(HOOK_ID))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("sigil_codex_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn install_creates_agents_md_and_hooks() {
        let root = tmpdir("install_fresh");
        install(&root).unwrap();
        assert!(root.join("AGENTS.md").exists());
        let v: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join(".codex/hooks.json")).unwrap())
                .unwrap();
        let pre = v["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 1);
        assert!(is_sigil_hook(&pre[0]));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn install_preserves_existing_agents_md_content() {
        let root = tmpdir("install_merge_md");
        let agents = root.join("AGENTS.md");
        std::fs::write(&agents, "# AGENTS\n\nexisting project rules.\n").unwrap();
        install(&root).unwrap();
        let content = std::fs::read_to_string(&agents).unwrap();
        assert!(content.contains("existing project rules"));
        assert!(content.contains("sigil map"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn install_is_idempotent() {
        let root = tmpdir("install_idempotent");
        install(&root).unwrap();
        let first = std::fs::read_to_string(root.join(".codex/hooks.json")).unwrap();
        install(&root).unwrap();
        let second = std::fs::read_to_string(root.join(".codex/hooks.json")).unwrap();
        assert_eq!(first, second);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn uninstall_restores_prior_state() {
        let root = tmpdir("uninstall_merge");
        std::fs::write(
            root.join("AGENTS.md"),
            "# AGENTS\n\nexisting.\n",
        )
        .unwrap();
        install(&root).unwrap();
        uninstall(&root).unwrap();
        let content = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(content.contains("existing."));
        assert!(!content.contains("sigil map"));
        assert!(
            !root.join(".codex/hooks.json").exists(),
            "empty hooks.json cleaned up"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn uninstall_without_install_is_noop() {
        let root = tmpdir("uninstall_noop");
        let steps = uninstall(&root).unwrap();
        assert!(matches!(steps[0], UninstallStep::AgentsMd(false)));
        assert!(matches!(steps[1], UninstallStep::Hooks(false)));
        std::fs::remove_dir_all(&root).ok();
    }
}
