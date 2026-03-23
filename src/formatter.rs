use colored::Colorize;
use crate::diff_json::{ChangeKind, DiffResult, EntityDiff};
use crate::output::*;

// ── Glyph constants ─────────────────────────────────────────────────────────

const GLYPH_MODIFIED: &str = "~";
const GLYPH_ADDED: &str = "+";
const GLYPH_REMOVED: &str = "\u{2212}";   // −
const GLYPH_RENAMED: &str = "\u{2248}";   // ≈
const GLYPH_FORMAT: &str = "\u{00B7}";    // ·
const GLYPH_PATTERN: &str = "\u{2261}";   // ≡
const GLYPH_MOVE: &str = "\u{21D2}";      // ⇒
const GLYPH_BREAKING: &str = "\u{26A0}";  // ⚠
const GLYPH_SEPARATOR: &str = "\u{2500}"; // ─
const GLYPH_ARROW: &str = "\u{2192}";     // →

const SEPARATOR_WIDTH: usize = 60;

// ── Format options ──────────────────────────────────────────────────────────

pub struct FormatOptions {
    pub show_lines: bool,
    pub show_context: bool,
    pub use_color: bool,
}

// ── Main entry point ────────────────────────────────────────────────────────

pub fn format_terminal_v2(output: &DiffOutput, opts: &FormatOptions) -> String {
    let mut out = String::new();

    // 1. Header with separator
    render_header(&mut out, output);

    // 2. Patterns section (if any)
    if !output.patterns.is_empty() {
        out.push('\n');
        out.push_str(&"patterns".dimmed().to_string());
        out.push('\n');
        out.push('\n');
        for pat in &output.patterns {
            render_pattern(&mut out, pat);
        }
    }

    // 3. Moves section (if any)
    if !output.moves.is_empty() {
        out.push('\n');
        out.push_str(&"moves".dimmed().to_string());
        out.push('\n');
        out.push('\n');
        for mv in &output.moves {
            render_move(&mut out, mv);
        }
    }

    // 4. Separator before file sections (if there are patterns or moves)
    if !output.patterns.is_empty() || !output.moves.is_empty() {
        out.push('\n');
        out.push_str(&separator());
        out.push('\n');
    }

    // 5. Per-file sections
    for section in &output.files {
        out.push('\n');
        render_file_section(&mut out, section, opts);
    }

    // 6. Summary footer
    out.push('\n');
    out.push_str(&separator());
    out.push('\n');
    render_summary(&mut out, output);
    render_breaking(&mut out, output);
    out.push_str(&separator());
    out.push('\n');

    out
}

// ── Header ──────────────────────────────────────────────────────────────────

fn render_header(out: &mut String, output: &DiffOutput) {
    out.push_str(&separator());
    out.push('\n');

    let refspec = format!("sigil diff  {}", output.meta.base_ref);
    let file_count = format!("{} files", output.summary.files_changed);

    let is_formatting_only = output.summary.added == 0
        && output.summary.removed == 0
        && output.summary.modified == 0
        && output.summary.renamed == 0
        && output.summary.moves == 0
        && output.summary.formatting_only > 0;

    let right_part = if output.summary.has_breaking {
        format!(
            "{}  {} {} breaking",
            file_count, GLYPH_BREAKING, output.breaking.len()
        )
    } else if is_formatting_only {
        format!("{}  {}", file_count, "(formatting only)".dimmed())
    } else {
        file_count
    };

    // Pad refspec to fill available space
    let pad = if refspec.len() < 40 { 40 - refspec.len() } else { 2 };
    out.push_str(&format!(
        "{}{:>width$}\n",
        refspec.bold(),
        right_part,
        width = pad + right_part.len()
    ));

    out.push_str(&separator());
    out.push('\n');
}

// ── Patterns ────────────────────────────────────────────────────────────────

