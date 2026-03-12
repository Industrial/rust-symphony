//! GitHub API HTTP client for listing and fetching issues (SPEC §11).
//!
//! ## Issue→PR resolution (SPEC_ADDENDUM_2 B.2)
//!
//! Fix-PR candidate set requires resolving the pull request for each issue. This implementation
//! uses **head branch pattern** resolution: the configured pattern (e.g. `symphony/issue-{number}`)
//! has `{number}` replaced by the issue number; we call GitHub `GET /repos/{owner}/{repo}/pulls?state=open&head=owner:branch`.
//! If exactly one open PR matches, that is the resolved PR. If none, the issue is treated as "wait" (do not dispatch).
//! Alternative strategies (e.g. "Fixes #N" in body/title) can be added later via config.

use std::time::Duration;

use reqwest::header::{ACCEPT, AUTHORIZATION, USER_AGENT};
use serde_json::Value;

use symphony_domain::Issue;

use crate::TrackerError;
use crate::filter::apply_label_filters;
use crate::normalize::github_issue_to_domain;

/// Resolved pull request for an issue (SPEC_ADDENDUM_2 B.2). Used when the orchestrator has found the PR for a fix-PR candidate.
#[derive(Debug, Clone)]
pub struct ResolvedPr {
  /// Head branch name (e.g. symphony/issue-18).
  pub head_ref: String,
  /// Pull request number.
  pub pr_number: u64,
  /// PR updated_at (ISO8601) for newness cutoff when fetching mentions (B.5.1).
  pub pr_updated_at: Option<String>,
}

/// One check run from the Checks API (SPEC_ADDENDUM_2 B.3). Used to determine if any check failed.
#[derive(Debug, Clone)]
pub struct CheckRunInfo {
  /// Conclusion when status is "completed": failure, success, cancelled, etc.
  pub conclusion: Option<String>,
  /// Status: queued, in_progress, completed, etc.
  pub status: String,
}

/// Combined commit status from the commit status API (SPEC_ADDENDUM_2 B.3).
#[derive(Debug, Clone)]
pub struct CombinedStatusInfo {
  /// Combined state: failure, pending, success.
  pub state: String,
}

/// Maximum number of issues to request per GitHub API page.
const PER_PAGE: u32 = 100;
/// HTTP timeout for GitHub API requests (seconds).
const REQUEST_TIMEOUT_SECS: u64 = 30;

/// GitHub API client with shared timeout and auth headers.
#[derive(Clone)]
pub struct GitHubApiClient {
  /// HTTP client used for requests.
  client: reqwest::Client,
  /// `Authorization` header value (e.g. `Bearer <token>`).
  auth_header: String,
}

impl GitHubApiClient {
  /// Build a client; errors if api_key is empty or client build fails.
  pub fn new(api_key: &str) -> Result<Self, TrackerError> {
    if api_key.is_empty() {
      return Err(TrackerError::MissingTrackerApiKey);
    }
    let client = reqwest::Client::builder()
      .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
      .build()
      .map_err(|e| TrackerError::GitHubApiRequest(e.to_string()))?;
    let auth_header = format!("Bearer {}", api_key.trim().trim_start_matches("Bearer "));
    Ok(Self {
      client,
      auth_header,
    })
  }

  /// Send GET to url with auth and standard headers; caller checks status and parses body.
  pub async fn get(&self, url: &str) -> Result<reqwest::Response, TrackerError> {
    self
      .client
      .get(url)
      .header(AUTHORIZATION, &self.auth_header)
      .header(ACCEPT, "application/vnd.github+json")
      .header(USER_AGENT, "rust-symphony")
      .send()
      .await
      .map_err(|e| TrackerError::GitHubApiRequest(e.to_string()))
  }

  /// Build the GitHub REST URL for repo issues (e.g. .../repos/owner/repo/issues?state=open).
  pub fn repo_issues_url(endpoint: &str, owner: &str, repo: &str, path_suffix: &str) -> String {
    let base = endpoint.trim_end_matches('/');
    format!("{}/repos/{}/{}/issues{}", base, owner, repo, path_suffix)
  }

