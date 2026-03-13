//! Integration test: fetch_candidate_issues with wiremock (SPEC §17.3).

use symphony_tracker::fetch_candidate_issues;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn github_issue_json(number: u64, title: &str, state: &str) -> serde_json::Value {
  serde_json::json!({
    "node_id": format!("N_{}", number),
    "number": number,
    "title": title,
    "state": state,
    "body": null,
    "html_url": format!("https://github.com/owner/repo/issues/{}", number),
    "labels": [],
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:00:00Z"
  })
}

/// PRs must be excluded from candidates (items with pull_request are skipped).
fn github_pr_json(number: u64) -> serde_json::Value {
  serde_json::json!({
    "node_id": format!("PR_{}", number),
    "number": number,
    "title": "PR title",
    "state": "open",
    "pull_request": {}
  })
}

#[tokio::test]
async fn fetch_candidate_issues_parses_and_excludes_prs() {
  let mock = MockServer::start().await;

  let body = serde_json::json!([
    github_issue_json(1, "First issue", "open"),
    github_pr_json(2),
    github_issue_json(3, "Third issue", "open")
  ]);

  Mock::given(method("GET"))
    .and(path("/repos/owner/repo/issues"))
    .respond_with(ResponseTemplate::new(200).set_body_json(&body))
    .mount(&mock)
    .await;

  let endpoint = mock.uri();
  let issues = fetch_candidate_issues(
    endpoint.as_str(),
    "test-token",
    "owner/repo",
    &["open".to_string()],
    None,
    None,
  )
  .await
  .expect("fetch_candidate_issues");

  assert_eq!(issues.len(), 2, "PRs must be excluded");
  assert_eq!(issues[0].identifier, "owner/repo#1");
  assert_eq!(issues[0].title, "First issue");
  assert_eq!(issues[1].identifier, "owner/repo#3");
  assert_eq!(issues[1].title, "Third issue");
}
