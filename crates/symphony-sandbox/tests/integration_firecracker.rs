//! Integration test: run a command in the Firecracker sandbox and assert on stdout/exit.
//!
//! Run with: `cargo test -p symphony-sandbox --features firecracker --test integration_firecracker -- --ignored`
//! Requires: SYMPHONY_SANDBOX_INTEGRATION=1, SYMPHONY_KERNEL_PATH, SYMPHONY_ROOTFS_PATH,
//! and a worktree path (temp dir is used for cwd; guest worktree may be empty unless rootfs mounts it).

#![cfg(feature = "firecracker")]

use std::path::PathBuf;
use tokio::io::AsyncReadExt;

fn require_env(name: &str) -> Option<PathBuf> {
  std::env::var(name)
    .ok()
    .map(PathBuf::from)
    .filter(|p| p.exists())
}

/// Ignored by default; run with `cargo test -p symphony-sandbox --features firecracker --test integration_firecracker run_command_in_vm_stdout_and_exit -- --ignored`
#[tokio::test]
#[ignore]
async fn run_command_in_vm_stdout_and_exit() {
  if std::env::var("SYMPHONY_SANDBOX_INTEGRATION")
    .ok()
    .as_deref()
    != Some("1")
  {
    eprintln!("SKIP: set SYMPHONY_SANDBOX_INTEGRATION=1 to run");
    return;
  }
  let kernel_path = match require_env("SYMPHONY_KERNEL_PATH") {
    Some(p) => p,
    None => {
      eprintln!("SKIP: SYMPHONY_KERNEL_PATH not set or path missing");
      return;
    }
  };
  let rootfs_path = match require_env("SYMPHONY_ROOTFS_PATH") {
    Some(p) => p,
    None => {
      eprintln!("SKIP: SYMPHONY_ROOTFS_PATH not set or path missing");
      return;
    }
  };

  let worktree = std::env::temp_dir().join("symphony-sandbox-test-worktree");
  let _ = std::fs::create_dir_all(&worktree);

  let config = symphony_sandbox::SandboxConfig {
    kernel_path: kernel_path.clone(),
    rootfs_path: rootfs_path.clone(),
    worktree_host_path: worktree.clone(),
    worktree_guest_path: PathBuf::from("/worktree"),
    vsock_port: 5000,
  };

  let mut child = symphony_sandbox::spawn(&config, "echo hello", &worktree)
    .await
    .expect("spawn");

  let mut stdout = child.stdout.take().expect("stdout");
  let mut buf = String::new();
  stdout.read_to_string(&mut buf).await.expect("read stdout");
  assert!(
    buf.contains("hello"),
    "stdout should contain 'hello', got: {:?}",
    buf
  );

  let status = child.wait().await.expect("wait");
  assert!(status.success(), "exit should be success: {:?}", status);
}
