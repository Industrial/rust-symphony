//! Typed config structs and dispatch validation (SPEC §6.3, §6.4).

use std::collections::HashMap;
use std::path::PathBuf;

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::ConfigValidationError;

/// Default active issue states when not specified in config.
static DEFAULT_ACTIVE_STATES: Lazy<Vec<String>> = Lazy::new(|| vec!["open".to_string()]);
/// Default terminal issue states when not specified in config.
static DEFAULT_TERMINAL_STATES: Lazy<Vec<String>> = Lazy::new(|| vec!["closed".to_string()]);

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct TrackerConfig {
  #[validate(length(min = 1, message = "tracker.repo required"))]
  pub repo: String,

  #[validate(length(min = 1, message = "tracker.api_key required after resolution"))]
  pub api_key: String,

  pub endpoint: Option<String>,
  pub active_states: Option<Vec<String>>,
  pub terminal_states: Option<Vec<String>>,

  /// SPEC_ADDENDUM_1 A.1.1: candidate must have at least one of these labels (if non-empty).
  pub include_labels: Option<Vec<String>>,
  /// SPEC_ADDENDUM_1 A.1.2: candidate must have none of these labels (if non-empty).
  pub exclude_labels: Option<Vec<String>>,
  /// SPEC_ADDENDUM_1 A.2.1: label the agent adds when claiming; included in effective exclude so claimed issues are not re-dispatched.
  pub claim_label: Option<String>,
  /// SPEC_ADDENDUM_1 A.3.6: optional label the agent may add when a PR is open; should be in exclude_labels if used.
  pub pr_open_label: Option<String>,
  /// SPEC_ADDENDUM_2 B.2: branch name pattern for issue→PR resolution; "{number}" is replaced by issue number (e.g. "symphony/issue-{number}").
  /// When None and fix_pr is used, the default is "symphony/issue-{number}".
  pub fix_pr_head_branch_pattern: Option<String>,
  /// SPEC_ADDENDUM_2 B.5: handle to look for in comments (e.g. "symphony" → @symphony). If set, qualifying mention triggers dispatch.
  pub mention_handle: Option<String>,
  /// Base branch for worker branches and PR target (e.g. main, develop). When unset, default is "main".
  pub pr_base_branch: Option<String>,
}

impl TrackerConfig {
  pub fn endpoint_or_default(&self) -> String {
    self
      .endpoint
      .as_deref()
      .unwrap_or("https://api.github.com")
      .to_string()
  }

  /// Exclude labels to use when fetching candidates: exclude_labels plus claim_label if set and not already present (SPEC_ADDENDUM_1 A.2.1).
  pub fn effective_exclude_labels(&self) -> Option<Vec<String>> {
    let mut base = self.exclude_labels.clone().unwrap_or_default();
    if let Some(ref claim) = self.claim_label {
      if !base.iter().any(|l| l.eq_ignore_ascii_case(claim)) {
        base.push(claim.clone());
      }
    }
    if base.is_empty() { None } else { Some(base) }
  }

  /// Active issue states for candidate fetch; defaults to `["open"]` if not set.
  pub fn active_states_slice(&self) -> &[String] {
    self
      .active_states
      .as_deref()
      .unwrap_or_else(|| DEFAULT_ACTIVE_STATES.as_slice())
  }

  /// Terminal issue states for reconciliation/cleanup; defaults to `["closed"]` if not set.
  pub fn terminal_states_slice(&self) -> &[String] {
    self
      .terminal_states
      .as_deref()
      .unwrap_or_else(|| DEFAULT_TERMINAL_STATES.as_slice())
  }

  /// Base branch for worker branches and PR target; defaults to "main" when not configured.
  pub fn effective_pr_base_branch(&self) -> &str {
    self.pr_base_branch.as_deref().unwrap_or("main")
  }
}

