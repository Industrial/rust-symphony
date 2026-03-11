//! ACP (Agent Client Protocol) client for Cursor-style agents (session/new, session/prompt).
//! See https://cursor.com/docs/cli/acp and https://agentclientprotocol.com/

use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

use crate::runner::{AgentExitReason, AgentRunOutcome, AgentRunnerError, AgentRunnerUpdate};

/// Maximum line length when reading agent stdout (10 MiB per SPEC).
const MAX_LINE_LEN: usize = 10 * 1024 * 1024;

/// Run the agent using ACP: initialize → authenticate → session/new → session/prompt.
#[allow(clippy::too_many_arguments)]
/// Waits for session/prompt result (with stopReason); handles session/request_permission by allowing once.
pub async fn run_agent_acp(
  command: &str,
  workspace_path: &Path,
  prompt: &str,
  issue_identifier: &str,
  _issue_title: &str,
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

  fn log_line(direction: &str, line: &str) {
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
    log_line("send", s);
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

  async fn read_line_timed(
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
    log_line("recv", &line);
    if line.len() > MAX_LINE_LEN {
      return Err(AgentRunnerError::Handshake("line too long".into()));
    }
    Ok(line)
  }

  fn parse_json_line(line: &str) -> Option<Value> {
    let line = line.trim();
    if line.is_empty() {
      return None;
    }
    serde_json::from_str(line).ok()
  }

  let mut reader = BufReader::new(stdout).lines();
  let mut next_id: u64 = 1;

  // 1. initialize (JSON-RPC 2.0)
  let init_req = serde_json::json!({
    "jsonrpc": "2.0",
    "id": next_id,
    "method": "initialize",
    "params": {
      "protocolVersion": 1,
      "clientCapabilities": { "fs": { "readTextFile": false, "writeTextFile": false }, "terminal": false },
      "clientInfo": { "name": "symphony", "version": "1.0" }
    }
  });
  next_id += 1;
  write_line(&mut stdin, &serde_json::to_string(&init_req).unwrap()).await?;
  let line = read_line_timed(&mut reader, read_dur, "initialize response").await?;
  let init_resp = parse_json_line(&line)
    .ok_or_else(|| AgentRunnerError::Handshake("initialize: no JSON".into()))?;
  if init_resp
    .get("error")
    .and_then(|e| e.as_object())
    .map(|o| !o.is_empty())
    .unwrap_or(false)
  {
    return Err(AgentRunnerError::Handshake(format!(
      "initialize error: {:?}",
      init_resp.get("error")
    )));
  }

  // 2. authenticate
  let auth_req = serde_json::json!({
    "jsonrpc": "2.0",
    "id": next_id,
    "method": "authenticate",
    "params": { "methodId": "cursor_login" }
  });
  next_id += 1;
  write_line(&mut stdin, &serde_json::to_string(&auth_req).unwrap()).await?;
  let line = read_line_timed(&mut reader, read_dur, "authenticate response").await?;
  let auth_resp = parse_json_line(&line)
    .ok_or_else(|| AgentRunnerError::Handshake("authenticate: no JSON".into()))?;
  if auth_resp
    .get("error")
    .and_then(|e| e.as_object())
    .map(|o| !o.is_empty())
    .unwrap_or(false)
  {
    return Err(AgentRunnerError::Handshake(format!(
      "authenticate error: {:?}",
      auth_resp.get("error")
    )));
  }

  // 3. session/new
  let session_new_req = serde_json::json!({
    "jsonrpc": "2.0",
    "id": next_id,
    "method": "session/new",
    "params": { "cwd": cwd_abs, "mcpServers": [] }
  });
  next_id += 1;
  write_line(
    &mut stdin,
    &serde_json::to_string(&session_new_req).unwrap(),
  )
  .await?;
  let line = read_line_timed(&mut reader, read_dur, "session/new response").await?;
  let session_resp = parse_json_line(&line)
    .ok_or_else(|| AgentRunnerError::Handshake("session/new: no JSON".into()))?;
  let session_id = session_resp
    .get("result")
    .and_then(|r| r.get("sessionId"))
    .and_then(|v| v.as_str())
    .map(String::from)
    .ok_or_else(|| AgentRunnerError::Handshake("session/new: missing result.sessionId".into()))?;

  if let Some(ref tx) = update_tx {
    let _ = tx.send(AgentRunnerUpdate {
      session_id: Some(session_id.clone()),
      thread_id: Some(session_id.clone()),
      turn_id: Some("1".to_string()),
      turn_count: Some(1),
      ..Default::default()
    });
  }
  tracing::info!(
    session_id = %session_id,
    identifier = %issue_identifier,
    "agent session started (ACP), sending prompt"
  );

  // 4. session/prompt
  let prompt_req_id = next_id;
  let prompt_req = serde_json::json!({
    "jsonrpc": "2.0",
    "id": prompt_req_id,
    "method": "session/prompt",
    "params": {
      "sessionId": session_id,
      "prompt": [{ "type": "text", "text": prompt }]
    }
  });
  write_line(&mut stdin, &serde_json::to_string(&prompt_req).unwrap()).await?;

  // 5. Read until we get the session/prompt response (id match); handle session/request_permission and session/update on the way
  let exit_reason = loop {
    let line_result = timeout(turn_dur, reader.next_line()).await;
    match line_result {
      Ok(Ok(Some(line))) => {
        log_line("recv", &line);
        if line.len() > MAX_LINE_LEN {
          break AgentExitReason::ProcessError("line too long".into());
        }
        let v = match parse_json_line(&line) {
          Some(j) => j,
          None => continue,
        };
        let obj = match v.as_object() {
          Some(o) => o,
          None => continue,
        };
        // Response to a request (has id and result or error)
        if let Some(id) = obj.get("id").and_then(|i| i.as_u64()) {
          if id == prompt_req_id {
            if obj
              .get("error")
              .and_then(|e| e.as_object())
              .map(|o| !o.is_empty())
              .unwrap_or(false)
            {
              break AgentExitReason::TurnFailed;
            }
            let _stop_reason = obj
              .get("result")
              .and_then(|r| r.get("stopReason"))
              .and_then(|s| s.as_str());
            break AgentExitReason::Normal;
          }
        }
        // Notification: session/request_permission -> respond allow-once
        if obj.get("method").and_then(|m| m.as_str()) == Some("session/request_permission") {
          if let Some(notif_id) = obj.get("id").and_then(|i| i.as_u64()) {
            let response = serde_json::json!({
              "jsonrpc": "2.0",
              "id": notif_id,
              "result": { "outcome": { "outcome": "selected", "optionId": "allow-once" } }
            });
            let _ = write_line(&mut stdin, &serde_json::to_string(&response).unwrap()).await;
          }
        }
        // session/update: optional token/session updates; we could parse and forward
      }
      Ok(Ok(None)) => break AgentExitReason::ProcessError("stdout closed".into()),
      Ok(Err(e)) => break AgentExitReason::ProcessError(e.to_string()),
      Err(_) => {
        tracing::info!(identifier = %issue_identifier, "agent turn timed out (ACP)");
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
    token_totals: (0, 0, 0),
  })
}
