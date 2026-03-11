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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn run_attempt_status_serde_roundtrip() {
    let statuses = [
      RunAttemptStatus::PreparingWorkspace,
      RunAttemptStatus::Succeeded,
      RunAttemptStatus::Failed,
      RunAttemptStatus::CanceledByReconciliation,
    ];
    for status in statuses {
      let j = serde_json::to_string(&status).unwrap();
      let out: RunAttemptStatus = serde_json::from_str(&j).unwrap();
      assert_eq!(out, status);
    }
  }

  #[test]
  fn run_attempt_creation() {
    let started = Utc::now();
    let attempt = RunAttempt {
      issue_id: "id".into(),
      issue_identifier: "repo#1".into(),
      attempt: Some(1),
      workspace_path: PathBuf::from("/tmp/ws"),
      started_at: started,
      status: RunAttemptStatus::StreamingTurn,
      error: None,
    };
    assert_eq!(attempt.attempt, Some(1));
    assert!(matches!(attempt.status, RunAttemptStatus::StreamingTurn));
  }
}
