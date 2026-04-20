//! GitHub Copilot CLI installer.
//!
//! Copilot CLI loads skills from `~/.copilot/skills/<name>/SKILL.md`.
//! Unlike the other installers, this one writes to the user's home
//! directory rather than the project root — skills are per-user, not
//! per-project. `--root` is still accepted for consistency but it's
//! ignored for the install path (the caller's home still takes effect).
//!
//! Override via `SIGIL_COPILOT_HOME` for testing.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

use super::{capability_block, UpsertResult};

const SKILL_DIR: &str = ".copilot/skills/sigil";

fn skill_home() -> Result<PathBuf> {
    if let Ok(custom) = std::env::var("SIGIL_COPILOT_HOME") {
        return Ok(PathBuf::from(custom));
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("HOME is not set — cannot locate ~/.copilot/"))?;
    Ok(home)
}

pub fn install(_root: &Path) -> Result<UpsertResult> {
    let home = skill_home()?;
    let dir = home.join(SKILL_DIR);
    std::fs::create_dir_all(&dir).with_context(|| format!("mkdir {}", dir.display()))?;
    let path = dir.join("SKILL.md");
    let content = format!(
        "# sigil skill\n\n{}\n",
        capability_block()
    );
    if path.exists() {
        let existing = std::fs::read_to_string(&path)?;
        if existing == content {
            return Ok(UpsertResult::Unchanged);
        }
        std::fs::write(&path, content)?;
        return Ok(UpsertResult::Updated);
    }
    std::fs::write(&path, content)?;
    Ok(UpsertResult::Created)
}

pub fn uninstall(_root: &Path) -> Result<bool> {
    let home = skill_home()?;
    let dir = home.join(SKILL_DIR);
    if !dir.exists() {
        return Ok(false);
    }
    std::fs::remove_dir_all(&dir).with_context(|| format!("remove {}", dir.display()))?;
    // Leave the parent `~/.copilot/skills/` alone — other skills may live there.
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    /// The copilot tests all touch the same SIGIL_COPILOT_HOME env var.
    /// Rust's default test runner parallelizes, so we serialize these via
    /// a process-wide mutex. Each test acquires the guard for its whole
    /// body; the guard drops at test end and releases the lock.
    fn env_guard() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    fn fresh_home(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("sigil_copilot_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        // SAFETY: Tests holding `env_guard()` are the only ones touching
        // SIGIL_COPILOT_HOME, so no concurrent reader/writer races.
        unsafe {
            std::env::set_var("SIGIL_COPILOT_HOME", &p);
        }
        p
    }

    fn cleanup(home: &Path) {
        std::fs::remove_dir_all(home).ok();
        unsafe {
            std::env::remove_var("SIGIL_COPILOT_HOME");
        }
    }

    #[test]
    fn install_creates_skill_file() {
        let _g = env_guard();
        let home = fresh_home("install");
        let r = install(Path::new(".")).unwrap();
        assert_eq!(r, UpsertResult::Created);
        let path = home.join(SKILL_DIR).join("SKILL.md");
        assert!(path.exists());
        let content = std::fs::read_to_string(path).unwrap();
        assert!(content.contains("sigil map"));
        cleanup(&home);
    }

    #[test]
    fn install_is_idempotent() {
        let _g = env_guard();
        let home = fresh_home("idempotent");
        install(Path::new(".")).unwrap();
        let r = install(Path::new(".")).unwrap();
        assert_eq!(r, UpsertResult::Unchanged);
        cleanup(&home);
    }

    #[test]
    fn uninstall_removes_only_sigil_skill() {
        let _g = env_guard();
        let home = fresh_home("uninstall");
        // Seed another skill so we can verify it stays intact.
        let other = home.join(".copilot/skills/other");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("SKILL.md"), "other content").unwrap();
        install(Path::new(".")).unwrap();
        assert!(uninstall(Path::new(".")).unwrap());
        assert!(!home.join(SKILL_DIR).exists());
        assert!(other.join("SKILL.md").exists(), "sibling skill preserved");
        cleanup(&home);
    }

    #[test]
    fn uninstall_without_install_is_noop() {
        let _g = env_guard();
        let home = fresh_home("noop");
        assert!(!uninstall(Path::new(".")).unwrap());
        cleanup(&home);
    }
}
