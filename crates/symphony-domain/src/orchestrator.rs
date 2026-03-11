//! Orchestrator runtime state (SPEC §4.1.8).

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::issue::Issue;
use crate::retry::RetryEntry;
use crate::session::LiveSession;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningEntry {
  pub identifier: String,
  pub issue: Issue,
  pub session: LiveSession,
  pub started_at: chrono::DateTime<chrono::Utc>,
  pub retry_attempt: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentTotals {
  pub input_tokens: u64,
  pub output_tokens: u64,
  pub total_tokens: u64,
  pub seconds_running: f64,
}

#[derive(Debug, Clone, Default)]
pub struct OrchestratorState {
  pub poll_interval_ms: u64,
  pub max_concurrent_agents: u32,
  pub running: HashMap<String, RunningEntry>,
  pub claimed: HashSet<String>,
  pub retry_attempts: HashMap<String, RetryEntry>,
  pub completed: HashSet<String>,
  pub agent_totals: AgentTotals,
  pub agent_rate_limits: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::issue::Issue;

  fn sample_issue() -> Issue {
    Issue {
      id: "id".into(),
      identifier: "repo#1".into(),
      title: "T".into(),
      description: None,
      priority: None,
      state: "open".into(),
      branch_name: None,
      url: None,
      labels: vec![],
      blocked_by: vec![],
      created_at: None,
      updated_at: None,
    }
  }

  #[test]
  fn agent_totals_default() {
    let t = AgentTotals::default();
    assert_eq!(t.input_tokens, 0);
    assert_eq!(t.seconds_running, 0.0);
  }

  #[test]
  fn orchestrator_state_default() {
    let s = OrchestratorState::default();
    assert!(s.running.is_empty());
    assert!(s.claimed.is_empty());
    assert!(s.retry_attempts.is_empty());
  }

  #[test]
  fn running_entry_creation() {
    let e = RunningEntry {
      identifier: "repo#1".into(),
      issue: sample_issue(),
      session: LiveSession::default(),
      started_at: chrono::Utc::now(),
      retry_attempt: 0,
    };
    assert_eq!(e.identifier, "repo#1");
    assert_eq!(e.issue.id, "id");
  }
}
