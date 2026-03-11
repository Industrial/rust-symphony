//! Issue claim state derived from orchestrator state (SPEC §7.1).

use symphony_domain::OrchestratorState;

/// Claim state for an issue: Unclaimed, Claimed, Running, RetryQueued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimState {
  /// Issue not in running, retry_attempts, or claimed.
  Unclaimed,
  /// Issue in claimed set (in running or retry_attempts).
  Claimed,
  /// Issue in running map.
  Running,
  /// Issue in retry_attempts (and in claimed).
  RetryQueued,
  /// Claim removed (released).
  Released,
}

/// Derive claim state for an issue from orchestrator state.
/// No separate enum in state: derive from running, claimed, retry_attempts.
pub fn claim_state(issue_id: &str, state: &OrchestratorState) -> ClaimState {
  let in_running = state.running.contains_key(issue_id);
  let in_retry = state.retry_attempts.contains_key(issue_id);
  let in_claimed = state.claimed.contains(issue_id);

  if in_running {
    ClaimState::Running
  } else if in_retry {
    ClaimState::RetryQueued
  } else if in_claimed {
    ClaimState::Claimed
  } else {
    ClaimState::Unclaimed
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::{HashMap, HashSet};
  use symphony_domain::OrchestratorState;

  fn empty_state() -> OrchestratorState {
    OrchestratorState {
      poll_interval_ms: 5000,
      max_concurrent_agents: 10,
      running: HashMap::new(),
      claimed: HashSet::new(),
      retry_attempts: HashMap::new(),
      completed: HashSet::new(),
      agent_totals: Default::default(),
      agent_rate_limits: None,
    }
  }

  #[test]
  fn claim_state_unclaimed() {
    let state = empty_state();
    assert_eq!(claim_state("issue-1", &state), ClaimState::Unclaimed);
  }

  #[test]
  fn claim_state_claimed_only() {
    let mut state = empty_state();
    state.claimed.insert("issue-1".into());
    assert_eq!(claim_state("issue-1", &state), ClaimState::Claimed);
  }

  #[test]
  fn claim_state_retry_queued() {
    let mut state = empty_state();
    state.claimed.insert("issue-1".into());
    state.retry_attempts.insert(
      "issue-1".into(),
      symphony_domain::RetryEntry {
        issue_id: "issue-1".into(),
        identifier: "repo#1".into(),
        attempt: 1,
        due_at_ms: 0,
        error: None,
      },
    );
    assert_eq!(claim_state("issue-1", &state), ClaimState::RetryQueued);
  }

  #[test]
  fn claim_state_running() {
    let mut state = empty_state();
    state.claimed.insert("issue-1".into());
    state.running.insert(
      "issue-1".into(),
      symphony_domain::RunningEntry {
        identifier: "repo#1".into(),
        issue: symphony_domain::Issue {
          id: "issue-1".into(),
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
        },
        session: Default::default(),
        started_at: chrono::Utc::now(),
        retry_attempt: 0,
      },
    );
    assert_eq!(claim_state("issue-1", &state), ClaimState::Running);
  }
}
