use crate::output::*;

/// Options controlling markdown output rendering.
pub struct MarkdownOptions {
    pub use_emoji: bool,
    pub show_context: bool,
}

/// Return the display glyph for a given concept, respecting emoji preference.
fn glyph(concept: &str, use_emoji: bool) -> &'static str {
    match (concept, use_emoji) {
        ("breaking", true) => "\u{26A0}\u{FE0F}",
        ("breaking", false) => "!",
        ("added", true) => "\u{2726}",
        ("added", false) => "+",
        ("modified", _) => "~",
        ("removed", true) => "\u{2212}",
        ("removed", false) => "-",
        ("renamed", _) => "\u{2248}",
        ("pattern", _) => "\u{2261}",
        ("move", true) => "\u{2197}",
        ("move", false) => "=>",
        ("format", _) => "\u{00B7}",
        _ => "?",
    }
}

/// Format a `DiffOutput` as a markdown string.
pub fn format_markdown(output: &DiffOutput, opts: &MarkdownOptions) -> String {
    let mut md = String::new();

    // Check if there are no structural changes
    let total_changes = output.summary.added
        + output.summary.removed
        + output.summary.modified
        + output.summary.renamed
        + output.summary.moves
        + output.summary.formatting_only;

    if total_changes == 0 && output.files.is_empty() && output.moves.is_empty() && output.patterns.is_empty() {
        let check = if opts.use_emoji { "\u{2713}" } else { "ok" };
        md.push_str(&format!(
            "{} sigil diff `{}` \u{2014} no structural changes\n",
            check,
            format_ref_spec(&output.meta.base_ref, &output.meta.head_ref),
        ));
        return md;
    }

    // Header line
    render_header(&mut md, output, opts);
    md.push_str("\n\n---\n");

    // Patterns section
    if !output.patterns.is_empty() {
        md.push_str("\n**patterns**\n");
        for pat in &output.patterns {
            render_pattern(&mut md, pat, opts);
        }
    }

    // Moves section
    if !output.moves.is_empty() {
        if !output.patterns.is_empty() {
            md.push('\n');
        }
        md.push_str("\n**moves**\n");
        for mv in &output.moves {
            render_move(&mut md, mv, opts);
        }
    }

    // Separator before file sections (only if patterns or moves were rendered)
    if !output.patterns.is_empty() || !output.moves.is_empty() {
        md.push_str("\n---\n");
    }

    // File sections
    for file_section in &output.files {
        md.push('\n');
        render_file_section(&mut md, file_section, opts);
    }

    // Final separator + summary
    md.push_str("\n---\n\n");
    render_summary(&mut md, output, opts);

    md
}

/// Build a display ref spec string from base and head refs.
fn format_ref_spec(base_ref: &str, head_ref: &str) -> String {
    if head_ref == "HEAD" || head_ref.is_empty() {
        base_ref.to_string()
    } else {
        format!("{}..{}", base_ref, head_ref)
    }
}

/// Render the header line.
fn render_header(md: &mut String, output: &DiffOutput, opts: &MarkdownOptions) {
    let ref_spec = format_ref_spec(&output.meta.base_ref, &output.meta.head_ref);
    let files = output.summary.files_changed;

    if output.summary.has_breaking {
        let brk = glyph("breaking", opts.use_emoji);
        let breaking_count = output.breaking.len();
        md.push_str(&format!(
            "{} **sigil diff** `{}` \u{2014} {} file{}, **{} breaking**",
            brk,
            ref_spec,
            files,
            if files == 1 { "" } else { "s" },
            breaking_count,
        ));
    } else {
        md.push_str(&format!(
            "**sigil diff** `{}` \u{2014} {} file{}",
            ref_spec,
            files,
            if files == 1 { "" } else { "s" },
        ));
    }

    if let Some(ref summary_line) = output.summary.summary_line {
        md.push_str(&format!("\n\n_{}_", summary_line));
    }
}

