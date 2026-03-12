//! Agent runner protocol types and subprocess runner (SPEC §10): NDJSON, handshake, turn loop.
//! Supports Codex-style protocol and ACP (Cursor).
//!
//! See `docs/09-agent-runner.md`.

mod acp;
mod protocol;
mod runner;
mod stream_cli;

pub use protocol::{AgentMessage, parse_line};
pub use runner::{
  AgentExitReason, AgentRunOutcome, AgentRunnerError, AgentRunnerUpdate, run_agent_codex,
};

/// Protocol to use when talking to the agent subprocess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerProtocol {
  /// Codex-style: initialize, thread/start, turn/start, turn/completed.
  Codex,
  /// ACP (Cursor): initialize, authenticate, session/new, session/prompt.
  Acp,
  /// Cursor CLI non-interactive: prompt as argument, parse stream-json until result/success.
  Cli,
}

/// Run the agent using the configured protocol (codex or acp).
#[allow(clippy::too_many_arguments)]
pub async fn run_agent_with_protocol(
  protocol: RunnerProtocol,
  command: &str,
  workspace_path: &std::path::Path,
  prompt: &str,
  issue_identifier: &str,
  issue_title: &str,
  turn_timeout_ms: u64,
  read_timeout_ms: u64,
  update_tx: Option<tokio::sync::mpsc::UnboundedSender<AgentRunnerUpdate>>,
) -> Result<AgentRunOutcome, AgentRunnerError> {
  match protocol {
    RunnerProtocol::Codex => {
      run_agent_codex(
        command,
        workspace_path,
        prompt,
        issue_identifier,
        issue_title,
        turn_timeout_ms,
        read_timeout_ms,
        update_tx,
      )
      .await
    }
    RunnerProtocol::Acp => {
      acp::run_agent_acp(
        command,
        workspace_path,
        prompt,
        issue_identifier,
        issue_title,
        turn_timeout_ms,
        read_timeout_ms,
        update_tx,
      )
      .await
    }
    RunnerProtocol::Cli => {
      stream_cli::run_agent_cli(
        command,
        workspace_path,
        prompt,
        issue_identifier,
        issue_title,
        turn_timeout_ms,
        read_timeout_ms,
        update_tx,
      )
      .await
    }
  }
}
