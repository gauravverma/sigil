//! Gemini CLI installer.
//!
//! Same shape as Claude + Codex: a capability block in a top-level
//! `GEMINI.md` plus a `BeforeTool` hook in `.gemini/settings.json` that
//! echoes a one-line reminder before file-reading tools fire.

use std::path::Path;

use anyhow::{Context as _, Result};
use serde_json::{json, Value};

use super::{capability_block, remove_marker_block, upsert_marker_block, UpsertResult};

const HOOK_ID: &str = "sigil-hint";
const HINT_LINE: &str = "sigil in this repo — for structural questions reach for sigil FIRST: `sigil where X` (find definition), `sigil context X` (full bundle), `sigil callers/callees X`, `sigil symbols F --depth 1`. Empty result prints `Did you mean?` on stderr — retry with suggestion before grep.";

pub fn install(root: &Path) -> Result<Vec<InstallStep>> {
    let mut steps = Vec::new();

    let gemini_md = root.join("GEMINI.md");
    let md = upsert_marker_block(&gemini_md, &capability_block())
        .with_context(|| format!("upsert {}", gemini_md.display()))?;
    steps.push(InstallStep::GeminiMd(md));

    let settings = root.join(".gemini").join("settings.json");
    let changed =
        upsert_gemini_hook(&settings).with_context(|| format!("update {}", settings.display()))?;
    steps.push(InstallStep::Settings(if changed {
        UpsertResult::Updated
    } else {
        UpsertResult::Unchanged
    }));
    Ok(steps)
}

pub fn uninstall(root: &Path) -> Result<Vec<UninstallStep>> {
    let mut steps = Vec::new();
    let gemini_md = root.join("GEMINI.md");
    steps.push(UninstallStep::GeminiMd(remove_marker_block(&gemini_md)?));
    let settings = root.join(".gemini").join("settings.json");
    steps.push(UninstallStep::Settings(remove_gemini_hook(&settings)?));
    Ok(steps)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStep {
    GeminiMd(UpsertResult),
    Settings(UpsertResult),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UninstallStep {
    GeminiMd(bool),
    Settings(bool),
}

fn upsert_gemini_hook(path: &Path) -> Result<bool> {
    let mut settings: Value = if path.exists() {
        let text = std::fs::read_to_string(path)?;
        if text.trim().is_empty() {
            json!({})
        } else {
            serde_json::from_str(&text)?
        }
    } else {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        json!({})
    };

    let root_obj = settings
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("settings.json root must be an object"))?;
    let hooks = root_obj
        .entry("hooks".to_string())
        .or_insert_with(|| json!({}));
    let hooks_obj = hooks
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("settings.hooks must be an object"))?;
    let before = hooks_obj
        .entry("BeforeTool".to_string())
        .or_insert_with(|| json!([]));
    let arr = before
        .as_array_mut()
        .ok_or_else(|| anyhow::anyhow!("BeforeTool must be an array"))?;
    arr.retain(|e| !is_sigil_hook(e));
    arr.push(json!({
        "matcher": "ReadFile|ReadManyFiles|Grep|Glob",
        "command": format!("echo '{}' # {}", super::shell_escape_single_quoted(HINT_LINE), HOOK_ID),
    }));

    let mut text = serde_json::to_string_pretty(&settings)?;
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

fn remove_gemini_hook(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let text = std::fs::read_to_string(path)?;
    if text.trim().is_empty() {
        return Ok(false);
    }
    let mut settings: Value = serde_json::from_str(&text)?;
    let Some(hooks) = settings.get_mut("hooks") else {
        return Ok(false);
    };
    let Some(before) = hooks.get_mut("BeforeTool") else {
        return Ok(false);
    };
    let Some(arr) = before.as_array_mut() else {
        return Ok(false);
    };
    let before_len = arr.len();
    arr.retain(|e| !is_sigil_hook(e));
    if arr.len() == before_len {
        return Ok(false);
    }
    if arr.is_empty() {
        if let Some(obj) = hooks.as_object_mut() {
            obj.remove("BeforeTool");
            if obj.is_empty() {
                settings.as_object_mut().unwrap().remove("hooks");
            }
        }
    }
    let empty = settings.as_object().map(|o| o.is_empty()).unwrap_or(false);
    if empty {
        std::fs::remove_file(path)?;
    } else {
        let mut out = serde_json::to_string_pretty(&settings)?;
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
        let p = std::env::temp_dir().join(format!("sigil_gemini_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn install_creates_gemini_md_and_hook() {
        let root = tmpdir("fresh");
        install(&root).unwrap();
        assert!(root.join("GEMINI.md").exists());
        let v: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join(".gemini/settings.json")).unwrap())
                .unwrap();
        let arr = v["hooks"]["BeforeTool"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert!(is_sigil_hook(&arr[0]));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn install_idempotent_and_uninstall_clean() {
        let root = tmpdir("rt");
        install(&root).unwrap();
        let first = std::fs::read_to_string(root.join(".gemini/settings.json")).unwrap();
        install(&root).unwrap();
        let second = std::fs::read_to_string(root.join(".gemini/settings.json")).unwrap();
        assert_eq!(first, second);
        uninstall(&root).unwrap();
        assert!(!root.join(".gemini/settings.json").exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn uninstall_preserves_sibling_user_hooks() {
        let root = tmpdir("sibling");
        let settings = root.join(".gemini/settings.json");
        std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
        std::fs::write(
            &settings,
            r#"{"hooks": {"BeforeTool": [{"matcher": "Write", "command": "echo user"}]}}"#,
        )
        .unwrap();
        install(&root).unwrap();
        uninstall(&root).unwrap();
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&settings).unwrap()).unwrap();
        let arr = v["hooks"]["BeforeTool"].as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert!(!is_sigil_hook(&arr[0]));
        std::fs::remove_dir_all(&root).ok();
    }
}
