//! Git worktree (SPEC §4.1.4) and worktree key sanitization (SPEC §4.2).
//! Per-issue directory is implemented as a git worktree when main_repo_path is set.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Worktree {
  #[serde(with = "crate::path_serde")]
  pub path: PathBuf,
  pub worktree_key: String,
  pub created_now: bool,
}

/// Replace any character not in `[A-Za-z0-9._-]` with `_`.
/// Used for git worktree directory names under the worktree root.
pub fn sanitize_worktree_key(identifier: &str) -> String {
  identifier
    .chars()
    .map(|c| {
      if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' {
        c
      } else {
        '_'
      }
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn sanitize_worktree_key_preserves_safe_chars() {
    assert_eq!(sanitize_worktree_key("abc123"), "abc123");
    assert_eq!(
      sanitize_worktree_key("owner.repo_42-name"),
      "owner.repo_42-name"
    );
  }

  #[test]
  fn sanitize_worktree_key_replaces_special_chars() {
    assert_eq!(sanitize_worktree_key("owner/repo#42"), "owner_repo_42");
    assert_eq!(sanitize_worktree_key("a b\tc"), "a_b_c");
  }

  #[test]
  fn sanitize_worktree_key_empty() {
    assert_eq!(sanitize_worktree_key(""), "");
  }

  #[test]
  fn worktree_serde_roundtrip() {
    let w = Worktree {
      path: PathBuf::from("/tmp/worktree"),
      worktree_key: "repo_42".to_string(),
      created_now: true,
    };
    let json = serde_json::to_string(&w).unwrap();
    let w2: Worktree = serde_json::from_str(&json).unwrap();
    assert_eq!(w2.path, w.path);
    assert_eq!(w2.worktree_key, w.worktree_key);
    assert_eq!(w2.created_now, w.created_now);
  }
}
