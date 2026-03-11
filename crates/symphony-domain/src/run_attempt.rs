//! RunAttempt and RunAttemptStatus (SPEC §4.1.5).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunAttemptStatus {
    PreparingWorkspace,
    BuildingPrompt,
    LaunchingAgentProcess,
    InitializingSession,
    StreamingTurn,
    Finishing,
    Succeeded,
    Failed,
    TimedOut,
    Stalled,
    CanceledByReconciliation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunAttempt {
    pub issue_id: String,
    pub issue_identifier: String,
    pub attempt: Option<u32>,
    #[serde(with = "crate::path_serde")]
    pub workspace_path: PathBuf,
    pub started_at: DateTime<Utc>,
    pub status: RunAttemptStatus,
    pub error: Option<String>,
}
