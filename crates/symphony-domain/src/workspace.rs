//! Workspace (SPEC §4.1.4) and workspace key sanitization (SPEC §4.2).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
  #[serde(with = "crate::path_serde")]
  pub path: PathBuf,
  pub workspace_key: String,
  pub created_now: bool,
}

/// Replace any character not in `[A-Za-z0-9._-]` with `_`.
pub fn sanitize_workspace_key(identifier: &str) -> String {
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
  fn sanitize_workspace_key_preserves_safe_chars() {
    assert_eq!(sanitize_workspace_key("abc123"), "abc123");
    assert_eq!(
      sanitize_workspace_key("owner.repo_42-name"),
      "owner.repo_42-name"
    );
  }

  #[test]
  fn sanitize_workspace_key_replaces_special_chars() {
    assert_eq!(sanitize_workspace_key("owner/repo#42"), "owner_repo_42");
    assert_eq!(sanitize_workspace_key("a b\tc"), "a_b_c");
  }

  #[test]
  fn sanitize_workspace_key_empty() {
    assert_eq!(sanitize_workspace_key(""), "");
  }

  #[test]
  fn workspace_serde_roundtrip() {
    let w = Workspace {
      path: PathBuf::from("/tmp/workspace"),
      workspace_key: "repo_42".to_string(),
      created_now: true,
    };
    let json = serde_json::to_string(&w).unwrap();
    let w2: Workspace = serde_json::from_str(&json).unwrap();
    assert_eq!(w2.path, w.path);
    assert_eq!(w2.workspace_key, w.workspace_key);
    assert_eq!(w2.created_now, w.created_now);
  }
}
