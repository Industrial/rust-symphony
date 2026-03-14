//! State transition helpers for orchestrator messages (SPEC §7.3, §7.4).
//!
//! Pure in-place updates to OrchestratorState. The orchestrator task calls these
//! when handling OrchestratorMessage variants.

use chrono::Utc;
use symphony_domain::{OrchestratorState, RetryEntry};

use crate::messages::{AgentUpdatePayload, WorkerExitReason};
use crate::scheduling::retry_delay_ms;

/// Number of slots available for new dispatch (SPEC §7.4).
pub fn available_slots(state: &OrchestratorState, max_concurrent: u32) -> u32 {
  tracing::trace!("available_slots");
  let running = state.running.len() as u32;
  max_concurrent.saturating_sub(running)
}

/// Pre-dispatch check: issue not claimed, not running, and at least one slot (SPEC §7.4).
pub fn can_dispatch(state: &OrchestratorState, issue_id: &str, max_concurrent: u32) -> bool {
  tracing::trace!("can_dispatch");
  !state.claimed.contains(issue_id)
    && !state.running.contains_key(issue_id)
    && available_slots(state, max_concurrent) > 0
}

/// Apply WorkerExit: remove from running, add to agent_totals, schedule retry or keep claimed in sync.
///
/// Uses `now_ms` (monotonic or wall clock) for `due_at_ms`. Replaces any existing retry entry for this issue.
pub fn apply_worker_exit(
  state: &mut OrchestratorState,
  issue_id: String,
  reason: WorkerExitReason,
  runtime_seconds: f64,
  token_totals: (u64, u64, u64),
  now_ms: u64,
  max_retry_backoff_ms: u64,
) {
  tracing::trace!("apply_worker_exit");
  let entry = match state.running.remove(&issue_id) {
    Some(e) => e,
    None => return,
  };

  state.agent_totals.seconds_running += runtime_seconds;
  state.agent_totals.input_tokens += token_totals.0;
  state.agent_totals.output_tokens += token_totals.1;
  state.agent_totals.total_tokens += token_totals.2;

  let continuation = matches!(reason, WorkerExitReason::Normal);
  let current_attempt = state
    .retry_attempts
    .get(&issue_id)
    .map(|r| r.attempt)
    .unwrap_or(0);
  let next_attempt = current_attempt + 1;
  let delay_ms = retry_delay_ms(next_attempt, max_retry_backoff_ms, continuation);
  let error_msg = match &reason {
    WorkerExitReason::Normal => None,
    WorkerExitReason::Failed(s) => Some(s.clone()),
    WorkerExitReason::TimedOut => Some("timed_out".to_string()),
    WorkerExitReason::Stalled => Some("stalled".to_string()),
    WorkerExitReason::CanceledByReconciliation => Some("canceled_by_reconciliation".to_string()),
  };

  state.retry_attempts.insert(
    issue_id.clone(),
    RetryEntry {
      issue_id: issue_id.clone(),
      identifier: entry.identifier,
      attempt: next_attempt,
      due_at_ms: now_ms + delay_ms,
      error: error_msg,
    },
  );
  state.claimed.insert(issue_id);
}

/// Apply AgentUpdate: update the running entry's LiveSession and optional rate_limits (SPEC §7.3).
pub fn apply_agent_update(
  state: &mut OrchestratorState,
  issue_id: &str,
  update: AgentUpdatePayload,
) {
  tracing::trace!("apply_agent_update");
  let Some(entry) = state.running.get_mut(issue_id) else {
    return;
  };

  if let Some(s) = update.session_id {
    entry.session.session_id = Some(s);
  }
  if let Some(s) = update.thread_id {
    entry.session.thread_id = Some(s);
  }
  if let Some(s) = update.turn_id {
    entry.session.turn_id = Some(s);
  }
  if let Some(n) = update.input_tokens {
    entry.session.agent_input_tokens = n;
    entry.session.last_reported_input_tokens = n;
  }
  if let Some(n) = update.output_tokens {
    entry.session.agent_output_tokens = n;
    entry.session.last_reported_output_tokens = n;
  }
  if let Some(n) = update.total_tokens {
    entry.session.agent_total_tokens = n;
    entry.session.last_reported_total_tokens = n;
  }
  if let Some(n) = update.turn_count {
    entry.session.turn_count = n;
  }
  entry.session.last_agent_timestamp = Some(Utc::now());
}

