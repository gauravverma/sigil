use std::process::Command;
use std::path::PathBuf;

fn run_git(dir: &std::path::Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git failed");
    assert!(output.status.success(), "git {:?} failed: {}", args, String::from_utf8_lossy(&output.stderr));
}

/// Create a temp git repo, commit an initial version, modify, commit again.
fn make_diff_repo() -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "sigil_diff_test_{}_{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    run_git(&dir, &["init"]);
    run_git(&dir, &["config", "user.email", "test@test.com"]);
    run_git(&dir, &["config", "user.name", "Test"]);

    // Initial commit: two functions
    std::fs::write(dir.join("app.py"), "\
def process_payment(order, card):
    if not validate(card):
        return False
    return charge(order, card)

def calculate_total(items):
    return sum(i.price for i in items)
").unwrap();
    run_git(&dir, &["add", "."]);
    run_git(&dir, &["commit", "-m", "initial"]);

    // Second commit: rename function, modify body, reformat one, add new
    std::fs::write(dir.join("app.py"), "\
def execute_payment(order, card, key=None):
    if not validate(card):
        return False
    return charge_with_retry(order, card, key)

def calculate_total(items):
    return sum(
        i.price for i in items
    )

def audit_log(event):
    print(event)
").unwrap();
    run_git(&dir, &["add", "."]);
    run_git(&dir, &["commit", "-m", "refactor payments"]);

    dir
}

fn run_sigil_diff(dir: &std::path::Path, ref_spec: &str, extra_args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_sigil"))
        .arg("diff")
        .arg(ref_spec)
        .arg("--root")
        .arg(dir)
        .arg("--json")
        .args(extra_args)
        .output()
        .expect("failed to run sigil");

    assert!(
        output.status.success(),
        "sigil diff failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("invalid utf8")
}

/// Run sigil diff with arbitrary flags (does NOT force --json).
fn run_sigil_diff_raw(dir: &std::path::Path, ref_spec: &str, args: &[&str]) -> std::process::Output {
    let output = Command::new(env!("CARGO_BIN_EXE_sigil"))
        .arg("diff")
        .arg(ref_spec)
        .arg("--root")
        .arg(dir)
        .args(args)
        .output()
        .expect("failed to run sigil");

    assert!(
        output.status.success(),
        "sigil diff failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

#[test]
fn diff_detects_changes() {
    let dir = make_diff_repo();
    let json_str = run_sigil_diff(&dir, "HEAD~1", &[]);
    let result: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    // Entities are now nested inside files[]
    let files = result["files"].as_array().unwrap();
    assert!(!files.is_empty(), "should detect file changes");

    let all_entities: Vec<&serde_json::Value> = files.iter()
        .flat_map(|f| f["entities"].as_array().unwrap())
        .collect();
    assert!(!all_entities.is_empty(), "should detect entity changes");

    // Check summary has at least 1 added (audit_log)
    let summary = &result["summary"];
    assert!(summary["added"].as_u64().unwrap() >= 1, "should have at least 1 added");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn diff_json_has_required_fields() {
    let dir = make_diff_repo();
    let json_str = run_sigil_diff(&dir, "HEAD~1", &[]);
    let result: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    // V2 output shape: meta, summary, breaking, patterns, moves, files
    assert!(result["meta"].is_object(), "missing meta");
    assert!(result["meta"]["base_ref"].is_string(), "missing meta.base_ref");
    assert!(result["meta"]["head_ref"].is_string(), "missing meta.head_ref");
    assert!(result["summary"].is_object(), "missing summary");
    assert!(result["patterns"].is_array(), "missing patterns");
    assert!(result["files"].is_array(), "missing files");

    let summary = &result["summary"];
    for field in &["added", "removed", "modified", "moves", "renamed", "formatting_only"] {
        assert!(summary[field].is_number(), "summary missing field: {}", field);
    }
    assert!(summary["has_breaking"].is_boolean(), "summary missing has_breaking");

    // Check entity fields inside files
    for file_section in result["files"].as_array().unwrap() {
        assert!(file_section["file"].is_string(), "file section missing file field");
        assert!(file_section["entities"].is_array(), "file section missing entities");
        for entity in file_section["entities"].as_array().unwrap() {
            assert!(entity["change"].is_string(), "entity missing change field");
            assert!(entity["name"].is_string(), "entity missing name field");
            assert!(entity["kind"].is_string(), "entity missing kind field");
            assert!(entity["breaking"].is_boolean(), "entity missing breaking field");
        }
    }

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn diff_terminal_output_works() {
    let dir = make_diff_repo();
    let output = Command::new(env!("CARGO_BIN_EXE_sigil"))
        .arg("diff")
        .arg("HEAD~1")
        .arg("--root")
        .arg(&dir)
        .output()
        .expect("failed to run sigil");

    // Exit code 1 means structural changes detected (expected for this test)
    assert!(
        output.status.code().unwrap_or(3) < 3,
        "sigil diff failed with error: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // V2 terminal format uses glyphs like +, ~, - instead of ADDED/MODIFIED labels
    assert!(!stdout.is_empty(), "terminal output should not be empty: {}", stdout);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn diff_deterministic() {
    let dir = make_diff_repo();
    let out1 = run_sigil_diff(&dir, "HEAD~1", &[]);
    let out2 = run_sigil_diff(&dir, "HEAD~1", &[]);
    assert_eq!(out1, out2, "diff output must be deterministic");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn diff_modified_entities_have_token_changes() {
    let dir = make_diff_repo();
    let json_str = run_sigil_diff(&dir, "HEAD~1", &[]);
    let result: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    let all_entities: Vec<&serde_json::Value> = result["files"].as_array().unwrap()
        .iter()
        .flat_map(|f| f["entities"].as_array().unwrap())
        .collect();

    let modified: Vec<&&serde_json::Value> = all_entities.iter()
        .filter(|e| e["change"] == "modified")
        .collect();

    // At least one modified entity should have token_changes
    let has_changes = modified.iter().any(|e| {
        e.get("token_changes").is_some_and(|d| d.is_array() && !d.as_array().unwrap().is_empty())
    });
    assert!(has_changes, "modified entities should have token_changes");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn exit_code_0_for_no_changes() {
    let dir = make_diff_repo();
    let output = Command::new(env!("CARGO_BIN_EXE_sigil"))
        .args(["diff", "HEAD..HEAD", "--root"])
        .arg(&dir)
        .output()
        .expect("failed to run sigil");
    assert!(output.status.success(), "HEAD..HEAD should exit 0");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn exit_code_0_for_structural_changes() {
    let dir = make_diff_repo();
    let output = Command::new(env!("CARGO_BIN_EXE_sigil"))
        .args(["diff", "HEAD~1", "--root"])
        .arg(&dir)
        .output()
        .expect("failed to run sigil");
    assert!(output.status.success(), "diff with changes should still exit 0");
    std::fs::remove_dir_all(&dir).ok();
}

// ── New output mode integration tests ──────────────────────────────────────

#[test]
fn json_v2_schema_has_required_keys() {
    let dir = make_diff_repo();
    let output = run_sigil_diff_raw(&dir, "HEAD~1", &["--json"]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert!(v["meta"].is_object(), "missing meta");
    assert!(v["meta"]["base_ref"].is_string(), "missing meta.base_ref");
    assert!(v["meta"]["head_ref"].is_string(), "missing meta.head_ref");
    assert!(v["meta"]["sigil_version"].is_string(), "missing meta.sigil_version");
    assert!(v["summary"].is_object(), "missing summary");
    assert!(v["summary"]["has_breaking"].is_boolean(), "missing summary.has_breaking");
    assert!(v["summary"]["files_changed"].is_number(), "missing summary.files_changed");
    assert!(v["breaking"].is_array(), "missing breaking");
    assert!(v["patterns"].is_array(), "missing patterns");
    assert!(v["moves"].is_array(), "missing moves");
    assert!(v["files"].is_array(), "missing files");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn markdown_output_has_markdown_elements() {
    let dir = make_diff_repo();
    let output = run_sigil_diff_raw(&dir, "HEAD~1", &["--markdown"]);
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Should contain markdown separator
    assert!(stdout.contains("---"), "markdown output should contain --- separator");
    // Should contain backtick-wrapped paths (file names)
    assert!(stdout.contains("`"), "markdown output should contain backtick-wrapped identifiers");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn lines_flag_shows_line_numbers() {
    let dir = make_diff_repo();
    let output = run_sigil_diff_raw(&dir, "HEAD~1", &["--lines"]);
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Line numbers appear as :N after entity names
    assert!(stdout.contains(":"), "expected line numbers in output with --lines");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn no_color_flag_removes_ansi() {
    let dir = make_diff_repo();
    let output = run_sigil_diff_raw(&dir, "HEAD~1", &["--no-color"]);
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(
        !stdout.contains("\x1b["),
        "output should not contain ANSI escape codes with --no-color"
    );

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn json_with_context_default() {
    let dir = make_diff_repo();
    // Context is now on by default (3 lines)
    let output = run_sigil_diff_raw(&dir, "HEAD~1", &["--json"]);
    let stdout = String::from_utf8(output.stdout).unwrap();
    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert!(v["files"].is_array(), "JSON output should have files array");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn markdown_no_emoji_combined() {
    let dir = make_diff_repo();
    let output = run_sigil_diff_raw(&dir, "HEAD~1", &["--markdown", "--no-emoji"]);
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Should still contain markdown structure
    assert!(stdout.contains("---"), "markdown output should contain --- separator");
    // With --no-emoji, should use ASCII glyphs like +, -, ! instead of emoji
    // Verify no warning-sign emoji (U+26A0) is present
    assert!(!stdout.contains('\u{26A0}'), "should not contain warning emoji with --no-emoji");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn json_diff_with_derived_and_arrays() {
    let dir = std::env::temp_dir().join(format!(
        "sigil_json_diff_test_{}_{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    run_git(&dir, &["init"]);
    run_git(&dir, &["config", "user.email", "test@test.com"]);
    run_git(&dir, &["config", "user.name", "Test"]);

    // Initial commit
    let old_json = r#"{
  "body": {
    "text": "Hello world",
    "_parsed_text": "Hello {{1}}"
  },
  "header": {
    "text": "",
    "_parsed_text": ""
  },
  "buttons": [
    {
      "text": "Click",
      "type": "URL"
    }
  ]
}"#;
    std::fs::write(dir.join("template.json"), old_json).unwrap();
    run_git(&dir, &["add", "."]);
    run_git(&dir, &["commit", "-m", "initial"]);

    // Second commit: modify body.text only
    let new_json = r#"{
  "body": {
    "text": "Hello universe",
    "_parsed_text": "Hello {{1}} universe"
  },
  "header": {
    "text": "",
    "_parsed_text": ""
  },
  "buttons": [
    {
      "text": "Click",
      "type": "URL"
    }
  ]
}"#;
    std::fs::write(dir.join("template.json"), new_json).unwrap();
    run_git(&dir, &["add", "."]);
    run_git(&dir, &["commit", "-m", "update body text"]);

    // Run sigil diff --json
    let bin = env!("CARGO_BIN_EXE_sigil");
    let output = Command::new(bin)
        .args(&["diff", "HEAD~1", "--json"])
        .current_dir(&dir)
        .output()
        .expect("sigil failed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("invalid JSON output: {}\n{}", e, stdout));

    // Check entities — should NOT contain _parsed_text (derived)
    let entities = json["files"].as_array().unwrap()
        .iter()
        .flat_map(|f| f["entities"].as_array().unwrap())
        .collect::<Vec<_>>();

    let names: Vec<&str> = entities.iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();

    // body.text should show as modified (qualified name, parent suppressed)
    assert!(names.contains(&"body.text"), "body.text should be in diff output, got: {:?}", names);
    // body parent should be suppressed (children carry the detail)
    assert!(!names.contains(&"body"), "body parent should be suppressed, got: {:?}", names);
    // _parsed_text should NOT appear (derived)
    assert!(!names.contains(&"_parsed_text"), "_parsed_text should be suppressed, got: {:?}", names);

    // Cleanup
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_color_lines_combined() {
    let dir = make_diff_repo();
    let output = run_sigil_diff_raw(&dir, "HEAD~1", &["--no-color", "--lines"]);
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(
        !stdout.contains("\x1b["),
        "output should not contain ANSI escape codes with --no-color --lines"
    );

    std::fs::remove_dir_all(&dir).ok();
}
