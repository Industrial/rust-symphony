//! LiveSession (SPEC §4.1.6).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LiveSession {
  pub session_id: Option<String>,
  pub thread_id: Option<String>,
  pub turn_id: Option<String>,
  pub agent_pid: Option<String>,
  pub last_agent_event: Option<String>,
  pub last_agent_timestamp: Option<DateTime<Utc>>,
  pub last_agent_message: Option<String>,
  pub agent_input_tokens: u64,
  pub agent_output_tokens: u64,
  pub agent_total_tokens: u64,
  pub last_reported_input_tokens: u64,
  pub last_reported_output_tokens: u64,
  pub last_reported_total_tokens: u64,
  pub turn_count: u32,
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn live_session_default() {
    let s = LiveSession::default();
    assert!(s.session_id.is_none());
    assert_eq!(s.agent_input_tokens, 0);
    assert_eq!(s.turn_count, 0);
  }

  #[test]
  fn live_session_with_ids() {
    let s = LiveSession {
      session_id: Some("s1-1".into()),
      thread_id: Some("s1".into()),
      turn_id: Some("1".into()),
      ..Default::default()
    };
    assert_eq!(s.session_id.as_deref(), Some("s1-1"));
  }
}
