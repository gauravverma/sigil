use crate::entity::Entity;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchKind {
    ExactMatch,   // same file + same name
    Moved,        // same name, different file
    Renamed,      // different name, same body_hash
    Added,        // only in new
    Removed,      // only in old
}

#[derive(Debug, Clone)]
pub struct EntityMatch {
    pub old: Option<Entity>,
    pub new: Option<Entity>,
    pub match_kind: MatchKind,
    pub confidence: f64, // 0.0-1.0; always 1.0 until fuzzy matching is added
}

/// Match entities across two versions.
///
/// Strategy (layered):
/// 1. Exact match: same (file, name) in both → ExactMatch
/// 2. Name match across files: same name, different file → Moved
/// 3. Body hash match: different name, same body_hash (non-null) → Renamed
/// 4. Remaining old → Removed, remaining new → Added
///
/// Entities with identical struct_hash are considered unchanged and excluded.
pub fn match_entities(old: &[Entity], new: &[Entity]) -> Vec<EntityMatch> {
    let mut matches = Vec::new();
    let mut used_old: HashSet<usize> = HashSet::new();
    let mut used_new: HashSet<usize> = HashSet::new();

    // Pass 1: Exact match (file + name)
    let old_by_key: HashMap<(&str, &str), usize> = old.iter().enumerate()
        .map(|(i, e)| ((e.file.as_str(), e.name.as_str()), i))
        .collect();

    for (ni, ne) in new.iter().enumerate() {
        let key = (ne.file.as_str(), ne.name.as_str());
        if let Some(&oi) = old_by_key.get(&key) {
            if !used_old.contains(&oi) {
                let oe = &old[oi];
                // Skip unchanged entities
                if oe.struct_hash == ne.struct_hash {
                    used_old.insert(oi);
                    used_new.insert(ni);
                    continue;
                }
                matches.push(EntityMatch {
                    old: Some(oe.clone()),
                    new: Some(ne.clone()),
                    match_kind: MatchKind::ExactMatch,
                    confidence: 1.0,
                });
                used_old.insert(oi);
                used_new.insert(ni);
            }
        }
    }

    // Pass 2: Name match across files (Moved)
    let remaining_old_by_name: HashMap<&str, usize> = old.iter().enumerate()
        .filter(|(i, _)| !used_old.contains(i))
        .map(|(i, e)| (e.name.as_str(), i))
        .collect();

    for (ni, ne) in new.iter().enumerate() {
        if used_new.contains(&ni) { continue; }
        if let Some(&oi) = remaining_old_by_name.get(ne.name.as_str()) {
            if !used_old.contains(&oi) {
                matches.push(EntityMatch {
                    old: Some(old[oi].clone()),
                    new: Some(ne.clone()),
                    match_kind: MatchKind::Moved,
                    confidence: 1.0,
                });
                used_old.insert(oi);
                used_new.insert(ni);
            }
        }
    }

    // Pass 3: Body hash match (Renamed)
    // Only for entities with non-null body_hash and non-import kind
    let remaining_old_by_body: HashMap<&str, usize> = old.iter().enumerate()
        .filter(|(i, e)| !used_old.contains(i) && e.body_hash.is_some() && e.kind != "import")
        .filter_map(|(i, e)| e.body_hash.as_deref().map(|bh| (bh, i)))
        .collect();

    for (ni, ne) in new.iter().enumerate() {
        if used_new.contains(&ni) { continue; }
        if ne.kind == "import" || ne.body_hash.is_none() { continue; }
        if let Some(&oi) = remaining_old_by_body.get(ne.body_hash.as_deref().unwrap()) {
            if !used_old.contains(&oi) {
                matches.push(EntityMatch {
                    old: Some(old[oi].clone()),
                    new: Some(ne.clone()),
                    match_kind: MatchKind::Renamed,
                    confidence: 1.0,
                });
                used_old.insert(oi);
                used_new.insert(ni);
            }
        }
    }

    // Pass 4: Remaining → Added / Removed
    for (oi, oe) in old.iter().enumerate() {
        if !used_old.contains(&oi) {
            matches.push(EntityMatch {
                old: Some(oe.clone()),
                new: None,
                match_kind: MatchKind::Removed,
                confidence: 1.0,
            });
        }
    }
    for (ni, ne) in new.iter().enumerate() {
        if !used_new.contains(&ni) {
            matches.push(EntityMatch {
                old: None,
                new: Some(ne.clone()),
                match_kind: MatchKind::Added,
                confidence: 1.0,
            });
        }
    }

    matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::Entity;

    fn entity(file: &str, name: &str, body_hash: Option<&str>, sig_hash: Option<&str>, struct_hash: &str) -> Entity {
        Entity {
            file: file.to_string(),
            name: name.to_string(),
            kind: "function".to_string(),
            line_start: 1, line_end: 5,
            parent: None,
            sig: Some(format!("def {}():", name)),
            meta: None,
            body_hash: body_hash.map(|s| s.to_string()),
            sig_hash: sig_hash.map(|s| s.to_string()),
            struct_hash: struct_hash.to_string(),
        }
    }

    #[test]
    fn exact_match_same_file_same_name() {
        let old = vec![entity("a.py", "foo", Some("bh1"), Some("sh1"), "st1")];
        let new = vec![entity("a.py", "foo", Some("bh2"), Some("sh1"), "st2")];
        let matches = match_entities(&old, &new);
        assert_eq!(matches.len(), 1);
        assert!(matches[0].old.is_some());
        assert!(matches[0].new.is_some());
        assert_eq!(matches[0].match_kind, MatchKind::ExactMatch);
    }

    #[test]
    fn moved_entity_detected() {
        let old = vec![entity("a.py", "foo", Some("bh1"), Some("sh1"), "st1")];
        let new = vec![entity("b.py", "foo", Some("bh1"), Some("sh1"), "st1")];
        let matches = match_entities(&old, &new);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].match_kind, MatchKind::Moved);
    }

    #[test]
    fn renamed_entity_detected_by_body_hash() {
        let old = vec![entity("a.py", "foo", Some("bh1"), Some("sh1"), "st1")];
        let new = vec![entity("a.py", "bar", Some("bh1"), Some("sh2"), "st2")];
        let matches = match_entities(&old, &new);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].match_kind, MatchKind::Renamed);
    }

    #[test]
    fn added_and_removed() {
        let old = vec![entity("a.py", "foo", Some("bh1"), Some("sh1"), "st1")];
        let new = vec![entity("a.py", "bar", Some("bh2"), Some("sh2"), "st2")];
        let matches = match_entities(&old, &new);
        let added = matches.iter().filter(|m| m.match_kind == MatchKind::Added).count();
        let removed = matches.iter().filter(|m| m.match_kind == MatchKind::Removed).count();
        assert_eq!(added, 1);
        assert_eq!(removed, 1);
    }

    #[test]
    fn imports_not_renamed() {
        let mut old_e = entity("a.py", "os", None, None, "st1");
        old_e.kind = "import".to_string();
        let mut new_e = entity("a.py", "sys", None, None, "st2");
        new_e.kind = "import".to_string();
        let matches = match_entities(&[old_e], &[new_e]);
        let added = matches.iter().filter(|m| m.match_kind == MatchKind::Added).count();
        let removed = matches.iter().filter(|m| m.match_kind == MatchKind::Removed).count();
        assert_eq!(added, 1);
        assert_eq!(removed, 1);
    }

    #[test]
    fn unchanged_entity_excluded() {
        let e = entity("a.py", "foo", Some("bh1"), Some("sh1"), "st1");
        let matches = match_entities(&[e.clone()], &[e]);
        assert!(matches.is_empty());
    }
}
