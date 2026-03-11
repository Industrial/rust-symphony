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
