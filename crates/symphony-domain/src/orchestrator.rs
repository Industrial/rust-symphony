//! Orchestrator runtime state (SPEC §4.1.8).

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::issue::Issue;
use crate::retry::RetryEntry;
use crate::session::LiveSession;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunningEntry {
    pub identifier: String,
    pub issue: Issue,
    pub session: LiveSession,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub retry_attempt: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentTotals {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    pub seconds_running: f64,
}

#[derive(Debug, Clone, Default)]
pub struct OrchestratorState {
    pub poll_interval_ms: u64,
    pub max_concurrent_agents: u32,
    pub running: HashMap<String, RunningEntry>,
    pub claimed: HashSet<String>,
    pub retry_attempts: HashMap<String, RetryEntry>,
    pub completed: HashSet<String>,
    pub agent_totals: AgentTotals,
    pub agent_rate_limits: Option<serde_json::Value>,
}
