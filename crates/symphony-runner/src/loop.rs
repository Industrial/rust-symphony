//! Orchestrator task: owns state, receives OrchestratorMessage, applies transitions (SPEC §6–7).
//!
//! Single-owner + message passing. PollTick runs the 07 sequence (reconcile, validate,
//! fetch candidates, sort, process due retries, dispatch, notify); other variants
//! use symphony_orchestration transition helpers.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use symphony_config::ServiceConfig;
use symphony_domain::{Issue, LiveSession, OrchestratorState, RunningEntry};
use symphony_orchestration::{
  OrchestratorMessage, WorkerExitReason, apply_agent_update, apply_worker_exit, can_dispatch,
  release_claim, remove_retry_on_dispatch,
};

/// Abort handle for a spawned worker task (so we can cancel on TerminateWorker).
pub type WorkerHandle = tokio::task::JoinHandle<()>;

/// Run the orchestrator task. Owns `state`; receives on `rx`; passes `tx` to spawned workers.
/// Uses `start_instant` for retry due_at_ms (monotonic).
pub async fn run_orchestrator(
  mut state: OrchestratorState,
  config: Arc<ServiceConfig>,
  mut rx: mpsc::UnboundedReceiver<OrchestratorMessage>,
  tx: mpsc::UnboundedSender<OrchestratorMessage>,
  start_instant: Instant,
  mut worker_handles: HashMap<String, WorkerHandle>,
) {
  while let Some(msg) = rx.recv().await {
    match msg {
      OrchestratorMessage::PollTick => {
        if let Err(e) = config.validate_dispatch() {
          warn!(%e, "dispatch preflight validation failed, skipping tick");
          continue;
        }
        let now_ms = start_instant.elapsed().as_millis() as u64;
        poll_tick(&mut state, &config, now_ms, tx.clone(), &mut worker_handles).await;
      }
      OrchestratorMessage::WorkerExit {
        issue_id,
        reason,
        runtime_seconds,
        token_totals,
      } => {
        worker_handles.remove(&issue_id);
        let now_ms = start_instant.elapsed().as_millis() as u64;
        apply_worker_exit(
          &mut state,
          issue_id,
          reason,
          runtime_seconds,
          token_totals,
          now_ms,
          config.agent.max_retry_backoff_ms,
        );
      }
      OrchestratorMessage::AgentUpdate { issue_id, update } => {
        apply_agent_update(&mut state, &issue_id, update);
      }
      OrchestratorMessage::TerminateWorker {
        issue_id,
        cleanup_workspace: _,
      } => {
        if let Some(handle) = worker_handles.remove(&issue_id) {
          handle.abort();
        }
        release_claim(&mut state, &issue_id);
      }
    }
  }
  debug!("orchestrator task exiting");
}

/// One PollTick: reconcile (stub), validate (caller did), fetch candidates (stub), sort,
/// process due retries, dispatch new, notify (log).
async fn poll_tick(
  state: &mut OrchestratorState,
  config: &ServiceConfig,
  now_ms: u64,
  tx: mpsc::UnboundedSender<OrchestratorMessage>,
  worker_handles: &mut HashMap<String, WorkerHandle>,
) {
  let max_concurrent = config.agent.max_concurrent_agents;

  // 5. Process due retries: for each entry with due_at_ms <= now_ms, release or re-dispatch (stub: release).
  let due: Vec<String> = state
    .retry_attempts
    .iter()
    .filter(|(_, e)| e.due_at_ms <= now_ms)
    .map(|(id, _)| id.clone())
    .collect();
  for issue_id in due {
    state.retry_attempts.remove(&issue_id);
    // Stub: no tracker fetch; release claim. When tracker is wired, fetch issue and dispatch if eligible.
    release_claim(state, &issue_id);
    debug!(%issue_id, "processed due retry (stub: released)");
  }

  // 6. Dispatch new: fetch candidates (stub: empty), sort, for each eligible and slot dispatch.
  let candidates: Vec<Issue> = fetch_candidates_stub();
  let mut sorted = candidates;
  symphony_orchestration::sort_for_dispatch(&mut sorted);

  for issue in sorted {
    if !can_dispatch(state, &issue.id, max_concurrent) {
      break;
    }
    dispatch_worker(state, &issue, tx.clone(), worker_handles).await;
  }

  // 7. Notify (log summary)
  if !state.running.is_empty() || !state.retry_attempts.is_empty() {
    info!(
      running = state.running.len(),
      retry_queued = state.retry_attempts.len(),
      "tick"
    );
  }
}

/// Stub: returns no candidates. Replace with tracker client when available.
fn fetch_candidates_stub() -> Vec<Issue> {
  vec![]
}

/// Spawn a worker for the issue and insert into state.running/claimed. Stub worker sends WorkerExit after delay.
async fn dispatch_worker(
  state: &mut OrchestratorState,
  issue: &Issue,
  tx: mpsc::UnboundedSender<OrchestratorMessage>,
  worker_handles: &mut HashMap<String, WorkerHandle>,
) {
  let issue_id = issue.id.clone();
  let identifier = issue.identifier.clone();
  remove_retry_on_dispatch(state, &issue_id);
  state.claimed.insert(issue_id.clone());

  let entry = RunningEntry {
    identifier: identifier.clone(),
    issue: issue.clone(),
    session: LiveSession::default(),
    started_at: chrono::Utc::now(),
    retry_attempt: 0,
  };
  state.running.insert(issue_id.clone(), entry);

  let handle = tokio::spawn(async move {
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    let _ = tx.send(OrchestratorMessage::WorkerExit {
      issue_id: issue_id.clone(),
      reason: WorkerExitReason::Normal,
      runtime_seconds: 0.1,
      token_totals: (0, 0, 0),
    });
  });
  worker_handles.insert(issue_id, handle);
  debug!(%issue_id, "dispatched (stub worker)");
}