  /// Build the GitHub REST URL for repo pull requests (e.g. .../repos/owner/repo/pulls?state=open).
  pub fn repo_pulls_url(endpoint: &str, owner: &str, repo: &str, path_suffix: &str) -> String {
    let base = endpoint.trim_end_matches('/');
    format!("{}/repos/{}/{}/pulls{}", base, owner, repo, path_suffix)
  }

  /// Build the GitHub REST URL for repo commits (e.g. .../repos/owner/repo/commits/ref/check-runs).
  pub fn repo_commits_url(endpoint: &str, owner: &str, repo: &str, path_suffix: &str) -> String {
    let base = endpoint.trim_end_matches('/');
    format!("{}/repos/{}/{}/commits{}", base, owner, repo, path_suffix)
  }
}

/// Parse "owner/repo" into (owner, repo). Returns error if format invalid.
pub fn parse_repo(repo: &str) -> Result<(String, String), TrackerError> {
  let parts: Vec<&str> = repo.split('/').collect();
  if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
    return Err(TrackerError::MissingTrackerRepo);
  }
  Ok((parts[0].to_string(), parts[1].to_string()))
}

/// Parse "owner/repo#123" to get issue number. Returns None if format invalid.
pub fn parse_issue_number(identifier: &str) -> Option<u64> {
  identifier.rsplit_once('#')?.1.parse().ok()
}

/// Fetch all issues in the given states (e.g. open). Excludes pull requests.
/// Optionally filters by include_labels (whitelist) and exclude_labels (blacklist) per SPEC_ADDENDUM_1 A.1.
pub async fn fetch_candidate_issues(
  endpoint: &str,
  api_key: &str,
  repo: &str,
  active_states: &[String],
  include_labels: Option<&[String]>,
  exclude_labels: Option<&[String]>,
) -> Result<Vec<Issue>, TrackerError> {
  let (owner, repo_name) = parse_repo(repo)?;
  let api = GitHubApiClient::new(api_key)?;

  let mut all = Vec::new();
  let states = if active_states.is_empty() {
    vec!["open".to_string()]
  } else {
    active_states.to_vec()
  };

  for state in &states {
    let mut page = 1u32;
    loop {
      let url = GitHubApiClient::repo_issues_url(
        endpoint,
        &owner,
        &repo_name,
        &format!("?state={}&per_page={}&page={}", state, PER_PAGE, page),
      );
      let res = api.get(&url).await?;

      if !res.status().is_success() {
        return Err(TrackerError::GitHubApiStatus(res.status().as_u16()));
      }

      let body: Vec<Value> = res
        .json()
        .await
        .map_err(|e| TrackerError::GitHubUnknownPayload(e.to_string()))?;

      if body.is_empty() {
        break;
      }

      let page_len = body.len();
      for value in &body {
        if value.get("pull_request").is_some() {
          continue;
        }
        match github_issue_to_domain(value, &owner, &repo_name) {
          Ok(issue) => all.push(issue),
          Err(_) => continue,
        }
      }

      if page_len < PER_PAGE as usize {
        break;
      }
      page += 1;
    }
  }

  let filtered = apply_label_filters(all, include_labels, exclude_labels);
  Ok(filtered)
}

