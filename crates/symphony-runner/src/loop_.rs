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
  check_run_conclusion_is_failed, commit_status_state_is_failed, fetch_candidate_issues,
  fetch_check_runs_for_ref, fetch_commit_status_for_ref, fetch_has_qualifying_mention,
  fetch_issue_states_by_ids, fetch_issues_with_label, issue_passes_label_filters,
  parse_issue_number, resolve_pr_for_issue,
};
use symphony_workspace::{ensure_workspace_dir, ensure_worktree_dir, run_hook};

/// One poll cycle in dry-run: fetch candidates, sort, apply concurrency; log what would be dispatched; no workers or tracker writes.
pub async fn dry_run_one_poll(
  config: &ServiceConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let max_concurrent = config.agent.max_concurrent_agents;
  let active_states = config.tracker.active_states_slice();
  let exclude_labels = config.tracker.effective_exclude_labels();
  let candidates = fetch_candidate_issues(
    &config.tracker.endpoint_or_default(),
    &config.tracker.api_key,
    &config.tracker.repo,
    active_states,
    config.tracker.include_labels.as_deref(),
    exclude_labels.as_deref(),
  )
  .await?;
  let mut sorted = candidates;
  symphony_orchestration::sort_for_dispatch(&mut sorted);
  let num_candidates = sorted.len();
  let would_dispatch_count = num_candidates.min(max_concurrent as usize);
  let would_dispatch: Vec<&str> = sorted
    .iter()
    .take(would_dispatch_count)
    .map(|i| i.identifier.as_str())
    .collect();
  info!(
    candidates = num_candidates,
    would_dispatch = would_dispatch_count,
    identifiers = ?would_dispatch,
    "dry-run: one poll cycle complete (no workers started, no tracker writes)"
  );
  Ok(())
}

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
/// SPEC_ADDENDUM_2 B.1: Fix-PR logic (candidate set for PR-open issues, check fetch, mention fetch, re-dispatch)
/// MUST be gated on `config.fix_pr == true`; when false, do not call PR or Checks API for fix-PR purposes.
async fn poll_tick(
  state: &mut OrchestratorState,
  config: &ServiceConfig,
  prompt_template: &str,
  now_ms: u64,
  tx: mpsc::UnboundedSender<OrchestratorMessage>,
  worker_handles: &mut HashMap<String, WorkerHandle>,
) {
  reconcile_running(state, config, tx.clone(), worker_handles).await;
  terminate_stalled(state, config, now_ms, worker_handles).await;
  process_due_retries(
    state,
    config,
    prompt_template,
    now_ms,
    tx.clone(),
    worker_handles,
  )
  .await;
  let num_candidates =
    dispatch_new_candidates(state, config, prompt_template, tx, worker_handles).await;
  log_tick_summary(state, num_candidates);
}

