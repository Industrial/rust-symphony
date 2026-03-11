//! Typed config structs and dispatch validation (SPEC §6.3, §6.4).

use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::ConfigValidationError;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct TrackerConfig {
    #[validate(length(min = 1, message = "tracker.repo required"))]
    pub repo: String,

    #[validate(length(min = 1, message = "tracker.api_key required after resolution"))]
    pub api_key: String,

    pub endpoint: Option<String>,
    pub active_states: Option<Vec<String>>,
    pub terminal_states: Option<Vec<String>>,
}

impl TrackerConfig {
    pub fn endpoint_or_default(&self) -> String {
        self.endpoint
            .as_deref()
            .unwrap_or("https://api.github.com")
            .to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RunnerConfig {
    #[validate(length(min = 1, message = "runner.command required"))]
    pub command: String,

    pub turn_timeout_ms: Option<u64>,
    pub read_timeout_ms: Option<u64>,
    pub stall_timeout_ms: Option<u64>,
}

impl RunnerConfig {
    pub fn turn_timeout_ms(&self) -> u64 {
        self.turn_timeout_ms.unwrap_or(3_600_000)
    }
    pub fn read_timeout_ms(&self) -> u64 {
        self.read_timeout_ms.unwrap_or(5_000)
    }
    pub fn stall_timeout_ms(&self) -> u64 {
        self.stall_timeout_ms.unwrap_or(300_000)
    }
}

/// Resolved and validated config for dispatch preflight (SPEC §6.3).
#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub tracker: TrackerConfig,
    pub runner: RunnerConfig,
}

impl ServiceConfig {
    /// Run before startup and before each dispatch cycle.
    pub fn validate_dispatch(&self) -> Result<(), ConfigValidationError> {
        self.tracker
            .validate()
            .map_err(ConfigValidationError::Tracker)?;
        self.runner
            .validate()
            .map_err(ConfigValidationError::Runner)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_dispatch_passes() {
        let s = ServiceConfig {
            tracker: TrackerConfig {
                repo: "owner/repo".into(),
                api_key: "key".into(),
                endpoint: None,
                active_states: None,
                terminal_states: None,
            },
            runner: RunnerConfig {
                command: "codex app-server".into(),
                turn_timeout_ms: None,
                read_timeout_ms: None,
                stall_timeout_ms: None,
            },
        };
        assert!(s.validate_dispatch().is_ok());
    }

    #[test]
    fn validate_dispatch_fails_empty_repo() {
        let s = ServiceConfig {
            tracker: TrackerConfig {
                repo: "".into(),
                api_key: "k".into(),
                endpoint: None,
                active_states: None,
                terminal_states: None,
            },
            runner: RunnerConfig {
                command: "cmd".into(),
                turn_timeout_ms: None,
                read_timeout_ms: None,
                stall_timeout_ms: None,
            },
        };
        assert!(s.validate_dispatch().is_err());
    }

    #[test]
    fn runner_timeout_getters() {
        let r = RunnerConfig {
            command: "c".into(),
            turn_timeout_ms: Some(100),
            read_timeout_ms: None,
            stall_timeout_ms: Some(200),
        };
        assert_eq!(r.turn_timeout_ms(), 100);
        assert_eq!(r.read_timeout_ms(), 5_000);
        assert_eq!(r.stall_timeout_ms(), 200);
    }
}
