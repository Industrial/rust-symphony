//! Symphony runner binary: load config, spawn orchestrator task and tick, run until shutdown.
//!
//! See docs/06-orchestration.md (Orchestrator Task Structure), docs/07-polling-scheduling.md.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{RwLock, mpsc};
use tracing::{info, warn};

use symphony_config::{ServiceConfig, from_workflow_config};
use symphony_domain::OrchestratorState;
use symphony_orchestration::OrchestratorMessage;
use symphony_tracker::fetch_issues_by_states;
use symphony_workflow::{load_workflow, resolve_workflow_path};
use symphony_workspace::workspace_path;

use crate::loop_::run_orchestrator;

mod loop_;

const WORKFLOW_RELOAD_POLL_SECS: u64 = 5;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  tracing_subscriber::fmt()
    .with_env_filter(
      tracing_subscriber::EnvFilter::from_default_env()
        .add_directive("symphony_runner=info".parse()?),
    )
    .init();

  let workflow_path_arg = std::env::args().nth(1).map(PathBuf::from);
  let resolved_path = resolve_workflow_path(workflow_path_arg.clone())?;
  let content = std::fs::read_to_string(&resolved_path)?;
  let definition = load_workflow(&content)?;
  let config = from_workflow_config(&definition.config)?;
  config.validate_dispatch()?;

  let poll_interval_ms = config.polling.interval_ms;
  let state = OrchestratorState {
    poll_interval_ms,
    max_concurrent_agents: config.agent.max_concurrent_agents,
    ..Default::default()
  };

  let workflow_state: Arc<RwLock<(ServiceConfig, String)>> = Arc::new(RwLock::new((
    config.clone(),
    definition.prompt_template.clone(),
  )));
  let initial_mtime = std::fs::metadata(&resolved_path)
    .ok()
    .and_then(|m| m.modified().ok());

  // Startup: remove workspace dirs for issues already in terminal state (cleanup from previous runs).
  if let Some(terminal) = config.tracker.terminal_states.as_deref() {
    if !terminal.is_empty() {
      let endpoint = config.tracker.endpoint_or_default();
      match fetch_issues_by_states(
        &endpoint,
        &config.tracker.api_key,
        &config.tracker.repo,
        terminal,
      )
      .await
      {
        Ok(issues) => {
          for issue in issues {
            let path = workspace_path(&config.workspace.root, &issue.identifier);
            if path.exists() {
              if let Err(e) = tokio::fs::remove_dir_all(&path).await {
                warn!(path = %path.display(), %e, "startup cleanup: failed to remove workspace");
              } else {
                info!(identifier = %issue.identifier, "startup cleanup: removed terminal workspace");
              }
            }
          }
        }
        Err(e) => warn!(%e, "startup cleanup: fetch terminal issues failed, continuing"),
      }
    }
  }

  let (tx, rx) = mpsc::unbounded_channel();
  let start = Instant::now();
  let worker_handles = std::collections::HashMap::new();

  let exit_on_failure = std::env::var("SYMPHONY_EXIT_ON_WORKER_FAILURE")
    .is_ok_and(|v| matches!(v.as_str(), "1" | "true" | "yes"));
  let (exit_tx, exit_rx) = if exit_on_failure {
    let (t, r) = tokio::sync::oneshot::channel();
    (Some(t), Some(r))
  } else {
    (None, None)
  };
  if exit_on_failure {
    info!(
      "SYMPHONY_EXIT_ON_WORKER_FAILURE=1: process will exit with code 1 on first worker failure"
    );
  }

  let workflow_state_orch = Arc::clone(&workflow_state);
  let orchestrator_handle = tokio::spawn(run_orchestrator(
    state,
    workflow_state_orch,
    rx,
    tx.clone(),
    start,
    worker_handles,
    exit_tx,
  ));

  // Optional: reload WORKFLOW.md on mtime change (keep last good config on error).
  let workflow_state_reload = Arc::clone(&workflow_state);
  let reload_handle = tokio::spawn(async move {
    let mut last_mtime = initial_mtime;
    loop {
      tokio::time::sleep(tokio::time::Duration::from_secs(WORKFLOW_RELOAD_POLL_SECS)).await;
      let path = match resolve_workflow_path(workflow_path_arg.clone()) {
        Ok(p) => p,
        Err(_) => continue,
      };
      let meta = match tokio::fs::metadata(&path).await {
        Ok(m) => m,
        Err(_) => continue,
      };
      let modified = match meta.modified() {
        Ok(t) => t,
        Err(_) => continue,
      };
      if Some(modified) != last_mtime {
        match std::fs::read_to_string(&path) {
          Ok(content) => match load_workflow(&content) {
            Ok(def) => match from_workflow_config(&def.config) {
              Ok(cfg) => {
                if cfg.validate_dispatch().is_ok() {
                  *workflow_state_reload.write().await = (cfg, def.prompt_template);
                  last_mtime = Some(modified);
                  info!("workflow reloaded");
                } else {
                  warn!("workflow reload: validation failed, keeping previous");
                }
              }
              Err(e) => warn!(%e, "workflow reload: config failed, keeping previous"),
            },
            Err(e) => warn!(%e, "workflow reload: parse failed, keeping previous"),
          },
          Err(e) => warn!(%e, "workflow reload: read failed"),
        }
      }
    }
  });

  let tick_tx = tx.clone();
  let tick_handle = tokio::spawn(async move {
    let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(poll_interval_ms));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
      interval.tick().await;
      if tick_tx.send(OrchestratorMessage::PollTick).is_err() {
        break;
      }
    }
  });

  info!(
    "symphony runner started (workflow loaded, poll_interval_ms={})",
    poll_interval_ms
  );

  if let Some(exit_rx) = exit_rx {
    tokio::select! {
      _ = tokio::signal::ctrl_c() => {
        info!("received ctrl-c, shutting down");
      }
      r = orchestrator_handle => {
        if let Err(e) = r {
          tracing::error!(%e, "orchestrator task panicked");
        }
      }
      code = exit_rx => {
        let code = code.unwrap_or(1);
        info!(%code, "exiting on worker failure (SYMPHONY_EXIT_ON_WORKER_FAILURE)");
        std::process::exit(code);
      }
    }
  } else {
    tokio::select! {
      _ = tokio::signal::ctrl_c() => {
        info!("received ctrl-c, shutting down");
      }
      r = orchestrator_handle => {
        if let Err(e) = r {
          tracing::error!(%e, "orchestrator task panicked");
        }
      }
    }
  }

  drop(tx);
  tick_handle.abort();
  reload_handle.abort();
  Ok(())
}
