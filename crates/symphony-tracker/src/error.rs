//! Tracker errors (SPEC §11.4).

#[derive(Debug, thiserror::Error)]
pub enum TrackerError {
  #[error("missing tracker API key")]
  MissingTrackerApiKey,

  #[error("missing tracker repo")]
  MissingTrackerRepo,

  #[error("GitHub API request failed: {0}")]
  GitHubApiRequest(String),

  #[error("GitHub API returned status {0}")]
  GitHubApiStatus(u16),

  #[error("GitHub payload parse error: {0}")]
  GitHubUnknownPayload(String),
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn missing_api_key_display() {
    let e = TrackerError::MissingTrackerApiKey;
    assert!(e.to_string().contains("API key"));
  }

  #[test]
  fn github_status_display() {
    let e = TrackerError::GitHubApiStatus(404);
    assert!(e.to_string().contains("404"));
  }
}
