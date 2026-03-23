use colored::Colorize;
use crate::diff_json::{ChangeKind, DiffResult, EntityDiff};

/// Format a diff result for terminal output.
pub fn format_terminal(result: &DiffResult) -> String {
    let mut output = String::new();

    // Group by file
    let mut by_file: std::collections::BTreeMap<&str, Vec<&EntityDiff>> = std::collections::BTreeMap::new();
    for diff in &result.entities {
        by_file.entry(&diff.file).or_default().push(diff);
    }

    for (file, diffs) in &by_file {
        output.push_str(&format!("\n{}\n", file.bold()));

        for diff in diffs {
            output.push_str(&format_entity_diff(diff));
            output.push('\n');
        }
    }

    // Cross-file patterns
    if !result.patterns.is_empty() {
        output.push_str(&format!("\n{}\n", "Cross-file patterns:".bold()));
        for p in &result.patterns {
            output.push_str(&format!("  ▸ {} ({})\n",
                p.description.cyan(),
                p.files.join(", ").dimmed()
            ));
        }
    }

    // Summary
    let s = &result.summary;
    output.push_str(&format!(
        "\n{}\n",
        format!(
            "{} added, {} removed, {} modified, {} moved, {} renamed, {} formatting only",
            s.added, s.removed, s.modified, s.moved, s.renamed, s.formatting_only
        ).dimmed()
    ));

    if s.has_breaking_change {
        output.push_str(&format!("{}\n", "⚠ BREAKING CHANGES DETECTED".red().bold()));
    }

    output
}

fn format_entity_diff(diff: &EntityDiff) -> String {
    let label = match &diff.change {
        ChangeKind::Added => "▸ ADDED".green().bold().to_string(),
        ChangeKind::Removed => "▸ REMOVED".red().bold().to_string(),
        ChangeKind::Modified => "▸ MODIFIED".yellow().bold().to_string(),
        ChangeKind::Moved => "▸ MOVED".cyan().bold().to_string(),
        ChangeKind::Renamed => "▸ RENAMED".magenta().bold().to_string(),
        ChangeKind::FormattingOnly => "▸ FORMATTING ONLY".dimmed().to_string(),
    };

    let mut line = format!("  {} {} ({})", label, diff.name.bold(), diff.kind);

    match &diff.change {
        ChangeKind::Moved => {
            if let Some(old_file) = &diff.old_file {
                line.push_str(&format!("\n  │ {} → {}", old_file.dimmed(), diff.file));
            }
        }
        ChangeKind::Renamed => {
            if let Some(old_name) = &diff.old_name {
                line.push_str(&format!("  {} → {}", old_name.dimmed(), diff.name));
            }
        }
        ChangeKind::Modified => {
            let mut details = Vec::new();
            if diff.sig_changed == Some(true) { details.push("signature changed"); }
            if diff.body_changed == Some(true) { details.push("body changed"); }
            if !details.is_empty() {
                line.push_str(&format!("\n  │ {}", details.join(", ")));
            }
        }
        _ => {}
    }

    if diff.breaking {
        line.push_str(&format!("\n  │ {}", "⚠ BREAKING".red()));
    }

    // Change details (token-level)
    if let Some(ref details) = diff.change_details {
        for detail in details {
            let formatted = match detail.kind {
                crate::change_detail::DetailKind::ValueChanged =>
                    format!("  │   {}", detail.description.yellow()),
                crate::change_detail::DetailKind::IdentifierChanged =>
                    format!("  │   {}", detail.description.cyan()),
                crate::change_detail::DetailKind::ArgumentAdded =>
                    format!("  │   {}", detail.description.green()),
                crate::change_detail::DetailKind::ArgumentRemoved =>
                    format!("  │   {}", detail.description.red()),
                crate::change_detail::DetailKind::Comment =>
                    format!("  │   {}", detail.description.dimmed()),
                crate::change_detail::DetailKind::LineAdded =>
                    format!("  │   {}", detail.description.green()),
                crate::change_detail::DetailKind::LineRemoved =>
                    format!("  │   {}", detail.description.red()),
            };
            line.push_str(&format!("\n{}", formatted));
        }
    }

    // Inline diff lines
    if let Some(ref lines) = diff.inline_diff {
        // Limit to 10 lines to avoid flooding terminal
        let display_lines = if lines.len() > 10 { &lines[..10] } else { lines };
        for dl in display_lines {
            let formatted = match dl.kind {
                crate::inline_diff::DiffLineKind::Removed => format!("  │   {}", format!("- {}", dl.text).red()),
                crate::inline_diff::DiffLineKind::Added => format!("  │   {}", format!("+ {}", dl.text).green()),
                crate::inline_diff::DiffLineKind::Context => format!("  │     {}", dl.text.dimmed()),
            };
            line.push_str(&format!("\n{}", formatted));
        }
        if lines.len() > 10 {
            line.push_str(&format!("\n  │   {}", format!("... and {} more lines", lines.len() - 10).dimmed()));
        }
    }

    line
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff_json::{DiffResult, DiffSummary};

    #[test]
    fn format_terminal_includes_summary() {
        let result = DiffResult {
            base_ref: "abc".into(),
            head_ref: "def".into(),
            base_sha: None,
            head_sha: None,
            entities: vec![],
            patterns: vec![],
            summary: DiffSummary {
                added: 1, removed: 0, modified: 2,
                moved: 0, renamed: 0, formatting_only: 3,
                has_breaking_change: false,
            },
            old_sources: None,
            new_sources: None,
        };
        let output = format_terminal(&result);
        assert!(output.contains("1 added"));
        assert!(output.contains("2 modified"));
        assert!(output.contains("3 formatting only"));
    }

    #[test]
    fn format_terminal_shows_breaking_warning() {
        let result = DiffResult {
            base_ref: "abc".into(),
            head_ref: "def".into(),
            base_sha: None,
            head_sha: None,
            entities: vec![],
            patterns: vec![],
            summary: DiffSummary {
                added: 0, removed: 0, modified: 1,
                moved: 0, renamed: 0, formatting_only: 0,
                has_breaking_change: true,
            },
            old_sources: None,
            new_sources: None,
        };
        let output = format_terminal(&result);
        assert!(output.contains("BREAKING"));
    }
}
