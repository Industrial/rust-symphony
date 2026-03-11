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
