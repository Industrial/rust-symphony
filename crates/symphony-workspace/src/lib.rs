//! Workspace path resolution and safety (SPEC §9).
//!
//! See `docs/08-workspace-management.md`.

mod path;

pub use path::{is_path_under_root, workspace_path};
