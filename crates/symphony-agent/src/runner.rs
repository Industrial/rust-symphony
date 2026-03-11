//! Spawn agent subprocess, handshake, and turn loop (SPEC §10.1–10.3).

use std::path::Path;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

use crate::protocol::{AgentMessage, parse_line};

/// Outcome of a single agent run (one turn for now).
#[derive(Debug, Clone)]
pub struct AgentRunOutcome {
  pub exit_reason: AgentExitReason,
  pub runtime_seconds: f64,
  pub token_totals: (u64, u64, u64),
}

/// Why the agent run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentExitReason {
  Normal,
  TurnTimeout,
  TurnFailed,
  TurnCancelled,
  ResponseTimeout,
  ProcessError(String),
}

/// Update emitted during the run (session ids, tokens). Map to orchestrator AgentUpdatePayload.
#[derive(Debug, Clone, Default)]
pub struct AgentRunnerUpdate {
  pub session_id: Option<String>,
  pub thread_id: Option<String>,
  pub turn_id: Option<String>,
  pub input_tokens: Option<u64>,
  pub output_tokens: Option<u64>,
  pub total_tokens: Option<u64>,
  pub turn_count: Option<u32>,
}

/// Error from runner (spawn, I/O, timeout).
#[derive(Debug, thiserror::Error)]
pub enum AgentRunnerError {
  #[error("spawn failed: {0}")]
  Spawn(std::io::Error),

  #[error("stdin taken")]
  StdinTaken,

  #[error("stdout taken")]
  StdoutTaken,

  #[error("write failed: {0}")]
  Write(String),

  #[error("read timeout")]
  ReadTimeout,

  #[error("handshake failed: {0}")]
  Handshake(String),

  #[error("process exit: {0}")]
  ProcessExit(String),
}

/// Maximum line length when reading agent stdout (10 MiB per SPEC).
const MAX_LINE_LEN: usize = 10 * 1024 * 1024;

