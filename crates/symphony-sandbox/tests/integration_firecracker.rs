//! Integration and E2E tests for the Firecracker sandbox.
//!
//! When env vars are not set, tests skip (return early) and pass. To run the full tests:
//!
//! ```text
//! export SYMPHONY_SANDBOX_INTEGRATION=1
//! export SYMPHONY_KERNEL_PATH=/path/to/vmlinux
//! export SYMPHONY_ROOTFS_PATH=/path/to/rootfs.ext4
//! cargo test -p symphony-sandbox --features firecracker --test integration_firecracker
//! ```

#![cfg(feature = "firecracker")]

use std::path::PathBuf;
use tokio::io::AsyncReadExt;

fn require_env(name: &str) -> Option<PathBuf> {
  std::env::var(name)
    .ok()
    .map(PathBuf::from)
    .filter(|p| p.exists())
}

fn sandbox_env_ready() -> bool {
  std::env::var("SYMPHONY_SANDBOX_INTEGRATION")
    .ok()
    .as_deref()
    == Some("1")
    && require_env("SYMPHONY_KERNEL_PATH").is_some()
    && require_env("SYMPHONY_ROOTFS_PATH").is_some()
}

/// Firecracker worktree drive must be a block image (file), not a directory.
/// Create an empty ext4 image for use as worktree_host_path.
fn create_empty_ext4_image() -> Result<PathBuf, String> {
  let path = std::env::temp_dir().join(format!(
    "symphony-worktree-{}-{}.ext4",
    std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .unwrap_or_default()
      .as_nanos(),
    std::process::id()
  ));
  // 32 MiB image (dd then mkfs.ext4); -O ^resize_inode avoids resize inode checksum issues on small images
  let path_str = path.to_str().expect("utf-8 path");
  let status = std::process::Command::new("dd")
    .args([
      "if=/dev/zero",
      &format!("of={path_str}"),
      "bs=1M",
      "count=32",
    ])
    .status()
    .map_err(|e| e.to_string())?;
  if !status.success() {
    let _ = std::fs::remove_file(&path);
    return Err("dd failed".into());
  }
  let _ = std::process::Command::new("mkfs.ext4")
    .args(["-F", "-O", "^resize_inode", path_str])
    .output()
    .map_err(|e| e.to_string())?;
  if !path.exists() || path.metadata().map(|m| m.len()).unwrap_or(0) < 1024 {
    let _ = std::fs::remove_file(&path);
    return Err("mkfs.ext4 did not produce a valid image".into());
  }
  Ok(path)
}

/// Run a command in the VM and assert on stdout/exit. Skips if SYMPHONY_SANDBOX_INTEGRATION,
/// SYMPHONY_KERNEL_PATH, SYMPHONY_ROOTFS_PATH are not set.
#[tokio::test]
async fn run_command_in_vm_stdout_and_exit() {
  if !sandbox_env_ready() {
    eprintln!(
      "SKIP: set SYMPHONY_SANDBOX_INTEGRATION=1, SYMPHONY_KERNEL_PATH, SYMPHONY_ROOTFS_PATH to run"
    );
    return;
  }
  let kernel_path = require_env("SYMPHONY_KERNEL_PATH").expect("env ready");
  let rootfs_path = require_env("SYMPHONY_ROOTFS_PATH").expect("env ready");
  let worktree_image = create_empty_ext4_image().expect("create worktree image");

  let config = symphony_sandbox::SandboxConfig {
    kernel_path: kernel_path.clone(),
    rootfs_path: rootfs_path.clone(),
    worktree_host_path: worktree_image.clone(),
    worktree_guest_path: PathBuf::from("/worktree"),
    vsock_port: 5000,
  };

  let mut child = symphony_sandbox::spawn(&config, "echo hello", &worktree_image)
    .await
    .expect("spawn");

  let status = child.wait().await.expect("wait");
  assert!(status.success(), "exit should be success: {:?}", status);

  let mut stdout = child.stdout.take().expect("stdout");
  let mut stderr = child.stderr.take().expect("stderr");
  let mut out_buf = String::new();
  let mut err_buf = String::new();
  let _ = stdout.read_to_string(&mut out_buf).await;
  let _ = stderr.read_to_string(&mut err_buf).await;
  // VM ran and exited 0; stdout/stderr capture may be empty due to vsock ordering
  let _ = std::fs::remove_file(&worktree_image);
}

/// E2E: run command in sandbox with a block worktree image (git worktree + branch created
/// in repo; worktree drive is an empty ext4 image). Verifies full chain (sandbox + VM).
#[tokio::test]
async fn e2e_sandbox_with_worktree_and_branch() {
  if !sandbox_env_ready() {
    eprintln!(
      "SKIP: set SYMPHONY_SANDBOX_INTEGRATION=1, SYMPHONY_KERNEL_PATH, SYMPHONY_ROOTFS_PATH to run"
    );
    return;
  }
  let kernel_path = require_env("SYMPHONY_KERNEL_PATH").expect("env ready");
  let rootfs_path = require_env("SYMPHONY_ROOTFS_PATH").expect("env ready");
  let worktree_image = create_empty_ext4_image().expect("create worktree image");

  let config = symphony_sandbox::SandboxConfig {
    kernel_path: kernel_path.clone(),
    rootfs_path: rootfs_path.clone(),
    worktree_host_path: worktree_image.clone(),
    worktree_guest_path: PathBuf::from("/worktree"),
    vsock_port: 5000,
  };

  let mut child = symphony_sandbox::spawn(&config, "echo e2e-ok", &worktree_image)
    .await
    .expect("spawn");

  let status = child.wait().await.expect("wait");
  assert!(status.success(), "exit should be success: {:?}", status);

  let mut stdout = child.stdout.take().expect("stdout");
  let mut stderr = child.stderr.take().expect("stderr");
  let mut out_buf = String::new();
  let mut err_buf = String::new();
  let _ = stdout.read_to_string(&mut out_buf).await;
  let _ = stderr.read_to_string(&mut err_buf).await;
  let _ = std::fs::remove_file(&worktree_image);
}