/// Runner protocol: Codex-style (thread/start, turn/start), ACP (Cursor: agent acp), or CLI (Cursor: prompt as arg, stream-json).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RunnerType {
  #[default]
  Codex,
  Acp,
  /// Cursor non-interactive: run `command "$SYMPHONY_PROMPT"`, parse NDJSON stdout until type=result, subtype=success.
  Cli,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RunnerConfig {
  #[validate(length(min = 1, message = "runner.command required"))]
  pub command: String,

  /// Protocol the agent speaks: "codex" (default) or "acp" (Cursor ACP).
  #[serde(rename = "type", default)]
  pub runner_type: RunnerType,

  pub turn_timeout_ms: Option<u64>,
  pub read_timeout_ms: Option<u64>,
  pub stall_timeout_ms: Option<u64>,
}

impl RunnerConfig {
  pub fn turn_timeout_ms(&self) -> u64 {
    self.turn_timeout_ms.unwrap_or(3_600_000)
  }
  pub fn read_timeout_ms(&self) -> u64 {
    self.read_timeout_ms.unwrap_or(5_000)
  }
  pub fn stall_timeout_ms(&self) -> u64 {
    self.stall_timeout_ms.unwrap_or(300_000)
  }
}

/// Polling config (SPEC §6.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollingConfig {
  pub interval_ms: u64,
}

impl Default for PollingConfig {
  fn default() -> Self {
    Self {
      interval_ms: 30_000,
    }
  }
}

/// Workspace config (SPEC §6.4). Root is resolved and absolute.
/// When main_repo_path is set, per-issue workspaces are created as git worktrees (SPEC_ADDENDUM_1 A.3.1).
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
  pub root: PathBuf,
  /// Path to the main git repo (worktree or clone). When set, ensure_worktree is used so each issue gets a worktree.
  pub main_repo_path: Option<PathBuf>,
}

/// Hooks config (SPEC §6.4).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HooksConfig {
  pub after_create: Option<String>,
  pub before_run: Option<String>,
  pub after_run: Option<String>,
  pub before_remove: Option<String>,
  pub timeout_ms: u64,
}

impl HooksConfig {
  pub fn timeout_ms(&self) -> u64 {
    if self.timeout_ms == 0 {
      60_000
    } else {
      self.timeout_ms
    }
  }
}

/// Agent config (SPEC §6.4). Keys in max_concurrent_agents_by_state are normalized lowercase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
  pub max_concurrent_agents: u32,
  pub max_turns: u32,
  pub max_retry_backoff_ms: u64,
  pub max_concurrent_agents_by_state: HashMap<String, u32>,
}

impl Default for AgentConfig {
  fn default() -> Self {
    Self {
      max_concurrent_agents: 10,
      max_turns: 20,
      max_retry_backoff_ms: 300_000,
      max_concurrent_agents_by_state: HashMap::new(),
    }
  }
}

/// Resolved and validated config for dispatch preflight (SPEC §6.3).
/// SPEC_ADDENDUM_2 B.1: fix_pr gates fix-PR logic (re-dispatch on failing checks or mention); default false.
#[derive(Debug, Clone)]
pub struct ServiceConfig {
  /// When true, orchestrator applies fix-PR logic for issues with pr_open_label. When false or omitted, no fix-PR polling.
  pub fix_pr: bool,
  pub tracker: TrackerConfig,
  pub runner: RunnerConfig,
  pub polling: PollingConfig,
  pub workspace: WorkspaceConfig,
  pub hooks: HooksConfig,
  pub agent: AgentConfig,
}

