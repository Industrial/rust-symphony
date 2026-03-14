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
  /// When set, sending triggers VM shutdown (firecracker backend). Cleared when sent.
  pub(crate) shutdown_tx: Option<oneshot::Sender<()>>,
}

impl SandboxChild {
  /// Wait for the agent process inside the VM to exit.
  pub async fn wait(&mut self) -> std::result::Result<ExitStatus, SandboxError> {
    tracing::trace!("SandboxChild::wait");
    let result = match self.exit_rx.try_recv() {
      Ok(r) => r,
      Err(oneshot::error::TryRecvError::Closed) => return Err(SandboxError::VmExited),
      Err(oneshot::error::TryRecvError::Empty) => {
        let (_, placeholder) = oneshot::channel();
        let exit_rx = std::mem::replace(&mut self.exit_rx, placeholder);
        exit_rx.await.map_err(|_| SandboxError::VmExited)?
      }
    };
    if let Some(tx) = self.shutdown_tx.take() {
      let _ = tx.send(());
    }
    result
  }
}

impl Drop for SandboxChild {
  fn drop(&mut self) {
    if let Some(tx) = self.shutdown_tx.take() {
      let _ = tx.send(());
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

/// Firecracker microVM sandbox: VM lifecycle and vsock guest protocol.
#[cfg(feature = "firecracker")]
mod firecracker {
  use super::*;
  use std::os::unix::process::ExitStatusExt;
  use std::path::PathBuf;
  use std::process::ExitStatus;
  use std::time::{SystemTime, UNIX_EPOCH};
  use tokio::io::{AsyncReadExt, AsyncWriteExt};
  use tokio::net::UnixStream;
  use tokio::sync::mpsc;

  use fctools::{
    process_spawner::DirectProcessSpawner,
    runtime::tokio::TokioRuntime,
    vm::{
      Vm,
      configuration::{InitMethod, VmConfiguration, VmConfigurationData},
      models::{BootSource, Drive, MachineConfiguration, VsockDevice},
      shutdown::{VmShutdownAction, VmShutdownMethod},
    },
    vmm::{
      arguments::{VmmApiSocket, VmmArguments},
      executor::unrestricted::UnrestrictedVmmExecutor,
      installation::VmmInstallation,
      ownership::VmmOwnershipModel,
      resource::{MovedResourceType, ResourceType, system::ResourceSystem},
    },
  };

  type SandboxVm = Vm<UnrestrictedVmmExecutor, DirectProcessSpawner, TokioRuntime>;

  const GUEST_CID: u32 = 3;
  const TAG_STDOUT: u8 = 1;
  const TAG_STDERR: u8 = 2;
  const TAG_EXIT: u8 = 3;

  fn find_firecracker() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("SYMPHONY_FIRECRACKER") {
      let p = PathBuf::from(path.trim());
      if p.is_absolute() && p.exists() {
        return Some(p);
      }
      let out = std::process::Command::new("which")
        .arg(path.trim())
        .output()
        .ok()?;
      if out.status.success() {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() {
          return Some(PathBuf::from(s));
        }
      }
      return None;
    }
    let out = std::process::Command::new("which")
      .arg("firecracker")
      .output()
      .ok()?;
    if out.status.success() {
      let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
      if !s.is_empty() {
        return Some(PathBuf::from(s));
      }
    }
    None
  }

  fn temp_path(prefix: &str, suffix: &str) -> PathBuf {
    let nanos = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .unwrap_or_default()
      .as_nanos();
    PathBuf::from(format!("/tmp/{prefix}-{nanos}{suffix}"))
  }

  /// Demux reader: implements AsyncRead by receiving bytes from a channel (fed by the frame reader task).
  struct DemuxRead {
    rx: mpsc::UnboundedReceiver<Vec<u8>>,
    buf: Option<Vec<u8>>,
    offset: usize,
  }

  impl DemuxRead {
    fn new(rx: mpsc::UnboundedReceiver<Vec<u8>>) -> Self {
      Self {
        rx,
        buf: None,
        offset: 0,
      }
    }
  }

  impl tokio::io::AsyncRead for DemuxRead {
    fn poll_read(
      self: std::pin::Pin<&mut Self>,
      cx: &mut std::task::Context<'_>,
      buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
      let this = self.get_mut();
      loop {
        if let Some(ref b) = this.buf {
          let remain = &b[this.offset..];
          if !remain.is_empty() {
            let n = remain.len().min(buf.remaining());
            buf.put_slice(&remain[..n]);
            this.offset += n;
            if this.offset >= b.len() {
              this.buf = None;
              this.offset = 0;
            }
            return std::task::Poll::Ready(Ok(()));
          }
        }
        match this.rx.poll_recv(cx) {
          std::task::Poll::Ready(Some(chunk)) => {
            this.buf = Some(chunk);
            this.offset = 0;
          }
          std::task::Poll::Ready(None) => return std::task::Poll::Ready(Ok(())),
          std::task::Poll::Pending => return std::task::Poll::Pending,
        }
      }
    }
  }

  pub(super) async fn spawn_vm(
    config: &SandboxConfig,
    command: &str,
    _worktree_path: &Path,
  ) -> Result<SandboxChild, SandboxError> {
    tracing::trace!("spawn_vm");
    let firecracker_path = find_firecracker().ok_or_else(|| {
      SandboxError::Unavailable(
        "Firecracker binary not found. Set SYMPHONY_FIRECRACKER or ensure 'firecracker' is in PATH.".into(),
      )
    })?;
    let installation = VmmInstallation::new(
      firecracker_path.clone(),
      firecracker_path.clone(),
      firecracker_path,
    );

    let api_socket_path = temp_path("symphony-fc-api", ".sock");
    let vsock_uds_path = temp_path("symphony-fc-vsock", ".sock");

    let ownership = VmmOwnershipModel::Shared;
    let mut resource_system = ResourceSystem::new(DirectProcessSpawner, TokioRuntime, ownership);

    let kernel_resource = resource_system
      .create_resource(
        config.kernel_path.clone(),
        ResourceType::Moved(MovedResourceType::Copied),
      )
      .map_err(|e| SandboxError::VmStart(e.to_string()))?;
    let rootfs_resource = resource_system
      .create_resource(
        config.rootfs_path.clone(),
        ResourceType::Moved(MovedResourceType::Copied),
      )
      .map_err(|e| SandboxError::VmStart(e.to_string()))?;
    let worktree_resource = resource_system
      .create_resource(
        config.worktree_host_path.clone(),
        ResourceType::Moved(MovedResourceType::HardLinkedOrCopied),
      )
      .map_err(|e| SandboxError::VmStart(e.to_string()))?;
    let vsock_uds_resource = resource_system
      .create_resource(vsock_uds_path.clone(), ResourceType::Produced)
      .map_err(|e| SandboxError::VmStart(e.to_string()))?;

    let boot_source = BootSource {
      kernel_image: kernel_resource,
      boot_args: Some("console=ttyS0 reboot=k panic=1 pci=off".into()),
      initrd: None,
    };
    let drives = vec![
      Drive {
        drive_id: "rootfs".to_string(),
        is_root_device: true,
        cache_type: None,
        partuuid: None,
        is_read_only: Some(true),
        block: Some(rootfs_resource),
        rate_limiter: None,
        io_engine: None,
        socket: None,
      },
      Drive {
        drive_id: "worktree".to_string(),
        is_root_device: false,
        cache_type: None,
        partuuid: None,
        is_read_only: Some(false),
        block: Some(worktree_resource),
        rate_limiter: None,
        io_engine: None,
        socket: None,
      },
    ];
    let machine_config = MachineConfiguration {
      vcpu_count: 1,
      mem_size_mib: 128,
      smt: None,
      track_dirty_pages: Some(true),
      huge_pages: None,
    };
    let vsock_device = VsockDevice {
      guest_cid: GUEST_CID,
      uds: vsock_uds_resource,
    };
    let vm_data = VmConfigurationData {
      boot_source,
      drives,
      machine_configuration: machine_config,
      cpu_template: None,
      network_interfaces: vec![],
      balloon_device: None,
      vsock_device: Some(vsock_device),
      logger_system: None,
      metrics_system: None,
      mmds_configuration: None,
      entropy_device: None,
    };
    let vm_config = VmConfiguration::New {
      init_method: InitMethod::ViaApiCalls,
      data: vm_data,
    };

    let executor =
      UnrestrictedVmmExecutor::new(VmmArguments::new(VmmApiSocket::Enabled(api_socket_path)));

    let mut vm: SandboxVm = Vm::prepare(executor, resource_system, installation, vm_config)
      .await
      .map_err(|e: fctools::vm::VmError| SandboxError::VmStart(e.to_string()))?;

    vm.start(std::time::Duration::from_secs(30))
      .await
      .map_err(|e: fctools::vm::VmError| SandboxError::VmStart(e.to_string()))?;

    let vsock_path = vm
      .get_configuration()
      .get_data()
      .vsock_device
      .as_ref()
      .and_then(|v| v.uds.get_effective_path())
      .map(PathBuf::from)
      .ok_or_else(|| SandboxError::VsockConnect("vsock UDS path not available".into()))?;

    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;

    const VSOCK_CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);
    let mut stream = tokio::time::timeout(VSOCK_CONNECT_TIMEOUT, UnixStream::connect(&vsock_path))
      .await
      .map_err(|_| {
        SandboxError::VsockConnect("vsock connect timeout (guest may not be ready)".into())
      })?
      .map_err(|e: std::io::Error| SandboxError::VsockConnect(e.to_string()))?;

    let request = serde_json::json!({
      "command": command,
      "cwd": config.worktree_guest_path,
    });
    let line = format!("{}\n", request);
    stream
      .write_all(line.as_bytes())
      .await
      .map_err(|e: std::io::Error| SandboxError::Protocol(e.to_string()))?;
    stream
      .flush()
      .await
      .map_err(|e: std::io::Error| SandboxError::Protocol(e.to_string()))?;

    let (stdout_tx, stdout_rx) = mpsc::unbounded_channel();
    let (stderr_tx, stderr_rx) = mpsc::unbounded_channel();
    let (exit_tx, exit_rx) = oneshot::channel();

    let (mut read_half, write_half) = stream.into_split();

    tokio::spawn(async move {
      let mut buf = [0u8; 1 + 4];
      let mut exit_sent = false;
      loop {
        if read_half.read_exact(&mut buf).await.is_err() {
          if !exit_sent {
            let _ = exit_tx.send(Err(SandboxError::Protocol(
              "guest closed connection without exit frame".into(),
            )));
          }
          break;
        }
        let tag = buf[0];
        let len = u32::from_be_bytes(buf[1..5].try_into().unwrap()) as usize;
        let mut payload = vec![0u8; len];
        if len > 0 && read_half.read_exact(&mut payload).await.is_err() {
          if !exit_sent {
            let _ = exit_tx.send(Err(SandboxError::Protocol(
              "guest closed connection while reading frame payload".into(),
            )));
          }
          break;
        }
        match tag {
          TAG_STDOUT => {
            let _ = stdout_tx.send(payload);
          }
          TAG_STDERR => {
            let _ = stderr_tx.send(payload);
          }
          TAG_EXIT => {
            let code = if payload.len() >= 4 {
              i32::from_be_bytes(payload[0..4].try_into().unwrap())
            } else {
              -1
            };
            let raw = ((code as u32) & 0xff) << 8;
            let status = ExitStatus::from_raw(raw as i32);
            let _ = exit_tx.send(Ok(status));
            exit_sent = true;
            break;
          }
          _ => {}
        }
      }
    });

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let mut vm_guard = vm;
    tokio::spawn(async move {
      let _ = shutdown_rx.await;
      let timeout = std::time::Duration::from_secs(5);
      let _ = vm_guard
        .shutdown([
          VmShutdownAction {
            method: VmShutdownMethod::CtrlAltDel,
            timeout: Some(timeout),
            graceful: true,
          },
          VmShutdownAction {
            method: VmShutdownMethod::PauseThenKill,
            timeout: Some(timeout / 2),
            graceful: false,
          },
        ])
        .await;
      let _ = vm_guard.cleanup().await;
    });

    Ok(SandboxChild {
      stdin: Some(SandboxStdin::new(Box::new(write_half))),
      stdout: Some(SandboxStdout::new(Box::new(DemuxRead::new(stdout_rx)))),
      stderr: Some(SandboxStderr::new(Box::new(DemuxRead::new(stderr_rx)))),
      exit_rx,
      shutdown_tx: Some(shutdown_tx),
    })
  }
}
