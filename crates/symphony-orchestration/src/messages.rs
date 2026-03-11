//! Orchestrator messages and worker exit reasons (SPEC §7.3).

use serde::{Deserialize, Serialize};

/// Payload for live agent updates (session metadata, tokens, rate limits).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentUpdatePayload {
  pub session_id: Option<String>,
  pub thread_id: Option<String>,
  pub turn_id: Option<String>,
  pub input_tokens: Option<u64>,
  pub output_tokens: Option<u64>,
  pub total_tokens: Option<u64>,
  pub turn_count: Option<u32>,
}

/// Reason a worker process exited.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkerExitReason {
  Normal,
  Failed(String),
  TimedOut,
  Stalled,
  CanceledByReconciliation,
}

/// Messages sent to the orchestrator task (SPEC §7.3).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum OrchestratorMessage {
  /// Poll tick: reconcile, validate, fetch candidates, dispatch, process due retries.
  PollTick,

  /// Worker finished (normal or abnormal).
  WorkerExit {
    issue_id: String,
    reason: WorkerExitReason,
    runtime_seconds: f64,
    token_totals: (u64, u64, u64),
  },

  /// Live update from agent (session metadata, tokens, rate limits).
  AgentUpdate {
    issue_id: String,
    update: AgentUpdatePayload,
  },

  /// Request to terminate a running worker (reconciliation or stall).
  TerminateWorker {
    issue_id: String,
    cleanup_workspace: bool,
  },
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn worker_exit_reason_serialize() {
    let r = WorkerExitReason::TimedOut;
    let j = serde_json::to_string(&r).unwrap();
    assert!(j.contains("timed_out"));
  }

  #[test]
  fn orchestrator_message_poll_tick() {
    let m = OrchestratorMessage::PollTick;
    let j = serde_json::to_string(&m).unwrap();
    assert!(j.contains("poll_tick"));
  }

  #[test]
  fn orchestrator_message_worker_exit() {
    let m = OrchestratorMessage::WorkerExit {
      issue_id: "i1".into(),
      reason: WorkerExitReason::Normal,
      runtime_seconds: 1.5,
      token_totals: (10, 20, 30),
    };
    let j = serde_json::to_string(&m).unwrap();
    assert!(j.contains("i1"));
    assert!(j.contains("worker_exit"));
  }

  #[test]
  fn agent_update_payload_default() {
    let p = AgentUpdatePayload::default();
    assert!(p.session_id.is_none());
    assert!(p.input_tokens.is_none());
  }
}
