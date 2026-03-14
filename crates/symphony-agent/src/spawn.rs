//! Spawn agent process on host or inside Firecracker sandbox.

use std::path::Path;
use std::process::ExitStatus;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::process::Command;
use tokio::sync::oneshot;

use symphony_config::FirecrackerSandboxConfig;
use symphony_sandbox::{SandboxConfig, spawn as sandbox_spawn};

use crate::runner::AgentRunnerError;

/// Unified handle for an agent process (host or sandboxed).
/// Use take_stdin(), take_stdout(), take_stderr() then wait().
pub struct AgentProcessHandle {
  /// Process stdin; take to send input.
  pub stdin: Option<Box<dyn AsyncWrite + Send + Unpin>>,
  /// Process stdout; take to read output.
  pub stdout: Option<Box<dyn AsyncRead + Send + Unpin>>,
  /// Process stderr; take to read stderr.
  pub stderr: Option<Box<dyn AsyncRead + Send + Unpin>>,
  /// Receiver for process exit status (internal).
  wait_rx: oneshot::Receiver<Result<ExitStatus, AgentRunnerError>>,
}

impl AgentProcessHandle {
  /// Build a handle from boxed streams and a wait receiver (for CLI host path).
  pub(crate) fn from_parts(
    stdin: Option<Box<dyn AsyncWrite + Send + Unpin>>,
    stdout: Option<Box<dyn AsyncRead + Send + Unpin>>,
    stderr: Option<Box<dyn AsyncRead + Send + Unpin>>,
    wait_rx: oneshot::Receiver<Result<ExitStatus, AgentRunnerError>>,
  ) -> Self {
    tracing::trace!("AgentProcessHandle::from_parts");
    Self {
      stdin,
      stdout,
      stderr,
      wait_rx,
    }
  }

  /// Wait for the process to exit.
  pub async fn wait(&mut self) -> Result<ExitStatus, AgentRunnerError> {
    tracing::trace!("AgentProcessHandle::wait");
    let (_, placeholder) = oneshot::channel();
    let wait_rx = std::mem::replace(&mut self.wait_rx, placeholder);
    wait_rx
      .await
      .map_err(|_| AgentRunnerError::ProcessExit("process handle dropped".into()))?
  }
}

/// Spawn the agent process: on host (sh -lc command) or in Firecracker sandbox when config is Some.
pub async fn spawn_agent_process(
  command: &str,
  worktree_path: &Path,
  sandbox_config: Option<&FirecrackerSandboxConfig>,
) -> Result<AgentProcessHandle, AgentRunnerError> {
  tracing::trace!("spawn_agent_process");
  match sandbox_config {
    None => spawn_host(command, worktree_path).await,
    Some(fc) => spawn_sandbox(fc, command, worktree_path).await,
  }
}

/// Spawn the agent as a host process (sh -lc command) in the worktree directory.
async fn spawn_host(
  command: &str,
  worktree_path: &Path,
) -> Result<AgentProcessHandle, AgentRunnerError> {
  tracing::trace!("spawn_host");
  let mut child = Command::new("sh")
    .args(["-lc", command])
    .current_dir(worktree_path)
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .map_err(AgentRunnerError::Spawn)?;

  let stdin = child
    .stdin
    .take()
    .map(|s| Box::new(s) as Box<dyn AsyncWrite + Send + Unpin>);
  let stdout = child
    .stdout
    .take()
    .map(|s| Box::new(s) as Box<dyn AsyncRead + Send + Unpin>);
  let stderr = child
    .stderr
    .take()
    .map(|s| Box::new(s) as Box<dyn AsyncRead + Send + Unpin>);

  let (tx, wait_rx) = oneshot::channel();
  tokio::spawn(async move {
    let status = child
      .wait()
      .await
      .map_err(|e| AgentRunnerError::ProcessExit(e.to_string()));
    let _ = tx.send(status);
  });

  Ok(AgentProcessHandle {
    stdin,
    stdout,
    stderr,
    wait_rx,
  })
}

/// Spawn the agent inside the Firecracker sandbox using the given config.
async fn spawn_sandbox(
  fc: &FirecrackerSandboxConfig,
  command: &str,
  worktree_path: &Path,
) -> Result<AgentProcessHandle, AgentRunnerError> {
  tracing::trace!("spawn_sandbox");
  let config = SandboxConfig {
    kernel_path: fc.kernel_path.clone(),
    rootfs_path: fc.rootfs_path.clone(),
    worktree_host_path: worktree_path.to_path_buf(),
    worktree_guest_path: fc.worktree_guest_path.clone(),
    vsock_port: fc.vsock_port,
  };

  let mut child = sandbox_spawn(&config, command, worktree_path)
    .await
    .map_err(|e| AgentRunnerError::Spawn(std::io::Error::other(e.to_string())))?;

  let stdin = child
    .stdin
    .take()
    .map(|s| Box::new(s) as Box<dyn AsyncWrite + Send + Unpin>);
  let stdout = child
    .stdout
    .take()
    .map(|s| Box::new(s) as Box<dyn AsyncRead + Send + Unpin>);
  let stderr = child
    .stderr
    .take()
    .map(|s| Box::new(s) as Box<dyn AsyncRead + Send + Unpin>);

  let (tx, wait_rx) = oneshot::channel();
  tokio::spawn(async move {
    let status = child
      .wait()
      .await
      .map_err(|e| AgentRunnerError::ProcessExit(e.to_string()));
    let _ = tx.send(status);
  });

  Ok(AgentProcessHandle {
    stdin,
    stdout,
    stderr,
    wait_rx,
  })
}
