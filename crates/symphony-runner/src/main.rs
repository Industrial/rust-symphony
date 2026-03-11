//! Symphony runner binary: load config, spawn orchestrator task and tick, run until shutdown.
//!
//! See docs/06-orchestration.md (Orchestrator Task Structure), docs/07-polling-scheduling.md.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc;
use tracing::info;

use symphony_config::from_workflow_config;
use symphony_domain::OrchestratorState;
use symphony_orchestration::OrchestratorMessage;
use symphony_workflow::load_workflow_file;

use crate::loop_::run_orchestrator;

mod loop_;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  tracing_subscriber::fmt()
    .with_env_filter(
      tracing_subscriber::EnvFilter::from_default_env()
        .add_directive("symphony_runner=info".parse()?),
    )
    .init();

  let workflow_path = std::env::args().nth(1).map(PathBuf::from);
  let definition = load_workflow_file(workflow_path)?;
  let config = Arc::new(from_workflow_config(&definition.config)?);
  config.validate_dispatch()?;

  let poll_interval_ms = config.polling.interval_ms;
  let state = OrchestratorState {
    poll_interval_ms,
    max_concurrent_agents: config.agent.max_concurrent_agents,
    ..Default::default()
  };

  let (tx, rx) = mpsc::unbounded_channel();
  let start = Instant::now();
  let worker_handles = std::collections::HashMap::new();

  let orchestrator_handle = tokio::spawn(run_orchestrator(
    state,
    Arc::clone(&config),
    rx,
    tx.clone(),
    start,
    worker_handles,
  ));

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

  drop(tx);
  tick_handle.abort();
  Ok(())
}
