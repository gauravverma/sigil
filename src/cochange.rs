//! Co-change mining: which files tend to change together in history?
//!
//! Used by `sigil review` to surface "co-change misses" — files that
//! historically move with one of the files in the PR but didn't this time.
//! That's a high-signal stale-twin detector. When a developer edits
//! `api/handler.rs` but not `api/types.rs`, and those two have moved
//! together in 80% of historical commits, the reviewer should notice.
//!
//! ## Algorithm
//!
//! 1. Walk `git log --name-only -n <N>` and bucket files by commit.
//! 2. For each commit, count every unordered file pair.
//! 3. Normalize with the cosine-ish coefficient
//!        weight(A, B) = support(A,B) / sqrt(count(A) * count(B))
//!    so very-commonly-changed files (README.md, Cargo.lock) don't flood
//!    the partner list.
//! 4. Drop pairs with support < `min_support` (default 2) — a single
//!    coincidental co-change isn't signal.
//!
//! ## Storage
//!
//! `.sigil/cochange.json`. Derived artifact; gitignored in the installer
//! (same pattern as rank.json). Scanning 500 commits of a medium repo
//! takes under a second — cheap to refresh alongside `sigil index`.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// On-disk form. Pairs are sorted by weight descending for stable output.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CochangeManifest {
    pub version: String,
    pub sigil_version: String,
    pub commits_scanned: u32,
    pub min_support: u32,
    pub file_count: usize,
    pub pairs: Vec<Pair>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pair {
    pub a: String,
    pub b: String,
    pub weight: f64,
    pub support: u32,
}

/// Tunable knobs.
#[derive(Debug, Clone)]
pub struct CochangeConfig {
    /// Number of historical commits to walk. Default 500 — empirically
    /// enough signal without slowing down big repos.
    pub commits: u32,
    /// Drop pairs seen together fewer than this many times.
    pub min_support: u32,
    /// Skip commits that touch more than this many files (probably
    /// refactors / mass-renames that dilute co-change signal).
    pub max_files_per_commit: u32,
}

impl Default for CochangeConfig {
    fn default() -> Self {
        Self {
            commits: 500,
            min_support: 2,
            max_files_per_commit: 30,
        }
    }
}

/// Run git-log and compute the manifest. Returns an empty manifest if the
/// repo has no history (fresh `git init`).
pub fn mine(root: &Path, cfg: &CochangeConfig) -> Result<CochangeManifest> {
    let out = Command::new("git")
        .args([
            "log",
            &format!("-n{}", cfg.commits),
            "--name-only",
            "--pretty=format:---%H",
            "--no-renames",
        ])
        .current_dir(root)
        .output()
        .with_context(|| "git log failed — is git on PATH and is this a repo?")?;

    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        anyhow::bail!("git log exited non-zero: {stderr}");
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let commits = parse_git_log(&text, cfg.max_files_per_commit);
    Ok(compute_manifest(&commits, cfg))
}

/// Parse the `git log --pretty=format:---%H --name-only` output into a
/// Vec<HashSet<String>>. Exposed for unit tests.
pub(crate) fn parse_git_log(text: &str, max_files: u32) -> Vec<BTreeSet<String>> {
    let mut commits: Vec<BTreeSet<String>> = Vec::new();
    let mut current: Option<BTreeSet<String>> = None;

    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("---") {
            if let Some(c) = current.take() {
                if !c.is_empty() && c.len() as u32 <= max_files {
                    commits.push(c);
                }
            }
            current = Some(BTreeSet::new());
        } else if !line.is_empty()
            && let Some(ref mut c) = current
        {
            c.insert(line.to_string());
        }
    }
    if let Some(c) = current {
        if !c.is_empty() && c.len() as u32 <= max_files {
            commits.push(c);
        }
    }
    commits
}

