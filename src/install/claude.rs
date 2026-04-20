//! Claude Code installer.
//!
//! Two artifacts:
//!   1. `CLAUDE.md` — marker-delimited capability block appended (or
//!      upgraded in place).
//!   2. `.claude/settings.json` — PreToolUse hook that fires before Grep
//!      and Glob and prints a one-line reminder about sigil's commands
//!      so Claude notices the tool exists before searching the repo
//!      blind.
//!
//! The hook is wired via the Claude Code settings.json schema: an entry
//! in `hooks.PreToolUse` with `matcher: "Grep|Glob"` and a shell command
//! that echoes a single-line hint and exits 0. No exit-2 blocking
//! behavior — hooks should inform, not interrupt.

use std::path::Path;

use anyhow::{Context as _, Result};
use serde_json::{json, Value};

use super::{capability_block, remove_marker_block, upsert_marker_block, UpsertResult};

const HOOK_ID: &str = "sigil-hint";
const HINT_LINE: &str = "sigil available in this repo: use `sigil map` for orientation, `sigil context <symbol>` for a focused bundle, `sigil callers/callees` for exact lookups. See .sigil/SIGIL_MAP.md if present.";

/// Install both the markdown block and the PreToolUse hook.
pub fn install(root: &Path) -> Result<Vec<InstallStep>> {
    let mut steps = Vec::new();

    // CLAUDE.md block — upsert_marker_block handles the marker wrapping.
    let claude_md = root.join("CLAUDE.md");
    let result = upsert_marker_block(&claude_md, &capability_block())
        .with_context(|| format!("upsert {}", claude_md.display()))?;
    steps.push(InstallStep::ClaudeMd(result));

    // .claude/settings.json hook
    let settings_path = root.join(".claude").join("settings.json");
    let changed = upsert_claude_hook(&settings_path)
        .with_context(|| format!("update {}", settings_path.display()))?;
    steps.push(InstallStep::Settings(if changed {
        UpsertResult::Updated
    } else {
        UpsertResult::Unchanged
    }));

    Ok(steps)
}

/// Remove the block and the hook entry. Leaves unrelated settings alone.
pub fn uninstall(root: &Path) -> Result<Vec<UninstallStep>> {
    let mut steps = Vec::new();

    let claude_md = root.join("CLAUDE.md");
    let removed_md = remove_marker_block(&claude_md)
        .with_context(|| format!("clean {}", claude_md.display()))?;
    steps.push(UninstallStep::ClaudeMd(removed_md));

    let settings_path = root.join(".claude").join("settings.json");
    let removed_hook = remove_claude_hook(&settings_path)
        .with_context(|| format!("clean {}", settings_path.display()))?;
    steps.push(UninstallStep::Settings(removed_hook));

    Ok(steps)
}

/// Public for CLI printing / tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStep {
    ClaudeMd(UpsertResult),
    Settings(UpsertResult),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallStep {
    ClaudeMd(bool),
    Settings(bool),
}

fn upsert_claude_hook(path: &Path) -> Result<bool> {
    let mut settings: Value = if path.exists() {
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

    let hooks_obj = settings
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("settings.json root must be an object"))?
        .entry("hooks".to_string())
        .or_insert_with(|| json!({}));
    let hooks_obj = hooks_obj
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("settings.hooks must be an object"))?;
    let pre = hooks_obj
        .entry("PreToolUse".to_string())
        .or_insert_with(|| json!([]));
    let pre_arr = pre
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("hooks.PreToolUse must be an array"))?;

    // Remove any prior sigil entry (recognized via the HOOK_ID-bearing
    // command) to keep the upsert idempotent.
    pre_arr.retain(|entry| !is_sigil_hook(entry));

    let sigil_entry = json!({
        "matcher": "Grep|Glob",
        "hooks": [
            {
                "type": "command",
                "command": format!("echo '{}' # {}", super::shell_escape_single_quoted(HINT_LINE), HOOK_ID),
            }
        ]
    });
    pre_arr.push(sigil_entry);

    let mut text = serde_json::to_string_pretty(&settings)?;
    text.push('\n');

    // Idempotency check — avoid touching mtime if the file already matches.
    if path.exists() {
        let existing = std::fs::read_to_string(path)?;
        if existing == text {
            return Ok(false);
        }
    }
    std::fs::write(path, text)?;
    Ok(true)
}

