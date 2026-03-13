//! Integration test: load WORKFLOW.md from fixture and build ServiceConfig (SPEC §17.1).

use std::path::PathBuf;

use symphony_config::from_workflow_config;
use symphony_workflow::load_workflow_file;

fn fixture_path() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/WORKFLOW.md")
}

#[test]
fn load_workflow_from_fixture_parses_config_and_prompt() {
  std::env::set_var("GITHUB_TOKEN", "test-token");
  let path = fixture_path();
  assert!(path.exists(), "fixture WORKFLOW.md must exist at {:?}", path);

  let def = load_workflow_file(Some(path)).expect("load_workflow_file");
  assert!(!def.prompt_template.is_empty());
  assert!(def.prompt_template.contains("{{ issue.identifier }}"));

  let config = from_workflow_config(&def.config).expect("from_workflow_config");
  assert_eq!(config.tracker.repo, "owner/repo");
  assert_eq!(config.tracker.api_key, "test-token");
  assert_eq!(config.runner.command, "echo agent");
  assert_eq!(config.polling.interval_ms, 30_000);
  assert_eq!(config.agent.max_concurrent_agents, 2);
  assert_eq!(config.agent.max_turns, 10);
  assert!(config.worktree.root.to_string_lossy().contains(".worktrees"));
}