impl ServiceConfig {
  /// Run before startup and before each dispatch cycle.
  pub fn validate_dispatch(&self) -> Result<(), ConfigValidationError> {
    self
      .tracker
      .validate()
      .map_err(ConfigValidationError::Tracker)?;
    self
      .runner
      .validate()
      .map_err(ConfigValidationError::Runner)?;
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn minimal_service_config() -> ServiceConfig {
    ServiceConfig {
      fix_pr: false,
      tracker: TrackerConfig {
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
        pr_base_branch: None,
      },
      runner: RunnerConfig {
        command: "codex app-server".into(),
        runner_type: RunnerType::Codex,
        turn_timeout_ms: None,
        read_timeout_ms: None,
        stall_timeout_ms: None,
      },
      polling: PollingConfig::default(),
      workspace: WorkspaceConfig {
        root: std::env::temp_dir().join("symphony_workspaces"),
        main_repo_path: None,
      },
      hooks: HooksConfig::default(),
      agent: AgentConfig::default(),
    }
  }

  #[test]
  fn validate_dispatch_passes() {
    let s = minimal_service_config();
    assert!(s.validate_dispatch().is_ok());
  }

  #[test]
  fn validate_dispatch_fails_empty_repo() {
    let mut s = minimal_service_config();
    s.tracker.repo = String::new();
    assert!(s.validate_dispatch().is_err());
  }

  #[test]
  fn runner_timeout_getters() {
    let r = RunnerConfig {
      command: "c".into(),
      runner_type: RunnerType::Codex,
      turn_timeout_ms: Some(100),
      read_timeout_ms: None,
      stall_timeout_ms: Some(200),
    };
    assert_eq!(r.turn_timeout_ms(), 100);
    assert_eq!(r.read_timeout_ms(), 5_000);
    assert_eq!(r.stall_timeout_ms(), 200);
  }

  #[test]
  fn polling_config_default() {
    let p = PollingConfig::default();
    assert_eq!(p.interval_ms, 30_000);
  }

  #[test]
  fn hooks_config_timeout_default() {
    let h = HooksConfig::default();
    assert_eq!(h.timeout_ms(), 60_000);
  }

  #[test]
  fn hooks_config_timeout_explicit() {
    let h = HooksConfig {
      timeout_ms: 90_000,
      ..Default::default()
    };
    assert_eq!(h.timeout_ms(), 90_000);
  }

  #[test]
  fn agent_config_default() {
    let a = AgentConfig::default();
    assert_eq!(a.max_concurrent_agents, 10);
    assert_eq!(a.max_turns, 20);
    assert_eq!(a.max_retry_backoff_ms, 300_000);
    assert!(a.max_concurrent_agents_by_state.is_empty());
  }

  #[test]
  fn effective_exclude_labels_none_when_both_empty() {
    let t = TrackerConfig {
      repo: "r".into(),
      api_key: "k".into(),
      endpoint: None,
      active_states: None,
      terminal_states: None,
      include_labels: None,
      exclude_labels: None,
      claim_label: None,
      pr_open_label: None,
      fix_pr_head_branch_pattern: None,
      mention_handle: None,
      pr_base_branch: None,
    };
    assert!(t.effective_exclude_labels().is_none());
  }

  #[test]
  fn effective_exclude_labels_exclude_only() {
    let t = TrackerConfig {
      repo: "r".into(),
      api_key: "k".into(),
      endpoint: None,
      active_states: None,
      terminal_states: None,
      include_labels: None,
      exclude_labels: Some(vec!["a".into(), "b".into()]),
      claim_label: None,
      pr_open_label: None,
      fix_pr_head_branch_pattern: None,
      mention_handle: None,
      pr_base_branch: None,
    };
    assert_eq!(
      t.effective_exclude_labels(),
      Some(vec!["a".into(), "b".into()])
    );
  }

  #[test]
  fn effective_exclude_labels_claim_merged() {
    let t = TrackerConfig {
      repo: "r".into(),
      api_key: "k".into(),
      endpoint: None,
      active_states: None,
      terminal_states: None,
      include_labels: None,
      exclude_labels: Some(vec!["a".into()]),
      claim_label: Some("symphony-claimed".into()),
      pr_open_label: None,
      fix_pr_head_branch_pattern: None,
      mention_handle: None,
      pr_base_branch: None,
    };
    let eff = t.effective_exclude_labels().unwrap();
    assert_eq!(eff.len(), 2);
    assert!(eff.contains(&"a".to_string()));
    assert!(eff.contains(&"symphony-claimed".to_string()));
  }

  #[test]
  fn effective_exclude_labels_claim_only() {
    let t = TrackerConfig {
      repo: "r".into(),
      api_key: "k".into(),
      endpoint: None,
      active_states: None,
      terminal_states: None,
      include_labels: None,
      exclude_labels: None,
      claim_label: Some("claimed".into()),
      pr_open_label: None,
      fix_pr_head_branch_pattern: None,
      mention_handle: None,
      pr_base_branch: None,
    };
    assert_eq!(t.effective_exclude_labels(), Some(vec!["claimed".into()]));
  }

  #[test]
  fn effective_exclude_labels_claim_already_in_exclude_no_duplicate() {
    let t = TrackerConfig {
      repo: "r".into(),
      api_key: "k".into(),
      endpoint: None,
      active_states: None,
      terminal_states: None,
      include_labels: None,
      exclude_labels: Some(vec!["a".into(), "claimed".into()]),
      claim_label: Some("claimed".into()),
      pr_open_label: None,
      fix_pr_head_branch_pattern: None,
      mention_handle: None,
      pr_base_branch: None,
    };
    let eff = t.effective_exclude_labels().unwrap();
    assert_eq!(eff.len(), 2);
    assert_eq!(eff, vec!["a", "claimed"]);
  }

  #[test]
  fn active_states_slice_default() {
    let t = TrackerConfig {
      repo: "r".into(),
      api_key: "k".into(),
      endpoint: None,
      active_states: None,
      terminal_states: None,
      include_labels: None,
      exclude_labels: None,
      claim_label: None,
      pr_open_label: None,
      fix_pr_head_branch_pattern: None,
      mention_handle: None,
      pr_base_branch: None,
    };
    assert_eq!(t.active_states_slice(), &["open".to_string()]);
  }

  #[test]
  fn active_states_slice_explicit() {
    let t = TrackerConfig {
      repo: "r".into(),
      api_key: "k".into(),
      endpoint: None,
      active_states: Some(vec!["open".to_string(), "in_progress".to_string()]),
      terminal_states: None,
      include_labels: None,
      exclude_labels: None,
      claim_label: None,
      pr_open_label: None,
      fix_pr_head_branch_pattern: None,
      mention_handle: None,
      pr_base_branch: None,
    };
    assert_eq!(
      t.active_states_slice(),
      &["open".to_string(), "in_progress".to_string()]
    );
  }

  #[test]
  fn terminal_states_slice_default() {
    let t = TrackerConfig {
      repo: "r".into(),
      api_key: "k".into(),
      endpoint: None,
      active_states: None,
      terminal_states: None,
      include_labels: None,
      exclude_labels: None,
      claim_label: None,
      pr_open_label: None,
      fix_pr_head_branch_pattern: None,
      mention_handle: None,
      pr_base_branch: None,
    };
    assert_eq!(t.terminal_states_slice(), &["closed".to_string()]);
  }

  #[test]
  fn terminal_states_slice_explicit() {
    let t = TrackerConfig {
      repo: "r".into(),
      api_key: "k".into(),
      endpoint: None,
      active_states: None,
      terminal_states: Some(vec!["closed".to_string(), "done".to_string()]),
      include_labels: None,
      exclude_labels: None,
      claim_label: None,
      pr_open_label: None,
      fix_pr_head_branch_pattern: None,
      mention_handle: None,
      pr_base_branch: None,
    };
    assert_eq!(
      t.terminal_states_slice(),
      &["closed".to_string(), "done".to_string()]
    );
  }

  #[test]
  fn effective_pr_base_branch_default() {
    let t = TrackerConfig {
      repo: "r".into(),
      api_key: "k".into(),
      endpoint: None,
      active_states: None,
      terminal_states: None,
      include_labels: None,
      exclude_labels: None,
      claim_label: None,
      pr_open_label: None,
      fix_pr_head_branch_pattern: None,
      mention_handle: None,
      pr_base_branch: None,
    };
    assert_eq!(t.effective_pr_base_branch(), "main");
  }

  #[test]
  fn effective_pr_base_branch_explicit() {
    let t = TrackerConfig {
      repo: "r".into(),
      api_key: "k".into(),
      endpoint: None,
      active_states: None,
      terminal_states: None,
      include_labels: None,
      exclude_labels: None,
      claim_label: None,
      pr_open_label: None,
      fix_pr_head_branch_pattern: None,
      mention_handle: None,
      pr_base_branch: Some("develop".into()),
    };
    assert_eq!(t.effective_pr_base_branch(), "develop");
  }
}
