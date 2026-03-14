//! Build ServiceConfig from workflow config JSON (SPEC §6.1, §6.4).

use std::collections::HashMap;

use serde::Deserialize;

use crate::ConfigError;
use crate::config::{
  AgentConfig, FirecrackerSandboxConfig, HooksConfig, PollingConfig, RunnerConfig, RunnerType,
  SandboxMode, ServiceConfig, TrackerConfig, WorktreeConfig,
};
use crate::resolve::{resolve_var, resolve_worktree_root};

/// Raw tracker map from workflow front matter (before env resolution).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct RawTracker {
  repo: Option<String>,
  api_key: Option<String>,
  endpoint: Option<String>,
  active_states: Option<Vec<String>>,
  terminal_states: Option<Vec<String>>,
  include_labels: Option<Vec<String>>,
  exclude_labels: Option<Vec<String>>,
  claim_label: Option<String>,
  pr_open_label: Option<String>,
  fix_pr_head_branch_pattern: Option<String>,
  mention_handle: Option<String>,
  pr_base_branch: Option<String>,
}

/// Raw runner map from workflow front matter.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct RawRunner {
  command: Option<String>,
  #[serde(rename = "type")]
  runner_type: Option<String>,
  sandbox: Option<String>,
  firecracker: Option<RawFirecrackerSandbox>,
  turn_timeout_ms: Option<u64>,
  read_timeout_ms: Option<u64>,
  stall_timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct RawFirecrackerSandbox {
  kernel_path: Option<String>,
  rootfs_path: Option<String>,
  worktree_guest_path: Option<String>,
  vsock_port: Option<u32>,
}

/// Raw polling map.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct RawPolling {
  interval_ms: Option<u64>,
}

/// Raw worktree map (root supports $VAR and ~). main_repo_path: when set, create per-issue git worktrees.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct RawWorktree {
  root: Option<String>,
  main_repo_path: Option<String>,
}

/// Raw hooks map.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
struct RawHooks {
  after_create: Option<String>,
  before_run: Option<String>,
  after_run: Option<String>,
  before_remove: Option<String>,
  timeout_ms: Option<u64>,
}

/// Raw agent map. max_concurrent_agents_by_state keys normalized to lowercase.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
struct RawAgent {
  max_concurrent_agents: Option<u32>,
  max_turns: Option<u32>,
  max_retry_backoff_ms: Option<u64>,
  max_concurrent_agents_by_state: Option<HashMap<String, u32>>,
}

/// Raw workflow config root. SPEC_ADDENDUM_2 B.1: fix_pr is top-level, default false.
#[derive(Debug, Deserialize)]
struct RawConfig {
  fix_pr: Option<bool>,
  tracker: Option<RawTracker>,
  runner: Option<RawRunner>,
  polling: Option<RawPolling>,
  #[serde(alias = "workspace")]
  worktree: Option<RawWorktree>,
  hooks: Option<RawHooks>,
  agent: Option<RawAgent>,
}

