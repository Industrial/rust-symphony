//! Firecracker microVM sandbox for agent runs.
//!
//! When `runner.sandbox: firecracker` is set, the agent is run inside a Firecracker microVM
//! with the git worktree mounted. This crate provides a process-like handle (stdin/stdout/stderr,
//! exit status) so the existing agent protocol layer stays unchanged.
//!
//! # Required assets
//!
//! Operators must supply:
//! - **Kernel**: path to a Linux kernel (e.g. `vmlinux`) built for Firecracker.
//! - **Rootfs**: path to a rootfs image (e.g. ext4) that includes an **agent-runner** service
//!   listening on vsock (see below).
//!
//! # Guest agent-runner protocol
//!
//! The rootfs must run a service that:
//! 1. Listens on the configured vsock port (default 5000).
//! 2. Accepts one connection per run.
//! 3. Reads one JSON line: `{"command":"sh -lc ...","cwd":"/worktree"}`.
//! 4. Runs the command in the given `cwd` (worktree is mounted there).
//! 5. Proxies: host → stdin of the process; process stdout/stderr → host over framed messages.
//! 6. Sends exit code when the process exits.
//!
//! Frame format from guest to host: `[u8 tag][u32 len_be][bytes]` where tag 1 = stdout, 2 = stderr, 3 = exit (len 4, i32 exit code).
//! Host to guest: raw stdin bytes.

mod error;

pub use error::SandboxError;

use std::path::Path;
use std::process::ExitStatus;
use std::task::Poll;
use tokio::io::{AsyncBufRead, AsyncRead, AsyncWrite, BufReader};
use tokio::sync::oneshot;

/// Configuration for a Firecracker sandbox run.
/// Maps from workflow config (kernel_path, rootfs_path, worktree_guest_path, vsock_port) plus the host worktree path.
#[derive(Debug, Clone)]
pub struct SandboxConfig {
  /// Path to the kernel image (e.g. vmlinux).
  pub kernel_path: std::path::PathBuf,
  /// Path to the rootfs image.
  pub rootfs_path: std::path::PathBuf,
  /// Path on the host to the git worktree (mounted in the guest at worktree_guest_path).
  pub worktree_host_path: std::path::PathBuf,
  /// Path inside the guest where the worktree is mounted (e.g. /worktree).
  pub worktree_guest_path: std::path::PathBuf,
  /// Vsock port the guest agent-runner listens on.
  pub vsock_port: u32,
}

/// Process-like handle for an agent run inside the sandbox.
/// Provides stdin, stdout, stderr and wait() like `tokio::process::Child`;
/// use `.stdin.take()`, `.stdout.take()`, `.stderr.take()` then `.wait().await`.
pub struct SandboxChild {
  pub stdin: Option<SandboxStdin>,
  pub stdout: Option<SandboxStdout>,
  pub stderr: Option<SandboxStderr>,
  /// Receiver for exit status from the guest (filled when process exits).
  exit_rx: oneshot::Receiver<std::result::Result<ExitStatus, SandboxError>>,
}

impl SandboxChild {
  /// Wait for the agent process inside the VM to exit.
  pub async fn wait(&mut self) -> std::result::Result<ExitStatus, SandboxError> {
    tracing::trace!("SandboxChild::wait");
    match self.exit_rx.try_recv() {
      Ok(result) => result,
      Err(oneshot::error::TryRecvError::Closed) => Err(SandboxError::VmExited),
      Err(oneshot::error::TryRecvError::Empty) => {
        let (_, placeholder) = oneshot::channel();
        let exit_rx = std::mem::replace(&mut self.exit_rx, placeholder);
        exit_rx.await.map_err(|_| SandboxError::VmExited)?
      }
    }
  }
}

/// Stdin handle for the sandboxed process (implements AsyncWrite).
pub struct SandboxStdin {
  /// Underlying writer (vsock or similar); None if already taken.
  inner: Option<Box<dyn AsyncWrite + Unpin + Send>>,
}

impl SandboxStdin {
  /// Create from an async writer (used when building SandboxChild from VM/vsock).
  #[allow(dead_code)]
  pub(crate) fn new(w: Box<dyn AsyncWrite + Unpin + Send>) -> Self {
    tracing::trace!("SandboxStdin::new");
    Self { inner: Some(w) }
  }
}

