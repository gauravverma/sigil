use crate::entity::{Entity, Reference};
use crate::hasher;

/// Parse a markdown file and extract structural entities.
pub fn parse_markdown_file(
    source: &str,
    file_path: &str,
) -> Result<(Vec<Entity>, Vec<Reference>), String> {
    if source.trim().is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }

    let lines: Vec<&str> = source.lines().collect();
    let mut entities = Vec::new();

    // TODO: state machine parser

    entities.sort_by(|a: &Entity, b: &Entity| a.line_start.cmp(&b.line_start));
    Ok((entities, Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_file() {
        let (entities, refs) = parse_markdown_file("", "test.md").unwrap();
        assert!(entities.is_empty());
        assert!(refs.is_empty());
    }
}
