//! Symphony runner binary: load config, spawn orchestrator task and tick, run until shutdown.
//!
//! See docs/06-orchestration.md (Orchestrator Task Structure), docs/07-polling-scheduling.md.

use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{RwLock, mpsc};
use tracing::info;

use clap::Parser;
use symphony_config::{ServiceConfig, from_workflow_config};
use symphony_domain::OrchestratorState;
use symphony_orchestration::OrchestratorMessage;
use symphony_workflow::{load_workflow, resolve_workflow_path};

use crate::cli::Cli;
use crate::loop_::{dry_run_one_poll, run_orchestrator};
use crate::reload::spawn_workflow_reload_task;
use crate::startup::run_startup_cleanup;

mod cli;
mod loop_;
mod reload;
mod startup;

const WORKFLOW_RELOAD_POLL_SECS: u64 = 5;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  tracing_subscriber::fmt()
    .with_env_filter(
      tracing_subscriber::EnvFilter::from_default_env()
        .add_directive("symphony_runner=info".parse()?),
    )
    .init();

  let cli = Cli::parse();

  let resolved_path = resolve_workflow_path(cli.workflow_path.clone())?;
  let content = std::fs::read_to_string(&resolved_path)?;
  let definition = load_workflow(&content)?;
  let config = from_workflow_config(&definition.config)?;
  config.validate_dispatch()?;

  if cli.dry_run {
    dry_run_one_poll(&config).await?;
    return Ok(());
  }

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

  run_startup_cleanup(&config).await;

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

  let reload_handle = spawn_workflow_reload_task(
    Arc::clone(&workflow_state),
    cli.workflow_path,
    WORKFLOW_RELOAD_POLL_SECS,
  );

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
