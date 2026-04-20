//! Git hook installer — `sigil hook install`.
//!
//! Writes `.git/hooks/post-commit` and `.git/hooks/post-checkout` that
//! re-run `sigil index` incrementally in the background so `.sigil/`
//! and `.sigil/rank.json` stay fresh without a daemon.
//!
//! The hook scripts are idempotent: re-running install overwrites only
//! sigil's script (identified via a sentinel comment). If a user has
//! their own post-commit hook, `sigil hook install` chains sigil onto
//! the existing script rather than clobbering it.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

const SENTINEL: &str = "# sigil:post-commit";
const CHECKOUT_SENTINEL: &str = "# sigil:post-checkout";
const HOOKS: &[&str] = &["post-commit", "post-checkout"];

pub fn install(root: &Path) -> Result<Vec<HookStep>> {
    let git_hooks = resolve_hooks_dir(root)?;
    let mut steps = Vec::new();
    for hook in HOOKS {
        let sentinel = sentinel_for(hook);
        let path = git_hooks.join(hook);
        let result = upsert_hook_script(&path, sentinel)
            .with_context(|| format!("install {}", path.display()))?;
        steps.push(HookStep {
            name: (*hook).to_string(),
            result,
        });
    }
    Ok(steps)
}

pub fn uninstall(root: &Path) -> Result<Vec<HookStep>> {
    let git_hooks = resolve_hooks_dir(root)?;
    let mut steps = Vec::new();
    for hook in HOOKS {
        let sentinel = sentinel_for(hook);
        let path = git_hooks.join(hook);
        let result = remove_hook_script(&path, sentinel)
            .with_context(|| format!("clean {}", path.display()))?;
        steps.push(HookStep {
            name: (*hook).to_string(),
            result: if result {
                HookResult::Removed
            } else {
                HookResult::NotPresent
            },
        });
    }
    Ok(steps)
}

#[derive(Debug, Clone)]
pub struct HookStep {
    pub name: String,
    pub result: HookResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookResult {
    Created,
    Updated,
    Unchanged,
    Removed,
    NotPresent,
}

fn sentinel_for(hook: &str) -> &'static str {
    match hook {
        "post-commit" => SENTINEL,
        "post-checkout" => CHECKOUT_SENTINEL,
        _ => SENTINEL,
    }
}

/// Run `git rev-parse --git-path hooks` to handle worktrees / custom
/// hooksPath settings. Falls back to `.git/hooks/` on any error.
fn resolve_hooks_dir(root: &Path) -> Result<PathBuf> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--git-path", "hooks"])
        .current_dir(root)
        .output()
        .with_context(|| "git rev-parse failed — is this a git repo?")?;
    if !out.status.success() {
        anyhow::bail!(
            "git rev-parse --git-path hooks failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let rel = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let rel_path = PathBuf::from(&rel);
    let absolute = if rel_path.is_absolute() {
        rel_path
    } else {
        root.join(rel_path)
    };
    std::fs::create_dir_all(&absolute)?;
    Ok(absolute)
}

fn sigil_block(sentinel: &str) -> String {
    format!(
        "{sentinel}\n# Rebuild sigil's index in the background. Exits quietly; any\n# parse issues surface on the next explicit `sigil index` run.\n( sigil index --no-rank >/dev/null 2>&1 & ) || true\n{sentinel} end\n"
    )
}

fn upsert_hook_script(path: &Path, sentinel: &str) -> Result<HookResult> {
    let block = sigil_block(sentinel);
    if !path.exists() {
        let script = format!("#!/bin/sh\n{block}");
        std::fs::write(path, script)?;
        make_executable(path)?;
        return Ok(HookResult::Created);
    }
    let existing = std::fs::read_to_string(path)?;
    if let Some((before, after)) = split_existing_block(&existing, sentinel) {
        let updated = format!("{before}{block}{after}");
        if updated == existing {
            return Ok(HookResult::Unchanged);
        }
        std::fs::write(path, updated)?;
        make_executable(path)?;
        return Ok(HookResult::Updated);
    }
    // Append sigil block, preserving whatever the user had.
    let mut script = existing;
    if !script.ends_with('\n') {
        script.push('\n');
    }
    script.push_str(&block);
    std::fs::write(path, script)?;
    make_executable(path)?;
    Ok(HookResult::Updated)
}

fn remove_hook_script(path: &Path, sentinel: &str) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let existing = std::fs::read_to_string(path)?;
    let Some((before, after)) = split_existing_block(&existing, sentinel) else {
        return Ok(false);
    };
    let mut remaining = format!("{}{}", before.trim_end(), after);
    if !remaining.ends_with('\n') {
        remaining.push('\n');
    }
    // If the file becomes just a shebang or blank, drop it so we don't leave
    // orphan hooks around.
    if remaining.trim() == "#!/bin/sh" || remaining.trim().is_empty() {
        std::fs::remove_file(path)?;
    } else {
        std::fs::write(path, remaining)?;
    }
    Ok(true)
}