fn render_pattern(out: &mut String, pat: &OutputPattern) {
    // Line 1:  ≡  rename    validate_* → check_*   ×8 files   function
    let description = if let (Some(from), Some(to)) = (&pat.from_glob, &pat.to_glob) {
        format!("{} {} {}", from, GLYPH_ARROW, to)
    } else if let Some(name) = &pat.entity_name {
        name.clone()
    } else {
        pat.entities.first().cloned().unwrap_or_default()
    };

    out.push_str(&format!(
        "  {}  {:<9} {}   \u{00D7}{} files   {}\n",
        GLYPH_PATTERN.cyan(),
        pat.pattern_type,
        description.bold(),
        pat.file_count,
        pat.entity_kind.dimmed(),
    ));

    // Line 2: first 3 files, then +N more
    let display_files: Vec<&str> = pat.files.iter().take(3).map(|s| s.as_str()).collect();
    let mut files_line = format!("     {}", display_files.join("  "));
    if pat.files.len() > 3 {
        files_line.push_str(&format!("  +{} more", pat.files.len() - 3));
    }
    out.push_str(&format!("{}\n", files_line.dimmed()));
}

// ── Moves ───────────────────────────────────────────────────────────────────

fn render_move(out: &mut String, mv: &MoveEntry) {
    // Line 1:  ⇒  execute_payment      function       ⚠ breaking
    let breaking_flag = if mv.breaking {
        format!("       {} breaking", GLYPH_BREAKING).red().to_string()
    } else {
        String::new()
    };

    out.push_str(&format!(
        "  {}  {:<20} {}{}\n",
        GLYPH_MOVE.cyan(),
        mv.entity.bold(),
        mv.kind.dimmed(),
        breaking_flag,
    ));

    // Line 2: from_file → to_file
    out.push_str(&format!(
        "     {} {} {}\n",
        mv.from_file.dimmed(),
        GLYPH_ARROW,
        mv.to_file,
    ));
}

// ── Per-file section ────────────────────────────────────────────────────────

fn render_file_section(out: &mut String, section: &FileSection, opts: &FormatOptions) {
    // File header: filename bold, right-aligned +N ~N ·N
    let counts = build_file_counts(&section.summary);
    let file_str = section.file.bold().to_string();

    if counts.is_empty() {
        out.push_str(&format!("{}\n", file_str));
    } else {
        let pad = if section.file.len() < 50 {
            50 - section.file.len()
        } else {
            2
        };
        out.push_str(&format!(
            "{}{:>width$}\n",
            file_str,
            counts,
            width = pad + counts.len()
        ));
    }

    // Collapse formatting-only entities when color is off
    if !opts.use_color {
        let formatting_entities: Vec<&OutputEntity> = section
            .entities
            .iter()
            .filter(|e| e.change == "formatting_only")
            .collect();

        let non_formatting: Vec<&OutputEntity> = section
            .entities
            .iter()
            .filter(|e| e.change != "formatting_only")
            .collect();

        // Render non-formatting entities
        for entity in &non_formatting {
            render_entity_row(out, entity, opts);
        }

        // Collapse formatting into single line
        if !formatting_entities.is_empty() {
            let display_names: Vec<&str> = formatting_entities
                .iter()
                .take(3)
                .map(|e| e.name.as_str())
                .collect();
            let mut line = format!(
                "  {}  {} formatting only: {}",
                GLYPH_FORMAT,
                formatting_entities.len(),
                display_names.join("  "),
            );
            if formatting_entities.len() > 3 {
                line.push_str(&format!("  +{} more", formatting_entities.len() - 3));
            }
            out.push_str(&format!("{}\n", line));
        }
    } else {
        // With color, render all entities normally
        for entity in &section.entities {
            render_entity_row(out, entity, opts);
        }
    }

    out.push('\n');
}

fn build_file_counts(summary: &FileSummary) -> String {
    let mut parts = Vec::new();
    if summary.added > 0 {
        parts.push(format!("{}{}", GLYPH_ADDED, summary.added));
    }
    if summary.modified > 0 {
        parts.push(format!("{}{}", GLYPH_MODIFIED, summary.modified));
    }
    if summary.removed > 0 {
        parts.push(format!("{}{}", GLYPH_REMOVED, summary.removed));
    }
    if summary.renamed > 0 {
        parts.push(format!("{}{}", GLYPH_RENAMED, summary.renamed));
    }
    if summary.formatting_only > 0 {
        parts.push(format!("{}{}", GLYPH_FORMAT, summary.formatting_only));
    }
    parts.join(" ")
}

// ── Entity row ──────────────────────────────────────────────────────────────

