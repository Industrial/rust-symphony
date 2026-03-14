//! Runtime snapshot for observability (SPEC §13.3).

use serde::Serialize;
use symphony_domain::{AgentTotals, OrchestratorState};

/// One row in the running list: issue id, identifier, turn count.
#[derive(Debug, Clone, Serialize)]
pub struct SessionRow {
  pub issue_id: String,
  pub identifier: String,
  pub turn_count: u32,
}

/// One row in the retrying list.
#[derive(Debug, Clone, Serialize)]
pub struct RetryRow {
  pub issue_id: String,
  pub due_at_ms: u64,
}

/// Snapshot of orchestrator state for status/observability.
#[derive(Debug, Clone, Serialize)]
pub struct OrchestratorSnapshot {
  pub running: Vec<SessionRow>,
  pub retrying: Vec<RetryRow>,
  pub agent_totals: AgentTotals,
  pub rate_limits: Option<serde_json::Value>,
}

/// Build a snapshot from current orchestrator state.
pub fn snapshot_from_state(state: &OrchestratorState) -> OrchestratorSnapshot {
  tracing::trace!("snapshot_from_state");
  let running: Vec<SessionRow> = state
    .running
    .iter()
    .map(|(id, e)| SessionRow {
      issue_id: id.clone(),
      identifier: e.identifier.clone(),
      turn_count: e.session.turn_count,
    })
    .collect();
  let retrying: Vec<RetryRow> = state
    .retry_attempts
    .iter()
    .map(|(id, e)| RetryRow {
      issue_id: id.clone(),
      due_at_ms: e.due_at_ms,
    })
    .collect();
  OrchestratorSnapshot {
    running,
    retrying,
    agent_totals: state.agent_totals.clone(),
    rate_limits: state.agent_rate_limits.clone(),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::{HashMap, HashSet};

  #[test]
  fn snapshot_from_state_empty() {
    let state = OrchestratorState {
      poll_interval_ms: 5000,
      max_concurrent_agents: 10,
      running: HashMap::new(),
      claimed: HashSet::new(),
      retry_attempts: HashMap::new(),
      completed: HashSet::new(),
      agent_totals: AgentTotals::default(),
      agent_rate_limits: None,
    };
    let snap = snapshot_from_state(&state);
    assert!(snap.running.is_empty());
    assert!(snap.retrying.is_empty());
    assert_eq!(snap.agent_totals.input_tokens, 0);
  }
}
