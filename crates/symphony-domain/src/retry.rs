//! RetryEntry (SPEC §4.1.7).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryEntry {
    pub issue_id: String,
    pub identifier: String,
    pub attempt: u32,
    pub due_at_ms: u64,
    pub error: Option<String>,
}