fn render_entity_row(out: &mut String, entity: &OutputEntity, opts: &FormatOptions) {
    let (glyph, verb, glyph_color) = match entity.change.as_str() {
        "added" => (GLYPH_ADDED, "added", "green"),
        "removed" => (GLYPH_REMOVED, "removed", "red"),
        "modified" => (GLYPH_MODIFIED, "modified", "yellow"),
        "renamed" => (GLYPH_RENAMED, "renamed", "magenta"),
        "formatting_only" => (GLYPH_FORMAT, "format", "dimmed"),
        _ => (GLYPH_MODIFIED, "modified", "yellow"),
    };

    let colored_glyph = match glyph_color {
        "green" => glyph.green().bold().to_string(),
        "red" => glyph.red().bold().to_string(),
        "yellow" => glyph.yellow().bold().to_string(),
        "magenta" => glyph.magenta().bold().to_string(),
        "dimmed" => glyph.dimmed().to_string(),
        _ => glyph.to_string(),
    };

    let line_suffix = if opts.show_lines {
        format!(":{}", entity.line).dimmed().to_string()
    } else {
        String::new()
    };

    let breaking_flag = if entity.breaking {
        format!("           {} breaking", GLYPH_BREAKING).red().to_string()
    } else {
        String::new()
    };

    // For renamed entities, show old_name → new_name
    let name_part = if entity.change == "renamed" {
        if let Some(old_name) = &entity.old_name {
            format!(
                "{} {} {}",
                old_name.dimmed(),
                GLYPH_ARROW,
                entity.name.bold()
            )
        } else {
            format!("{}{}", entity.name.bold(), line_suffix)
        }
    } else {
        format!("{}{}", entity.name.bold(), line_suffix)
    };

    out.push_str(&format!(
        "  {}  {:<9} {:<20} {}{}\n",
        colored_glyph,
        verb,
        name_part,
        entity.kind.dimmed(),
        breaking_flag,
    ));

    // Continuation line for modified entities
    if entity.change == "modified" {
        render_modified_continuation(out, entity);
    }

    // Context snippets (--context mode)
    if opts.show_context {
        if let Some(ref ctx) = entity.context {
            if entity.change != "formatting_only" {
                render_context(out, ctx);
            }
        }
    }
}

fn render_modified_continuation(out: &mut String, entity: &OutputEntity) {
    let hash_dim = match (entity.sig_changed, entity.body_changed) {
        (Some(true), Some(true)) => "sig+body",
        (Some(true), Some(false)) => "sig only",
        (Some(false), Some(true)) => "body only",
        _ => "sig+body",
    };

    let mut parts: Vec<String> = vec![hash_dim.dimmed().to_string()];

    // Token diffs (max 4)
    let max_tokens = 4;
    for (i, tc) in entity.token_changes.iter().enumerate() {
        if i >= max_tokens {
            parts.push(format!("+{} more", entity.token_changes.len() - max_tokens).dimmed().to_string());
            break;
        }
        let desc = if !tc.from.is_empty() && !tc.to.is_empty() {
            format!(
                "{} {} {}",
                format!("\"{}\"", tc.from).dimmed(),
                GLYPH_ARROW,
                format!("\"{}\"", tc.to).dimmed(),
            )
        } else if tc.from.is_empty() {
            format!("+{}", tc.to).green().to_string()
        } else {
            format!("-{}", tc.from).red().to_string()
        };
        parts.push(desc);
    }

    // Pattern reference
    if let Some(ref pat_ref) = entity.pattern_ref {
        parts.push(format!("\u{2282} {}", pat_ref).cyan().to_string());
    }

    if !parts.is_empty() {
        let joined = parts.join(&format!("  {}  ", GLYPH_FORMAT));
        out.push_str(&format!("     {}\n", joined));
    }
}

fn render_context(out: &mut String, ctx: &SnippetContext) {
    // Show base → head diff as indented snippet
    if !ctx.base_snippet.is_empty() {
        for line in ctx.base_snippet.lines() {
            out.push_str(&format!("       {}\n", format!("- {}", line).red()));
        }
    }
    if !ctx.head_snippet.is_empty() {
        for line in ctx.head_snippet.lines() {
            out.push_str(&format!("       {}\n", format!("+ {}", line).green()));
        }
    }
}

// ── Summary footer ──────────────────────────────────────────────────────────

