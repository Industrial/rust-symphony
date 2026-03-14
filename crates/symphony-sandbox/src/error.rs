//! Sandbox errors.

/// Errors from the sandbox (spawn, VM lifecycle, vsock, protocol).
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
  #[error("sandbox unavailable: {0}")]
  Unavailable(String),

  #[error("VM spawn or start failed: {0}")]
  VmStart(String),

  #[error("vsock connect failed: {0}")]
  VsockConnect(String),

  #[error("guest protocol error: {0}")]
  Protocol(String),

  #[error("VM exited unexpectedly")]
  VmExited,

  #[error("I/O: {0}")]
  Io(#[from] std::io::Error),
}
