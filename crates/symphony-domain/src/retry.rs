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

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn retry_entry_creation() {
    let e = RetryEntry {
      issue_id: "id".into(),
      identifier: "repo#1".into(),
      attempt: 2,
      due_at_ms: 60_000,
      error: Some("timeout".into()),
    };
    assert_eq!(e.attempt, 2);
    assert_eq!(e.due_at_ms, 60_000);
  }

  #[test]
  fn retry_entry_serde_roundtrip() {
    let e = RetryEntry {
      issue_id: "i1".into(),
      identifier: "r#1".into(),
      attempt: 1,
      due_at_ms: 0,
      error: None,
    };
    let j = serde_json::to_string(&e).unwrap();
    let e2: RetryEntry = serde_json::from_str(&j).unwrap();
    assert_eq!(e2.issue_id, e.issue_id);
    assert_eq!(e2.attempt, e.attempt);
  }
}
