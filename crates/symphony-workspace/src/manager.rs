//! Workspace directory creation and hook execution (SPEC §9.2, §9.4).

use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;

use crate::path::workspace_path;

/// Error from workspace ensure-dir or hook execution.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
  #[error("failed to create workspace directory: {0}")]
  CreateDir(std::io::Error),

  #[error("hook execution failed: {0}")]
  Hook(String),

  #[error("hook timed out after {0}ms")]
  HookTimeout(u64),
}

/// Ensure the workspace directory exists. Creates it with `create_dir_all` if missing.
/// Returns `(path, true)` if the dir was just created, `(path, false)` if it already existed.
pub async fn ensure_workspace_dir(
  root: &Path,
  identifier: &str,
) -> Result<(PathBuf, bool), WorkspaceError> {
  let path = workspace_path(root, identifier);
  let existed = tokio::fs::metadata(&path)
    .await
    .map(|m| m.is_dir())
    .unwrap_or(false);
  if !existed {
    tokio::fs::create_dir_all(&path)
      .await
      .map_err(WorkspaceError::CreateDir)?;
    Ok((path, true))
  } else {
    Ok((path, false))
  }
}

/// Run a hook command with `sh -lc <script>` in the given `cwd`, with a timeout.
/// On timeout the child process is killed and `WorkspaceError::HookTimeout` is returned.
pub async fn run_hook(script: &str, cwd: &Path, timeout_ms: u64) -> Result<(), WorkspaceError> {
  let mut child = Command::new("sh")
    .args(["-lc", script])
    .current_dir(cwd)
    .spawn()
    .map_err(|e| WorkspaceError::Hook(e.to_string()))?;

  let dur = Duration::from_millis(timeout_ms);
  match timeout(dur, child.wait()).await {
    Ok(Ok(status)) => {
      if status.success() {
        Ok(())
      } else {
        Err(WorkspaceError::Hook(format!(
          "exit code {:?}",
          status.code()
        )))
      }
    }
    Ok(Err(e)) => Err(WorkspaceError::Hook(e.to_string())),
    Err(_) => {
      let _ = child.kill().await;
      Err(WorkspaceError::HookTimeout(timeout_ms))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[tokio::test]
  async fn ensure_workspace_dir_creates_new() {
    let root = std::env::temp_dir().join("symphony_ws_ensure_test");
    let _ = tokio::fs::remove_dir_all(&root).await;
    let (path, created) = ensure_workspace_dir(&root, "owner/repo#1").await.unwrap();
    assert!(created);
    assert!(path.is_dir());
    assert!(path.ends_with("owner_repo_1"));
    let (_, created2) = ensure_workspace_dir(&root, "owner/repo#1").await.unwrap();
    assert!(!created2);
    let _ = tokio::fs::remove_dir_all(&root).await;
  }

  #[tokio::test]
  async fn run_hook_success() {
    let cwd = std::env::temp_dir();
    let r = run_hook("echo ok", cwd.as_path(), 5000).await;
    assert!(r.is_ok());
  }

  #[tokio::test]
  async fn run_hook_exit_nonzero() {
    let cwd = std::env::temp_dir();
    let r = run_hook("exit 1", cwd.as_path(), 5000).await;
    assert!(matches!(r, Err(WorkspaceError::Hook(_))));
  }

  #[tokio::test]
  async fn run_hook_timeout() {
    let cwd = std::env::temp_dir();
    let r = run_hook("sleep 10", cwd.as_path(), 100).await;
    assert!(matches!(r, Err(WorkspaceError::HookTimeout(100))));
  }
}
