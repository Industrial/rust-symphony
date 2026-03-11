//! Workspace path resolution and safety (SPEC §9.1, §9.5).

use std::path::{Path, PathBuf};

use symphony_domain::sanitize_workspace_key;

/// Per-issue workspace path: `root.join(sanitize_workspace_key(identifier))`.
pub fn workspace_path(root: &Path, identifier: &str) -> PathBuf {
  root.join(sanitize_workspace_key(identifier))
}

/// Require that `path` is under `root` (path component semantics). Used for safety (SPEC §9.5).
pub fn is_path_under_root(path: &Path, root: &Path) -> bool {
  path.starts_with(root)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn workspace_path_joins_sanitized_key() {
    let root = Path::new("/tmp/workspaces");
    let p = workspace_path(root, "owner/repo#42");
    assert_eq!(p, PathBuf::from("/tmp/workspaces/owner_repo_42"));
  }

  #[test]
  fn workspace_path_empty_identifier() {
    let root = Path::new("/root");
    let p = workspace_path(root, "");
    assert_eq!(p, PathBuf::from("/root"));
  }

  #[test]
  fn is_path_under_root_true() {
    let root = Path::new("/tmp/root");
    let path = Path::new("/tmp/root/issue_1");
    assert!(is_path_under_root(path, root));
  }

  #[test]
  fn is_path_under_root_false() {
    let root = Path::new("/tmp/root");
    let path = Path::new("/tmp/other/issue_1");
    assert!(!is_path_under_root(path, root));
  }

  #[test]
  fn is_path_under_root_root_equals_path() {
    let root = Path::new("/tmp/root");
    assert!(is_path_under_root(root, root));
  }
}