/// Run the agent (Codex-style): spawn, handshake (initialize, thread/start, turn/start), then read until
/// turn/completed, turn/failed, turn/cancelled, or turn timeout. Stderr is logged in a background task.
/// `update_tx`: if provided, send updates when we have thread_id/turn_id.
#[allow(clippy::too_many_arguments)]
pub async fn run_agent_codex(
  command: &str,
  workspace_path: &Path,
  prompt: &str,
  issue_identifier: &str,
  issue_title: &str,
  turn_timeout_ms: u64,
  read_timeout_ms: u64,
  update_tx: Option<tokio::sync::mpsc::UnboundedSender<AgentRunnerUpdate>>,
) -> Result<AgentRunOutcome, AgentRunnerError> {
  let start = std::time::Instant::now();
  let cwd_abs = workspace_path
    .canonicalize()
    .map_err(|e| AgentRunnerError::Handshake(e.to_string()))?
    .to_string_lossy()
    .to_string();

  let mut child = Command::new("sh")
    .args(["-lc", command])
    .current_dir(workspace_path)
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .map_err(AgentRunnerError::Spawn)?;

  let mut stdin = child.stdin.take().ok_or(AgentRunnerError::StdinTaken)?;
  let stdout = child.stdout.take().ok_or(AgentRunnerError::StdoutTaken)?;

  // Stderr: log lines in background
  let stderr = child.stderr.take();
  if let Some(stderr) = stderr {
    tokio::spawn(async move {
      let mut reader = BufReader::new(stderr);
      let mut line = String::new();
      while reader.read_line(&mut line).await.is_ok() && !line.is_empty() {
        tracing::debug!(agent_stderr = %line.trim());
        line.clear();
      }
    });
  }

  let read_dur = Duration::from_millis(read_timeout_ms);
  let turn_dur = Duration::from_millis(turn_timeout_ms);

  fn log_agent_line(direction: &str, line: &str) {
    const MAX_LOG: usize = 800;
    let s = if line.len() > MAX_LOG {
      format!("{}... ({} bytes)", &line[..MAX_LOG], line.len())
    } else {
      line.to_string()
    };
    tracing::debug!(agent_direction = direction, agent_line = %s);
  }

  async fn write_line(
    stdin: &mut tokio::process::ChildStdin,
    s: &str,
  ) -> Result<(), AgentRunnerError> {
    log_agent_line("send", s);
    use tokio::io::AsyncWriteExt;
    stdin
      .write_all(s.as_bytes())
      .await
      .map_err(|e| AgentRunnerError::Write(e.to_string()))?;
    stdin
      .write_all(b"\n")
      .await
      .map_err(|e| AgentRunnerError::Write(e.to_string()))?;
    stdin
      .flush()
      .await
      .map_err(|e| AgentRunnerError::Write(e.to_string()))?;
    Ok(())
  }

  async fn read_line_timeout(
    reader: &mut tokio::io::Lines<BufReader<tokio::process::ChildStdout>>,
    dur: Duration,
    step: &str,
  ) -> Result<String, AgentRunnerError> {
    let line = timeout(dur, reader.next_line())
      .await
      .map_err(|_| {
        AgentRunnerError::Handshake(format!(
          "read timeout ({}ms) waiting for {}",
          dur.as_millis(),
          step
        ))
      })?
      .map_err(|e| AgentRunnerError::Write(e.to_string()))?
      .unwrap_or_default();
    log_agent_line("recv", &line);
    if line.len() > MAX_LINE_LEN {
      return Err(AgentRunnerError::Handshake("line too long".into()));
    }
    Ok(line)
  }

  let mut reader = BufReader::new(stdout).lines();

  // 1. initialize
  let init = serde_json::json!({
    "id": 1,
    "method": "initialize",
    "params": {
      "clientInfo": { "name": "symphony", "version": "1.0" },
      "capabilities": {}
    }
  });
  write_line(&mut stdin, &serde_json::to_string(&init).unwrap()).await?;
  let line = read_line_timeout(&mut reader, read_dur, "initialize response").await?;
  let msg = parse_line(&line);
  match &msg {
    AgentMessage::Response { id, error, .. } => {
      if *id != Some(1) {
        return Err(AgentRunnerError::Handshake("expected id 1".into()));
      }
      if error.is_some() {
        return Err(AgentRunnerError::Handshake(format!(
          "initialize error: {:?}",
          error
        )));
      }
    }
    _ => {
      return Err(AgentRunnerError::Handshake(
        "expected initialize response".into(),
      ));
    }
  }

  // 2. initialized notification
  let initialized = serde_json::json!({ "method": "initialized", "params": {} });
  write_line(&mut stdin, &serde_json::to_string(&initialized).unwrap()).await?;

  // 3. thread/start
  let thread_start = serde_json::json!({
    "id": 2,
    "method": "thread/start",
    "params": { "cwd": cwd_abs }
  });
  write_line(&mut stdin, &serde_json::to_string(&thread_start).unwrap()).await?;
  let line = read_line_timeout(&mut reader, read_dur, "thread/start response").await?;
  let msg = parse_line(&line);
  let thread_id = match &msg {
    AgentMessage::Response {
      id, result, error, ..
    } => {
      if *id != Some(2) {
        return Err(AgentRunnerError::Handshake("expected id 2".into()));
      }
      if error.is_some() {
        return Err(AgentRunnerError::Handshake("thread/start error".into()));
      }
      result
        .as_ref()
        .and_then(|r| r.get("thread"))
        .and_then(|t| t.get("id"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .ok_or_else(|| AgentRunnerError::Handshake("missing thread.id".into()))?
    }
    _ => {
      return Err(AgentRunnerError::Handshake(
        "expected thread/start response".into(),
      ));
    }
  };

  if let Some(ref tx) = update_tx {
    let _ = tx.send(AgentRunnerUpdate {
      thread_id: Some(thread_id.clone()),
      ..Default::default()
    });
  }

  // 4. turn/start
  let turn_title = format!("{}: {}", issue_identifier, issue_title);
  let turn_start = serde_json::json!({
    "id": 3,
    "method": "turn/start",
    "params": {
      "threadId": thread_id,
      "input": [{ "type": "text", "text": prompt }],
      "cwd": cwd_abs,
      "title": turn_title
    }
  });
  write_line(&mut stdin, &serde_json::to_string(&turn_start).unwrap()).await?;
  let line = read_line_timeout(&mut reader, read_dur, "turn/start response").await?;
  let msg = parse_line(&line);
  let turn_id = match &msg {
    AgentMessage::Response {
      id, result, error, ..
    } => {
      if *id != Some(3) {
        return Err(AgentRunnerError::Handshake("expected id 3".into()));
      }
      if error.is_some() {
        return Err(AgentRunnerError::Handshake("turn/start error".into()));
      }
      result
        .as_ref()
        .and_then(|r| r.get("turn"))
        .and_then(|t| t.get("id"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| "unknown".to_string())
    }
    _ => "unknown".to_string(),
  };

  let session_id = format!("{}-{}", thread_id, turn_id);
  tracing::info!(
    session_id = %session_id,
    identifier = %issue_identifier,
    "agent session started, turn in progress"
  );
  if let Some(ref tx) = update_tx {
    let _ = tx.send(AgentRunnerUpdate {
      session_id: Some(session_id.clone()),
      thread_id: Some(thread_id),
      turn_id: Some(turn_id),
      turn_count: Some(1),
      ..Default::default()
    });
  }

  // 5. Turn loop: read until turn/completed, turn/failed, turn/cancelled or timeout
  let exit_reason = loop {
    let line_result = timeout(turn_dur, reader.next_line()).await;
    match line_result {
      Ok(Ok(Some(line))) => {
        log_agent_line("recv", &line);
        if line.len() > MAX_LINE_LEN {
          break AgentExitReason::ProcessError("line too long".into());
        }
        let msg = parse_line(&line);
        if let AgentMessage::Notification { method, params } = &msg {
          if method == "turn/completed" {
            tracing::info!(identifier = %issue_identifier, "agent turn completed");
            break AgentExitReason::Normal;
          }
          if method == "turn/failed" {
            tracing::info!(identifier = %issue_identifier, "agent turn failed");
            break AgentExitReason::TurnFailed;
          }
          if method == "turn/cancelled" {
            tracing::info!(identifier = %issue_identifier, "agent turn cancelled");
            break AgentExitReason::TurnCancelled;
          }
          // Optional: extract token counts from params and send update
          if let Some(ref p) = params {
            if p.get("inputTokens").or(p.get("outputTokens")).is_some() {
              let input_tokens = p.get("inputTokens").and_then(|v| v.as_u64());
              let output_tokens = p.get("outputTokens").and_then(|v| v.as_u64());
              let total = input_tokens
                .zip(output_tokens)
                .map(|(i, o)| i + o)
                .or_else(|| p.get("totalTokens").and_then(|v| v.as_u64()));
              if let Some(ref tx) = update_tx {
                let _ = tx.send(AgentRunnerUpdate {
                  input_tokens,
                  output_tokens: output_tokens.or(total),
                  total_tokens: total,
                  ..Default::default()
                });
              }
            }
          }
        }
      }
      Ok(Ok(None)) => break AgentExitReason::ProcessError("stdout closed".into()),
      Ok(Err(e)) => break AgentExitReason::ProcessError(e.to_string()),
      Err(_) => {
        tracing::info!(identifier = %issue_identifier, "agent turn timed out");
        break AgentExitReason::TurnTimeout;
      }
    }
  };

  let _ = stdin.shutdown().await;
  let status = child
    .wait()
    .await
    .map_err(|e| AgentRunnerError::ProcessExit(e.to_string()))?;
  if !status.success() {
    return Err(AgentRunnerError::ProcessExit(format!(
      "exit code {:?}",
      status.code()
    )));
  }

  let runtime_seconds = start.elapsed().as_secs_f64();
  Ok(AgentRunOutcome {
    exit_reason,
    runtime_seconds,
    token_totals: (0, 0, 0), // TODO: from last update if needed
  })
}
