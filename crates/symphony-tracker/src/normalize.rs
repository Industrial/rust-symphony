//! Map GitHub API issue JSON to domain Issue (SPEC §11.3).

use chrono::{DateTime, Utc};
use serde_json::Value;

use symphony_domain::Issue;

use crate::TrackerError;

/// Normalize a GitHub API issue object to domain Issue. owner and repo are from config.
pub fn github_issue_to_domain(
  value: &Value,
  owner: &str,
  repo: &str,
) -> Result<Issue, TrackerError> {
  let obj = value
    .as_object()
    .ok_or_else(|| TrackerError::GitHubUnknownPayload("expected object".into()))?;

  let number = obj
    .get("number")
    .and_then(|n| n.as_u64())
    .ok_or_else(|| TrackerError::GitHubUnknownPayload("missing number".into()))?;

  let id = obj
    .get("node_id")
    .and_then(|n| n.as_str())
    .map(String::from)
    .or_else(|| {
      obj
        .get("id")
        .and_then(|i| i.as_u64())
        .map(|i| i.to_string())
    })
    .ok_or_else(|| TrackerError::GitHubUnknownPayload("missing node_id/id".into()))?;

  let title = obj
    .get("title")
    .and_then(|t| t.as_str())
    .unwrap_or("")
    .to_string();
  if title.is_empty() {
    return Err(TrackerError::GitHubUnknownPayload("missing title".into()));
  }

  let state = obj
    .get("state")
    .and_then(|s| s.as_str())
    .unwrap_or("open")
    .to_string()
    .to_lowercase();

  let body = obj.get("body").and_then(|b| b.as_str()).map(String::from);

  let html_url = obj
    .get("html_url")
    .and_then(|u| u.as_str())
    .map(String::from);

  let labels: Vec<String> = obj
    .get("labels")
    .and_then(|l| l.as_array())
    .map(|arr| {
      arr
        .iter()
        .filter_map(|l| l.get("name").and_then(|n| n.as_str()))
        .map(|s| s.to_lowercase())
        .collect()
    })
    .unwrap_or_default();

  let created_at = obj
    .get("created_at")
    .and_then(|c| c.as_str())
    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
    .map(|dt| dt.with_timezone(&Utc));

  let updated_at = obj
    .get("updated_at")
    .and_then(|u| u.as_str())
    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
    .map(|dt| dt.with_timezone(&Utc));

  let identifier = format!("{}/{}#{}", owner, repo, number);

  Ok(Issue {
    id,
    identifier,
    title,
    description: body,
    priority: None,
    state,
    branch_name: None,
    url: html_url,
    labels,
    blocked_by: vec![],
    created_at,
    updated_at,
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn github_issue_to_domain_minimal() {
    let json = serde_json::json!({
        "node_id": "N_abc",
        "number": 42,
        "title": "Fix bug",
        "state": "open"
    });
    let issue = github_issue_to_domain(&json, "owner", "repo").unwrap();
    assert_eq!(issue.id, "N_abc");
    assert_eq!(issue.identifier, "owner/repo#42");
    assert_eq!(issue.title, "Fix bug");
    assert_eq!(issue.state, "open");
    assert!(issue.labels.is_empty());
  }

  #[test]
  fn github_issue_to_domain_state_lowercase() {
    let json = serde_json::json!({
        "id": 1,
        "number": 1,
        "title": "T",
        "state": "CLOSED"
    });
    let issue = github_issue_to_domain(&json, "o", "r").unwrap();
    assert_eq!(issue.state, "closed");
  }

  #[test]
  fn github_issue_to_domain_missing_title_fails() {
    let json = serde_json::json!({
        "node_id": "N",
        "number": 1,
        "title": "",
        "state": "open"
    });
    let r = github_issue_to_domain(&json, "o", "r");
    assert!(matches!(r, Err(TrackerError::GitHubUnknownPayload(_))));
  }
}