/// Render a single pattern entry.
fn render_pattern(md: &mut String, pat: &OutputPattern, opts: &MarkdownOptions) {
    let g = glyph("pattern", opts.use_emoji);
    let kind = &pat.entity_kind;

    match pat.pattern_type.as_str() {
        "rename" => {
            let from_display = pat.from_glob.as_deref().unwrap_or("*");
            let to_display = pat.to_glob.as_deref().unwrap_or("*");
            md.push_str(&format!(
                "- {} rename `{}` \u{2192} `{}`  \u{00D7}{} files  ({})\n",
                g, from_display, to_display, pat.file_count, kind,
            ));
        }
        _ => {
            // body_identical or other
            let name = pat.entity_name.as_deref().unwrap_or("?");
            md.push_str(&format!(
                "- {} {} `{}`  \u{00D7}{} files  ({})\n",
                g, pat.pattern_type, name, pat.file_count, kind,
            ));
        }
    }

    // File listing: first 3, then +N more
    render_collapsed_list(md, &pat.files, 3, "  ");
}

/// Render a single move entry.
fn render_move(md: &mut String, mv: &MoveEntry, opts: &MarkdownOptions) {
    let g = glyph("move", opts.use_emoji);
    let brk_suffix = if mv.breaking {
        format!(" {} breaking", glyph("breaking", opts.use_emoji))
    } else {
        String::new()
    };
    md.push_str(&format!(
        "- {} `{}` ({}){}  \n",
        g, mv.entity, mv.kind, brk_suffix,
    ));
    md.push_str(&format!(
        "  `{}` \u{2192} `{}`\n",
        mv.from_file, mv.to_file,
    ));
}

/// Render a file section with its entities.
fn render_file_section(md: &mut String, section: &FileSection, opts: &MarkdownOptions) {
    // File header: `path`  +A ~M -R ·F
    let s = &section.summary;
    let mut counts: Vec<String> = Vec::new();
    if s.added > 0 {
        counts.push(format!("+{}", s.added));
    }
    if s.modified > 0 {
        counts.push(format!("~{}", s.modified));
    }
    if s.removed > 0 {
        counts.push(format!("-{}", s.removed));
    }
    if s.renamed > 0 {
        counts.push(format!("r{}", s.renamed));
    }
    if s.formatting_only > 0 {
        counts.push(format!("\u{00B7}{}", s.formatting_only));
    }

    let count_str = counts.join(" ");
    md.push_str(&format!("`{}`  {}\n", section.file, count_str));

    // Separate entities into structural changes vs formatting-only
    let mut structural: Vec<&OutputEntity> = Vec::new();
    let mut formatting_names: Vec<&str> = Vec::new();

    for entity in &section.entities {
        if entity.change == "formatting_only" {
            formatting_names.push(&entity.name);
        } else {
            structural.push(entity);
        }
    }

    // Render structural entities
    for entity in &structural {
        render_entity(md, entity, opts);
    }

    // Render formatting-only as collapsed single line
    if !formatting_names.is_empty() {
        let g = glyph("format", opts.use_emoji);
        let count = formatting_names.len();
        let first_three: Vec<String> = formatting_names.iter()
            .take(3)
            .map(|n| format!("`{}`", n))
            .collect();
        let names_str = first_three.join(" ");
        if count > 3 {
            md.push_str(&format!(
                "- {} {} formatting only: {} +{} more\n",
                g, count, names_str, count - 3,
            ));
        } else {
            md.push_str(&format!(
                "- {} {} formatting only: {}\n",
                g, count, names_str,
            ));
        }
    }
}

/// Render a single entity bullet.
fn render_entity(md: &mut String, entity: &OutputEntity, opts: &MarkdownOptions) {
    let change_glyph = match entity.change.as_str() {
        "added" => glyph("added", opts.use_emoji),
        "modified" => glyph("modified", opts.use_emoji),
        "removed" => glyph("removed", opts.use_emoji),
        "renamed" => glyph("renamed", opts.use_emoji),
        _ => "?",
    };

    let brk_prefix = if entity.breaking {
        format!("{} ", glyph("breaking", opts.use_emoji))
    } else {
        String::new()
    };

    // Change detail suffix for modified entities
    let detail_suffix = if entity.change == "modified" {
        build_change_detail_suffix(entity)
    } else {
        String::new()
    };

    // Pattern cross-ref
    let pat_suffix = if let Some(ref pat_id) = entity.pattern_ref {
        format!(" \u{2282} {}", pat_id)
    } else {
        String::new()
    };

    // Old name for renamed entities
    let rename_info = if let Some(ref old_name) = entity.old_name {
        format!(" (was `{}`)", old_name)
    } else {
        String::new()
    };

    md.push_str(&format!(
        "- {}{} {} `{}` ({}){}{}{}\n",
        brk_prefix,
        change_glyph,
        entity.change,
        entity.name,
        entity.kind,
        detail_suffix,
        rename_info,
        pat_suffix,
    ));

    // Token changes as blockquote
    if !entity.token_changes.is_empty() {
        let tokens: Vec<String> = entity.token_changes.iter()
            .map(|tc| {
                if tc.from.is_empty() {
                    format!("+`{}`", tc.to)
                } else if tc.to.is_empty() {
                    format!("-`{}`", tc.from)
                } else {
                    format!("`{}` \u{2192} `{}`", tc.from, tc.to)
                }
            })
            .collect();
        let token_line = tokens.join(" \u{00B7} ");
        md.push_str(&format!("  > {}\n", token_line));
    }

    // Context code block
    if opts.show_context {
        if let Some(ref ctx) = entity.context {
            render_context_block(md, ctx);
        }
    }
}