/// Fetch issues that have the given label and are in one of the active states (SPEC_ADDENDUM_2 B.2 fix-PR candidate set).
/// Excludes pull requests. Does not apply include/exclude label filters.
pub async fn fetch_issues_with_label(
  endpoint: &str,
  api_key: &str,
  repo: &str,
  label: &str,
  active_states: &[String],
) -> Result<Vec<Issue>, TrackerError> {
  let (owner, repo_name) = parse_repo(repo)?;
  let api = GitHubApiClient::new(api_key)?;

  let mut all = Vec::new();
  let states = if active_states.is_empty() {
    vec!["open".to_string()]
  } else {
    active_states.to_vec()
  };

  for state in &states {
    let mut page = 1u32;
    loop {
      let labels_param = urlencoding::encode(label);
      let url = GitHubApiClient::repo_issues_url(
        endpoint,
        &owner,
        &repo_name,
        &format!(
          "?state={}&labels={}&per_page={}&page={}",
          state, labels_param, PER_PAGE, page
        ),
      );
      let res = api.get(&url).await?;

      if !res.status().is_success() {
        return Err(TrackerError::GitHubApiStatus(res.status().as_u16()));
      }

      let body: Vec<Value> = res
        .json()
        .await
        .map_err(|e| TrackerError::GitHubUnknownPayload(e.to_string()))?;

      if body.is_empty() {
        break;
      }

      let page_len = body.len();
      for value in &body {
        if value.get("pull_request").is_some() {
          continue;
        }
        match github_issue_to_domain(value, &owner, &repo_name) {
          Ok(issue) => all.push(issue),
          Err(_) => continue,
        }
      }

      if page_len < PER_PAGE as usize {
        break;
      }
      page += 1;
    }
  }

  Ok(all)
}

/// Resolve the pull request for an issue by head branch pattern (SPEC_ADDENDUM_2 B.2).
/// Pattern may contain "{number}" which is replaced by the issue number (e.g. "symphony/issue-{number}").
/// Returns the first open PR whose head ref matches the pattern, or None if no such PR exists.
pub async fn resolve_pr_for_issue(
  endpoint: &str,
  api_key: &str,
  repo: &str,
  issue_number: u64,
  head_branch_pattern: &str,
) -> Result<Option<ResolvedPr>, TrackerError> {
  let (owner, repo_name) = parse_repo(repo)?;
  let api = GitHubApiClient::new(api_key)?;

  let head_branch = head_branch_pattern.replace("{number}", &issue_number.to_string());
  let head_param = format!("{}:{}", owner, head_branch);
  let head_encoded = urlencoding::encode(&head_param);
  let url = GitHubApiClient::repo_pulls_url(
    endpoint,
    &owner,
    &repo_name,
    &format!("?state=open&head={}&per_page=5", head_encoded),
  );
  let res = api.get(&url).await?;

  if !res.status().is_success() {
    return Err(TrackerError::GitHubApiStatus(res.status().as_u16()));
  }

  let body: Vec<Value> = res
    .json()
    .await
    .map_err(|e| TrackerError::GitHubUnknownPayload(e.to_string()))?;

  let pr = body.first().and_then(|v| {
    let obj = v.as_object()?;
    let pr_number = obj.get("number")?.as_u64()?;
    let head = obj.get("head")?.as_object()?;
    let head_ref = head.get("ref")?.as_str()?.to_string();
    let pr_updated_at = obj
      .get("updated_at")
      .and_then(|u| u.as_str())
      .map(|s| s.to_string());
    Some(ResolvedPr {
      head_ref,
      pr_number,
      pr_updated_at,
    })
  });

  Ok(pr)
}