/// Reconcile: fetch current state for running issues; terminate if terminal or no longer active.
async fn reconcile_running(
  state: &mut OrchestratorState,
  config: &ServiceConfig,
  tx: mpsc::UnboundedSender<OrchestratorMessage>,
  worker_handles: &mut HashMap<String, WorkerHandle>,
) {
  if state.running.is_empty() {
    return;
  }
  let identifiers: Vec<String> = state
    .running
    .values()
    .map(|e| e.identifier.clone())
    .collect();
  let endpoint = config.tracker.endpoint_or_default();
  let active = config.tracker.active_states_slice();
  let terminal = config.tracker.terminal_states_slice();
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

/// Terminate running workers that have exceeded stall_timeout_ms since last agent activity.
async fn terminate_stalled(
  state: &mut OrchestratorState,
  config: &ServiceConfig,
  now_ms: u64,
  worker_handles: &mut HashMap<String, WorkerHandle>,
) {
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
}

/// Process due retries: fetch current issue state; if still active, re-dispatch; else release claim.
async fn process_due_retries(
  state: &mut OrchestratorState,
  config: &ServiceConfig,
  prompt_template: &str,
  now_ms: u64,
  tx: mpsc::UnboundedSender<OrchestratorMessage>,
  worker_handles: &mut HashMap<String, WorkerHandle>,
) {
  let max_concurrent = config.agent.max_concurrent_agents;
  let due: Vec<(String, String)> = state
    .retry_attempts
    .iter()
    .filter(|(_, e)| e.due_at_ms <= now_ms)
    .map(|(id, e)| (id.clone(), e.identifier.clone()))
    .collect();
  if due.is_empty() {
    return;
  }
  let identifiers: Vec<String> = due.iter().map(|(_, ident)| ident.clone()).collect();
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
      let active = config.tracker.active_states_slice();
      let terminal = config.tracker.terminal_states_slice();
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

/// Dispatch new candidates: fix-PR candidates (when fix_pr) then normal candidates; sort, dispatch up to concurrency. Returns candidate count.
async fn dispatch_new_candidates(
  state: &mut OrchestratorState,
  config: &ServiceConfig,
  prompt_template: &str,
  tx: mpsc::UnboundedSender<OrchestratorMessage>,
  worker_handles: &mut HashMap<String, WorkerHandle>,
) -> usize {
  let max_concurrent = config.agent.max_concurrent_agents;
  let active_states = config.tracker.active_states_slice();
  let mut total_candidates = 0usize;

  if config.fix_pr {
    if let Some(ref pr_open_label) = config.tracker.pr_open_label {
      let head_pattern = config
        .tracker
        .fix_pr_head_branch_pattern
        .as_deref()
        .unwrap_or("symphony/issue-{number}");
      match fetch_issues_with_label(
        &config.tracker.endpoint_or_default(),
        &config.tracker.api_key,
        &config.tracker.repo,
        pr_open_label,
        active_states,
      )
      .await
      {
        Ok(fix_pr_issues) => {
          let fix_pr_candidates: Vec<_> = fix_pr_issues
            .into_iter()
            .filter(|issue| !state.running.contains_key(&issue.id))
            .collect();
          total_candidates += fix_pr_candidates.len();
          for issue in fix_pr_candidates {
            if !can_dispatch(state, &issue.id, max_concurrent) {
              break;
            }
            let issue_number = match parse_issue_number(&issue.identifier) {
              Some(n) => n,
              None => {
                warn!(%issue.id, "fix-PR candidate missing issue number in identifier, skipping");
                continue;
              }
            };
            match resolve_pr_for_issue(
              &config.tracker.endpoint_or_default(),
              &config.tracker.api_key,
              &config.tracker.repo,
              issue_number,
              head_pattern,
            )
            .await
            {
              Ok(Some(pr)) => {
                let endpoint = config.tracker.endpoint_or_default();
                let api_key = &config.tracker.api_key;
                let repo = &config.tracker.repo;

                // SPEC_ADDENDUM_2 B.4: dispatch if any_check_failed || has_qualifying_mention; otherwise wait.
                let any_check_failed = match fetch_check_runs_for_ref(
                  &endpoint,
                  api_key,
                  repo,
                  &pr.head_ref,
                )
                .await
                {
                  Ok(runs) => runs
                    .iter()
                    .any(|r| check_run_conclusion_is_failed(r.conclusion.as_deref())),
                  Err(e) => {
                    debug!(%issue.id, %e, "fetch check runs for fix-PR failed, treating as no failure");
                    false
                  }
                };

                let commit_failed = if !any_check_failed {
                  match fetch_commit_status_for_ref(&endpoint, api_key, repo, &pr.head_ref).await {
                    Ok(status) => commit_status_state_is_failed(&status.state),
                    Err(e) => {
                      debug!(%issue.id, %e, "fetch commit status for fix-PR failed, treating as no failure");
                      false
                    }
                  }
                } else {
                  false
                };

                let has_mention = match config.tracker.mention_handle.as_deref() {
                  Some(handle) => match fetch_has_qualifying_mention(
                    &endpoint,
                    api_key,
                    repo,
                    issue_number,
                    pr.pr_number,
                    handle,
                    pr.pr_updated_at.as_deref(),
                  )
                  .await
                  {
                    Ok(b) => b,
                    Err(e) => {
                      debug!(%issue.id, %e, "fetch mentions for fix-PR failed, treating as no mention");
                      false
                    }
                  },
                  None => false,
                };

                let should_dispatch = any_check_failed || commit_failed || has_mention;
                if should_dispatch {
                  dispatch_worker(
                    state,
                    &issue,
                    prompt_template,
                    config,
                    tx.clone(),
                    worker_handles,
                  )
                  .await;
                } else {
                  debug!(%issue.id, "fix-PR candidate: no failed checks and no qualifying mention, waiting");
                }
              }
              Ok(None) => {
                debug!(%issue.id, "fix-PR candidate has no PR (head branch pattern), waiting");
              }
              Err(e) => {
                warn!(%issue.id, %e, "resolve PR for fix-PR candidate failed, skipping");
              }
            }
          }
        }
        Err(e) => {
          warn!(%e, "fetch fix-PR candidates (issues with pr_open_label) failed, skipping fix-PR this tick");
        }
      }
    }
  }

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
      return total_candidates;
    }
  };
  let mut sorted = candidates;
  symphony_orchestration::sort_for_dispatch(&mut sorted);
  let num_candidates = sorted.len();
  total_candidates += num_candidates;
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
  total_candidates
}