pub(crate) fn compute_manifest(
    commits: &[BTreeSet<String>],
    cfg: &CochangeConfig,
) -> CochangeManifest {
    let mut file_counts: HashMap<&str, u32> = HashMap::new();
    // key = (a, b) with a < b lexicographically to dedupe ordering.
    let mut pair_counts: HashMap<(&str, &str), u32> = HashMap::new();

    for commit in commits {
        let files: Vec<&str> = commit.iter().map(String::as_str).collect();
        for f in &files {
            *file_counts.entry(*f).or_insert(0) += 1;
        }
        for i in 0..files.len() {
            for j in (i + 1)..files.len() {
                let (a, b) = if files[i] < files[j] {
                    (files[i], files[j])
                } else {
                    (files[j], files[i])
                };
                *pair_counts.entry((a, b)).or_insert(0) += 1;
            }
        }
    }

    let mut pairs: Vec<Pair> = pair_counts
        .into_iter()
        .filter(|(_, support)| *support >= cfg.min_support)
        .map(|((a, b), support)| {
            let ca = *file_counts.get(a).unwrap_or(&1) as f64;
            let cb = *file_counts.get(b).unwrap_or(&1) as f64;
            let weight = support as f64 / (ca * cb).sqrt();
            Pair {
                a: a.to_string(),
                b: b.to_string(),
                weight,
                support,
            }
        })
        .collect();
    pairs.sort_by(|x, y| {
        y.weight
            .partial_cmp(&x.weight)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| x.a.cmp(&y.a))
            .then_with(|| x.b.cmp(&y.b))
    });

    CochangeManifest {
        version: "1".to_string(),
        sigil_version: env!("CARGO_PKG_VERSION").to_string(),
        commits_scanned: commits.len() as u32,
        min_support: cfg.min_support,
        file_count: file_counts.len(),
        pairs,
    }
}

/// Write the manifest to `.sigil/cochange.json`.
pub fn save(manifest: &CochangeManifest, root: &Path, pretty: bool) -> Result<()> {
    let dir = root.join(".sigil");
    std::fs::create_dir_all(&dir)?;
    let content = if pretty {
        serde_json::to_string_pretty(manifest)
    } else {
        serde_json::to_string(manifest)
    }?;
    std::fs::write(dir.join("cochange.json"), content)?;
    Ok(())
}

/// Read `.sigil/cochange.json`. Missing file → empty manifest (caller just
/// gets no co-change signal rather than an error). Mirrors how
/// `map::load_rank_manifest` handles missing rank.json.
pub fn load(root: &Path) -> Result<CochangeManifest> {
    let path = root.join(".sigil").join("cochange.json");
    if !path.exists() {
        return Ok(CochangeManifest::default());
    }
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("parse {}", path.display()))
}

/// All partners of `file` — pairs where `file` appears on either side —
/// sorted by weight desc. Callers cap with `.take(N)` or a min-weight filter.
pub fn partners_of<'a>(manifest: &'a CochangeManifest, file: &str) -> Vec<Partner<'a>> {
    manifest
        .pairs
        .iter()
        .filter_map(|p| {
            if p.a == file {
                Some(Partner {
                    file: &p.b,
                    weight: p.weight,
                    support: p.support,
                })
            } else if p.b == file {
                Some(Partner {
                    file: &p.a,
                    weight: p.weight,
                    support: p.support,
                })
            } else {
                None
            }
        })
        .collect()
}

