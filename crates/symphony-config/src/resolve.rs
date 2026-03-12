//! Environment variable and path resolution (SPEC §6.1).

use std::path::PathBuf;

use shellexpand::{env_with_context_no_errors, tilde};

use crate::ConfigError;

/// Expand `$VAR_NAME` and `${VAR_NAME}` from the environment.
/// Use for values that support indirection (e.g. `tracker.api_key`, `worktree.root`).
pub fn resolve_var(s: &str) -> String {
  env_with_context_no_errors(s, |key| std::env::var(key).ok()).into_owned()
}

/// Resolve git worktree root: apply $VAR, expand `~`, then normalize to absolute path.
/// Relative paths are joined with `std::env::current_dir()`.
pub fn resolve_worktree_root(s: &str) -> Result<PathBuf, ConfigError> {
  let with_vars = resolve_var(s);
  let with_tilde = tilde(&with_vars).into_owned();
  let path = PathBuf::from(with_tilde.trim());
  let path = if path.is_relative() {
    std::env::current_dir()?.join(path)
  } else {
    path
  };
  Ok(path)
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn resolve_var_passthrough() {
    assert_eq!(resolve_var("hello"), "hello");
    assert_eq!(resolve_var(""), "");
  }

  #[test]
  fn resolve_var_expands_env() {
    std::env::set_var("SYMPHONY_TEST_VAR", "secret");
    let out = resolve_var("token=$SYMPHONY_TEST_VAR");
    std::env::remove_var("SYMPHONY_TEST_VAR");
    assert_eq!(out, "token=secret");
  }

  #[test]
  fn resolve_worktree_root_relative() {
    let root = resolve_worktree_root("symphony_worktrees").unwrap();
    assert!(root.is_absolute());
    assert!(root.ends_with("symphony_worktrees"));
  }

  #[test]
  fn resolve_worktree_root_with_var() {
    std::env::set_var("SYMPHONY_WS_ROOT", "my_worktrees");
    let root = resolve_worktree_root("$SYMPHONY_WS_ROOT").unwrap();
    std::env::remove_var("SYMPHONY_WS_ROOT");
    assert!(root.is_absolute());
    assert!(root.ends_with("my_worktrees"));
  }

  #[test]
  fn resolve_worktree_root_tilde_expands() {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let root = resolve_worktree_root("~/symphony_ws").unwrap();
    assert!(root.is_absolute());
    assert_eq!(root.to_string_lossy(), format!("{}/symphony_ws", home));
  }
}
