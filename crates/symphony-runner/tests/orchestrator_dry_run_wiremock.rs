//! Integration test: dry_run_one_poll with wiremock (SPEC §17.4 — fetch, sort, no workers).

use symphony_config::{
  AgentConfig, HooksConfig, PollingConfig, RunnerConfig, RunnerType, ServiceConfig, TrackerConfig,
  WorktreeConfig,
};
use symphony_runner::dry_run_one_poll;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn issue_json(number: u64, title: &str) -> serde_json::Value {
  serde_json::json!({
    "node_id": format!("N_{}", number),
    "number": number,
    "title": title,
    "state": "open",
    "body": null,
    "html_url": format!("https://github.com/owner/repo/issues/{}", number),
    "labels": [],
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:00:00Z"
  })
}

fn service_config_with_endpoint(endpoint: &str) -> ServiceConfig {
  ServiceConfig {
    fix_pr: false,
    tracker: TrackerConfig {
      repo: "owner/repo".to_string(),
      api_key: "test-token".to_string(),
      endpoint: Some(endpoint.to_string()),
      active_states: Some(vec!["open".to_string()]),
      terminal_states: Some(vec!["closed".to_string()]),
      include_labels: None,
      exclude_labels: None,
      claim_label: None,
      pr_open_label: None,
      fix_pr_head_branch_pattern: None,
      mention_handle: None,
      pr_base_branch: None,
    },
    runner: RunnerConfig {
      command: "echo agent".to_string(),
      runner_type: RunnerType::Codex,
      turn_timeout_ms: None,
      read_timeout_ms: None,
      stall_timeout_ms: None,
    },
    polling: PollingConfig::default(),
    worktree: WorktreeConfig {
      root: std::env::temp_dir().join("symphony_worktrees_test"),
      main_repo_path: None,
    },
    hooks: HooksConfig::default(),
    agent: AgentConfig::default(),
  }
}

#[tokio::test]
async fn dry_run_one_poll_fetches_and_sorts_via_wiremock() {
  let mock = MockServer::start().await;
  let body = serde_json::json!([issue_json(1, "First"), issue_json(2, "Second")]);

  Mock::given(method("GET"))
    .and(path("/repos/owner/repo/issues"))
    .respond_with(ResponseTemplate::new(200).set_body_json(&body))
    .mount(&mock)
    .await;

  let config = service_config_with_endpoint(mock.uri().as_str());
  config.validate_dispatch().expect("validate_dispatch");

  let result = dry_run_one_poll(&config).await;
  result.expect("dry_run_one_poll");
}