#[derive(Debug, Clone, Serialize)]
pub struct Partner<'a> {
    pub file: &'a str,
    pub weight: f64,
    pub support: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_git_log_splits_commits_on_marker() {
        let log = "\
---abc123
src/a.rs
src/b.rs

---def456
src/b.rs
src/c.rs
";
        let commits = parse_git_log(log, 30);
        assert_eq!(commits.len(), 2);
        assert!(commits[0].contains("src/a.rs"));
        assert!(commits[0].contains("src/b.rs"));
        assert!(commits[1].contains("src/c.rs"));
    }

    #[test]
    fn parse_git_log_skips_massive_commits() {
        let mut log = String::from("---abc\n");
        for i in 0..50 {
            log.push_str(&format!("f{i}.rs\n"));
        }
        log.push_str("---def\nsmall.rs\n");
        let commits = parse_git_log(&log, 30);
        // First commit (50 files) filtered out; second (1 file) kept.
        assert_eq!(commits.len(), 1);
        assert!(commits[0].contains("small.rs"));
    }

    #[test]
    fn compute_manifest_assigns_weight_and_support() {
        let cfg = CochangeConfig {
            min_support: 1,
            commits: 10,
            max_files_per_commit: 10,
        };
        let c1: BTreeSet<String> = ["a.rs", "b.rs"].iter().map(|s| s.to_string()).collect();
        let c2: BTreeSet<String> = ["a.rs", "b.rs"].iter().map(|s| s.to_string()).collect();
        let c3: BTreeSet<String> = ["a.rs", "c.rs"].iter().map(|s| s.to_string()).collect();
        let m = compute_manifest(&[c1, c2, c3], &cfg);
        assert_eq!(m.commits_scanned, 3);

        // a appears in 3 commits, b in 2, c in 1. Pair (a,b) support=2.
        // weight = 2 / sqrt(3 * 2) ≈ 0.816.
        let ab = m
            .pairs
            .iter()
            .find(|p| p.a == "a.rs" && p.b == "b.rs")
            .expect("(a,b) pair missing");
        assert_eq!(ab.support, 2);
        assert!((ab.weight - 0.8164965809277261).abs() < 1e-9);

        let ac = m
            .pairs
            .iter()
            .find(|p| p.a == "a.rs" && p.b == "c.rs")
            .expect("(a,c) pair missing");
        assert_eq!(ac.support, 1);

        // (a,b) has stronger weight than (a,c), so it sorts first.
        assert_eq!(m.pairs[0].a, "a.rs");
        assert_eq!(m.pairs[0].b, "b.rs");
    }

    #[test]
    fn min_support_filters_weak_pairs() {
        let cfg = CochangeConfig {
            min_support: 2,
            commits: 10,
            max_files_per_commit: 10,
        };
        let c1: BTreeSet<String> = ["a.rs", "b.rs"].iter().map(|s| s.to_string()).collect();
        let c2: BTreeSet<String> = ["a.rs", "c.rs"].iter().map(|s| s.to_string()).collect();
        // (a,b) and (a,c) each appear only once → dropped with min_support=2.
        let m = compute_manifest(&[c1, c2], &cfg);
        assert!(m.pairs.is_empty());
    }

    #[test]
    fn partners_of_returns_matches_from_both_sides() {
        let manifest = CochangeManifest {
            version: "1".to_string(),
            sigil_version: "test".to_string(),
            commits_scanned: 5,
            min_support: 1,
            file_count: 3,
            pairs: vec![
                Pair { a: "a.rs".to_string(), b: "b.rs".to_string(), weight: 0.8, support: 4 },
                Pair { a: "a.rs".to_string(), b: "c.rs".to_string(), weight: 0.3, support: 2 },
                Pair { a: "d.rs".to_string(), b: "b.rs".to_string(), weight: 0.5, support: 3 },
            ],
        };
        let for_b = partners_of(&manifest, "b.rs");
        // b.rs appears in pairs (a,b) and (d,b) — both picked up.
        let files: Vec<&str> = for_b.iter().map(|p| p.file).collect();
        assert!(files.contains(&"a.rs"));
        assert!(files.contains(&"d.rs"));
        assert_eq!(for_b.len(), 2);
    }

    #[test]
    fn load_missing_returns_empty_manifest() {
        let tmp = std::env::temp_dir().join(format!("sigil_cochange_{}", std::process::id()));
        std::fs::create_dir_all(&tmp).ok();
        let m = load(&tmp).unwrap();
        assert_eq!(m.file_count, 0);
        assert!(m.pairs.is_empty());
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn save_and_load_roundtrip() {
        let tmp = std::env::temp_dir().join(format!("sigil_cochange_rt_{}", std::process::id()));
        std::fs::create_dir_all(tmp.join(".sigil")).unwrap();
        let before = CochangeManifest {
            version: "1".to_string(),
            sigil_version: "test".to_string(),
            commits_scanned: 3,
            min_support: 2,
            file_count: 2,
            pairs: vec![Pair {
                a: "a.rs".to_string(),
                b: "b.rs".to_string(),
                weight: 0.5,
                support: 3,
            }],
        };
        save(&before, &tmp, false).unwrap();
        let after = load(&tmp).unwrap();
        assert_eq!(after.commits_scanned, 3);
        assert_eq!(after.pairs.len(), 1);
        assert!((after.pairs[0].weight - 0.5).abs() < 1e-9);
        std::fs::remove_dir_all(&tmp).ok();
    }
}
