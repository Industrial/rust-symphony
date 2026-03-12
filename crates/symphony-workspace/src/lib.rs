//! Workspace path resolution, directory creation, and hooks (SPEC §9).
//!
//! See `docs/08-workspace-management.md`.

mod manager;
mod path;

pub use manager::{WorkspaceError, ensure_workspace_dir, ensure_worktree_dir, run_hook};
pub use path::{is_path_under_root, workspace_path};
