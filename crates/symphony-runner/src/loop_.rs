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

use symphony_agent::{AgentExitReason, AgentRunnerUpdate, RunnerProtocol, run_agent_with_protocol};
use symphony_config::RunnerType;
use symphony_config::ServiceConfig;
use symphony_domain::{Issue, LiveSession, OrchestratorState, RunningEntry};
use symphony_orchestration::{
  AgentUpdatePayload, OrchestratorMessage, WorkerExitReason, apply_agent_update, apply_worker_exit,
  available_slots, can_dispatch, release_claim, remove_retry_on_dispatch,
};
use symphony_prompt::render_prompt;
use symphony_tracker::{
  fetch_candidate_issues, fetch_issue_states_by_ids, issue_passes_label_filters,
};
use symphony_workspace::{ensure_workspace_dir, run_hook};

/// Abort handle for a spawned worker task (so we can cancel on TerminateWorker).
pub type WorkerHandle = tokio::task::JoinHandle<()>;

/// Optional oneshot to signal main to exit with non-zero code when a worker fails (env SYMPHONY_EXIT_ON_WORKER_FAILURE).
pub type ExitOnFailureSender = Option<tokio::sync::oneshot::Sender<i32>>;

/// Run the orchestrator task. Owns `state`; receives on `rx`; passes `tx` to spawned workers.
/// Uses `start_instant` for retry due_at_ms (monotonic). Config and prompt are read from `workflow_state` each tick (reload-safe).
/// If `exit_on_failure_tx` is Some and a worker exits with Failed(..), sends 1 and returns so the process can exit non-zero.
pub async fn run_orchestrator(
  mut state: OrchestratorState,
  workflow_state: Arc<tokio::sync::RwLock<(ServiceConfig, String)>>,
  mut rx: mpsc::UnboundedReceiver<OrchestratorMessage>,
  tx: mpsc::UnboundedSender<OrchestratorMessage>,
  start_instant: Instant,
  mut worker_handles: HashMap<String, WorkerHandle>,
  mut exit_on_failure_tx: ExitOnFailureSender,
) {
  while let Some(msg) = rx.recv().await {
    match msg {
      OrchestratorMessage::PollTick => {
        let (config, prompt_template) = workflow_state.read().await.clone();
        if let Err(e) = config.validate_dispatch() {
          warn!(%e, "dispatch preflight validation failed, skipping tick");
          continue;
        }
        let now_ms = start_instant.elapsed().as_millis() as u64;
        poll_tick(
          &mut state,
          &config,
          &prompt_template,
          now_ms,
          tx.clone(),
          &mut worker_handles,
        )
        .await;
      }
      OrchestratorMessage::WorkerExit {
        issue_id,
        reason,
        runtime_seconds,
        token_totals,
      } => {
        worker_handles.remove(&issue_id);
        info!(
          %issue_id,
          ?reason,
          runtime_secs = %runtime_seconds,
          "worker exited"
        );
        if matches!(reason, WorkerExitReason::Failed(_)) {
          if let Some(tx_exit) = exit_on_failure_tx.take() {
            let _ = tx_exit.send(1);
            return;
          }
        }
        let now_ms = start_instant.elapsed().as_millis() as u64;
        let max_retry = workflow_state.read().await.0.agent.max_retry_backoff_ms;
        apply_worker_exit(
          &mut state,
          issue_id,
          reason,
          runtime_seconds,
          token_totals,
          now_ms,
          max_retry,
        );
      }
      OrchestratorMessage::AgentUpdate { issue_id, update } => {
        if update.session_id.is_some()
          || update.thread_id.is_some()
          || update.total_tokens.is_some()
        {
          debug!(
            %issue_id,
            session = ?update.session_id,
            tokens = ?update.total_tokens,
            "agent update"
          );
        }
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

/// One PollTick: reconcile, validate (caller did), fetch candidates, sort,
/// process due retries, dispatch new, notify (log).
async fn poll_tick(
  state: &mut OrchestratorState,
  config: &ServiceConfig,
  prompt_template: &str,
  now_ms: u64,
  tx: mpsc::UnboundedSender<OrchestratorMessage>,
  worker_handles: &mut HashMap<String, WorkerHandle>,
) {
  let max_concurrent = config.agent.max_concurrent_agents;

  // Reconcile: fetch current state for running issues; terminate if terminal or no longer active.
  if !state.running.is_empty() {
    let identifiers: Vec<String> = state
      .running
      .values()
      .map(|e| e.identifier.clone())
      .collect();
    let endpoint = config.tracker.endpoint_or_default();
    let default_active: Vec<String> = vec!["open".to_string()];
    let default_terminal: Vec<String> = vec!["closed".to_string()];
    let active = config
      .tracker
      .active_states
      .as_deref()
      .unwrap_or_else(|| default_active.as_slice());
    let terminal = config
      .tracker
      .terminal_states
      .as_deref()
      .unwrap_or_else(|| default_terminal.as_slice());
    match fetch_issue_states_by_ids(
      &endpoint,
      &config.tracker.api_key,
      &config.tracker.repo,
      &identifiers,
    )
    .await
    {
      Ok(issues) => {
        for issue in issues {
          let is_terminal = terminal
            .iter()
            .any(|s| s.eq_ignore_ascii_case(&issue.state));
          let is_active = active.iter().any(|s| s.eq_ignore_ascii_case(&issue.state));
          if is_terminal || !is_active {
            let _ = tx.send(OrchestratorMessage::TerminateWorker {
              issue_id: issue.id.clone(),
              cleanup_workspace: true,
            });
            if let Some(h) = worker_handles.remove(&issue.id) {
              h.abort();
            }
            release_claim(state, &issue.id);
            debug!(%issue.id, state = %issue.state, "reconcile: terminated (terminal or inactive)");
          }
        }
      }
      Err(e) => {
        warn!(%e, "reconcile: fetch issue states failed, skipping this tick");
      }
    }
  }

  // Stall detection: terminate running workers that have exceeded stall_timeout_ms since last agent activity.
  let stall_timeout_ms = config.runner.stall_timeout_ms();
  let now = chrono::Utc::now();
  let stalled: Vec<String> = state
    .running
    .iter()
    .filter_map(|(issue_id, entry)| {
      let ts = entry
        .session
        .last_agent_timestamp
        .unwrap_or(entry.started_at);
      let elapsed_ms = (now - ts).num_milliseconds().max(0) as u64;
      if elapsed_ms >= stall_timeout_ms {
        Some(issue_id.clone())
      } else {
        None
      }
    })
    .collect();
  for issue_id in stalled {
    if let Some(handle) = worker_handles.remove(&issue_id) {
      handle.abort();
    }
    if let Some(entry) = state.running.remove(&issue_id) {
      let runtime_seconds = (now - entry.started_at).num_milliseconds().max(0) as f64 / 1000.0;
      let token_totals = (
        entry.session.agent_input_tokens,
        entry.session.agent_output_tokens,
        entry.session.agent_total_tokens,
      );
      apply_worker_exit(
        state,
        issue_id.clone(),
        WorkerExitReason::Stalled,
        runtime_seconds,
        token_totals,
        now_ms,
        config.agent.max_retry_backoff_ms,
      );
      debug!(%issue_id, "stall: terminated (no agent activity for {}ms)", stall_timeout_ms);
    }
  }

  // 5. Process due retries: fetch current issue state; if still active, re-dispatch; else release claim.
  let due: Vec<(String, String)> = state
    .retry_attempts
    .iter()
    .filter(|(_, e)| e.due_at_ms <= now_ms)
    .map(|(id, e)| (id.clone(), e.identifier.clone()))
    .collect();
  if !due.is_empty() {
    let identifiers: Vec<String> = due.iter().map(|(_, ident)| ident.clone()).collect();
    let default_active: Vec<String> = vec!["open".to_string()];
    let default_terminal: Vec<String> = vec!["closed".to_string()];
    let active = config
      .tracker
      .active_states
      .as_deref()
      .unwrap_or_else(|| default_active.as_slice());
    let terminal = config
      .tracker
      .terminal_states
      .as_deref()
      .unwrap_or_else(|| default_terminal.as_slice());
    let endpoint = config.tracker.endpoint_or_default();
    match fetch_issue_states_by_ids(
      &endpoint,
      &config.tracker.api_key,
      &config.tracker.repo,
      &identifiers,
    )
    .await
    {
      Ok(issues) => {
        let fetched: std::collections::HashMap<String, symphony_domain::Issue> = issues
          .into_iter()
          .map(|i| (i.identifier.clone(), i))
          .collect();
        let retry_exclude_labels = config.tracker.effective_exclude_labels();
        for (issue_id, identifier) in due {
          state.retry_attempts.remove(&issue_id);
          if let Some(issue) = fetched.get(&identifier) {
            let is_active = active.iter().any(|s| s.eq_ignore_ascii_case(&issue.state));
            let is_terminal = terminal
              .iter()
              .any(|s| s.eq_ignore_ascii_case(&issue.state));
            let label_eligible = issue_passes_label_filters(
              issue,
              config.tracker.include_labels.as_deref(),
              retry_exclude_labels.as_deref(),
            );
            if is_active
              && !is_terminal
              && label_eligible
              && !state.running.contains_key(&issue_id)
              && available_slots(state, max_concurrent) > 0
            {
              dispatch_worker(
                state,
                issue,
                prompt_template,
                config,
                tx.clone(),
                worker_handles,
              )
              .await;
              debug!(%issue_id, "due retry: re-dispatched");
            } else {
              release_claim(state, &issue_id);
              debug!(%issue_id, "due retry: no longer eligible, released");
            }
          } else {
            release_claim(state, &issue_id);
            debug!(%issue_id, "due retry: fetch missed, released");
          }
        }
      }
      Err(e) => {
        warn!(%e, "due retry: fetch failed, releasing all due");
        for (issue_id, _) in due {
          state.retry_attempts.remove(&issue_id);
          release_claim(state, &issue_id);
        }
      }
    }
  }

  // 6. Dispatch new: fetch candidates from tracker, sort, for each eligible and slot dispatch.
  let default_active: Vec<String> = vec!["open".to_string()];
  let active_states = config
    .tracker
    .active_states
    .as_deref()
    .unwrap_or_else(|| default_active.as_slice());
  let exclude_labels = config.tracker.effective_exclude_labels();
  let candidates = match fetch_candidate_issues(
    &config.tracker.endpoint_or_default(),
    &config.tracker.api_key,
    &config.tracker.repo,
    active_states,
    config.tracker.include_labels.as_deref(),
    exclude_labels.as_deref(),
  )
  .await
  {
    Ok(c) => c,
    Err(e) => {
      warn!(%e, "fetch candidates failed, skipping this tick");
      vec![]
    }
  };
  let mut sorted = candidates;
  symphony_orchestration::sort_for_dispatch(&mut sorted);
  let num_candidates = sorted.len();

  if num_candidates > 0 {
    info!(candidates = num_candidates, "fetched candidates");
  }

  for issue in sorted {
    if !can_dispatch(state, &issue.id, max_concurrent) {
      break;
    }
    dispatch_worker(
      state,
      &issue,
      prompt_template,
      config,
      tx.clone(),
      worker_handles,
    )
    .await;
  }

  // 7. Notify (log summary)
  if !state.running.is_empty() || !state.retry_attempts.is_empty() {
    info!(
      running = state.running.len(),
      retry_queued = state.retry_attempts.len(),
      "tick"
    );
  } else if state.running.is_empty() && state.retry_attempts.is_empty() {
    info!(candidates = num_candidates, "poll tick (no work yet)");
  }
}

/// Spawn a worker: ensure workspace, run hooks, render prompt, launch agent; send AgentUpdate/WorkerExit.
async fn dispatch_worker(
  state: &mut OrchestratorState,
  issue: &Issue,
  prompt_template: &str,
  config: &ServiceConfig,
  tx: mpsc::UnboundedSender<OrchestratorMessage>,
  worker_handles: &mut HashMap<String, WorkerHandle>,
) {
  let issue_id = issue.id.clone();
  let identifier = issue.identifier.clone();
  remove_retry_on_dispatch(state, &issue_id);
  state.claimed.insert(issue_id.clone());

  let retry_attempt = state
    .retry_attempts
    .get(&issue_id)
    .map(|e| e.attempt)
    .unwrap_or(0);
  let entry = RunningEntry {
    identifier: identifier.clone(),
    issue: issue.clone(),
    session: LiveSession::default(),
    started_at: chrono::Utc::now(),
    retry_attempt,
  };
  state.running.insert(issue_id.clone(), entry);

  let config = config.clone();
  let prompt_template = prompt_template.to_string();
  let issue_id_for_worker = issue_id.clone();
  let issue_clone = issue.clone();
  let tx_worker = tx.clone();

  let handle = tokio::spawn(async move {
    let (path, created) = match ensure_workspace_dir(&config.workspace.root, &identifier).await {
      Ok(p) => p,
      Err(e) => {
        warn!(%issue_id_for_worker, %e, "ensure workspace failed");
        release_claim_and_send_exit(
          &tx_worker,
          &issue_id_for_worker,
          WorkerExitReason::Failed(e.to_string()),
          0.0,
        );
        return;
      }
    };

    if created {
      if let Some(ref script) = config.hooks.after_create {
        if let Err(e) = run_hook(script, &path, config.hooks.timeout_ms()).await {
          warn!(%issue_id_for_worker, %e, "after_create hook failed");
        }
      }
    }

    if let Some(ref script) = config.hooks.before_run {
      if let Err(e) = run_hook(script, &path, config.hooks.timeout_ms()).await {
        warn!(%issue_id_for_worker, %e, "before_run hook failed");
        release_claim_and_send_exit(
          &tx_worker,
          &issue_id_for_worker,
          WorkerExitReason::Failed(e.to_string()),
          0.0,
        );
        return;
      }
    }

    let prompt = match render_prompt(&prompt_template, &issue_clone, Some(retry_attempt)) {
      Ok(p) => p,
      Err(e) => {
        warn!(%issue_id_for_worker, %e, "render prompt failed");
        release_claim_and_send_exit(
          &tx_worker,
          &issue_id_for_worker,
          WorkerExitReason::Failed(e.to_string()),
          0.0,
        );
        return;
      }
    };

    let turn_timeout_ms = config.runner.turn_timeout_ms();
    let read_timeout_ms = config.runner.read_timeout_ms();
    let (update_tx, mut update_rx) = tokio::sync::mpsc::unbounded_channel::<AgentRunnerUpdate>();
    let tx_updates = tx_worker.clone();
    let issue_id_updates = issue_id_for_worker.clone();
    tokio::spawn(async move {
      while let Some(u) = update_rx.recv().await {
        let payload = AgentUpdatePayload {
          session_id: u.session_id,
          thread_id: u.thread_id,
          turn_id: u.turn_id,
          input_tokens: u.input_tokens,
          output_tokens: u.output_tokens,
          total_tokens: u.total_tokens,
          turn_count: u.turn_count,
        };
        let _ = tx_updates.send(OrchestratorMessage::AgentUpdate {
          issue_id: issue_id_updates.clone(),
          update: payload,
        });
      }
    });

    let protocol = match config.runner.runner_type {
      RunnerType::Codex => RunnerProtocol::Codex,
      RunnerType::Acp => RunnerProtocol::Acp,
      RunnerType::Cli => RunnerProtocol::Cli,
    };
    let outcome = run_agent_with_protocol(
      protocol,
      &config.runner.command,
      &path,
      &prompt,
      &identifier,
      &issue_clone.title,
      turn_timeout_ms,
      read_timeout_ms,
      Some(update_tx),
    )
    .await;

    let (reason, runtime_seconds, token_totals) = match outcome {
      Ok(out) => (
        match out.exit_reason {
          AgentExitReason::Normal => WorkerExitReason::Normal,
          AgentExitReason::TurnTimeout | AgentExitReason::ResponseTimeout => {
            WorkerExitReason::TimedOut
          }
          AgentExitReason::TurnFailed => WorkerExitReason::Failed("turn failed".into()),
          AgentExitReason::ProcessError(ref s) => WorkerExitReason::Failed(s.clone()),
          AgentExitReason::TurnCancelled => WorkerExitReason::CanceledByReconciliation,
        },
        out.runtime_seconds,
        out.token_totals,
      ),
      Err(e) => (WorkerExitReason::Failed(e.to_string()), 0.0, (0, 0, 0)),
    };

    let _ = tx_worker.send(OrchestratorMessage::WorkerExit {
      issue_id: issue_id_for_worker,
      reason,
      runtime_seconds,
      token_totals,
    });
  });

  worker_handles.insert(issue_id.clone(), handle);
  debug!(%issue_id, "dispatched");
}

fn release_claim_and_send_exit(
  tx: &mpsc::UnboundedSender<OrchestratorMessage>,
  issue_id: &str,
  reason: WorkerExitReason,
  runtime_seconds: f64,
) {
  let _ = tx.send(OrchestratorMessage::WorkerExit {
    issue_id: issue_id.to_string(),
    reason,
    runtime_seconds,
    token_totals: (0, 0, 0),
  });
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;
  use std::time::Instant;

  use symphony_config::ServiceConfig;
  use symphony_domain::OrchestratorState;
  use symphony_orchestration::OrchestratorMessage;
  use tokio::sync::{RwLock, mpsc};

  use super::run_orchestrator;

  fn test_config() -> ServiceConfig {
    ServiceConfig {
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
        root: std::env::temp_dir().join("symphony_ws"),
      },
      hooks: symphony_config::HooksConfig::default(),
      agent: symphony_config::AgentConfig::default(),
    }
  }

  #[tokio::test]
  async fn orchestrator_exits_when_channel_closed() {
    let (tx, rx) = mpsc::unbounded_channel();
    let workflow_state = Arc::new(RwLock::new((test_config(), String::new())));
    let state = OrchestratorState {
      poll_interval_ms: 1000,
      max_concurrent_agents: 2,
      ..Default::default()
    };
    drop(tx);
    run_orchestrator(
      state,
      workflow_state,
      rx,
      mpsc::unbounded_channel().0,
      Instant::now(),
      Default::default(),
      None,
    )
    .await;
  }

  #[tokio::test]
  async fn orchestrator_processes_poll_tick_then_exits() {
    let (tx, rx) = mpsc::unbounded_channel();
    let workflow_state = Arc::new(RwLock::new((test_config(), String::new())));
    let state = OrchestratorState {
      poll_interval_ms: 1000,
      max_concurrent_agents: 2,
      ..Default::default()
    };
    let _ = tx.send(OrchestratorMessage::PollTick);
    drop(tx);
    run_orchestrator(
      state,
      workflow_state,
      rx,
      mpsc::unbounded_channel().0,
      Instant::now(),
      Default::default(),
      None,
    )
    .await;
  }
}