fn remove_claude_hook(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let text = std::fs::read_to_string(path)?;
    if text.trim().is_empty() {
        return Ok(false);
    }
    let mut settings: Value = serde_json::from_str(&text)
        .with_context(|| format!("parse {}", path.display()))?;
    let Some(hooks) = settings.get_mut("hooks") else {
        return Ok(false);
    };
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
    // Prune empty containers so we don't leave skeleton keys behind.
    if arr.is_empty() {
        if let Some(obj) = hooks.as_object_mut() {
            obj.remove("PreToolUse");
            let is_empty = obj.is_empty();
            if is_empty {
                settings
                    .as_object_mut()
                    .expect("settings root is object")
                    .remove("hooks");
            }
        }
    }
    // If settings is entirely empty now, delete the file; otherwise write back.
    let empty_now = settings
        .as_object()
        .map(|o| o.is_empty())
        .unwrap_or(false);
    if empty_now {
        std::fs::remove_file(path)?;
    } else {
        let mut out = serde_json::to_string_pretty(&settings)?;
        out.push('\n');
        std::fs::write(path, out)?;
    }
    Ok(true)
}

fn is_sigil_hook(entry: &Value) -> bool {
    let Some(hooks) = entry.get("hooks").and_then(|h| h.as_array()) else {
        return false;
    };
    hooks.iter().any(|h| {
        h.get("command")
            .and_then(|c| c.as_str())
            .map(|s| s.contains(HOOK_ID))
            .unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("sigil_claude_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn install_creates_claude_md_and_settings() {
        let root = tmpdir("install_fresh");
        let steps = install(&root).unwrap();
        assert!(matches!(steps[0], InstallStep::ClaudeMd(UpsertResult::Created)));
        assert!(matches!(steps[1], InstallStep::Settings(UpsertResult::Updated)));
        assert!(root.join("CLAUDE.md").exists());
        let settings: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join(".claude/settings.json")).unwrap())
                .unwrap();
        let arr = settings["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert!(is_sigil_hook(&arr[0]));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn install_preserves_existing_settings() {
        let root = tmpdir("install_merge");
        let settings_path = root.join(".claude").join("settings.json");
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            r#"{
  "theme": "dark",
  "hooks": {
    "PreToolUse": [
      {"matcher": "Write", "hooks": [{"type": "command", "command": "echo user-hook"}]}
    ]
  }
}
"#,
        )
        .unwrap();
        install(&root).unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(v["theme"], "dark");
        let pre = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 2, "user hook preserved + sigil hook added");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn install_is_idempotent() {
        let root = tmpdir("install_idempotent");
        install(&root).unwrap();
        let first = std::fs::read_to_string(root.join(".claude/settings.json")).unwrap();
        let second_steps = install(&root).unwrap();
        let second = std::fs::read_to_string(root.join(".claude/settings.json")).unwrap();
        assert_eq!(first, second, "second install is a no-op");
        assert!(matches!(
            second_steps[1],
            InstallStep::Settings(UpsertResult::Unchanged)
        ));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn uninstall_removes_sigil_without_touching_other_hooks() {
        let root = tmpdir("uninstall_merge");
        let settings_path = root.join(".claude").join("settings.json");
        std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
        std::fs::write(
            &settings_path,
            r#"{
  "theme": "dark",
  "hooks": {
    "PreToolUse": [
      {"matcher": "Write", "hooks": [{"type": "command", "command": "echo user-hook"}]}
    ]
  }
}
"#,
        )
        .unwrap();
        install(&root).unwrap();
        let steps = uninstall(&root).unwrap();
        assert!(matches!(steps[0], UninstallStep::ClaudeMd(true)));
        assert!(matches!(steps[1], UninstallStep::Settings(true)));

        let v: Value = serde_json::from_str(&std::fs::read_to_string(&settings_path).unwrap()).unwrap();
        assert_eq!(v["theme"], "dark");
        let pre = v["hooks"]["PreToolUse"].as_array().unwrap();
        assert_eq!(pre.len(), 1);
        let cmd = pre[0]["hooks"][0]["command"].as_str().unwrap();
        assert!(cmd.contains("user-hook"));
        assert!(!cmd.contains(HOOK_ID));

        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn uninstall_without_install_is_noop() {
        let root = tmpdir("uninstall_empty");
        let steps = uninstall(&root).unwrap();
        assert!(matches!(steps[0], UninstallStep::ClaudeMd(false)));
        assert!(matches!(steps[1], UninstallStep::Settings(false)));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn uninstall_deletes_settings_if_emptied() {
        let root = tmpdir("uninstall_cleanup");
        install(&root).unwrap();
        uninstall(&root).unwrap();
        assert!(
            !root.join(".claude/settings.json").exists(),
            "empty settings.json should be deleted"
        );
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn escape_single_quotes_handles_shell_safety() {
        assert_eq!(super::super::shell_escape_single_quoted("hello"), "hello");
        assert_eq!(super::super::shell_escape_single_quoted("it's"), "it'\\''s");
    }
}
