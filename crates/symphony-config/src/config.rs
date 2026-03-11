//! Typed config structs and dispatch validation (SPEC §6.3, §6.4).

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::ConfigValidationError;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct TrackerConfig {
  #[validate(length(min = 1, message = "tracker.repo required"))]
  pub repo: String,

  #[validate(length(min = 1, message = "tracker.api_key required after resolution"))]
  pub api_key: String,

  pub endpoint: Option<String>,
  pub active_states: Option<Vec<String>>,
  pub terminal_states: Option<Vec<String>>,
}

impl TrackerConfig {
  pub fn endpoint_or_default(&self) -> String {
    self
      .endpoint
      .as_deref()
      .unwrap_or("https://api.github.com")
      .to_string()
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
#[derive(Debug, Clone)]
pub struct WorkspaceConfig {
  pub root: PathBuf,
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
#[derive(Debug, Clone)]
pub struct ServiceConfig {
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
      tracker: TrackerConfig {
        repo: "owner/repo".into(),
        api_key: "key".into(),
        endpoint: None,
        active_states: None,
        terminal_states: None,
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
}
