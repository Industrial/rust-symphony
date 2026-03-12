//! Git worktree path resolution, directory creation, and hooks (SPEC §9).
//!
//! See `docs/SPEC/08-worktree-management.md`.

mod manager;
mod path;

pub use manager::{WorktreeError, ensure_worktree_dir, ensure_worktree_plain_dir, run_hook};
pub use path::{is_path_under_root, worktree_path};