/// Fetch check runs for a commit ref (SPEC_ADDENDUM_2 B.3). Ref can be branch name or SHA.
/// Returns all check runs (paginated); caller determines if any have failed conclusion.
pub async fn fetch_check_runs_for_ref(
  endpoint: &str,
  api_key: &str,
  repo: &str,
  ref_: &str,
) -> Result<Vec<CheckRunInfo>, TrackerError> {
  let (owner, repo_name) = parse_repo(repo)?;
  let api = GitHubApiClient::new(api_key)?;

  let mut all = Vec::new();
  let mut page = 1u32;
  loop {
    let ref_encoded = urlencoding::encode(ref_);
    let path = format!(
      "/{}/check-runs?per_page={}&page={}",
      ref_encoded, PER_PAGE, page
    );
    let url = GitHubApiClient::repo_commits_url(endpoint, &owner, &repo_name, &path);
    let res = api.get(&url).await?;

    if !res.status().is_success() {
      return Err(TrackerError::GitHubApiStatus(res.status().as_u16()));
    }

    let body: Value = res
      .json()
      .await
      .map_err(|e| TrackerError::GitHubUnknownPayload(e.to_string()))?;

    let check_runs = body
      .get("check_runs")
      .and_then(|a| a.as_array())
      .map(|v| v.as_slice())
      .unwrap_or_else(|| &[]);
    if check_runs.is_empty() {
      break;
    }
    for run in check_runs {
      let obj = match run.as_object() {
        Some(o) => o,
        None => continue,
      };
      let conclusion = obj
        .get("conclusion")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string());
      let status = obj
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();
      all.push(CheckRunInfo { conclusion, status });
    }
    if check_runs.len() < PER_PAGE as usize {
      break;
    }
    page += 1;
  }
  Ok(all)
}

/// Fetch combined commit status for a ref (SPEC_ADDENDUM_2 B.3). Ref can be branch name or SHA.
pub async fn fetch_commit_status_for_ref(
  endpoint: &str,
  api_key: &str,
  repo: &str,
  ref_: &str,
) -> Result<CombinedStatusInfo, TrackerError> {
  let (owner, repo_name) = parse_repo(repo)?;
  let api = GitHubApiClient::new(api_key)?;

  let ref_encoded = urlencoding::encode(ref_);
  let path = format!("/{}/status", ref_encoded);
  let url = GitHubApiClient::repo_commits_url(endpoint, &owner, &repo_name, &path);
  let res = api.get(&url).await?;

  if !res.status().is_success() {
    return Err(TrackerError::GitHubApiStatus(res.status().as_u16()));
  }

  let body: Value = res
    .json()
    .await
    .map_err(|e| TrackerError::GitHubUnknownPayload(e.to_string()))?;

  let state = body
    .get("state")
    .and_then(|s| s.as_str())
    .unwrap_or("pending")
    .to_string();
  Ok(CombinedStatusInfo { state })
}

/// Fetch issue comments and PR review comments; return true if any contain @{mention_handle} (SPEC_ADDENDUM_2 B.5).
/// If created_after is Some (ISO8601 string, e.g. PR updated_at), only comments created after that time count (newness rule B.5.1).
pub async fn fetch_has_qualifying_mention(
  endpoint: &str,
  api_key: &str,
  repo: &str,
  issue_number: u64,
  pr_number: u64,
  mention_handle: &str,
  created_after: Option<&str>,
) -> Result<bool, TrackerError> {
  let (owner, repo_name) = parse_repo(repo)?;
  let api = GitHubApiClient::new(api_key)?;
  let needle = format!("@{}", mention_handle.trim_start_matches('@'));

  let check = |body: &str, created_at: &str| -> bool {
    if !body.contains(&needle) {
      return false;
    }
    if let Some(cutoff) = created_after {
      if created_at <= cutoff {
        return false;
      }
    }
    true
  };

  let issues_path = format!("/{}/comments?per_page={}", issue_number, PER_PAGE);
  let url = GitHubApiClient::repo_issues_url(endpoint, &owner, &repo_name, &issues_path);
  let res = api.get(&url).await?;
  if res.status().is_success() {
    let body: Vec<Value> = res
      .json()
      .await
      .map_err(|e| TrackerError::GitHubUnknownPayload(e.to_string()))?;
    for c in &body {
      if let (Some(b), Some(created)) = (
        c.get("body").and_then(|v| v.as_str()),
        c.get("created_at").and_then(|v| v.as_str()),
      ) {
        if check(b, created) {
          return Ok(true);
        }
      }
    }
  }

  let pr_path = format!("/{}/comments?per_page={}", pr_number, PER_PAGE);
  let url = GitHubApiClient::repo_pulls_url(endpoint, &owner, &repo_name, &pr_path);
  let res = api.get(&url).await?;
  if res.status().is_success() {
    let body: Vec<Value> = res
      .json()
      .await
      .map_err(|e| TrackerError::GitHubUnknownPayload(e.to_string()))?;
    for c in &body {
      if let (Some(b), Some(created)) = (
        c.get("body").and_then(|v| v.as_str()),
        c.get("created_at").and_then(|v| v.as_str()),
      ) {
        if check(b, created) {
          return Ok(true);
        }
      }
    }
  }

  Ok(false)
}