/// Log tick summary (running count, retry count, or "no work yet").
pub(crate) fn log_tick_summary(state: &OrchestratorState, num_candidates: usize) {
  if !state.running.is_empty() || !state.retry_attempts.is_empty() {
    info!(
      running = state.running.len(),
      retry_queued = state.retry_attempts.len(),
      "tick"
    );
  } else {
    info!(candidates = num_candidates, "poll tick (no work yet)");
  }
}

/// Forward agent runner updates to the orchestrator channel.
async fn forward_agent_updates(
  mut update_rx: tokio::sync::mpsc::UnboundedReceiver<AgentRunnerUpdate>,
  issue_id: String,
  tx: mpsc::UnboundedSender<OrchestratorMessage>,
) {
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
    let _ = tx.send(OrchestratorMessage::AgentUpdate {
      issue_id: issue_id.clone(),
      update: payload,
    });
  }
}

/// Branch name for the per-issue worktree (SPEC_ADDENDUM_1 A.3.1). Uses issue number when present.
fn worktree_branch_name(identifier: &str) -> String {
  match parse_issue_number(identifier) {
    Some(n) => format!("symphony/issue-{}", n),
    None => format!("symphony/issue-{}", identifier.replace(['/', '#'], "_")),
  }
}

/// Run one worker to completion: ensure workspace (or worktree when configured), hooks, render prompt, run agent, send WorkerExit.
async fn run_worker_to_completion(
  config: ServiceConfig,
  prompt_template: String,
  issue_id: String,
  identifier: String,
  issue: Issue,
  retry_attempt: u32,
  tx: mpsc::UnboundedSender<OrchestratorMessage>,
) {
  let (path, created) = match config.workspace.main_repo_path.as_ref() {
    Some(main_repo) => {
      let branch = worktree_branch_name(&identifier);
      match ensure_worktree_dir(&config.workspace.root, &identifier, main_repo, &branch).await {
        Ok(p) => p,
        Err(e) => {
          warn!(%issue_id, %e, "ensure worktree failed");
          release_claim_and_send_exit(&tx, &issue_id, WorkerExitReason::Failed(e.to_string()), 0.0);
          return;
        }
      }
    }
    None => match ensure_workspace_dir(&config.workspace.root, &identifier).await {
      Ok(p) => p,
      Err(e) => {
        warn!(%issue_id, %e, "ensure workspace failed");
        release_claim_and_send_exit(&tx, &issue_id, WorkerExitReason::Failed(e.to_string()), 0.0);
        return;
      }
    },
  };

  if created {
    if let Some(ref script) = config.hooks.after_create {
      if let Err(e) = run_hook(script, &path, config.hooks.timeout_ms()).await {
        warn!(%issue_id, %e, "after_create hook failed");
      }
    }
  }

  if let Some(ref script) = config.hooks.before_run {
    if let Err(e) = run_hook(script, &path, config.hooks.timeout_ms()).await {
      warn!(%issue_id, %e, "before_run hook failed");
      release_claim_and_send_exit(&tx, &issue_id, WorkerExitReason::Failed(e.to_string()), 0.0);
      return;
    }
  }

  let prompt = match render_prompt(&prompt_template, &issue, Some(retry_attempt)) {
    Ok(p) => p,
    Err(e) => {
      warn!(%issue_id, %e, "render prompt failed");
      release_claim_and_send_exit(&tx, &issue_id, WorkerExitReason::Failed(e.to_string()), 0.0);
      return;
    }
  };

  let turn_timeout_ms = config.runner.turn_timeout_ms();
  let read_timeout_ms = config.runner.read_timeout_ms();
  let (update_tx, update_rx) = tokio::sync::mpsc::unbounded_channel::<AgentRunnerUpdate>();
  tokio::spawn(forward_agent_updates(
    update_rx,
    issue_id.clone(),
    tx.clone(),
  ));

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
    &issue.title,
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

  let _ = tx.send(OrchestratorMessage::WorkerExit {
    issue_id,
    reason,
    runtime_seconds,
    token_totals,
  });
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
  let issue_clone = issue.clone();

  let handle = tokio::spawn(run_worker_to_completion(
    config,
    prompt_template,
    issue_id.clone(),
    identifier,
    issue_clone,
    retry_attempt,
    tx,
  ));

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

  use symphony_config::{RunnerType, ServiceConfig};
  use symphony_domain::{Issue, OrchestratorState};
  use symphony_orchestration::OrchestratorMessage;
  use tokio::sync::{RwLock, mpsc};

  use symphony_domain::RunningEntry;

  use super::{log_tick_summary, run_orchestrator, run_worker_to_completion};
  use symphony_workspace::workspace_path;

  fn test_config() -> ServiceConfig {
    ServiceConfig {
      fix_pr: false,
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
        fix_pr_head_branch_pattern: None,
        mention_handle: None,
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
        main_repo_path: None,
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

  #[test]
  fn log_tick_summary_empty_state() {
    let state = OrchestratorState::default();
    log_tick_summary(&state, 0);
    log_tick_summary(&state, 5);
  }

  #[test]
  fn log_tick_summary_with_running_and_retry() {
    let mut state = OrchestratorState::default();
    state.running.insert(
      "1".into(),
      RunningEntry {
        identifier: "o/r#1".into(),
        issue: symphony_domain::Issue {
          id: "1".into(),
          identifier: "o/r#1".into(),
          title: "t".into(),
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
        session: symphony_domain::LiveSession::default(),
        started_at: chrono::Utc::now(),
        retry_attempt: 0,
      },
    );
    state.retry_attempts.insert(
      "2".into(),
      symphony_domain::RetryEntry {
        issue_id: "2".into(),
        identifier: "o/r#2".into(),
        attempt: 1,
        due_at_ms: 0,
        error: None,
      },
    );
    log_tick_summary(&state, 10);
  }

  /// When main_repo_path is set, the orchestrator creates a worktree and the worker process is run with cwd = worktree path.
  #[tokio::test]
  async fn run_worker_to_completion_uses_worktree_and_agent_cwd_is_worktree_path() {
    let root = std::env::temp_dir().join("symphony_runner_wt_test");
    let _ = tokio::fs::remove_dir_all(&root).await;
    let main_repo = root.join("main");
    tokio::fs::create_dir_all(&main_repo).await.unwrap();
    let out = tokio::process::Command::new("git")
      .args(["init"])
      .current_dir(&main_repo)
      .output()
      .await
      .unwrap();
    assert!(out.status.success(), "git init failed");

    let mut config = test_config();
    config.workspace.root = root.join("ws");
    config.workspace.main_repo_path = Some(main_repo.clone());
    config.runner.runner_type = RunnerType::Cli;
    config.runner.command =
      "sh -c 'pwd > agent_cwd.txt; echo \"{\\\"type\\\":\\\"result\\\",\\\"subtype\\\":\\\"success\\\"}\"'"
        .to_string();

    let identifier = "o/r#1";
    let issue = Issue {
      id: "1".into(),
      identifier: identifier.into(),
      title: "Test".into(),
      description: None,
      priority: None,
      state: "open".into(),
      branch_name: None,
      url: None,
      labels: vec![],
      blocked_by: vec![],
      created_at: None,
      updated_at: None,
    };

    let (tx, mut rx) = mpsc::unbounded_channel();
    run_worker_to_completion(
      config.clone(),
      "prompt".to_string(),
      "1".into(),
      identifier.into(),
      issue,
      0,
      tx,
    )
    .await;

    let msg = rx.recv().await.expect("WorkerExit");
    match &msg {
      OrchestratorMessage::WorkerExit { reason, .. } => {
        assert!(matches!(
          reason,
          symphony_orchestration::WorkerExitReason::Normal
        ));
      }
      _ => panic!("expected WorkerExit"),
    }

    let expected_path = workspace_path(&config.workspace.root, identifier);
    let cwd_file = expected_path.join("agent_cwd.txt");
    let cwd_content = tokio::fs::read_to_string(&cwd_file).await.unwrap();
    let reported_cwd = cwd_content.trim();
    let expected_canonical = expected_path.canonicalize().unwrap();
    let expected_str = expected_canonical.to_string_lossy();
    assert!(
      reported_cwd.ends_with("o_r_1") || reported_cwd == expected_str.as_ref(),
      "agent cwd should be worktree path: reported={:?} expected_path={:?}",
      reported_cwd,
      expected_path
    );

    let _ = tokio::fs::remove_dir_all(&root).await;
  }
}
