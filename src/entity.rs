use serde::{Serialize, Deserialize};

// Eq is dropped because `rank: Option<f64>` can't implement Eq. Callers that
// need equality still have `PartialEq`; callers that want hashability on
// rank-less Entities can compare the struct_hash field directly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entity {
    pub file: String,
    pub name: String,
    pub kind: String,
    pub line_start: u32,
    pub line_end: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sig: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meta: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sig_hash: Option<String>,
    pub struct_hash: String,

    // Phase 1 additions — all Option<T> + skip_serializing_if so old v1 JSONL
    // round-trips as None and newer writes add the fields only when populated.
    // Populated after a rank pass (src/rank.rs); None when the caller opted
    // out via `--no-rank` or when the parser didn't emit visibility info.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blast_radius: Option<BlastRadius>,
}

/// Downstream impact summary for a single entity. Used by `sigil review`,
/// `sigil map`, `sigil blast`, and the Phase 1 ranking pipeline.
///
/// `direct_callers`   — number of reference rows targeting this entity's name.
/// `direct_files`     — distinct files those references live in.
/// `transitive_callers` — BFS over the reverse-call graph, capped at depth
/// 3 to avoid cycles and runaway cost on highly-connected symbols.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlastRadius {
    pub direct_callers: u32,
    pub direct_files: u32,
    pub transitive_callers: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Reference {
    pub file: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caller: Option<String>,
    pub name: String,
    pub ref_kind: String,
    pub line: u32,
}