/// Build ServiceConfig from workflow front matter (e.g. `WorkflowDefinition.config`).
/// Applies env resolution to `tracker.api_key` and `worktree.root`, then validates.
pub fn from_workflow_config(value: &serde_json::Value) -> Result<ServiceConfig, ConfigError> {
  tracing::trace!("from_workflow_config");
  let raw: RawConfig =
    serde_json::from_value(value.clone()).map_err(|e| ConfigError::Deserialize(e.to_string()))?;

  let tracker = raw
    .tracker
    .ok_or_else(|| ConfigError::MissingKey("tracker".to_string()))?;
  let repo = tracker
    .repo
    .map(|s| s.trim().to_string())
    .unwrap_or_default();
  let api_key_raw = tracker.api_key.unwrap_or_default();
  let api_key = resolve_var(&api_key_raw).trim().to_string();

  let claim_label = tracker
    .claim_label
    .as_ref()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
    .ok_or_else(|| ConfigError::InvalidConfig("tracker.claim_label is required".into()))?;
  let pr_open_label = tracker
    .pr_open_label
    .as_ref()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
    .ok_or_else(|| ConfigError::InvalidConfig("tracker.pr_open_label is required".into()))?;
  let pr_base_branch = tracker
    .pr_base_branch
    .as_ref()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())
    .ok_or_else(|| ConfigError::InvalidConfig("tracker.pr_base_branch is required".into()))?;

  let tracker_config = TrackerConfig {
    repo,
    api_key,
    endpoint: tracker.endpoint,
    active_states: tracker
      .active_states
      .or_else(|| Some(vec!["open".to_string()])),
    terminal_states: tracker
      .terminal_states
      .or_else(|| Some(vec!["closed".to_string()])),
    include_labels: tracker.include_labels,
    exclude_labels: tracker.exclude_labels,
    claim_label,
    pr_open_label,
    fix_pr_head_branch_pattern: tracker.fix_pr_head_branch_pattern,
    mention_handle: tracker.mention_handle,
    pr_base_branch,
  };

  let runner_raw = raw
    .runner
    .ok_or_else(|| ConfigError::MissingKey("runner".to_string()))?;
  let command = runner_raw
    .command
    .map(|s| resolve_var(&s).trim().to_string())
    .unwrap_or_default();
  let runner_type = match runner_raw.runner_type.as_deref() {
    Some("acp") => RunnerType::Acp,
    Some("cli") => RunnerType::Cli,
    _ => RunnerType::Codex,
  };

  let sandbox = match runner_raw.sandbox.as_deref() {
    Some("firecracker") => SandboxMode::Firecracker,
    _ => SandboxMode::None,
  };

  let firecracker = runner_raw.firecracker.and_then(|raw| {
    let kernel_path = raw
      .kernel_path
      .and_then(|s| resolve_worktree_root(s.trim()).ok())?;
    let rootfs_path = raw
      .rootfs_path
      .and_then(|s| resolve_worktree_root(s.trim()).ok())?;
    Some(FirecrackerSandboxConfig {
      kernel_path,
      rootfs_path,
      worktree_guest_path: raw
        .worktree_guest_path
        .map(|s| std::path::PathBuf::from(s.trim()))
        .unwrap_or_else(|| std::path::Path::new("/worktree").to_path_buf()),
      vsock_port: raw.vsock_port.unwrap_or(5000),
    })
  });

  let runner_config = RunnerConfig {
    command,
    runner_type,
    sandbox,
    firecracker,
    turn_timeout_ms: runner_raw.turn_timeout_ms.or(Some(3_600_000)),
    read_timeout_ms: runner_raw.read_timeout_ms.or(Some(5_000)),
    stall_timeout_ms: runner_raw.stall_timeout_ms.or(Some(300_000)),
  };

  if runner_config.sandbox == SandboxMode::Firecracker && runner_config.firecracker.is_none() {
    return Err(ConfigError::InvalidConfig(
      "runner.sandbox is firecracker but runner.firecracker (kernel_path, rootfs_path) is missing or invalid"
        .into(),
    ));
  }

  let polling_config = raw
    .polling
    .map(|p| PollingConfig {
      interval_ms: p.interval_ms.unwrap_or(30_000),
    })
    .unwrap_or_default();

  let worktree_root_raw = raw
    .worktree
    .as_ref()
    .and_then(|w| w.root.as_ref())
    .map(|s| s.trim())
    .filter(|s| !s.is_empty());
  let worktree_root = match worktree_root_raw {
    Some(s) => resolve_worktree_root(s)
      .map_err(|e| ConfigError::InvalidConfig(format!("worktree.root resolution failed: {}", e)))?,
    None => {
      return Err(ConfigError::InvalidConfig(
        "worktree.root is required".into(),
      ));
    }
  };
  let main_repo_path_raw = raw
    .worktree
    .as_ref()
    .and_then(|w| w.main_repo_path.as_ref())
    .map(|s| s.trim())
    .filter(|s| !s.is_empty());
  let main_repo_path = match main_repo_path_raw {
    Some(s) => resolve_worktree_root(s).map_err(|e| {
      ConfigError::InvalidConfig(format!("worktree.main_repo_path resolution failed: {}", e))
    })?,
    None => {
      return Err(ConfigError::InvalidConfig(
        "worktree.main_repo_path is required for worker development (git worktree and branch). \
         Set worktree.main_repo_path to the path of the main git repository."
          .into(),
      ));
    }
  };
  let worktree_config = WorktreeConfig {
    root: worktree_root,
    main_repo_path,
  };

  let hooks_raw = raw.hooks.unwrap_or_default();
  let hooks_config = HooksConfig {
    after_create: hooks_raw.after_create,
    before_run: hooks_raw.before_run,
    after_run: hooks_raw.after_run,
    before_remove: hooks_raw.before_remove,
    timeout_ms: hooks_raw.timeout_ms.unwrap_or(60_000),
  };

  let agent_raw = raw.agent.unwrap_or_default();
  let max_concurrent_agents_by_state = agent_raw
    .max_concurrent_agents_by_state
    .unwrap_or_default()
    .into_iter()
    .map(|(k, v)| (k.to_lowercase(), v))
    .collect();
  let agent_config = AgentConfig {
    max_concurrent_agents: agent_raw.max_concurrent_agents.unwrap_or(10),
    max_turns: agent_raw.max_turns.unwrap_or(20),
    max_retry_backoff_ms: agent_raw.max_retry_backoff_ms.unwrap_or(300_000),
    max_concurrent_agents_by_state,
  };

  let fix_pr = raw.fix_pr.unwrap_or(false);

  let service = ServiceConfig {
    fix_pr,
    tracker: tracker_config,
    runner: runner_config,
    polling: polling_config,
    worktree: worktree_config,
    hooks: hooks_config,
    agent: agent_config,
  };
  service
    .validate_dispatch()
    .map_err(ConfigError::Validation)?;
  Ok(service)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn from_workflow_config_success() {
    let value = serde_json::json!({
        "tracker": { "repo": "owner/repo", "api_key": "test-key", "claim_label": "symphony-claimed", "pr_open_label": "pr-open", "pr_base_branch": "main" },
        "runner": { "command": "codex app-server" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let config = from_workflow_config(&value).unwrap();
    assert_eq!(config.tracker.repo, "owner/repo");
    assert_eq!(config.tracker.api_key, "test-key");
    assert_eq!(config.runner.command, "codex app-server");
    assert_eq!(config.runner.runner_type, RunnerType::Codex);
  }

  #[test]
  fn from_workflow_config_runner_type_acp() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "claimed", "pr_open_label": "pr-open", "pr_base_branch": "main" },
        "runner": { "command": "agent acp", "type": "acp" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let config = from_workflow_config(&value).unwrap();
    assert_eq!(config.runner.runner_type, RunnerType::Acp);
    assert_eq!(config.runner.command, "agent acp");
  }

  #[test]
  fn from_workflow_config_runner_type_cli() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "claimed", "pr_open_label": "pr-open", "pr_base_branch": "main" },
        "runner": { "command": "cursor-agent -p --output-format stream-json", "type": "cli" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let config = from_workflow_config(&value).unwrap();
    assert_eq!(config.runner.runner_type, RunnerType::Cli);
  }

  #[test]
  fn from_workflow_config_missing_tracker() {
    let value = serde_json::json!({ "runner": { "command": "cmd" } });
    let r = from_workflow_config(&value);
    assert!(matches!(r, Err(ConfigError::MissingKey(_))));
  }

  #[test]
  fn from_workflow_config_empty_api_key_fails_validation() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "", "claim_label": "claimed", "pr_open_label": "pr-open", "pr_base_branch": "main" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let r = from_workflow_config(&value);
    assert!(matches!(r, Err(ConfigError::Validation(_))));
  }

  #[test]
  fn from_workflow_config_resolves_api_key_var() {
    std::env::set_var("TEST_GH_KEY", "resolved-secret");
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "$TEST_GH_KEY", "claim_label": "claimed", "pr_open_label": "pr-open", "pr_base_branch": "main" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let config = from_workflow_config(&value).unwrap();
    std::env::remove_var("TEST_GH_KEY");
    assert_eq!(config.tracker.api_key, "resolved-secret");
  }

  #[test]
  fn from_workflow_config_polling_defaults() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "claimed", "pr_open_label": "pr-open", "pr_base_branch": "main" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let config = from_workflow_config(&value).unwrap();
    assert_eq!(config.polling.interval_ms, 30_000);
  }

  #[test]
  fn from_workflow_config_polling_explicit() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "claimed", "pr_open_label": "pr-open", "pr_base_branch": "main" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." },
        "polling": { "interval_ms": 60_000 }
    });
    let config = from_workflow_config(&value).unwrap();
    assert_eq!(config.polling.interval_ms, 60_000);
  }

  #[test]
  fn from_workflow_config_worktree_root_required() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "c", "pr_open_label": "p", "pr_base_branch": "main" },
        "runner": { "command": "c" },
        "worktree": { "main_repo_path": "." }
    });
    let r = from_workflow_config(&value);
    assert!(
      matches!(r, Err(ConfigError::InvalidConfig(ref s)) if s.contains("worktree.root")),
      "expected InvalidConfig when worktree.root omitted, got {:?}",
      r
    );
  }

  #[test]
  fn from_workflow_config_worktree_root_and_main_repo_path() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "c", "pr_open_label": "p", "pr_base_branch": "main" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let config = from_workflow_config(&value).unwrap();
    assert!(config.worktree.root.is_absolute());
    assert!(!config.worktree.main_repo_path.as_os_str().is_empty());
  }

  #[test]
  fn from_workflow_config_worktree_root_resolved() {
    std::env::set_var("SYMPHONY_WS", "my_ws_dir");
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "c", "pr_open_label": "p", "pr_base_branch": "main" },
        "runner": { "command": "c" },
        "worktree": { "root": "$SYMPHONY_WS", "main_repo_path": "." }
    });
    let config = from_workflow_config(&value).unwrap();
    std::env::remove_var("SYMPHONY_WS");
    assert!(config.worktree.root.is_absolute());
    assert!(config.worktree.root.ends_with("my_ws_dir"));
  }

  #[test]
  fn from_workflow_config_hooks() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "c", "pr_open_label": "p", "pr_base_branch": "main" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." },
        "hooks": {
            "after_create": "echo created",
            "timeout_ms": 90_000
        }
    });
    let config = from_workflow_config(&value).unwrap();
    assert_eq!(config.hooks.after_create.as_deref(), Some("echo created"));
    assert_eq!(config.hooks.timeout_ms(), 90_000);
  }

  #[test]
  fn from_workflow_config_agent_defaults() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "c", "pr_open_label": "p", "pr_base_branch": "main" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let config = from_workflow_config(&value).unwrap();
    assert_eq!(config.agent.max_concurrent_agents, 10);
    assert_eq!(config.agent.max_turns, 20);
    assert_eq!(config.agent.max_retry_backoff_ms, 300_000);
    assert!(config.agent.max_concurrent_agents_by_state.is_empty());
  }

  #[test]
  fn from_workflow_config_agent_and_state_cap_normalized() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "c", "pr_open_label": "p", "pr_base_branch": "main" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." },
        "agent": {
            "max_concurrent_agents": 5,
            "max_turns": 30,
            "max_retry_backoff_ms": 120_000,
            "max_concurrent_agents_by_state": { "Open": 2, "In Progress": 3 }
        }
    });
    let config = from_workflow_config(&value).unwrap();
    assert_eq!(config.agent.max_concurrent_agents, 5);
    assert_eq!(config.agent.max_turns, 30);
    assert_eq!(config.agent.max_retry_backoff_ms, 120_000);
    assert_eq!(
      config.agent.max_concurrent_agents_by_state.get("open"),
      Some(&2u32)
    );
    assert_eq!(
      config
        .agent
        .max_concurrent_agents_by_state
        .get("in progress"),
      Some(&3u32)
    );
  }

  #[test]
  fn from_workflow_config_tracker_include_exclude_labels_omitted() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "c", "pr_open_label": "p", "pr_base_branch": "main" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let config = from_workflow_config(&value).unwrap();
    assert!(config.tracker.include_labels.is_none());
    assert!(config.tracker.exclude_labels.is_none());
  }

  #[test]
  fn from_workflow_config_tracker_include_exclude_labels_parsed() {
    let value = serde_json::json!({
        "tracker": {
            "repo": "r",
            "api_key": "k",
            "claim_label": "c",
            "pr_open_label": "p",
            "pr_base_branch": "main",
            "include_labels": ["symphony", "bot"],
            "exclude_labels": ["symphony-claimed", "wip"]
        },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let config = from_workflow_config(&value).unwrap();
    assert_eq!(
      config.tracker.include_labels,
      Some(vec!["symphony".to_string(), "bot".to_string()])
    );
    assert_eq!(
      config.tracker.exclude_labels,
      Some(vec!["symphony-claimed".to_string(), "wip".to_string()])
    );
  }

  #[test]
  fn from_workflow_config_tracker_empty_label_arrays() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "c", "pr_open_label": "p", "pr_base_branch": "main", "include_labels": [], "exclude_labels": [] },
        "worktree": { "root": ".", "main_repo_path": "." },
        "runner": { "command": "c" }
    });
    let config = from_workflow_config(&value).unwrap();
    assert_eq!(config.tracker.include_labels, Some(vec![]));
    assert_eq!(config.tracker.exclude_labels, Some(vec![]));
  }

  #[test]
  fn from_workflow_config_tracker_claim_label_missing_fails() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "pr_open_label": "p", "pr_base_branch": "main" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let r = from_workflow_config(&value);
    assert!(
      matches!(r, Err(ConfigError::InvalidConfig(ref s)) if s.contains("claim_label")),
      "expected InvalidConfig when claim_label omitted, got {:?}",
      r
    );
  }

  #[test]
  fn from_workflow_config_tracker_claim_label_parsed() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "symphony-claimed", "pr_open_label": "p", "pr_base_branch": "main" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let config = from_workflow_config(&value).unwrap();
    assert_eq!(config.tracker.claim_label, "symphony-claimed");
  }

  #[test]
  fn from_workflow_config_tracker_pr_open_label_missing_fails() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "c", "pr_base_branch": "main" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let r = from_workflow_config(&value);
    assert!(
      matches!(r, Err(ConfigError::InvalidConfig(ref s)) if s.contains("pr_open_label")),
      "expected InvalidConfig when pr_open_label omitted, got {:?}",
      r
    );
  }

  #[test]
  fn from_workflow_config_tracker_pr_open_label_parsed() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "c", "pr_open_label": "pr-open", "pr_base_branch": "main" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let config = from_workflow_config(&value).unwrap();
    assert_eq!(config.tracker.pr_open_label, "pr-open");
  }

  #[test]
  fn from_workflow_config_tracker_fix_pr_head_branch_pattern_omitted() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "c", "pr_open_label": "p", "pr_base_branch": "main" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let config = from_workflow_config(&value).unwrap();
    assert!(config.tracker.fix_pr_head_branch_pattern.is_none());
  }

  #[test]
  fn from_workflow_config_tracker_fix_pr_head_branch_pattern_parsed() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "c", "pr_open_label": "p", "pr_base_branch": "main", "fix_pr_head_branch_pattern": "agent/issue-{number}" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let config = from_workflow_config(&value).unwrap();
    assert_eq!(
      config.tracker.fix_pr_head_branch_pattern.as_deref(),
      Some("agent/issue-{number}")
    );
  }

  #[test]
  fn from_workflow_config_tracker_mention_handle_parsed() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "c", "pr_open_label": "p", "pr_base_branch": "main", "mention_handle": "symphony" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let config = from_workflow_config(&value).unwrap();
    assert_eq!(config.tracker.mention_handle.as_deref(), Some("symphony"));
  }

  #[test]
  fn from_workflow_config_tracker_pr_base_branch_missing_fails() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "c", "pr_open_label": "p" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let r = from_workflow_config(&value);
    assert!(
      matches!(r, Err(ConfigError::InvalidConfig(ref s)) if s.contains("pr_base_branch")),
      "expected InvalidConfig when pr_base_branch omitted, got {:?}",
      r
    );
  }

  #[test]
  fn from_workflow_config_tracker_pr_base_branch_parsed() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "c", "pr_open_label": "p", "pr_base_branch": "develop" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let config = from_workflow_config(&value).unwrap();
    assert_eq!(config.tracker.pr_base_branch, "develop");
    assert_eq!(config.tracker.effective_pr_base_branch(), "develop");
  }

  #[test]
  fn from_workflow_config_worktree_main_repo_path_required() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "c", "pr_open_label": "p", "pr_base_branch": "main" },
        "runner": { "command": "c" },
        "worktree": { "root": "/tmp/ws" }
    });
    let r = from_workflow_config(&value);
    assert!(
      matches!(r, Err(ConfigError::InvalidConfig(ref s)) if s.contains("main_repo_path")),
      "expected InvalidConfig when main_repo_path omitted, got {:?}",
      r
    );
  }

  #[test]
  fn from_workflow_config_worktree_main_repo_path_parsed() {
    let value = serde_json::json!({
        "tracker": { "repo": "r", "api_key": "k", "claim_label": "c", "pr_open_label": "p", "pr_base_branch": "main" },
        "runner": { "command": "c" },
        "worktree": { "root": ".", "main_repo_path": "." }
    });
    let config = from_workflow_config(&value).unwrap();
    assert!(!config.worktree.main_repo_path.as_os_str().is_empty());
  }
}