/// Release an issue: remove from claimed and retry_attempts (and optionally from running).
/// Used when reconciliation determines the issue is terminal or no longer eligible.
pub fn release_claim(state: &mut OrchestratorState, issue_id: &str) {
  tracing::trace!("release_claim");
  state.claimed.remove(issue_id);
  state.retry_attempts.remove(issue_id);
}

/// Remove a retry entry when dispatching the issue (claim stays; issue moves to running elsewhere).
pub fn remove_retry_on_dispatch(state: &mut OrchestratorState, issue_id: &str) {
  tracing::trace!("remove_retry_on_dispatch");
  state.retry_attempts.remove(issue_id);
}

#[cfg(test)]
mod tests {
  use std::collections::{HashMap, HashSet};

  use symphony_domain::{Issue, LiveSession, OrchestratorState, RetryEntry, RunningEntry};

  use super::*;
  use crate::messages::WorkerExitReason;

  fn sample_issue(id: &str, identifier: &str) -> Issue {
    Issue {
      id: id.to_string(),
      identifier: identifier.to_string(),
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

  fn empty_state(max_concurrent: u32) -> OrchestratorState {
    OrchestratorState {
      poll_interval_ms: 5000,
      max_concurrent_agents: max_concurrent,
      running: HashMap::new(),
      claimed: HashSet::new(),
      retry_attempts: HashMap::new(),
      completed: HashSet::new(),
      agent_totals: Default::default(),
      agent_rate_limits: None,
    }
  }

  #[test]
  fn available_slots_empty() {
    let state = empty_state(10);
    assert_eq!(available_slots(&state, 10), 10);
  }

  #[test]
  fn available_slots_partial() {
    let mut state = empty_state(10);
    state.running.insert(
      "i1".into(),
      RunningEntry {
        identifier: "r#1".into(),
        issue: sample_issue("i1", "r#1"),
        session: LiveSession::default(),
        started_at: chrono::Utc::now(),
        retry_attempt: 0,
      },
    );
    assert_eq!(available_slots(&state, 10), 9);
  }

  #[test]
  fn can_dispatch_true_when_empty() {
    let state = empty_state(10);
    assert!(can_dispatch(&state, "i1", 10));
  }

  #[test]
  fn can_dispatch_false_when_claimed() {
    let mut state = empty_state(10);
    state.claimed.insert("i1".into());
    assert!(!can_dispatch(&state, "i1", 10));
  }

  #[test]
  fn can_dispatch_false_when_running() {
    let mut state = empty_state(10);
    state.running.insert(
      "i1".into(),
      RunningEntry {
        identifier: "r#1".into(),
        issue: sample_issue("i1", "r#1"),
        session: LiveSession::default(),
        started_at: chrono::Utc::now(),
        retry_attempt: 0,
      },
    );
    assert!(!can_dispatch(&state, "i1", 10));
  }

  #[test]
  fn apply_worker_exit_removes_from_running_adds_retry() {
    let mut state = empty_state(10);
    state.claimed.insert("i1".into());
    state.running.insert(
      "i1".into(),
      RunningEntry {
        identifier: "owner/repo#42".into(),
        issue: sample_issue("i1", "owner/repo#42"),
        session: LiveSession::default(),
        started_at: chrono::Utc::now(),
        retry_attempt: 0,
      },
    );

    apply_worker_exit(
      &mut state,
      "i1".to_string(),
      WorkerExitReason::Normal,
      2.5,
      (100, 200, 300),
      1_000_000,
      300_000,
    );

    assert!(!state.running.contains_key("i1"));
    assert_eq!(state.agent_totals.seconds_running, 2.5);
    assert_eq!(state.agent_totals.input_tokens, 100);
    assert_eq!(state.agent_totals.total_tokens, 300);
    let retry = state.retry_attempts.get("i1").unwrap();
    assert_eq!(retry.attempt, 1);
    assert_eq!(retry.due_at_ms, 1_000_000 + 1000);
    assert!(retry.error.is_none());
    assert!(state.claimed.contains("i1"));
  }

  #[test]
  fn apply_worker_exit_failure_uses_backoff() {
    let mut state = empty_state(10);
    state.running.insert(
      "i1".into(),
      RunningEntry {
        identifier: "r#1".into(),
        issue: sample_issue("i1", "r#1"),
        session: LiveSession::default(),
        started_at: chrono::Utc::now(),
        retry_attempt: 0,
      },
    );

    apply_worker_exit(
      &mut state,
      "i1".to_string(),
      WorkerExitReason::Failed("error".into()),
      1.0,
      (0, 0, 0),
      0,
      300_000,
    );

    let retry = state.retry_attempts.get("i1").unwrap();
    assert_eq!(retry.attempt, 1);
    assert_eq!(retry.due_at_ms, 10_000);
    assert_eq!(retry.error.as_deref(), Some("error"));
  }

  #[test]
  fn apply_agent_update_modifies_session() {
    let mut state = empty_state(10);
    state.running.insert(
      "i1".into(),
      RunningEntry {
        identifier: "r#1".into(),
        issue: sample_issue("i1", "r#1"),
        session: LiveSession::default(),
        started_at: chrono::Utc::now(),
        retry_attempt: 0,
      },
    );

    apply_agent_update(
      &mut state,
      "i1",
      AgentUpdatePayload {
        session_id: Some("s1-1".into()),
        thread_id: Some("s1".into()),
        turn_id: Some("1".into()),
        input_tokens: Some(50),
        output_tokens: Some(60),
        total_tokens: Some(110),
        turn_count: Some(2),
      },
    );

    let entry = state.running.get("i1").unwrap();
    assert_eq!(entry.session.session_id.as_deref(), Some("s1-1"));
    assert_eq!(entry.session.agent_input_tokens, 50);
    assert_eq!(entry.session.turn_count, 2);
    assert!(
      entry.session.last_agent_timestamp.is_some(),
      "stall detection: last_agent_timestamp set on update"
    );
  }

  #[test]
  fn apply_agent_update_ignores_unknown_issue() {
    let mut state = empty_state(10);
    apply_agent_update(
      &mut state,
      "nonexistent",
      AgentUpdatePayload {
        session_id: Some("x".into()),
        ..Default::default()
      },
    );
    assert!(state.running.is_empty());
  }

  #[test]
  fn release_claim_removes_from_claimed_and_retry() {
    let mut state = empty_state(10);
    state.claimed.insert("i1".into());
    state.retry_attempts.insert(
      "i1".into(),
      RetryEntry {
        issue_id: "i1".into(),
        identifier: "r#1".into(),
        attempt: 1,
        due_at_ms: 0,
        error: None,
      },
    );

    release_claim(&mut state, "i1");

    assert!(!state.claimed.contains("i1"));
    assert!(!state.retry_attempts.contains_key("i1"));
  }

  #[test]
  fn remove_retry_on_dispatch_removes_entry() {
    let mut state = empty_state(10);
    state.retry_attempts.insert(
      "i1".into(),
      RetryEntry {
        issue_id: "i1".into(),
        identifier: "r#1".into(),
        attempt: 1,
        due_at_ms: 0,
        error: None,
      },
    );

    remove_retry_on_dispatch(&mut state, "i1");

    assert!(!state.retry_attempts.contains_key("i1"));
  }
}
