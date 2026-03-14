//! Integration test: worktree create and path under root (SPEC §17.2).

use symphony_workspace::{ensure_worktree_dir, is_path_under_root, worktree_path};
use tokio::process::Command;

#[tokio::test]
async fn worktree_dir_create_and_path_under_root() {
  let dir = tempfile::tempdir().expect("tempdir");
  let root = dir.path();
  let main_repo = root.join("main");
  tokio::fs::create_dir_all(&main_repo)
    .await
    .expect("create main");
  let out = Command::new("git")
    .args(["init"])
    .current_dir(&main_repo)
    .output()
    .await
    .expect("git init");
  assert!(out.status.success(), "git init failed");

  let (path, created_now) =
    ensure_worktree_dir(root, "owner/repo#42", &main_repo, "symphony/issue-42")
      .await
      .expect("ensure_worktree_dir");
  assert!(created_now, "first create should report created");
  assert!(path.exists());
  assert!(path.is_dir());
  assert!(
    is_path_under_root(&path, root),
    "worktree path must be under root"
  );
  assert_eq!(
    path,
    worktree_path(root, "owner/repo#42"),
    "path must be deterministic"
  );

  let (path2, created_now2) =
    ensure_worktree_dir(root, "owner/repo#42", &main_repo, "symphony/issue-42")
      .await
      .expect("second ensure");
  assert!(
    !created_now2,
    "second call should reuse (created_now = false)"
  );
  assert_eq!(path, path2);
}