/// Fetch current state for issues by identifier (e.g. "owner/repo#42").
/// Returns issues in same order as identifiers; missing/invalid IDs are skipped.
pub async fn fetch_issue_states_by_ids(
  endpoint: &str,
  api_key: &str,
  repo: &str,
  identifiers: &[String],
) -> Result<Vec<Issue>, TrackerError> {
  let (owner, repo_name) = parse_repo(repo)?;
  let api = GitHubApiClient::new(api_key)?;

  let mut results = Vec::with_capacity(identifiers.len());
  for id in identifiers {
    let number = match parse_issue_number(id) {
      Some(n) => n,
      None => continue,
    };
    let url =
      GitHubApiClient::repo_issues_url(endpoint, &owner, &repo_name, &format!("/{}", number));
    let res = api.get(&url).await?;

    if !res.status().is_success() {
      continue;
    }

    let value: Value = res
      .json()
      .await
      .map_err(|e| TrackerError::GitHubUnknownPayload(e.to_string()))?;
    if value.get("pull_request").is_some() {
      continue;
    }
    if let Ok(issue) = github_issue_to_domain(&value, &owner, &repo_name) {
      results.push(issue);
    }
  }
  Ok(results)
}

/// Fetch all issues in the given states (e.g. closed). For terminal-state cleanup.
/// Does not apply label filters (we want all terminal issues for cleanup).
pub async fn fetch_issues_by_states(
  endpoint: &str,
  api_key: &str,
  repo: &str,
  states: &[String],
) -> Result<Vec<Issue>, TrackerError> {
  if states.is_empty() {
    return Ok(vec![]);
  }
  fetch_candidate_issues(endpoint, api_key, repo, states, None, None).await
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_repo_ok() {
    let (o, r) = parse_repo("owner/repo").unwrap();
    assert_eq!(o, "owner");
    assert_eq!(r, "repo");
  }

  #[test]
  fn parse_repo_invalid() {
    assert!(parse_repo("").is_err());
    assert!(parse_repo("owner").is_err());
    assert!(parse_repo("owner/").is_err());
    assert!(parse_repo("/repo").is_err());
  }

  #[test]
  fn parse_issue_number_ok() {
    assert_eq!(parse_issue_number("owner/repo#42"), Some(42));
    assert_eq!(parse_issue_number("a/b#1"), Some(1));
  }

  #[test]
  fn parse_issue_number_invalid() {
    assert_eq!(parse_issue_number("owner/repo"), None);
    assert_eq!(parse_issue_number("owner/repo#"), None);
    assert_eq!(parse_issue_number("owner/repo#x"), None);
  }

  #[test]
  fn github_api_client_new_empty_key_err() {
    assert!(GitHubApiClient::new("").is_err());
  }

  #[test]
  fn github_api_client_new_ok() {
    assert!(GitHubApiClient::new("test-token").is_ok());
  }

  #[test]
  fn repo_issues_url_format() {
    let url =
      GitHubApiClient::repo_issues_url("https://api.github.com", "owner", "repo", "?state=open");
    assert_eq!(
      url,
      "https://api.github.com/repos/owner/repo/issues?state=open"
    );
  }

  #[test]
  fn repo_issues_url_trim_trailing_slash() {
    let url = GitHubApiClient::repo_issues_url("https://api.github.com/", "a", "b", "/42");
    assert_eq!(url, "https://api.github.com/repos/a/b/issues/42");
  }
}
