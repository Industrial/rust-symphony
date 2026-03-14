//! Startup cleanup: remove git worktree dirs for issues already in terminal state.

use tracing::{info, warn};

use symphony_config::ServiceConfig;
use symphony_tracker::fetch_issues_by_states;
use symphony_workspace::worktree_path;

/// Remove git worktree directories for issues that are already in a terminal state (e.g. closed).
/// Called once at startup to clean up from previous runs.
pub async fn run_startup_cleanup(config: &ServiceConfig) {
  tracing::trace!("run_startup_cleanup");
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
        let path = worktree_path(&config.worktree.root, &issue.identifier);
        if path.exists() {
          if let Err(e) = tokio::fs::remove_dir_all(&path).await {
            warn!(path = %path.display(), %e, "startup cleanup: failed to remove git worktree");
          } else {
            info!(identifier = %issue.identifier, "startup cleanup: removed terminal git worktree");
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
      fix_pr: false,
      tracker: symphony_config::TrackerConfig {
        repo: "owner/repo".into(),
        api_key: "key".into(),
        endpoint: None,
        active_states: None,
        terminal_states: Some(vec![]),
        include_labels: None,
        exclude_labels: None,
        claim_label: "symphony-claimed".into(),
        pr_open_label: "pr-open".into(),
        fix_pr_head_branch_pattern: None,
        mention_handle: None,
        pr_base_branch: "main".into(),
      },
      runner: symphony_config::RunnerConfig {
        command: "echo".into(),
        runner_type: symphony_config::RunnerType::Codex,
        sandbox: symphony_config::SandboxMode::None,
        firecracker: None,
        turn_timeout_ms: None,
        read_timeout_ms: None,
        stall_timeout_ms: None,
      },
      polling: symphony_config::PollingConfig::default(),
      worktree: symphony_config::WorktreeConfig {
        root: std::env::temp_dir().join("symphony_cleanup_test"),
        main_repo_path: std::env::temp_dir().join("symphony_cleanup_main"),
      },
      hooks: symphony_config::HooksConfig::default(),
      agent: symphony_config::AgentConfig::default(),
    };
    run_startup_cleanup(&config).await;
  }
}
