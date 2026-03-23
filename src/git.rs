use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    Added,
    Modified,
    Deleted,
}

#[derive(Debug, Clone)]
pub struct FileChange {
    pub path: String,
    pub status: FileStatus,
}

/// Resolve a git ref to a full SHA.
pub fn resolve_ref(root: &Path, ref_name: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args(["rev-parse", ref_name])
        .current_dir(root)
        .output()
        .map_err(|e| format!("failed to run git: {}", e))?;

    if !output.status.success() {
        return Err(format!("git rev-parse failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Get file contents at a specific git ref.
pub fn file_at_ref(root: &Path, ref_name: &str, file_path: &str) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .args(["show", &format!("{}:{}", ref_name, file_path)])
        .current_dir(root)
        .output()
        .map_err(|e| format!("failed to run git: {}", e))?;

    if !output.status.success() {
        return Err(format!("file not found at {}:{}", ref_name, file_path));
    }
    Ok(output.stdout)
}

/// Get list of changed files between two refs.
pub fn changed_files(root: &Path, base_ref: &str, head_ref: &str) -> Result<Vec<FileChange>, String> {
    let output = Command::new("git")
        .args(["diff", "--name-status", "--no-renames", base_ref, head_ref])
        .current_dir(root)
        .output()
        .map_err(|e| format!("failed to run git: {}", e))?;

    if !output.status.success() {
        return Err(format!("git diff failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut changes = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 2 {
            continue;
        }
        let status = match parts[0].chars().next() {
            Some('A') => FileStatus::Added,
            Some('M') => FileStatus::Modified,
            Some('D') => FileStatus::Deleted,
            _ => continue,
        };
        changes.push(FileChange {
            path: parts.last().unwrap().to_string(),
            status,
        });
    }

    Ok(changes)
}

/// Parse a ref spec like "main..HEAD", "HEAD~1", or "main...feature".
pub fn parse_ref_spec(spec: &str) -> Result<(String, String), String> {
    if let Some((base, head)) = spec.split_once("...") {
        Ok((base.to_string(), head.to_string()))
    } else if let Some((base, head)) = spec.split_once("..") {
        Ok((base.to_string(), head.to_string()))
    } else {
        // Single ref: compare it against HEAD
        Ok((spec.to_string(), "HEAD".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn make_temp_repo() -> std::path::PathBuf {
        let tid = std::thread::current().id();
        let dir = std::env::temp_dir().join(format!("sigil_git_test_{}_{:?}", std::process::id(), tid));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let run = |args: &[&str]| {
            Command::new("git").args(args).current_dir(&dir).output().expect("git failed")
        };
        run(&["init"]);
        run(&["config", "user.email", "test@test.com"]);
        run(&["config", "user.name", "Test"]);
        std::fs::write(dir.join("hello.py"), "def hello():\n    return 1\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-m", "initial"]);
        dir
    }

    #[test]
    fn resolve_ref_head() {
        let dir = make_temp_repo();
        let sha = resolve_ref(&dir, "HEAD").unwrap();
        assert_eq!(sha.len(), 40);
        assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn file_at_ref_reads_content() {
        let dir = make_temp_repo();
        let content = file_at_ref(&dir, "HEAD", "hello.py").unwrap();
        assert_eq!(content, b"def hello():\n    return 1\n");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn file_at_ref_missing_file() {
        let dir = make_temp_repo();
        let result = file_at_ref(&dir, "HEAD", "nonexistent.py");
        assert!(result.is_err());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn changed_files_detects_modification() {
        let dir = make_temp_repo();
        let run = |args: &[&str]| {
            Command::new("git").args(args).current_dir(&dir).output().unwrap()
        };
        std::fs::write(dir.join("hello.py"), "def hello():\n    return 2\n").unwrap();
        run(&["add", "."]);
        run(&["commit", "-m", "modify"]);
        let changes = changed_files(&dir, "HEAD~1", "HEAD").unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "hello.py");
        assert_eq!(changes[0].status, FileStatus::Modified);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn changed_files_detects_add_and_delete() {
        let dir = make_temp_repo();
        let run = |args: &[&str]| {
            Command::new("git").args(args).current_dir(&dir).output().unwrap()
        };
        std::fs::write(dir.join("new.py"), "x = 1\n").unwrap();
        std::fs::remove_file(dir.join("hello.py")).unwrap();
        run(&["add", "."]);
        run(&["commit", "-m", "add and delete"]);
        let changes = changed_files(&dir, "HEAD~1", "HEAD").unwrap();
        let statuses: Vec<_> = changes.iter().map(|c| (&c.path, &c.status)).collect();
        assert!(statuses.contains(&(&"hello.py".to_string(), &FileStatus::Deleted)));
        assert!(statuses.contains(&(&"new.py".to_string(), &FileStatus::Added)));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parse_ref_spec_two_dots() {
        let (base, head) = parse_ref_spec("main..HEAD").unwrap();
        assert_eq!(base, "main");
        assert_eq!(head, "HEAD");
    }

    #[test]
    fn parse_ref_spec_single_ref() {
        let (base, head) = parse_ref_spec("HEAD~1").unwrap();
        assert_eq!(base, "HEAD~1");
        assert_eq!(head, "HEAD");
    }

    #[test]
    fn parse_ref_spec_three_dots() {
        let (base, head) = parse_ref_spec("main...feature").unwrap();
        assert_eq!(base, "main");
        assert_eq!(head, "feature");
    }
}
