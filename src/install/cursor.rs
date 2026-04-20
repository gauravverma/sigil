//! Cursor installer.
//!
//! Cursor loads rules with `alwaysApply: true` into every conversation —
//! no hook mechanism needed, no settings.json surgery. Single file at
//! `.cursor/rules/sigil.mdc` wrapped in the standard sigil markers so
//! we can upgrade idempotently.
//!
//! The rule front-matter includes `alwaysApply: true`, `description`,
//! and `globs: ["**/*"]` per Cursor's schema. The body is the same
//! capability block every installer shares.

use std::path::Path;

use anyhow::{Context as _, Result};

use super::{capability_block, UpsertResult};

const RULE_PATH: &str = ".cursor/rules/sigil.mdc";

pub fn install(root: &Path) -> Result<UpsertResult> {
    let path = root.join(RULE_PATH);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let content = format!(
        "---\n\
description: Sigil — deterministic structural code intelligence.\n\
alwaysApply: true\n\
globs:\n\
  - \"**/*\"\n\
---\n\
\n\
{}\n",
        capability_block()
    );
    if path.exists() {
        let existing = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        if existing == content {
            return Ok(UpsertResult::Unchanged);
        }
        std::fs::write(&path, content)?;
        return Ok(UpsertResult::Updated);
    }
    std::fs::write(&path, content)?;
    Ok(UpsertResult::Created)
}

pub fn uninstall(root: &Path) -> Result<bool> {
    let path = root.join(RULE_PATH);
    if !path.exists() {
        return Ok(false);
    }
    std::fs::remove_file(&path)?;
    // If the rules directory is now empty, clean it up (don't touch
    // .cursor itself — users may have other configuration there).
    if let Some(parent) = path.parent()
        && parent.read_dir().map(|mut d| d.next().is_none()).unwrap_or(false)
    {
        let _ = std::fs::remove_dir(parent);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("sigil_cursor_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn install_creates_rule_file_with_frontmatter() {
        let root = tmpdir("install_fresh");
        let r = install(&root).unwrap();
        assert_eq!(r, UpsertResult::Created);
        let content = std::fs::read_to_string(root.join(RULE_PATH)).unwrap();
        assert!(content.starts_with("---"));
        assert!(content.contains("alwaysApply: true"));
        assert!(content.contains("sigil map"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn install_is_idempotent() {
        let root = tmpdir("install_idempotent");
        install(&root).unwrap();
        let r = install(&root).unwrap();
        assert_eq!(r, UpsertResult::Unchanged);
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn uninstall_removes_file_and_empty_rules_dir() {
        let root = tmpdir("uninstall");
        install(&root).unwrap();
        let removed = uninstall(&root).unwrap();
        assert!(removed);
        assert!(!root.join(RULE_PATH).exists());
        assert!(!root.join(".cursor/rules").exists(), "empty rules dir cleaned up");
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn uninstall_without_install_is_noop() {
        let root = tmpdir("uninstall_noop");
        assert!(!uninstall(&root).unwrap());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn uninstall_preserves_sibling_rule_files() {
        let root = tmpdir("uninstall_sibling");
        let rules = root.join(".cursor/rules");
        std::fs::create_dir_all(&rules).unwrap();
        std::fs::write(rules.join("user.mdc"), "user rule").unwrap();
        install(&root).unwrap();
        uninstall(&root).unwrap();
        assert!(rules.join("user.mdc").exists(), "other rules left intact");
        std::fs::remove_dir_all(&root).ok();
    }
}
