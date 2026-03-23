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

    assert!(output.status.success(), "sigil diff failed: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).expect("invalid utf8")
}

#[test]
fn diff_detects_changes() {
    let dir = make_diff_repo();
    let json_str = run_sigil_diff(&dir, "HEAD~1", &[]);
    let result: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    let entities = result["entities"].as_array().unwrap();
    assert!(!entities.is_empty(), "should detect entity changes");

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

    assert!(result["base_ref"].is_string());
    assert!(result["head_ref"].is_string());
    assert!(result["entities"].is_array());
    assert!(result["patterns"].is_array());
    assert!(result["summary"].is_object());

    let summary = &result["summary"];
    for field in &["added", "removed", "modified", "moved", "renamed", "formatting_only"] {
        assert!(summary[field].is_number(), "summary missing field: {}", field);
    }
    assert!(summary["has_breaking_change"].is_boolean());

    // Check entity diff fields
    for entity in result["entities"].as_array().unwrap() {
        assert!(entity["change"].is_string(), "entity missing change field");
        assert!(entity["name"].is_string(), "entity missing name field");
        assert!(entity["kind"].is_string(), "entity missing kind field");
        assert!(entity["file"].is_string(), "entity missing file field");
        assert!(entity["breaking"].is_boolean(), "entity missing breaking field");
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

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ADDED") || stdout.contains("MODIFIED") || stdout.contains("RENAMED")
        || stdout.contains("FORMATTING"), "terminal output should contain change labels: {}", stdout);

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
fn diff_modified_entities_have_change_details() {
    let dir = make_diff_repo();
    let json_str = run_sigil_diff(&dir, "HEAD~1", &[]);
    let result: serde_json::Value = serde_json::from_str(&json_str).unwrap();

    let modified: Vec<&serde_json::Value> = result["entities"].as_array().unwrap()
        .iter()
        .filter(|e| e["change"] == "modified")
        .collect();

    // At least one modified entity should have change_details
    let has_details = modified.iter().any(|e| {
        e.get("change_details").is_some_and(|d| d.is_array() && !d.as_array().unwrap().is_empty())
    });
    assert!(has_details, "modified entities should have change_details");

    std::fs::remove_dir_all(&dir).ok();
}
