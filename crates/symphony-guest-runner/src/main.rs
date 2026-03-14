//! Guest agent-runner: vsock server that accepts one connection per run, reads JSON
//! {"command","cwd"}, runs the command, and proxies stdin/stdout/stderr/exit per protocol.
//!
//! Frame format (guest → host): [u8 tag][u32 len_be][bytes]; tag 1=stdout, 2=stderr, 3=exit (len 4, i32 be).
//! Host → guest: raw stdin bytes.

use std::io;
use std::process::Stdio;

use serde::Deserialize;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio_vsock::{VsockAddr, VsockListener};

/// VMADDR_CID_ANY: accept connections from any CID (host connects to guest).
const VMADDR_CID_ANY: u32 = 0xFFFF_FFFF;

const TAG_STDOUT: u8 = 1;
const TAG_STDERR: u8 = 2;
const TAG_EXIT: u8 = 3;

#[derive(Debug, Deserialize)]
struct RunRequest {
  command: String,
  cwd: String,
}

fn env_port() -> u32 {
  std::env::var("SYMPHONY_GUEST_VSOCK_PORT")
    .ok()
    .and_then(|s| s.parse().ok())
    .unwrap_or(5000)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let port = env_port();
  let addr = VsockAddr::new(VMADDR_CID_ANY, port);
  let listener = VsockListener::bind(addr).map_err(|e| io::Error::other(e))?;
  loop {
    let (stream, _remote) = listener.accept().await.map_err(|e| io::Error::other(e))?;
    if let Err(e) = handle_one(stream).await {
      eprintln!("handle_one error: {}", e);
    }
  }
}

async fn handle_one(
  stream: tokio_vsock::VsockStream,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
  let (read_half, write_half) = stream.into_split();
  let mut read_half = BufReader::new(read_half);

  let mut line = String::new();
  read_half.read_line(&mut line).await?;
  let req: RunRequest = serde_json::from_str(line.trim())?;

  let mut child = Command::new("/bin/sh")
    .args(["-c", &req.command])
    .current_dir(&req.cwd)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;

  let mut stdin = child
    .stdin
    .take()
    .ok_or_else(|| io::Error::other("no stdin"))?;
  let mut stdout = child
    .stdout
    .take()
    .ok_or_else(|| io::Error::other("no stdout"))?;
  let mut stderr = child
    .stderr
    .take()
    .ok_or_else(|| io::Error::other("no stderr"))?;

  let write_half = std::sync::Arc::new(tokio::sync::Mutex::new(write_half));

  let stdin_task = tokio::spawn(async move {
    let mut buf = [0u8; 8192];
    loop {
      let n = read_half.read(&mut buf).await?;
      if n == 0 {
        break;
      }
      stdin.write_all(&buf[..n]).await?;
    }
    Ok::<_, io::Error>(())
  });

  let wh = write_half.clone();
  let stdout_task = tokio::spawn(async move {
    let mut buf = [0u8; 8192];
    loop {
      let n = stdout.read(&mut buf).await?;
      if n == 0 {
        break;
      }
      send_frame(&mut *wh.lock().await, TAG_STDOUT, &buf[..n]).await?;
    }
    Ok::<_, io::Error>(())
  });

  let wh = write_half.clone();
  let stderr_task = tokio::spawn(async move {
    let mut buf = [0u8; 8192];
    loop {
      let n = stderr.read(&mut buf).await?;
      if n == 0 {
        break;
      }
      send_frame(&mut *wh.lock().await, TAG_STDERR, &buf[..n]).await?;
    }
    Ok::<_, io::Error>(())
  });

  let status = child.wait().await?;
  let code = status.code().unwrap_or(-1);
  let _ = stdin_task.await;
  let _ = stdout_task.await;
  let _ = stderr_task.await;
  // Give the host time to receive stdout/stderr frames before we send TAG_EXIT (avoids vsock reordering).
  tokio::time::sleep(std::time::Duration::from_millis(100)).await;
  send_frame(
    &mut *write_half.lock().await,
    TAG_EXIT,
    &(code as i32).to_be_bytes(),
  )
  .await?;
  // Keep connection open briefly so the host can read the exit frame before we drop the stream.
  tokio::time::sleep(std::time::Duration::from_millis(50)).await;
  Ok(())
}

/// Build frame bytes for guest→host: [tag][u32 len_be][payload]. Used by send_frame and tests.
fn frame_bytes(tag: u8, payload: &[u8]) -> Vec<u8> {
  let mut out = Vec::with_capacity(1 + 4 + payload.len());
  out.push(tag);
  out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
  out.extend_from_slice(payload);
  out
}

async fn send_frame<W: AsyncWriteExt + Unpin>(
  w: &mut W,
  tag: u8,
  payload: &[u8],
) -> io::Result<()> {
  w.write_all(&frame_bytes(tag, payload)).await?;
  w.flush().await?;
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn run_request_parses_json_line() {
    let json = r#"{"command":"echo hello","cwd":"/worktree"}"#;
    let req: RunRequest = serde_json::from_str(json).unwrap();
    assert_eq!(req.command, "echo hello");
    assert_eq!(req.cwd, "/worktree");
  }

  #[test]
  fn run_request_requires_command_and_cwd() {
    assert!(serde_json::from_str::<RunRequest>(r#"{}"#).is_err());
    assert!(serde_json::from_str::<RunRequest>(r#"{"command":"x"}"#).is_err());
    assert!(serde_json::from_str::<RunRequest>(r#"{"cwd":"/y"}"#).is_err());
  }

  #[test]
  fn frame_bytes_stdout_tag_and_length() {
    let buf = frame_bytes(TAG_STDOUT, b"hi");
    assert_eq!(buf.len(), 1 + 4 + 2);
    assert_eq!(buf[0], TAG_STDOUT);
    assert_eq!(u32::from_be_bytes(buf[1..5].try_into().unwrap()), 2u32);
    assert_eq!(&buf[5..], b"hi");
  }

  #[test]
  fn frame_bytes_stderr_tag() {
    let buf = frame_bytes(TAG_STDERR, b"err");
    assert_eq!(buf[0], TAG_STDERR);
    assert_eq!(u32::from_be_bytes(buf[1..5].try_into().unwrap()), 3u32);
  }

  #[test]
  fn frame_bytes_exit_code_i32_be() {
    let code: i32 = 42;
    let buf = frame_bytes(TAG_EXIT, &code.to_be_bytes());
    assert_eq!(buf[0], TAG_EXIT);
    assert_eq!(buf.len(), 1 + 4 + 4);
    let payload = &buf[5..9];
    assert_eq!(i32::from_be_bytes(payload.try_into().unwrap()), 42);
  }

  #[test]
  fn frame_bytes_empty_payload() {
    let buf = frame_bytes(TAG_STDOUT, &[]);
    assert_eq!(buf, vec![TAG_STDOUT, 0, 0, 0, 0]);
  }
}
