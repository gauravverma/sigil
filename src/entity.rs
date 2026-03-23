use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entity {
    pub file: String,
    pub name: String,
    pub kind: String,
    pub line_start: u32,
    pub line_end: u32,
    pub parent: Option<String>,
    pub sig: Option<String>,
    pub meta: Option<Vec<String>>,
    pub body_hash: Option<String>,
    pub sig_hash: Option<String>,
    pub struct_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Reference {
    pub file: String,
    pub caller: Option<String>,
    pub name: String,
    pub ref_kind: String,
    pub line: u32,
}
