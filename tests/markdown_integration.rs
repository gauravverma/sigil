use std::process::Command;

fn run_sigil_index(fixture_dir: &str, extra_args: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_sigil"))
        .arg("index")
        .arg("--root")
        .arg(fixture_dir)
        .arg("--stdout")
        .arg("--full")
        .args(extra_args)
        .output()
        .expect("failed to run sigil");

    assert!(output.status.success(), "sigil failed: {}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).expect("invalid utf8")
}

fn fixture_path() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    format!("{}/tests/fixtures", manifest)
}

#[test]
fn indexes_markdown_fixture() {
    let output = run_sigil_index(&fixture_path(), &["--files", &format!("{}/sample.md", fixture_path())]);
    let entities: Vec<serde_json::Value> = output.lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();

    // Should find frontmatter, sections, code blocks, tables, lists, blockquotes, paragraphs
    assert!(entities.len() >= 10, "expected at least 10 entities, got {}", entities.len());

    let kinds: Vec<&str> = entities.iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert!(kinds.contains(&"frontmatter"), "missing frontmatter");
    assert!(kinds.contains(&"section"), "missing section");
    assert!(kinds.contains(&"code_block"), "missing code_block");
    assert!(kinds.contains(&"table"), "missing table");
    assert!(kinds.contains(&"list"), "missing list");
    assert!(kinds.contains(&"blockquote"), "missing blockquote");
    assert!(kinds.contains(&"paragraph"), "missing paragraph");

    // Verify heading hierarchy
    let installation = entities.iter().find(|e| e["name"].as_str() == Some("Installation")).unwrap();
    assert!(installation["parent"].is_null(), "top-level heading should have no parent");

    let prereqs = entities.iter().find(|e| e["name"].as_str() == Some("Prerequisites")).unwrap();
    assert_eq!(prereqs["parent"].as_str(), Some("Installation"));
}

#[test]
fn diff_markdown_files() {
    let output = Command::new(env!("CARGO_BIN_EXE_sigil"))
        .arg("diff")
        .arg("--files")
        .arg(format!("{}/sample.md", fixture_path()))
        .arg(format!("{}/sample_v2.md", fixture_path()))
        .arg("--json")
        .output()
        .expect("failed to run sigil diff");

    assert!(output.status.success(), "sigil diff failed: {}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("invalid utf8");
    let result: serde_json::Value = serde_json::from_str(&stdout).expect("invalid json");

    // Should have file-level diffs with entities
    let files = result["files"].as_array().expect("missing files array");
    assert!(!files.is_empty(), "diff should produce file-level changes");
    let entities = files[0]["entities"].as_array().expect("missing entities in file");
    assert!(!entities.is_empty(), "diff should produce entity-level changes");
}