/// Return (before_block, after_block) byte slices. None when the sentinel
/// isn't present.
fn split_existing_block<'a>(existing: &'a str, sentinel: &str) -> Option<(&'a str, &'a str)> {
    let begin = existing.find(sentinel)?;
    let end_marker = format!("{sentinel} end\n");
    let end_offset = existing[begin..].find(&end_marker)?;
    let end_abs = begin + end_offset + end_marker.len();
    Some((&existing[..begin], &existing[end_abs..]))
}

#[cfg(unix)]
fn make_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_repo(name: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("sigil_githook_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        // Initialize a git repo so `git rev-parse --git-path hooks` works.
        let out = std::process::Command::new("git")
            .args(["init", "-q"])
            .current_dir(&p)
            .output()
            .expect("git init");
        assert!(out.status.success(), "git init failed in test");
        p
    }

    #[test]
    fn install_creates_both_hooks_and_marks_executable() {
        let root = setup_repo("create");
        let steps = install(&root).unwrap();
        assert_eq!(steps.len(), 2);
        for s in &steps {
            assert!(matches!(s.result, HookResult::Created));
        }
        let post_commit = root.join(".git/hooks/post-commit");
        assert!(post_commit.exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&post_commit).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o755);
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn install_preserves_existing_user_hook() {
        let root = setup_repo("merge");
        let hooks = root.join(".git/hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        std::fs::write(
            hooks.join("post-commit"),
            "#!/bin/sh\n# user script\necho hello-from-user\n",
        )
        .unwrap();
        install(&root).unwrap();
        let content = std::fs::read_to_string(hooks.join("post-commit")).unwrap();
        assert!(content.contains("hello-from-user"), "user content preserved");
        assert!(content.contains("sigil index --no-rank"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn install_is_idempotent() {
        let root = setup_repo("idempotent");
        install(&root).unwrap();
        let first =
            std::fs::read_to_string(root.join(".git/hooks/post-commit")).unwrap();
        let steps = install(&root).unwrap();
        let second =
            std::fs::read_to_string(root.join(".git/hooks/post-commit")).unwrap();
        assert_eq!(first, second);
        for s in steps {
            assert!(matches!(s.result, HookResult::Unchanged));
        }
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn uninstall_leaves_user_hook_intact() {
        let root = setup_repo("rm_sibling");
        let hooks = root.join(".git/hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        std::fs::write(
            hooks.join("post-commit"),
            "#!/bin/sh\n# user script\necho hello-from-user\n",
        )
        .unwrap();
        install(&root).unwrap();
        uninstall(&root).unwrap();
        let content = std::fs::read_to_string(hooks.join("post-commit")).unwrap();
        assert!(content.contains("hello-from-user"));
        assert!(!content.contains("sigil index"));
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn uninstall_deletes_hook_files_created_solely_by_sigil() {
        let root = setup_repo("rm_solo");
        install(&root).unwrap();
        uninstall(&root).unwrap();
        assert!(!root.join(".git/hooks/post-commit").exists());
        assert!(!root.join(".git/hooks/post-checkout").exists());
        std::fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn uninstall_without_install_is_noop() {
        let root = setup_repo("rm_noop");
        let steps = uninstall(&root).unwrap();
        for s in steps {
            assert!(matches!(s.result, HookResult::NotPresent));
        }
        std::fs::remove_dir_all(&root).ok();
    }
}