fn render_summary(out: &mut String, output: &DiffOutput) {
    let s = &output.summary;

    let total = s.patterns + s.moves + s.added + s.modified + s.removed + s.renamed + s.formatting_only;

    if total == 0 {
        out.push_str(&format!("  {}\n", "no structural changes".dimmed()));
        return;
    }

    let mut parts: Vec<String> = Vec::new();

    if s.patterns > 0 {
        parts.push(format!("{} patterns", s.patterns));
    }
    if s.moves > 0 {
        parts.push(format!("{} moves", s.moves));
    }
    if s.added > 0 {
        parts.push(format!("{} added", s.added));
    }
    if s.modified > 0 {
        parts.push(format!("{} modified", s.modified));
    }
    if s.removed > 0 {
        parts.push(format!("{} removed", s.removed));
    }
    if s.renamed > 0 {
        parts.push(format!("{} renamed", s.renamed));
    }
    if s.formatting_only > 0 {
        parts.push(format!("{} formatting", s.formatting_only));
    }

    let joined = parts.join(&format!("  {}  ", GLYPH_FORMAT));
    out.push_str(&format!("  {}\n", joined));
}

fn render_breaking(out: &mut String, output: &DiffOutput) {
    if !output.summary.has_breaking {
        return;
    }

    let entries: Vec<String> = output
        .breaking
        .iter()
        .map(|b| format!("{} ({})", b.entity, b.reason))
        .collect();

    let joined = entries.join(&format!("  {}  ", GLYPH_FORMAT));
    out.push_str(&format!(
        "  {} {}  {}\n",
        GLYPH_BREAKING.red(),
        "breaking:".red().bold(),
        joined,
    ));
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn separator() -> String {
    GLYPH_SEPARATOR.repeat(SEPARATOR_WIDTH)
}

// ── Legacy API (kept for backward compatibility until main.rs is updated) ───

/// Format a diff result for terminal output.
///
/// Deprecated: use `format_terminal_v2()` with `DiffOutput` instead.
pub fn format_terminal(result: &DiffResult) -> String {
    let mut output = String::new();

    // Group by file
    let mut by_file: std::collections::BTreeMap<&str, Vec<&EntityDiff>> =
        std::collections::BTreeMap::new();
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
            output.push_str(&format!(
                "  \u{25B8} {} ({})\n",
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
        )
        .dimmed()
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
            if diff.sig_changed == Some(true) {
                details.push("signature changed");
            }
            if diff.body_changed == Some(true) {
                details.push("body changed");
            }
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
                crate::change_detail::DetailKind::ValueChanged => {
                    format!("  │   {}", detail.description.yellow())
                }
                crate::change_detail::DetailKind::IdentifierChanged => {
                    format!("  │   {}", detail.description.cyan())
                }
                crate::change_detail::DetailKind::ArgumentAdded => {
                    format!("  │   {}", detail.description.green())
                }
                crate::change_detail::DetailKind::ArgumentRemoved => {
                    format!("  │   {}", detail.description.red())
                }
                crate::change_detail::DetailKind::Comment => {
                    format!("  │   {}", detail.description.dimmed())
                }
                crate::change_detail::DetailKind::LineAdded => {
                    format!("  │   {}", detail.description.green())
                }
                crate::change_detail::DetailKind::LineRemoved => {
                    format!("  │   {}", detail.description.red())
                }
            };
            line.push_str(&format!("\n{}", formatted));
        }
    }

    // Inline diff lines
    if let Some(ref lines) = diff.inline_diff {
        let display_lines = if lines.len() > 10 {
            &lines[..10]
        } else {
            lines
        };
        for dl in display_lines {
            let formatted = match dl.kind {
                crate::inline_diff::DiffLineKind::Removed => {
                    format!("  │   {}", format!("- {}", dl.text).red())
                }
                crate::inline_diff::DiffLineKind::Added => {
                    format!("  │   {}", format!("+ {}", dl.text).green())
                }
                crate::inline_diff::DiffLineKind::Context => {
                    format!("  │     {}", dl.text.dimmed())
                }
            };
            line.push_str(&format!("\n{}", formatted));
        }
        if lines.len() > 10 {
            line.push_str(&format!(
                "\n  │   {}",
                format!("... and {} more lines", lines.len() - 10).dimmed()
            ));
        }
    }

    line
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::output::{
        BreakingEntry, DiffOutput, FileSection, FileSummary, Meta, MoveEntry, OutputEntity,
        OutputPattern, OutputSummary,
    };

    fn make_meta(base_ref: &str) -> Meta {
        Meta {
            base_ref: base_ref.to_string(),
            head_ref: "HEAD".to_string(),
            base_sha: None,
            head_sha: None,
            generated_at: String::new(),
            sigil_version: "0.1.0".to_string(),
        }
    }

    fn make_summary(
        files_changed: usize,
        added: usize,
        modified: usize,
        removed: usize,
        formatting_only: usize,
        has_breaking: bool,
    ) -> OutputSummary {
        OutputSummary {
            files_changed,
            patterns: 0,
            moves: 0,
            added,
            removed,
            modified,
            renamed: 0,
            formatting_only,
            has_breaking,
            natural_language: String::new(),
        }
    }

    fn make_entity(change: &str, name: &str, kind: &str) -> OutputEntity {
        OutputEntity {
            change: change.to_string(),
            name: name.to_string(),
            kind: kind.to_string(),
            line: 10,
            line_end: 20,
            sig_changed: None,
            body_changed: None,
            breaking: false,
            breaking_reason: None,
            pattern_ref: None,
            token_changes: vec![],
            old_name: None,
            context: None,
        }
    }

    fn default_opts() -> FormatOptions {
        FormatOptions {
            show_lines: false,
            show_context: false,
            use_color: false,
        }
    }

    fn empty_output(base_ref: &str) -> DiffOutput {
        DiffOutput {
            meta: make_meta(base_ref),
            summary: make_summary(0, 0, 0, 0, 0, false),
            breaking: vec![],
            patterns: vec![],
            moves: vec![],
            files: vec![],
        }
    }

    // ── Header tests ────────────────────────────────────────────────────

    #[test]
    fn header_shows_refspec_and_file_count() {
        let mut output = empty_output("HEAD~1");
        output.summary.files_changed = 4;
        let text = format_terminal_v2(&output, &default_opts());
        assert!(text.contains("sigil diff  HEAD~1"));
        assert!(text.contains("4 files"));
    }

    #[test]
    fn header_shows_breaking_count() {
        let mut output = empty_output("HEAD~1");
        output.summary.files_changed = 2;
        output.summary.has_breaking = true;
        output.summary.modified = 1;
        output.breaking = vec![BreakingEntry {
            entity: "execute_payment".to_string(),
            kind: "function".to_string(),
            file: "src/payments.py".to_string(),
            line: 47,
            reason: "public signature changed".to_string(),
        }];
        output.files = vec![FileSection {
            file: "src/payments.py".to_string(),
            summary: FileSummary {
                added: 0,
                modified: 1,
                removed: 0,
                renamed: 0,
                formatting_only: 0,
            },
            entities: vec![],
        }];
        let text = format_terminal_v2(&output, &default_opts());
        assert!(
            text.contains("breaking"),
            "expected 'breaking' in header, got:\n{}",
            text
        );
        assert!(text.contains("1 breaking"));
    }

    #[test]
    fn header_shows_formatting_only() {
        let mut output = empty_output("HEAD~1");
        output.summary.files_changed = 1;
        output.summary.formatting_only = 3;
        output.files = vec![FileSection {
            file: "src/a.py".to_string(),
            summary: FileSummary {
                added: 0,
                modified: 0,
                removed: 0,
                renamed: 0,
                formatting_only: 3,
            },
            entities: vec![make_entity("formatting_only", "foo", "function")],
        }];
        let text = format_terminal_v2(&output, &default_opts());
        assert!(
            text.contains("(formatting only)"),
            "expected '(formatting only)' in header, got:\n{}",
            text
        );
    }

    // ── Summary tests ───────────────────────────────────────────────────

    #[test]
    fn summary_with_mixed_counts() {
        let mut output = empty_output("HEAD~1");
        output.summary = OutputSummary {
            files_changed: 3,
            patterns: 2,
            moves: 1,
            added: 3,
            removed: 1,
            modified: 2,
            renamed: 0,
            formatting_only: 5,
            has_breaking: false,
            natural_language: String::new(),
        };
        let text = format_terminal_v2(&output, &default_opts());
        assert!(text.contains("2 patterns"));
        assert!(text.contains("1 moves"));
        assert!(text.contains("3 added"));
        assert!(text.contains("2 modified"));
        assert!(text.contains("1 removed"));
        assert!(text.contains("5 formatting"));
        // zero counts should be omitted
        assert!(!text.contains("renamed"));
    }

    #[test]
    fn empty_diff_shows_no_structural_changes() {
        let output = empty_output("HEAD~1");
        let text = format_terminal_v2(&output, &default_opts());
        assert!(
            text.contains("no structural changes"),
            "expected 'no structural changes', got:\n{}",
            text
        );
    }

    // ── Entity row tests ────────────────────────────────────────────────

    #[test]
    fn entity_rows_with_different_change_types() {
        let mut output = empty_output("HEAD~1");
        output.summary = make_summary(1, 1, 1, 1, 1, false);
        output.files = vec![FileSection {
            file: "src/payments.py".to_string(),
            summary: FileSummary {
                added: 1,
                modified: 1,
                removed: 1,
                renamed: 0,
                formatting_only: 1,
            },
            entities: vec![
                make_entity("added", "PaymentAuditLog", "class"),
                {
                    let mut e = make_entity("modified", "execute_payment", "function");
                    e.sig_changed = Some(true);
                    e.body_changed = Some(true);
                    e
                },
                make_entity("removed", "old_handler", "function"),
                make_entity("formatting_only", "calculate_total", "function"),
            ],
        }];
        let text = format_terminal_v2(&output, &default_opts());
        // Added
        assert!(text.contains("+"));
        assert!(text.contains("added"));
        assert!(text.contains("PaymentAuditLog"));
        // Modified
        assert!(text.contains("~"));
        assert!(text.contains("modified"));
        assert!(text.contains("execute_payment"));
        assert!(text.contains("sig+body"));
        // Removed
        assert!(text.contains("\u{2212}")); // −
        assert!(text.contains("removed"));
        assert!(text.contains("old_handler"));
        // Formatting only (collapsed in no-color mode)
        assert!(text.contains("formatting only"));
        assert!(text.contains("calculate_total"));
    }

    #[test]
    fn modified_entity_with_token_changes() {
        let mut output = empty_output("HEAD~1");
        let mut entity = make_entity("modified", "execute_payment", "function");
        entity.sig_changed = Some(false);
        entity.body_changed = Some(true);
        entity.token_changes = vec![
            TokenChange {
                change_type: "value_changed".to_string(),
                from: "true".to_string(),
                to: "false".to_string(),
            },
            TokenChange {
                change_type: "identifier_renamed".to_string(),
                from: "validate_card".to_string(),
                to: "check_card".to_string(),
            },
        ];
        entity.pattern_ref = Some("pat_1".to_string());
        output.summary = make_summary(1, 0, 1, 0, 0, false);
        output.files = vec![FileSection {
            file: "src/payments.py".to_string(),
            summary: FileSummary {
                added: 0,
                modified: 1,
                removed: 0,
                renamed: 0,
                formatting_only: 0,
            },
            entities: vec![entity],
        }];
        let text = format_terminal_v2(&output, &default_opts());
        assert!(text.contains("body only"));
        assert!(text.contains("\"true\""));
        assert!(text.contains("\"false\""));
        assert!(text.contains("\"validate_card\""));
        assert!(text.contains("\"check_card\""));
        assert!(text.contains("\u{2282} pat_1")); // ⊂ pat_1
    }

    // ── Patterns section tests ──────────────────────────────────────────

    #[test]
    fn patterns_section_rendering() {
        let mut output = empty_output("HEAD~1");
        output.summary.patterns = 1;
        output.patterns = vec![OutputPattern {
            id: "pat_1".to_string(),
            pattern_type: "rename".to_string(),
            entity_kind: "function".to_string(),
            from_glob: Some("validate_*".to_string()),
            to_glob: Some("check_*".to_string()),
            entity_name: None,
            file_count: 5,
            files: vec![
                "src/a.py".to_string(),
                "src/b.py".to_string(),
                "src/c.py".to_string(),
                "src/d.py".to_string(),
                "src/e.py".to_string(),
            ],
            entities: vec![],
        }];
        let text = format_terminal_v2(&output, &default_opts());
        assert!(text.contains("patterns"));
        assert!(text.contains("\u{2261}")); // ≡
        assert!(text.contains("rename"));
        assert!(text.contains("validate_*"));
        assert!(text.contains("check_*"));
        assert!(text.contains("\u{00D7}5 files")); // ×5 files
        assert!(text.contains("function"));
        // First 3 files shown
        assert!(text.contains("src/a.py"));
        assert!(text.contains("src/b.py"));
        assert!(text.contains("src/c.py"));
        // +2 more
        assert!(text.contains("+2 more"));
    }

    // ── Moves section tests ─────────────────────────────────────────────

    #[test]
    fn moves_section_rendering() {
        let mut output = empty_output("HEAD~1");
        output.summary.moves = 1;
        output.moves = vec![MoveEntry {
            entity: "execute_payment".to_string(),
            kind: "function".to_string(),
            from_file: "src/old.py".to_string(),
            to_file: "src/new.py".to_string(),
            from_line: 10,
            to_line: 20,
            breaking: true,
            confidence: 0.8,
        }];
        output.summary.has_breaking = true;
        output.breaking = vec![BreakingEntry {
            entity: "execute_payment".to_string(),
            kind: "function".to_string(),
            file: "src/new.py".to_string(),
            line: 20,
            reason: "public entity moved".to_string(),
        }];
        let text = format_terminal_v2(&output, &default_opts());
        assert!(text.contains("moves"));
        assert!(text.contains("\u{21D2}")); // ⇒
        assert!(text.contains("execute_payment"));
        assert!(text.contains("function"));
        assert!(text.contains("breaking"));
        assert!(text.contains("src/old.py"));
        assert!(text.contains("src/new.py"));
        assert!(text.contains("\u{2192}")); // →
    }

    // ── Breaking footer tests ───────────────────────────────────────────

    #[test]
    fn breaking_footer_shows_entries() {
        let mut output = empty_output("HEAD~1");
        output.summary.has_breaking = true;
        output.summary.modified = 1;
        output.summary.removed = 1;
        output.summary.files_changed = 1;
        output.breaking = vec![
            BreakingEntry {
                entity: "execute_payment".to_string(),
                kind: "function".to_string(),
                file: "src/payments.py".to_string(),
                line: 47,
                reason: "sig changed".to_string(),
            },
            BreakingEntry {
                entity: "old_handler".to_string(),
                kind: "function".to_string(),
                file: "src/payments.py".to_string(),
                line: 100,
                reason: "removed".to_string(),
            },
        ];
        let text = format_terminal_v2(&output, &default_opts());
        assert!(text.contains("\u{26A0}")); // ⚠
        assert!(text.contains("breaking:"));
        assert!(text.contains("execute_payment (sig changed)"));
        assert!(text.contains("old_handler (removed)"));
    }

    // ── Lines mode test ─────────────────────────────────────────────────

    #[test]
    fn lines_mode_appends_line_number() {
        let mut output = empty_output("HEAD~1");
        output.summary = make_summary(1, 1, 0, 0, 0, false);
        let mut entity = make_entity("added", "foo", "function");
        entity.line = 42;
        output.files = vec![FileSection {
            file: "src/a.py".to_string(),
            summary: FileSummary {
                added: 1,
                modified: 0,
                removed: 0,
                renamed: 0,
                formatting_only: 0,
            },
            entities: vec![entity],
        }];
        let opts = FormatOptions {
            show_lines: true,
            show_context: false,
            use_color: false,
        };
        let text = format_terminal_v2(&output, &opts);
        assert!(
            text.contains(":42"),
            "expected ':42' in output, got:\n{}",
            text
        );
    }

    // ── File header counts test ─────────────────────────────────────────

    #[test]
    fn file_header_shows_counts() {
        let mut output = empty_output("HEAD~1");
        output.summary = make_summary(1, 1, 2, 0, 3, false);
        output.files = vec![FileSection {
            file: "src/payments.py".to_string(),
            summary: FileSummary {
                added: 1,
                modified: 2,
                removed: 0,
                renamed: 0,
                formatting_only: 3,
            },
            entities: vec![],
        }];
        let text = format_terminal_v2(&output, &default_opts());
        assert!(text.contains("+1"), "expected +1 in file header");
        assert!(text.contains("~2"), "expected ~2 in file header");
        assert!(
            text.contains("\u{00B7}3"),
            "expected ·3 in file header"
        );
        // removed is 0, should not appear
        assert!(
            !text.contains("\u{2212}0"),
            "should not show −0 in file header"
        );
    }

    // ── Separator test ──────────────────────────────────────────────────

    #[test]
    fn output_contains_separators() {
        let output = empty_output("HEAD~1");
        let text = format_terminal_v2(&output, &default_opts());
        let sep = "\u{2500}".repeat(60);
        let count = text.matches(&sep).count();
        // Header (2) + footer (2) = at least 4 separator lines
        assert!(
            count >= 4,
            "expected at least 4 separator lines, found {}",
            count
        );
    }
}