impl AsyncWrite for SandboxStdin {
  fn poll_write(
    mut self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &[u8],
  ) -> std::task::Poll<std::result::Result<usize, std::io::Error>> {
    tracing::trace!("SandboxStdin::poll_write");
    match self.inner.as_mut() {
      Some(w) => std::pin::Pin::new(w).poll_write(cx, buf),
      None => std::task::Poll::Ready(Err(std::io::Error::new(
        std::io::ErrorKind::BrokenPipe,
        "stdin taken",
      ))),
    }
  }
  fn poll_flush(
    mut self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
    tracing::trace!("SandboxStdin::poll_flush");
    match self.inner.as_mut() {
      Some(w) => std::pin::Pin::new(w).poll_flush(cx),
      None => std::task::Poll::Ready(Ok(())),
    }
  }
  fn poll_shutdown(
    mut self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> std::task::Poll<std::result::Result<(), std::io::Error>> {
    tracing::trace!("SandboxStdin::poll_shutdown");
    match self.inner.as_mut() {
      Some(w) => std::pin::Pin::new(w).poll_shutdown(cx),
      None => std::task::Poll::Ready(Ok(())),
    }
  }
}

/// Stdout handle (implements AsyncRead / BufRead via inner).
pub struct SandboxStdout {
  /// Buffered reader over the guest stdout stream.
  inner: BufReader<Box<dyn AsyncRead + Unpin + Send>>,
}

impl SandboxStdout {
  /// Create from an async reader (used when building SandboxChild from VM/vsock).
  #[allow(dead_code)]
  pub(crate) fn new(r: Box<dyn AsyncRead + Unpin + Send>) -> Self {
    tracing::trace!("SandboxStdout::new");
    Self {
      inner: BufReader::new(r),
    }
  }
}

impl AsyncRead for SandboxStdout {
  fn poll_read(
    mut self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &mut tokio::io::ReadBuf<'_>,
  ) -> Poll<std::io::Result<()>> {
    tracing::trace!("SandboxStdout::poll_read");
    std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
  }
}

impl AsyncBufRead for SandboxStdout {
  fn poll_fill_buf(
    self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
  ) -> Poll<std::io::Result<&[u8]>> {
    tracing::trace!("SandboxStdout::poll_fill_buf");
    std::pin::Pin::new(&mut self.get_mut().inner).poll_fill_buf(cx)
  }
  fn consume(mut self: std::pin::Pin<&mut Self>, amt: usize) {
    tracing::trace!("SandboxStdout::consume");
    std::pin::Pin::new(&mut self.inner).consume(amt)
  }
}

/// Stderr handle (implements AsyncRead).
pub struct SandboxStderr {
  /// Buffered reader over the guest stderr stream.
  inner: BufReader<Box<dyn AsyncRead + Unpin + Send>>,
}

impl SandboxStderr {
  /// Create from an async reader (used when building SandboxChild from VM/vsock).
  #[allow(dead_code)]
  pub(crate) fn new(r: Box<dyn AsyncRead + Unpin + Send>) -> Self {
    tracing::trace!("SandboxStderr::new");
    Self {
      inner: BufReader::new(r),
    }
  }
}

impl AsyncRead for SandboxStderr {
  fn poll_read(
    mut self: std::pin::Pin<&mut Self>,
    cx: &mut std::task::Context<'_>,
    buf: &mut tokio::io::ReadBuf<'_>,
  ) -> std::task::Poll<std::io::Result<()>> {
    tracing::trace!("SandboxStderr::poll_read");
    std::pin::Pin::new(&mut self.inner).poll_read(cx, buf)
  }
}

/// Spawn the agent command inside a Firecracker microVM.
/// Returns a process-like handle; VM is torn down when the handle is dropped or wait() completes.
pub async fn spawn(
  config: &SandboxConfig,
  command: &str,
  _worktree_path: &Path,
) -> Result<SandboxChild, SandboxError> {
  tracing::trace!("spawn");
  let _ = (config, command);

  #[cfg(feature = "firecracker")]
  {
    crate::firecracker::spawn_vm(config, command, _worktree_path).await
  }

  #[cfg(not(feature = "firecracker"))]
  {
    Err(SandboxError::Unavailable(
      "symphony-sandbox was built without the 'firecracker' feature. \
       Enable it and provide kernel_path, rootfs_path, and a rootfs with the agent-runner vsock service. \
       See crate docs for the guest protocol."
        .into(),
    ))
  }
}

/// Firecracker microVM sandbox: VM lifecycle and vsock guest protocol (stub).
#[cfg(feature = "firecracker")]
mod firecracker {
  use super::*;

  /// Stub: full fctools VM lifecycle (ResourceSystem, VmmInstallation, Vm::prepare/start)
  /// and vsock connection to guest agent-runner is not yet implemented.
  /// Returns a clear error so callers can fall back or report.
  pub(super) async fn spawn_vm(
    _config: &SandboxConfig,
    _command: &str,
    _worktree_path: &Path,
  ) -> Result<SandboxChild, SandboxError> {
    tracing::trace!("spawn_vm");
    Err(SandboxError::Unavailable(
      "Firecracker VM lifecycle (fctools Vm::prepare/start, worktree drive, vsock to guest agent-runner) \
       is not yet implemented. Use runner.sandbox: none for host process. \
       See docs for required guest rootfs and protocol."
        .into(),
    ))
  }
}
