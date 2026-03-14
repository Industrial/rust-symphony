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

use std::path::{Path, PathBuf};
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

/// E2E: create a git repo, worktree on a branch, run command in sandbox with that worktree.
/// Verifies the full chain (worktree + branch + sandbox). Skips when env vars are not set.
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

  let tmp = std::env::temp_dir().join("symphony-e2e-sandbox-worktree");
  let _ = std::fs::remove_dir_all(&tmp);
  let main_repo = tmp.join("main");
  let worktree_path = tmp.join("worktree-e2e");
  std::fs::create_dir_all(&main_repo).expect("create main dir");

  // git init, first commit
  run_git(&main_repo, &["init"]).expect("git init");
  std::fs::write(main_repo.join("f.txt"), "e2e").expect("write f");
  run_git(&main_repo, &["add", "f.txt"]).expect("git add");
  run_git(&main_repo, &["commit", "-m", "initial"]).expect("git commit");

  // create worktree on branch symphony/issue-e2e
  let wt = worktree_path.to_str().expect("path is utf-8");
  run_git(
    &main_repo,
    &["worktree", "add", wt, "-b", "symphony/issue-e2e"],
  )
  .expect("worktree add");

  let config = symphony_sandbox::SandboxConfig {
    kernel_path: kernel_path.clone(),
    rootfs_path: rootfs_path.clone(),
    worktree_host_path: worktree_path.clone(),
    worktree_guest_path: PathBuf::from("/worktree"),
    vsock_port: 5001,
  };

  let mut child = symphony_sandbox::spawn(&config, "echo e2e-ok", &worktree_path)
    .await
    .expect("spawn");

  let mut stdout = child.stdout.take().expect("stdout");
  let mut buf = String::new();
  stdout.read_to_string(&mut buf).await.expect("read stdout");
  assert!(
    buf.contains("e2e-ok"),
    "stdout should contain 'e2e-ok', got: {:?}",
    buf
  );
  let status = child.wait().await.expect("wait");
  assert!(status.success(), "exit should be success: {:?}", status);

  let _ = std::fs::remove_dir_all(&tmp);
}

fn run_git(cwd: &Path, args: &[&str]) -> Result<std::process::Output, String> {
  let out = std::process::Command::new("git")
    .args(args)
    .current_dir(cwd)
    .output()
    .map_err(|e| e.to_string())?;
  if !out.status.success() {
    return Err(format!(
      "git {} failed: {}",
      args.join(" "),
      String::from_utf8_lossy(&out.stderr)
    ));
  }
  Ok(out)
}
