//! Issue and BlockerRef (SPEC §4.1.1).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockerRef {
  pub id: Option<String>,
  pub identifier: Option<String>,
  pub state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Issue {
  #[validate(length(min = 1))]
  pub id: String,
  #[validate(length(min = 1))]
  pub identifier: String,
  #[validate(length(min = 1))]
  pub title: String,
  pub description: Option<String>,
  pub priority: Option<i32>,
  #[validate(length(min = 1))]
  pub state: String,
  pub branch_name: Option<String>,
  pub url: Option<String>,
  pub labels: Vec<String>,
  pub blocked_by: Vec<BlockerRef>,
  pub created_at: Option<DateTime<Utc>>,
  pub updated_at: Option<DateTime<Utc>>,
}

#[cfg(test)]
mod tests {
  use super::*;

  fn valid_issue() -> Issue {
    Issue {
      id: "123".to_string(),
      identifier: "owner/repo#42".to_string(),
      title: "A task".to_string(),
      description: None,
      priority: Some(1),
      state: "open".to_string(),
      branch_name: None,
      url: None,
      labels: vec![],
      blocked_by: vec![],
      created_at: None,
      updated_at: None,
    }
  }

  #[test]
  fn issue_validate_passes_for_valid_issue() {
    let issue = valid_issue();
    assert!(issue.validate().is_ok());
  }

  #[test]
  fn issue_validate_fails_for_empty_id() {
    let mut issue = valid_issue();
    issue.id = String::new();
    assert!(issue.validate().is_err());
  }

  #[test]
  fn issue_validate_fails_for_empty_state() {
    let mut issue = valid_issue();
    issue.state = String::new();
    assert!(issue.validate().is_err());
  }

  #[test]
  fn blocker_ref_optional_fields() {
    let b = BlockerRef {
      id: Some("id".into()),
      identifier: Some("repo#1".into()),
      state: Some("closed".into()),
    };
    assert_eq!(b.id.as_deref(), Some("id"));
    assert_eq!(b.identifier.as_deref(), Some("repo#1"));
  }
}
