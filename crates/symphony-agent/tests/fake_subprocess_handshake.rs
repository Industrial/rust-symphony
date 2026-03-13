//! Integration test: fake subprocess NDJSON handshake (SPEC §17.5).

use std::path::PathBuf;

use symphony_agent::run_agent_codex;

fn fake_agent_script_path() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fake_agent_codex.sh")
}

#[tokio::test]
async fn codex_handshake_with_fake_subprocess_completes_normally() {
  let worktree = tempfile::tempdir().expect("tempdir");
  let worktree_path = worktree.path();
  let script = fake_agent_script_path();
  assert!(script.exists(), "fake_agent_codex.sh must exist");
  let command = script.to_string_lossy().to_string();

  let outcome = run_agent_codex(
    &command,
    worktree_path,
    "test prompt",
    "owner/repo#1",
    "Test issue",
    10_000,
    5_000,
    None,
  )
  .await
  .expect("run_agent_codex");

  assert!(matches!(
    outcome.exit_reason,
    symphony_agent::AgentExitReason::Normal
  ));
}
