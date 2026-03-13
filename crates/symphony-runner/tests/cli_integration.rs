//! Integration test: CLI workflow path and exit codes (SPEC §17.7).

use std::path::PathBuf;
use std::process::Command;

fn symphony_bin() -> PathBuf {
  PathBuf::from(env!("CARGO_BIN_EXE_symphony"))
}

#[test]
fn cli_missing_workflow_path_exits_nonzero() {
  let status = Command::new(symphony_bin())
    .arg("/nonexistent/WORKFLOW.md")
    .status()
    .expect("spawn symphony");
  assert!(!status.success(), "missing workflow path should exit non-zero");
}

#[test]
fn cli_valid_workflow_path_dry_run_exits_cleanly() {
  let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../symphony-config/tests/fixtures/WORKFLOW.md");
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
  assert!(code == Some(0) || code == Some(1), "exit 0 (success) or 1 (e.g. auth) expected; got {:?}", code);
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
  let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../symphony-config/tests/fixtures/WORKFLOW.md");
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
