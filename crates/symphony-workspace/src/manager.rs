//! Workspace directory creation, worktree creation, and hook execution (SPEC §9.2, §9.4).

use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;

use crate::path::workspace_path;

/// Error from workspace ensure-dir, worktree, or hook execution.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
  #[error("failed to create workspace directory: {0}")]
  CreateDir(std::io::Error),

  #[error("hook execution failed: {0}")]
  Hook(String),

  #[error("hook timed out after {0}ms")]
  HookTimeout(u64),

  #[error("main repo path is not a git repository: {0}")]
  MainRepoNotFound(PathBuf),

  #[error("git worktree add failed: {0}")]
  GitWorktreeAdd(String),

  #[error("workspace path exists but is not a git worktree: {0}")]
  PathExistsNotWorktree(PathBuf),
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

/// True if `path` is a git worktree (has a `.git` file containing "gitdir:").
fn is_git_worktree(path: &Path) -> bool {
  let git_file = path.join(".git");
  let meta = match std::fs::metadata(&git_file) {
    Ok(m) => m,
    Err(_) => return false,
  };
  if !meta.is_file() {
    return false;
  }
  let content = match std::fs::read_to_string(&git_file) {
    Ok(c) => c,
    Err(_) => return false,
  };
  content.trim_start().starts_with("gitdir:")
}

/// Ensure the per-issue path exists as a git worktree. Uses `main_repo_path` as the repo to run
/// `git worktree add <path> -b <branch_name>`. If the path already exists and is a worktree, returns
/// `(path, false)`. Returns `(path, true)` if the worktree was just created.
pub async fn ensure_worktree_dir(
  root: &Path,
  identifier: &str,
  main_repo_path: &Path,
  branch_name: &str,
) -> Result<(PathBuf, bool), WorkspaceError> {
  let path = workspace_path(root, identifier);
  if path.exists() {
    if is_git_worktree(&path) {
      return Ok((path, false));
    }
    return Err(WorkspaceError::PathExistsNotWorktree(path));
  }
  if let Some(parent) = path.parent() {
    tokio::fs::create_dir_all(parent)
      .await
      .map_err(WorkspaceError::CreateDir)?;
  }
  let main_meta = tokio::fs::metadata(main_repo_path)
    .await
    .map_err(|_| WorkspaceError::MainRepoNotFound(main_repo_path.to_path_buf()))?;
  if !main_meta.is_dir() {
    return Err(WorkspaceError::MainRepoNotFound(
      main_repo_path.to_path_buf(),
    ));
  }
  let git_dir = main_repo_path.join(".git");
  let git_exists = tokio::fs::metadata(&git_dir).await.is_ok();
  if !git_exists {
    return Err(WorkspaceError::MainRepoNotFound(
      main_repo_path.to_path_buf(),
    ));
  }
  let path_str = path.to_string_lossy();
  let output = Command::new("git")
    .args(["worktree", "add", path_str.as_ref(), "-b", branch_name])
    .current_dir(main_repo_path)
    .output()
    .await
    .map_err(|e| WorkspaceError::GitWorktreeAdd(e.to_string()))?;
  if !output.status.success() {
    let stderr = String::from_utf8_lossy(&output.stderr);
    return Err(WorkspaceError::GitWorktreeAdd(stderr.to_string()));
  }
  Ok((path, true))
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

  #[tokio::test]
  async fn ensure_worktree_dir_creates_worktree_and_path_under_root() {
    let root = std::env::temp_dir().join("symphony_wt_test");
    let _ = tokio::fs::remove_dir_all(&root).await;
    let main_repo = root.join("main");
    tokio::fs::create_dir_all(&main_repo).await.unwrap();
    let out = Command::new("git")
      .args(["init"])
      .current_dir(&main_repo)
      .output()
      .await
      .unwrap();
    assert!(out.status.success(), "git init failed");
    let (path, created) =
      ensure_worktree_dir(&root, "owner/repo#42", &main_repo, "symphony/issue-42")
        .await
        .unwrap();
    assert!(created);
    assert!(path.is_dir());
    assert!(path.starts_with(&root));
    assert!(path.ends_with("owner_repo_42"));
    assert!(is_git_worktree(&path));
    let branch_out = Command::new("git")
      .args(["branch", "--show-current"])
      .current_dir(&path)
      .output()
      .await
      .unwrap();
    let branch = String::from_utf8_lossy(&branch_out.stdout)
      .trim()
      .to_string();
    assert_eq!(branch, "symphony/issue-42");
    let _ = tokio::fs::remove_dir_all(&root).await;
  }

  #[tokio::test]
  async fn ensure_worktree_dir_idempotent() {
    let root = std::env::temp_dir().join("symphony_wt_idem");
    let _ = tokio::fs::remove_dir_all(&root).await;
    let main_repo = root.join("main");
    tokio::fs::create_dir_all(&main_repo).await.unwrap();
    Command::new("git")
      .args(["init"])
      .current_dir(&main_repo)
      .output()
      .await
      .unwrap();
    let (path1, created1) = ensure_worktree_dir(&root, "o/r#1", &main_repo, "symphony/issue-1")
      .await
      .unwrap();
    assert!(created1);
    let (path2, created2) = ensure_worktree_dir(&root, "o/r#1", &main_repo, "symphony/issue-1")
      .await
      .unwrap();
    assert!(!created2);
    assert_eq!(path1, path2);
    assert!(is_git_worktree(&path1));
    let _ = tokio::fs::remove_dir_all(&root).await;
  }

  #[tokio::test]
  async fn ensure_worktree_dir_main_repo_not_found() {
    let root = std::env::temp_dir().join("symphony_wt_norepo");
    let _ = tokio::fs::remove_dir_all(&root).await;
    let bad_main = root.join("nonexistent");
    let r = ensure_worktree_dir(&root, "o/r#1", &bad_main, "symphony/issue-1").await;
    assert!(matches!(r, Err(WorkspaceError::MainRepoNotFound(_))));
    let _ = tokio::fs::remove_dir_all(&root).await;
  }
}