/// Build "— sig+body" or "— sig" or "— body" suffix for modified entities.
fn build_change_detail_suffix(entity: &OutputEntity) -> String {
    let sig = entity.sig_changed.unwrap_or(false);
    let body = entity.body_changed.unwrap_or(false);
    match (sig, body) {
        (true, true) => " \u{2014} sig+body".to_string(),
        (true, false) => " \u{2014} sig".to_string(),
        (false, true) => " \u{2014} body".to_string(),
        (false, false) => String::new(),
    }
}

/// Render a fenced context code block for a snippet.
fn render_context_block(md: &mut String, ctx: &SnippetContext) {
    if let Some(ref hunks) = ctx.hunks {
        md.push_str("  ```diff\n");
        for line in hunks {
            match line.kind {
                crate::inline_diff::DiffLineKind::Context => {
                    md.push_str(&format!("   {}\n", line.text));
                }
                crate::inline_diff::DiffLineKind::Removed => {
                    md.push_str(&format!("  -{}\n", line.text));
                }
                crate::inline_diff::DiffLineKind::Added => {
                    md.push_str(&format!("  +{}\n", line.text));
                }
                crate::inline_diff::DiffLineKind::Separator => {
                    md.push_str("  ...\n");
                }
            }
        }
        md.push_str("  ```\n");
        return;
    }
    // Fallback for old-style base/head snippets (backward compat)
    md.push_str(&format!("  ```{}\n", ctx.language));
    md.push_str("  # before\n");
    for line in ctx.base_snippet.lines() {
        md.push_str(&format!("  {}\n", line));
    }
    md.push_str("  # after\n");
    for line in ctx.head_snippet.lines() {
        md.push_str(&format!("  {}\n", line));
    }
    md.push_str("  ```\n");
}

/// Render a collapsed inline list: first N items as backtick-wrapped, then +M more.
fn render_collapsed_list(md: &mut String, items: &[String], max_show: usize, indent: &str) {
    if items.is_empty() {
        return;
    }
    let shown: Vec<String> = items.iter()
        .take(max_show)
        .map(|f| format!("`{}`", f))
        .collect();
    let display = shown.join(" ");
    if items.len() > max_show {
        md.push_str(&format!(
            "{}{} +{} more\n",
            indent, display, items.len() - max_show,
        ));
    } else {
        md.push_str(&format!("{}{}\n", indent, display));
    }
}

