//! Cursor CLI non-interactive mode: prompt as argument, parse stream-json stdout.
//! Use when `agent acp` is not available (e.g. NixOS cursor-agent without acp subcommand).
//! See https://cursor.com/docs/cli/reference/output-format (stream-json).
//!
//! This is the **agent runner** for the "Cli" protocol, not the symphony binary's argv.

use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::time::timeout;

use crate::runner::{AgentExitReason, AgentRunOutcome, AgentRunnerError, AgentRunnerUpdate};

/// Maximum line length when reading agent stdout (10 MiB per SPEC).
const MAX_LINE_LEN: usize = 10 * 1024 * 1024;

/// Split command string into [program, arg1, arg2, ...] respecting double/single quotes (shell-like).
fn split_command(cmd: &str) -> Vec<&str> {
  let mut out = Vec::new();
  let mut rest = cmd.trim();
  while !rest.is_empty() {
    rest = rest.trim_start();
    if rest.is_empty() {
      break;
    }
    let (word, next) = if let Some(stripped) = rest.strip_prefix('"') {
      let end = stripped.find('"').unwrap_or(stripped.len());
      (&stripped[..end], stripped.get(end + 1..).unwrap_or(""))
    } else if let Some(stripped) = rest.strip_prefix('\'') {
      let end = stripped.find('\'').unwrap_or(stripped.len());
      (&stripped[..end], stripped.get(end + 1..).unwrap_or(""))
    } else {
      let pos = rest
        .find(|c: char| c.is_ascii_whitespace())
        .unwrap_or(rest.len());
      rest.split_at(pos)
    };
    out.push(word);
    rest = next;
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn split_command_simple() {
    let v = split_command("/run/current-system/sw/bin/cursor-agent --force --workspace .");
    assert_eq!(v[0], "/run/current-system/sw/bin/cursor-agent");
    assert_eq!(v[1], "--force");
    assert_eq!(v[2], "--workspace");
    assert_eq!(v[3], ".");
  }

  #[test]
  fn split_command_single_word() {
    let v = split_command("cursor-agent");
    assert_eq!(v.len(), 1);
    assert_eq!(v[0], "cursor-agent");
  }

  #[test]
  fn split_command_quoted_arg() {
    let v = split_command(r#"cursor-agent --workspace "/path with spaces""#);
    assert_eq!(v[0], "cursor-agent");
    assert_eq!(v[1], "--workspace");
    assert_eq!(v[2], "/path with spaces");
  }
}

/// Run the agent in Cursor CLI non-interactive mode: pass prompt as argument, read stream-json from stdout.
/// Command string is split into argv (respecting quotes); prompt is appended as the last argument.
/// Success: we see a line with `type: "result", subtype: "success"`.
#[allow(clippy::too_many_arguments)]
pub async fn run_agent_cli(
  command: &str,
  workspace_path: &Path,
  prompt: &str,
  issue_identifier: &str,
  _issue_title: &str,
  turn_timeout_ms: u64,
  _read_timeout_ms: u64,
  update_tx: Option<tokio::sync::mpsc::UnboundedSender<AgentRunnerUpdate>>,
) -> Result<AgentRunOutcome, AgentRunnerError> {
  let start = std::time::Instant::now();

  let argv = split_command(command);
  let (program, args) = argv
    .split_first()
    .ok_or_else(|| AgentRunnerError::Handshake("runner.command is empty".into()))?;

  let mut child = Command::new(program)
    .args(args)
    .arg(prompt)
    .current_dir(workspace_path)
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .map_err(AgentRunnerError::Spawn)?;

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

  fn parse_json_line(line: &str) -> Option<Value> {
    let line = line.trim();
    if line.is_empty() {
      return None;
    }
    serde_json::from_str(line).ok()
  }

  let mut reader = BufReader::new(stdout).lines();
  let mut session_id: Option<String> = None;

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
        let typ = obj.get("type").and_then(|t| t.as_str());
        let subtype = obj.get("subtype").and_then(|s| s.as_str());
        if session_id.is_none() {
          if let Some(sid) = obj.get("session_id").and_then(|s| s.as_str()) {
            session_id = Some(sid.to_string());
            if let Some(ref tx) = update_tx {
              let _ = tx.send(AgentRunnerUpdate {
                session_id: Some(sid.to_string()),
                thread_id: Some(sid.to_string()),
                turn_id: Some("1".to_string()),
                turn_count: Some(1),
                ..Default::default()
              });
            }
            tracing::info!(
              session_id = %sid,
              identifier = %issue_identifier,
              "agent session started (CLI stream-json)"
            );
          }
        }
        if typ == Some("result") && subtype == Some("success") {
          break AgentExitReason::Normal;
        }
        if typ == Some("result")
          && (subtype != Some("success")
            || obj
              .get("is_error")
              .and_then(|b| b.as_bool())
              .unwrap_or(false))
        {
          break AgentExitReason::TurnFailed;
        }
      }
      Ok(Ok(None)) => break AgentExitReason::ProcessError("stdout closed".into()),
      Ok(Err(e)) => break AgentExitReason::ProcessError(e.to_string()),
      Err(_) => {
        tracing::info!(identifier = %issue_identifier, "agent turn timed out (CLI)");
        break AgentExitReason::TurnTimeout;
      }
    }
  };

  // Cursor CLI can occasionally fail to terminate after emitting result (see cursor/cursor#3588)
  let status = match timeout(Duration::from_secs(10), child.wait()).await {
    Ok(Ok(s)) => s,
    Ok(Err(e)) => return Err(AgentRunnerError::ProcessExit(e.to_string())),
    Err(_) => {
      let _ = child.kill().await;
      if exit_reason == AgentExitReason::Normal {
        return Ok(AgentRunOutcome {
          exit_reason,
          runtime_seconds: start.elapsed().as_secs_f64(),
          token_totals: (0, 0, 0),
        });
      }
      return Err(AgentRunnerError::ProcessExit(
        "process did not exit within 10s".into(),
      ));
    }
  };
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
