//! Startup cleanup: remove workspace dirs for issues already in terminal state.

use tracing::{info, warn};

use symphony_config::ServiceConfig;
use symphony_tracker::fetch_issues_by_states;
use symphony_workspace::workspace_path;

/// Remove workspace directories for issues that are already in a terminal state (e.g. closed).
/// Called once at startup to clean up from previous runs.
pub async fn run_startup_cleanup(config: &ServiceConfig) {
  let terminal = config.tracker.terminal_states_slice();
  if terminal.is_empty() {
    return;
  }
  let endpoint = config.tracker.endpoint_or_default();
  match fetch_issues_by_states(
    &endpoint,
    &config.tracker.api_key,
    &config.tracker.repo,
    terminal,
  )
  .await
  {
    Ok(issues) => {
      for issue in issues {
        let path = workspace_path(&config.workspace.root, &issue.identifier);
        if path.exists() {
          if let Err(e) = tokio::fs::remove_dir_all(&path).await {
            warn!(path = %path.display(), %e, "startup cleanup: failed to remove workspace");
          } else {
            info!(identifier = %issue.identifier, "startup cleanup: removed terminal workspace");
          }
        }
      }
    }
    Err(e) => warn!(%e, "startup cleanup: fetch terminal issues failed, continuing"),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn run_startup_cleanup_empty_terminal_does_not_fetch() {
    let config = ServiceConfig {
      tracker: symphony_config::TrackerConfig {
        repo: "owner/repo".into(),
        api_key: "key".into(),
        endpoint: None,
        active_states: None,
        terminal_states: Some(vec![]),
        include_labels: None,
        exclude_labels: None,
        claim_label: None,
        pr_open_label: None,
      },
      runner: symphony_config::RunnerConfig {
        command: "echo".into(),
        runner_type: symphony_config::RunnerType::Codex,
        turn_timeout_ms: None,
        read_timeout_ms: None,
        stall_timeout_ms: None,
      },
      polling: symphony_config::PollingConfig::default(),
      workspace: symphony_config::WorkspaceConfig {
        root: std::env::temp_dir().join("symphony_cleanup_test"),
      },
      hooks: symphony_config::HooksConfig::default(),
      agent: symphony_config::AgentConfig::default(),
    };
    run_startup_cleanup(&config).await;
  }
}
