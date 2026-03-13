//! Integration test: worktree create and path under root (SPEC §17.2).

use symphony_workspace::{ensure_worktree_plain_dir, is_path_under_root, worktree_path};

#[tokio::test]
async fn worktree_plain_dir_create_and_path_under_root() {
  let dir = tempfile::tempdir().expect("tempdir");
  let root = dir.path();

  let (path, created_now) = ensure_worktree_plain_dir(root, "owner/repo#42")
    .await
    .expect("ensure_worktree_plain_dir");
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

  let (path2, created_now2) = ensure_worktree_plain_dir(root, "owner/repo#42")
    .await
    .expect("second ensure");
  assert!(
    !created_now2,
    "second call should reuse (created_now = false)"
  );
  assert_eq!(path, path2);
}
