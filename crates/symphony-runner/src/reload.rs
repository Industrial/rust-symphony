//! Workflow file reload: watch WORKFLOW.md mtime and reload config when it changes.

use std::path::PathBuf;

use tokio::sync::RwLock;
use tracing::{info, warn};

use symphony_config::{ServiceConfig, from_workflow_config};
use symphony_workflow::{load_workflow, resolve_workflow_path};

/// Spawn a task that periodically checks WORKFLOW.md mtime and reloads config + prompt into `workflow_state`.
/// Keeps last good config on parse/validation error.
pub fn spawn_workflow_reload_task(
  workflow_state: std::sync::Arc<RwLock<(ServiceConfig, String)>>,
  workflow_path_arg: Option<PathBuf>,
  poll_secs: u64,
) -> tokio::task::JoinHandle<()> {
  tokio::spawn(async move {
    let mut last_mtime = None::<std::time::SystemTime>;
    loop {
      tokio::time::sleep(tokio::time::Duration::from_secs(poll_secs)).await;
      let path = match resolve_workflow_path(workflow_path_arg.clone()) {
        Ok(p) => p,
        Err(_) => continue,
      };
      let meta = match tokio::fs::metadata(&path).await {
        Ok(m) => m,
        Err(_) => continue,
      };
      let modified = match meta.modified() {
        Ok(t) => t,
        Err(_) => continue,
      };
      if Some(modified) != last_mtime {
        match std::fs::read_to_string(&path) {
          Ok(content) => match load_workflow(&content) {
            Ok(def) => match from_workflow_config(&def.config) {
              Ok(cfg) => {
                if cfg.validate_dispatch().is_ok() {
                  *workflow_state.write().await = (cfg, def.prompt_template);
                  last_mtime = Some(modified);
                  info!("workflow reloaded");
                } else {
                  warn!("workflow reload: validation failed, keeping previous");
                }
              }
              Err(e) => warn!(%e, "workflow reload: config failed, keeping previous"),
            },
            Err(e) => warn!(%e, "workflow reload: parse failed, keeping previous"),
          },
          Err(e) => warn!(%e, "workflow reload: read failed"),
        }
      }
    }
  })
}

#[cfg(test)]
mod tests {
  use std::path::PathBuf;
  use std::sync::Arc;

  use tokio::sync::RwLock;

  use symphony_config::ServiceConfig;

  use super::spawn_workflow_reload_task;

  fn minimal_config() -> ServiceConfig {
    ServiceConfig {
      fix_pr: false,
      tracker: symphony_config::TrackerConfig {
        repo: "owner/repo".into(),
        api_key: "key".into(),
        endpoint: None,
        active_states: None,
        terminal_states: None,
        include_labels: None,
        exclude_labels: None,
        claim_label: None,
        pr_open_label: None,
        fix_pr_head_branch_pattern: None,
        mention_handle: None,
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
        root: std::env::temp_dir().join("symphony_reload_test"),
      },
      hooks: symphony_config::HooksConfig::default(),
      agent: symphony_config::AgentConfig::default(),
    }
  }

  #[tokio::test]
  async fn spawn_workflow_reload_task_returns_handle() {
    let workflow_state = Arc::new(RwLock::new((minimal_config(), String::new())));
    let handle = spawn_workflow_reload_task(workflow_state, None::<PathBuf>, 60);
    handle.abort();
    let _ = handle.await;
  }
}