/// Render the bottom summary and breaking entries.
fn render_summary(md: &mut String, output: &DiffOutput, opts: &MarkdownOptions) {
    let s = &output.summary;
    let mut parts: Vec<String> = Vec::new();
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
    md.push_str(&format!("{}\n", parts.join(" \u{00B7} ")));

    // Breaking entries
    if !output.breaking.is_empty() {
        let brk = glyph("breaking", opts.use_emoji);
        let entries: Vec<String> = output.breaking.iter()
            .map(|b| format!("`{}` ({})", b.entity, b.reason))
            .collect();
        md.push_str(&format!(
            "{} **breaking:** {}\n",
            brk,
            entries.join(" \u{00B7} "),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to build a minimal DiffOutput for testing.
    fn make_output(
        base_ref: &str,
        head_ref: &str,
        files: Vec<FileSection>,
        patterns: Vec<OutputPattern>,
        moves: Vec<MoveEntry>,
        breaking: Vec<BreakingEntry>,
    ) -> DiffOutput {
        let mut added = 0;
        let mut modified = 0;
        let mut removed = 0;
        let mut renamed = 0;
        let mut formatting_only = 0;
        let mut all_files = std::collections::HashSet::new();

        for f in &files {
            all_files.insert(f.file.clone());
            added += f.summary.added;
            modified += f.summary.modified;
            removed += f.summary.removed;
            renamed += f.summary.renamed;
            formatting_only += f.summary.formatting_only;
        }
        for m in &moves {
            all_files.insert(m.from_file.clone());
            all_files.insert(m.to_file.clone());
        }

        let has_breaking = !breaking.is_empty();

        DiffOutput {
            meta: Meta {
                base_ref: base_ref.to_string(),
                head_ref: head_ref.to_string(),
                base_sha: None,
                head_sha: None,
                generated_at: String::new(),
                sigil_version: "0.1.0".to_string(),
            },
            summary: OutputSummary {
                files_changed: all_files.len(),
                patterns: patterns.len(),
                moves: moves.len(),
                added,
                removed,
                modified,
                renamed,
                formatting_only,
                has_breaking,
                natural_language: String::new(),
                summary_line: None,
            },
            breaking,
            patterns,
            moves,
            files,
            groups: None,
        }
    }

    fn make_file_section(
        file: &str,
        entities: Vec<OutputEntity>,
    ) -> FileSection {
        let mut summary = FileSummary {
            added: 0,
            modified: 0,
            removed: 0,
            renamed: 0,
            formatting_only: 0,
        };
        for e in &entities {
            match e.change.as_str() {
                "added" => summary.added += 1,
                "modified" => summary.modified += 1,
                "removed" => summary.removed += 1,
                "renamed" => summary.renamed += 1,
                "formatting_only" => summary.formatting_only += 1,
                _ => {}
            }
        }
        FileSection {
            file: file.to_string(),
            summary,
            entities,
        }
    }

    fn make_entity(
        change: &str,
        name: &str,
        kind: &str,
        breaking: bool,
    ) -> OutputEntity {
        OutputEntity {
            change: change.to_string(),
            name: name.to_string(),
            kind: kind.to_string(),
            line: 1,
            line_end: 10,
            sig_changed: if change == "modified" { Some(true) } else { None },
            body_changed: if change == "modified" { Some(true) } else { None },
            breaking,
            breaking_reason: if breaking { Some("public signature changed".to_string()) } else { None },
            pattern_ref: None,
            token_changes: Vec::new(),
            old_name: None,
            context: None,
        }
    }

    #[test]
    fn test_full_markdown_output_mixed_entities() {
        let entities = vec![
            {
                let mut e = make_entity("modified", "execute_payment", "function", true);
                e.sig_changed = Some(true);
                e.body_changed = Some(true);
                e.token_changes = vec![
                    TokenChange {
                        change_type: "value_changed".to_string(),
                        from: "true".to_string(),
                        to: "false".to_string(),
                    },
                ];
                e.pattern_ref = Some("pat_1".to_string());
                e
            },
            make_entity("added", "PaymentAuditLog", "class", false),
            make_entity("removed", "old_handler", "function", true),
        ];
        let section = make_file_section("src/payments.py", entities);
        let breaking = vec![
            BreakingEntry {
                entity: "execute_payment".to_string(),
                kind: "function".to_string(),
                file: "src/payments.py".to_string(),
                line: 1,
                reason: "sig changed".to_string(),
                external_callers: None,
                callers_in_diff: None,
            },
            BreakingEntry {
                entity: "old_handler".to_string(),
                kind: "function".to_string(),
                file: "src/payments.py".to_string(),
                line: 1,
                reason: "removed".to_string(),
                external_callers: None,
                callers_in_diff: None,
            },
        ];
        let output = make_output("HEAD~1", "HEAD", vec![section], vec![], vec![], breaking);
        let opts = MarkdownOptions { use_emoji: true, show_context: false };
        let md = format_markdown(&output, &opts);

        // Header has breaking
        assert!(md.contains("\u{26A0}\u{FE0F} **sigil diff** `HEAD~1`"));
        assert!(md.contains("**2 breaking**"));
        // File section
        assert!(md.contains("`src/payments.py`  +1 ~1 -1"));
        // Modified entity
        assert!(md.contains("~ modified `execute_payment` (function) \u{2014} sig+body"));
        assert!(md.contains("\u{2282} pat_1"));
        // Token change blockquote
        assert!(md.contains("> `true` \u{2192} `false`"));
        // Added entity
        assert!(md.contains("\u{2726} added `PaymentAuditLog` (class)"));
        // Removed entity
        assert!(md.contains("\u{2212} removed `old_handler` (function)"));
        // Summary footer
        assert!(md.contains("1 added"));
        assert!(md.contains("1 modified"));
        assert!(md.contains("1 removed"));
        // Breaking footer
        assert!(md.contains("**breaking:**"));
        assert!(md.contains("`execute_payment` (sig changed)"));
        assert!(md.contains("`old_handler` (removed)"));
    }

    #[test]
    fn test_no_emoji_mode() {
        let section = make_file_section("src/main.rs", vec![
            make_entity("added", "new_fn", "function", false),
            make_entity("removed", "old_fn", "function", true),
        ]);
        let breaking = vec![
            BreakingEntry {
                entity: "old_fn".to_string(),
                kind: "function".to_string(),
                file: "src/main.rs".to_string(),
                line: 1,
                reason: "removed".to_string(),
                external_callers: None,
                callers_in_diff: None,
            },
        ];
        let output = make_output("HEAD~1", "HEAD", vec![section], vec![], vec![], breaking);
        let opts = MarkdownOptions { use_emoji: false, show_context: false };
        let md = format_markdown(&output, &opts);

        // ASCII glyphs
        assert!(md.contains("! **sigil diff**"));
        assert!(md.contains("+ added `new_fn`"));
        assert!(md.contains("- removed `old_fn`"));
        // No emoji characters
        assert!(!md.contains("\u{26A0}"));
        assert!(!md.contains("\u{2726}"));
        assert!(!md.contains("\u{2212}"));
    }

    #[test]
    fn test_empty_diff_output() {
        let output = make_output("HEAD~1", "HEAD", vec![], vec![], vec![], vec![]);
        let opts = MarkdownOptions { use_emoji: true, show_context: false };
        let md = format_markdown(&output, &opts);

        assert!(md.contains("no structural changes"));
        assert!(md.contains("`HEAD~1`"));
        // Should not contain separators or sections
        assert!(!md.contains("---"));
    }

    #[test]
    fn test_empty_diff_no_emoji() {
        let output = make_output("HEAD~1", "HEAD", vec![], vec![], vec![], vec![]);
        let opts = MarkdownOptions { use_emoji: false, show_context: false };
        let md = format_markdown(&output, &opts);

        assert!(md.contains("ok sigil diff `HEAD~1`"));
        assert!(md.contains("no structural changes"));
    }

    #[test]
    fn test_formatting_only_collapse() {
        let entities = vec![
            {
                let mut e = make_entity("formatting_only", "func_a", "function", false);
                e.change = "formatting_only".to_string();
                e.sig_changed = None;
                e.body_changed = None;
                e
            },
            {
                let mut e = make_entity("formatting_only", "func_b", "function", false);
                e.change = "formatting_only".to_string();
                e.sig_changed = None;
                e.body_changed = None;
                e
            },
            {
                let mut e = make_entity("formatting_only", "func_c", "function", false);
                e.change = "formatting_only".to_string();
                e.sig_changed = None;
                e.body_changed = None;
                e
            },
            {
                let mut e = make_entity("formatting_only", "func_d", "function", false);
                e.change = "formatting_only".to_string();
                e.sig_changed = None;
                e.body_changed = None;
                e
            },
            {
                let mut e = make_entity("formatting_only", "func_e", "function", false);
                e.change = "formatting_only".to_string();
                e.sig_changed = None;
                e.body_changed = None;
                e
            },
        ];
        let section = make_file_section("src/utils.py", entities);
        let output = make_output("HEAD~1", "HEAD", vec![section], vec![], vec![], vec![]);
        let opts = MarkdownOptions { use_emoji: true, show_context: false };
        let md = format_markdown(&output, &opts);

        // Should be collapsed to a single line
        assert!(md.contains("\u{00B7} 5 formatting only:"));
        assert!(md.contains("`func_a` `func_b` `func_c`"));
        assert!(md.contains("+2 more"));
        // Should NOT have individual bullets for formatting entities
        assert!(!md.contains("modified `func_a`"));
        assert!(!md.contains("added `func_a`"));
    }

    #[test]
    fn test_formatting_only_three_or_fewer() {
        let entities = vec![
            {
                let mut e = make_entity("formatting_only", "fn_x", "function", false);
                e.change = "formatting_only".to_string();
                e.sig_changed = None;
                e.body_changed = None;
                e
            },
            {
                let mut e = make_entity("formatting_only", "fn_y", "function", false);
                e.change = "formatting_only".to_string();
                e.sig_changed = None;
                e.body_changed = None;
                e
            },
        ];
        let section = make_file_section("src/lib.rs", entities);
        let output = make_output("HEAD~1", "HEAD", vec![section], vec![], vec![], vec![]);
        let opts = MarkdownOptions { use_emoji: true, show_context: false };
        let md = format_markdown(&output, &opts);

        // All names shown, no "+N more"
        assert!(md.contains("\u{00B7} 2 formatting only: `fn_x` `fn_y`"));
        assert!(!md.contains("+"));
    }

    #[test]
    fn test_formatting_only_not_empty_diff() {
        // A diff with ONLY formatting_only entities should render file sections,
        // NOT the "no structural changes" early return.
        let entities = vec![
            {
                let mut e = make_entity("formatting_only", "func_a", "function", false);
                e.change = "formatting_only".to_string();
                e.sig_changed = None;
                e.body_changed = None;
                e
            },
        ];
        let section = make_file_section("src/main.py", entities);
        let output = make_output("HEAD~1", "HEAD", vec![section], vec![], vec![], vec![]);
        let opts = MarkdownOptions { use_emoji: true, show_context: false };
        let md = format_markdown(&output, &opts);

        // Should NOT show "no structural changes"
        assert!(
            !md.contains("no structural changes"),
            "formatting-only diff must not say 'no structural changes', got:\n{}",
            md,
        );
        // Should show the file section
        assert!(md.contains("`src/main.py`"));
        // Should show the formatting count in summary
        assert!(md.contains("1 formatting"));
    }

    #[test]
    fn test_patterns_section() {
        let pattern = OutputPattern {
            id: "pat_1".to_string(),
            pattern_type: "rename".to_string(),
            entity_kind: "function".to_string(),
            from_glob: Some("validate_*".to_string()),
            to_glob: Some("check_*".to_string()),
            entity_name: None,
            file_count: 8,
            files: vec![
                "file1.py".to_string(),
                "file2.py".to_string(),
                "file3.py".to_string(),
                "file4.py".to_string(),
                "file5.py".to_string(),
            ],
            entities: vec!["validate_card".to_string()],
        };
        let section = make_file_section("file1.py", vec![
            make_entity("modified", "check_card", "function", false),
        ]);
        let output = make_output("HEAD~1", "HEAD", vec![section], vec![pattern], vec![], vec![]);
        let opts = MarkdownOptions { use_emoji: true, show_context: false };
        let md = format_markdown(&output, &opts);

        assert!(md.contains("**patterns**"));
        assert!(md.contains("\u{2261} rename `validate_*` \u{2192} `check_*`"));
        assert!(md.contains("\u{00D7}8 files"));
        assert!(md.contains("(function)"));
        // File listing: 3 shown + 2 more
        assert!(md.contains("`file1.py` `file2.py` `file3.py` +2 more"));
    }

    #[test]
    fn test_body_identical_pattern() {
        let pattern = OutputPattern {
            id: "pat_1".to_string(),
            pattern_type: "body_identical".to_string(),
            entity_kind: "function".to_string(),
            from_glob: None,
            to_glob: None,
            entity_name: Some("log_event".to_string()),
            file_count: 3,
            files: vec!["a.py".to_string(), "b.py".to_string(), "c.py".to_string()],
            entities: vec!["log_event".to_string()],
        };
        let output = make_output("HEAD~1", "HEAD", vec![], vec![pattern], vec![], vec![]);
        let opts = MarkdownOptions { use_emoji: true, show_context: false };
        let md = format_markdown(&output, &opts);

        assert!(md.contains("\u{2261} body_identical `log_event`  \u{00D7}3 files  (function)"));
        assert!(md.contains("`a.py` `b.py` `c.py`"));
    }

    #[test]
    fn test_moves_section() {
        let mv = MoveEntry {
            entity: "process_order".to_string(),
            kind: "function".to_string(),
            from_file: "src/old_module.py".to_string(),
            to_file: "src/new_module.py".to_string(),
            from_line: 10,
            to_line: 20,
            breaking: true,
            confidence: 0.8,
        };
        let breaking = vec![
            BreakingEntry {
                entity: "process_order".to_string(),
                kind: "function".to_string(),
                file: "src/new_module.py".to_string(),
                line: 20,
                reason: "moved".to_string(),
                external_callers: None,
                callers_in_diff: None,
            },
        ];
        let output = make_output("HEAD~1", "HEAD", vec![], vec![], vec![mv], breaking);
        let opts = MarkdownOptions { use_emoji: true, show_context: false };
        let md = format_markdown(&output, &opts);

        assert!(md.contains("**moves**"));
        assert!(md.contains("\u{2197} `process_order` (function) \u{26A0}\u{FE0F} breaking"));
        assert!(md.contains("`src/old_module.py` \u{2192} `src/new_module.py`"));
    }

    #[test]
    fn test_moves_no_emoji() {
        let mv = MoveEntry {
            entity: "handler".to_string(),
            kind: "function".to_string(),
            from_file: "a.py".to_string(),
            to_file: "b.py".to_string(),
            from_line: 1,
            to_line: 1,
            breaking: false,
            confidence: 1.0,
        };
        let output = make_output("HEAD~1", "HEAD", vec![], vec![], vec![mv], vec![]);
        let opts = MarkdownOptions { use_emoji: false, show_context: false };
        let md = format_markdown(&output, &opts);

        assert!(md.contains("=> `handler` (function)"));
        assert!(md.contains("`a.py` \u{2192} `b.py`"));
    }

    #[test]
    fn test_context_code_block_rendering() {
        let mut entity = make_entity("modified", "execute_payment", "function", true);
        entity.sig_changed = Some(true);
        entity.body_changed = Some(false);
        entity.context = Some(SnippetContext {
            base_snippet: "def execute_payment(commit: bool = True):".to_string(),
            head_snippet: "def execute_payment(commit: bool = False):".to_string(),
            language: "python".to_string(),
            snippet_kind: "signature".to_string(),
            hunks: None,
        });
        let section = make_file_section("src/payments.py", vec![entity]);
        let breaking = vec![
            BreakingEntry {
                entity: "execute_payment".to_string(),
                kind: "function".to_string(),
                file: "src/payments.py".to_string(),
                line: 1,
                reason: "sig changed".to_string(),
                external_callers: None,
                callers_in_diff: None,
            },
        ];
        let output = make_output("HEAD~1", "HEAD", vec![section], vec![], vec![], breaking);
        let opts = MarkdownOptions { use_emoji: true, show_context: true };
        let md = format_markdown(&output, &opts);

        // Should contain fenced code block
        assert!(md.contains("```python"));
        assert!(md.contains("# before"));
        assert!(md.contains("def execute_payment(commit: bool = True):"));
        assert!(md.contains("# after"));
        assert!(md.contains("def execute_payment(commit: bool = False):"));
        assert!(md.contains("```\n"));
    }

    #[test]
    fn test_context_hidden_when_disabled() {
        let mut entity = make_entity("modified", "some_fn", "function", false);
        entity.context = Some(SnippetContext {
            base_snippet: "old code".to_string(),
            head_snippet: "new code".to_string(),
            language: "rust".to_string(),
            snippet_kind: "full".to_string(),
            hunks: None,
        });
        let section = make_file_section("src/lib.rs", vec![entity]);
        let output = make_output("HEAD~1", "HEAD", vec![section], vec![], vec![], vec![]);
        let opts = MarkdownOptions { use_emoji: true, show_context: false };
        let md = format_markdown(&output, &opts);

        // Context should not appear
        assert!(!md.contains("```rust"));
        assert!(!md.contains("old code"));
    }

    #[test]
    fn test_no_breaking_header() {
        let section = make_file_section("src/lib.rs", vec![
            make_entity("added", "new_fn", "function", false),
        ]);
        let output = make_output("HEAD~1", "HEAD", vec![section], vec![], vec![], vec![]);
        let opts = MarkdownOptions { use_emoji: true, show_context: false };
        let md = format_markdown(&output, &opts);

        // Header should not mention breaking
        assert!(md.contains("**sigil diff** `HEAD~1`"));
        assert!(!md.contains("breaking"));
        assert!(!md.contains("\u{26A0}"));
    }

    #[test]
    fn test_change_detail_suffix_sig_only() {
        let mut entity = make_entity("modified", "fn_a", "function", false);
        entity.sig_changed = Some(true);
        entity.body_changed = Some(false);
        let section = make_file_section("src/lib.rs", vec![entity]);
        let output = make_output("HEAD~1", "HEAD", vec![section], vec![], vec![], vec![]);
        let opts = MarkdownOptions { use_emoji: true, show_context: false };
        let md = format_markdown(&output, &opts);

        assert!(md.contains("~ modified `fn_a` (function) \u{2014} sig"));
        assert!(!md.contains("sig+body"));
    }

    #[test]
    fn test_change_detail_suffix_body_only() {
        let mut entity = make_entity("modified", "fn_b", "function", false);
        entity.sig_changed = Some(false);
        entity.body_changed = Some(true);
        let section = make_file_section("src/lib.rs", vec![entity]);
        let output = make_output("HEAD~1", "HEAD", vec![section], vec![], vec![], vec![]);
        let opts = MarkdownOptions { use_emoji: true, show_context: false };
        let md = format_markdown(&output, &opts);

        assert!(md.contains("~ modified `fn_b` (function) \u{2014} body"));
    }

    #[test]
    fn test_ref_spec_formatting() {
        // HEAD ref is shown simply
        assert_eq!(format_ref_spec("HEAD~1", "HEAD"), "HEAD~1");
        assert_eq!(format_ref_spec("HEAD~1", ""), "HEAD~1");
        // Non-HEAD ref shows full range
        assert_eq!(format_ref_spec("main", "feature"), "main..feature");
    }

    #[test]
    fn test_glyph_values() {
        assert_eq!(glyph("breaking", true), "\u{26A0}\u{FE0F}");
        assert_eq!(glyph("breaking", false), "!");
        assert_eq!(glyph("added", true), "\u{2726}");
        assert_eq!(glyph("added", false), "+");
        assert_eq!(glyph("modified", true), "~");
        assert_eq!(glyph("modified", false), "~");
        assert_eq!(glyph("removed", true), "\u{2212}");
        assert_eq!(glyph("removed", false), "-");
        assert_eq!(glyph("pattern", true), "\u{2261}");
        assert_eq!(glyph("move", true), "\u{2197}");
        assert_eq!(glyph("move", false), "=>");
        assert_eq!(glyph("format", true), "\u{00B7}");
        assert_eq!(glyph("unknown_concept", true), "?");
    }

    #[test]
    fn test_token_changes_added_removed() {
        let mut entity = make_entity("modified", "fn_c", "function", false);
        entity.sig_changed = Some(true);
        entity.body_changed = Some(false);
        entity.token_changes = vec![
            TokenChange {
                change_type: "param_added".to_string(),
                from: String::new(),
                to: "timeout".to_string(),
            },
            TokenChange {
                change_type: "param_removed".to_string(),
                from: "retries".to_string(),
                to: String::new(),
            },
        ];
        let section = make_file_section("src/lib.rs", vec![entity]);
        let output = make_output("HEAD~1", "HEAD", vec![section], vec![], vec![], vec![]);
        let opts = MarkdownOptions { use_emoji: true, show_context: false };
        let md = format_markdown(&output, &opts);

        assert!(md.contains("+`timeout`"));
        assert!(md.contains("-`retries`"));
    }

    #[test]
    fn test_renamed_entity() {
        let mut entity = make_entity("modified", "check_card", "function", false);
        entity.old_name = Some("validate_card".to_string());
        entity.sig_changed = Some(false);
        entity.body_changed = Some(false);
        let section = make_file_section("src/lib.rs", vec![entity]);
        let output = make_output("HEAD~1", "HEAD", vec![section], vec![], vec![], vec![]);
        let opts = MarkdownOptions { use_emoji: true, show_context: false };
        let md = format_markdown(&output, &opts);

        assert!(md.contains("(was `validate_card`)"));
    }

    #[test]
    fn test_multiple_files() {
        let section1 = make_file_section("src/a.py", vec![
            make_entity("added", "fn_a", "function", false),
        ]);
        let section2 = make_file_section("src/b.py", vec![
            make_entity("removed", "fn_b", "function", false),
        ]);
        let output = make_output("HEAD~1", "HEAD", vec![section1, section2], vec![], vec![], vec![]);
        let opts = MarkdownOptions { use_emoji: true, show_context: false };
        let md = format_markdown(&output, &opts);

        assert!(md.contains("`src/a.py`  +1"));
        assert!(md.contains("`src/b.py`  -1"));
    }
}
