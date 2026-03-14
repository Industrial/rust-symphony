//! Integration test: CLI workflow path and exit codes (SPEC §17.7).

use std::path::PathBuf;
use std::process::Command;

fn symphony_bin() -> PathBuf {
  PathBuf::from(env!("CARGO_BIN_EXE_symphony"))
}

/// Workflow YAML missing a required key (tracker.claim_label) must cause the binary to exit non-zero.
#[test]
fn cli_workflow_missing_required_config_exits_nonzero() {
  let dir = tempfile::tempdir().expect("tempdir");
  let workflow_path = dir.path().join("WORKFLOW.md");
  let content = r#"---
tracker:
  repo: "owner/repo"
  api_key: "test-key"
  pr_open_label: "pr-open"
  pr_base_branch: "main"
runner:
  command: "echo agent"
worktree:
  root: "."
  main_repo_path: "."
---
# prompt
"#;
  std::fs::write(&workflow_path, content).expect("write fixture");
  let output = Command::new(symphony_bin())
    .arg(&workflow_path)
    .output()
    .expect("spawn symphony");
  assert!(
    !output.status.success(),
    "missing required config (claim_label) should exit non-zero; stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(
    stderr.contains("claim_label") || stderr.contains("required"),
    "stderr should mention the missing key or 'required'; stderr: {}",
    stderr
  );
}

/// Workflow with worktree but no worktree.root must cause the binary to exit non-zero.
#[test]
fn cli_workflow_missing_worktree_root_exits_nonzero() {
  let dir = tempfile::tempdir().expect("tempdir");
  let workflow_path = dir.path().join("WORKFLOW.md");
  let content = r#"---
tracker:
  repo: "owner/repo"
  api_key: "test-key"
  claim_label: "claimed"
  pr_open_label: "pr-open"
  pr_base_branch: "main"
runner:
  command: "echo agent"
worktree:
  main_repo_path: "."
---
# prompt
"#;
  std::fs::write(&workflow_path, content).expect("write fixture");
  let output = Command::new(symphony_bin())
    .arg(&workflow_path)
    .output()
    .expect("spawn symphony");
  assert!(
    !output.status.success(),
    "missing worktree.root should exit non-zero; stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(
    stderr.contains("worktree") || stderr.contains("required"),
    "stderr should mention worktree or 'required'; stderr: {}",
    stderr
  );
}

#[test]
fn cli_missing_workflow_path_exits_nonzero() {
  let status = Command::new(symphony_bin())
    .arg("/nonexistent/WORKFLOW.md")
    .status()
    .expect("spawn symphony");
  assert!(
    !status.success(),
    "missing workflow path should exit non-zero"
  );
}

#[test]
fn cli_valid_workflow_path_dry_run_exits_cleanly() {
  let fixture =
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../symphony-config/tests/fixtures/WORKFLOW.md");
  if !fixture.exists() {
    eprintln!("SKIP: fixture {:?} not found", fixture);
    return;
  }
  let output = Command::new(symphony_bin())
    .arg("--dry-run")
    .arg(&fixture)
    .env("GITHUB_TOKEN", "test-token")
    .output()
    .expect("spawn symphony");
  let code = output.status.code();
  assert!(
    code != Some(101),
    "binary should not panic (code 101); stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  assert!(
    code == Some(0) || code == Some(1),
    "exit 0 (success) or 1 (e.g. auth) expected; got {:?}",
    code
  );
}

/// Real tracker smoke: requires GITHUB_TOKEN. Run with `cargo test -p symphony-runner real_tracker_smoke -- --ignored`
/// or `cargo test -p symphony-runner --features integration real_tracker_smoke`.
#[cfg_attr(not(feature = "integration"), ignore)]
#[test]
fn real_tracker_smoke() {
  if std::env::var("GITHUB_TOKEN").is_err() {
    eprintln!("SKIP: GITHUB_TOKEN not set");
    return;
  }
  let fixture =
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../symphony-config/tests/fixtures/WORKFLOW.md");
  if !fixture.exists() {
    eprintln!("SKIP: fixture {:?} not found", fixture);
    return;
  }
  let output = Command::new(symphony_bin())
    .arg("--dry-run")
    .arg(&fixture)
    .output()
    .expect("spawn symphony");
  let code = output.status.code();
  assert!(
    code == Some(0) || code == Some(1),
    "real tracker dry-run should exit 0 (success) or 1 (e.g. auth); got {:?}; stderr: {}",
    code,
    String::from_utf8_lossy(&output.stderr)
  );
}
