//! Aider installer.
//!
//! Aider doesn't support tool hooks, so the integration is a single
//! marker-scoped capability block in `AGENTS.md`. Static, always-on
//! context for the agent via the existing read-every-session mechanism.

use std::path::Path;

use anyhow::{Context as _, Result};

use super::{capability_block, remove_marker_block, upsert_marker_block, UpsertResult};

pub fn install(root: &Path) -> Result<UpsertResult> {
    let path = root.join("AGENTS.md");
    upsert_marker_block(&path, &capability_block())
        .with_context(|| format!("upsert {}", path.display()))
        .map_err(Into::into)
}

pub fn uninstall(root: &Path) -> Result<bool> {
    let path = root.join("AGENTS.md");
    remove_marker_block(&path)
        .with_context(|| format!("clean {}", path.display()))
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("sigil_aider_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn install_writes_agents_md() {
        let root = tmpdir("install");
        let r = install(&root).unwrap();
        assert_eq!(r, UpsertResult::Created);
        let content = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(content.contains("sigil map"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn install_preserves_existing_agents_md() {
        let root = tmpdir("merge");
        std::fs::write(root.join("AGENTS.md"), "# AGENTS\n\nuser rules.\n").unwrap();
        install(&root).unwrap();
        let content = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(content.contains("user rules"));
        assert!(content.contains("sigil map"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn uninstall_roundtrips_cleanly() {
        let root = tmpdir("rt");
        std::fs::write(root.join("AGENTS.md"), "# AGENTS\n\npre-existing.\n").unwrap();
        install(&root).unwrap();
        assert!(uninstall(&root).unwrap());
        let content = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(content.contains("pre-existing"));
        assert!(!content.contains("sigil map"));
        std::fs::remove_dir_all(&root).ok();
    }
}
