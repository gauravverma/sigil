use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct Cache {
    pub version: String,
    pub files: HashMap<String, String>,
}

impl Cache {
    pub fn new() -> Self {
        Cache {
            version: "1".into(),
            files: HashMap::new(),
        }
    }

    pub fn load(sigil_dir: &Path) -> Option<Self> {
        let path = sigil_dir.join("cache.json");
        let content = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn save(&self, sigil_dir: &Path) -> std::io::Result<()> {
        let path = sigil_dir.join("cache.json");
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(&path, content)
    }

    pub fn file_changed(&self, relative_path: &str, current_hash: &str) -> bool {
        match self.files.get(relative_path) {
            Some(cached_hash) => cached_hash != current_hash,
            None => true,
        }
    }
}

/// Compute BLAKE3 hash of file contents (full hex string).
pub fn hash_file_contents(contents: &[u8]) -> String {
    blake3::hash(contents).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn hash_file_contents_deterministic() {
        let a = hash_file_contents(b"hello world");
        let b = hash_file_contents(b"hello world");
        assert_eq!(a, b);
    }

    #[test]
    fn hash_file_contents_differs() {
        let a = hash_file_contents(b"hello world");
        let b = hash_file_contents(b"hello world!");
        assert_ne!(a, b);
    }

    #[test]
    fn file_changed_new_file() {
        let cache = Cache { version: "1".into(), files: std::collections::HashMap::new() };
        assert!(cache.file_changed("new.py", "abc123"));
    }

    #[test]
    fn file_changed_same_hash() {
        let mut files = std::collections::HashMap::new();
        files.insert("old.py".to_string(), "abc123".to_string());
        let cache = Cache { version: "1".into(), files };
        assert!(!cache.file_changed("old.py", "abc123"));
    }

    #[test]
    fn file_changed_different_hash() {
        let mut files = std::collections::HashMap::new();
        files.insert("old.py".to_string(), "abc123".to_string());
        let cache = Cache { version: "1".into(), files };
        assert!(cache.file_changed("old.py", "def456"));
    }

    #[test]
    fn roundtrip_save_load() {
        let dir = std::env::temp_dir().join("sigil_cache_test");
        std::fs::create_dir_all(&dir).unwrap();

        let mut files = std::collections::HashMap::new();
        files.insert("a.py".to_string(), "hash_a".to_string());
        let cache = Cache { version: "1".into(), files };
        cache.save(&dir).unwrap();

        let loaded = Cache::load(&dir).unwrap();
        assert_eq!(loaded.version, "1");
        assert_eq!(loaded.files.get("a.py").unwrap(), "hash_a");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_missing_returns_none() {
        let result = Cache::load(Path::new("/nonexistent/path"));
        assert!(result.is_none());
    }
}
