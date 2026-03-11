//! Build ServiceConfig from workflow config JSON (SPEC §6.1, §6.4).

use serde::Deserialize;

use crate::config::{RunnerConfig, ServiceConfig, TrackerConfig};
use crate::resolve::resolve_var;
use crate::ConfigError;

/// Raw tracker map from workflow front matter (before env resolution).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct RawTracker {
    repo: Option<String>,
    api_key: Option<String>,
    endpoint: Option<String>,
    active_states: Option<Vec<String>>,
    terminal_states: Option<Vec<String>>,
}

/// Raw runner map from workflow front matter.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct RawRunner {
    command: Option<String>,
    turn_timeout_ms: Option<u64>,
    read_timeout_ms: Option<u64>,
    stall_timeout_ms: Option<u64>,
}

/// Raw workflow config root (tracker, runner only for minimal dispatch validation).
#[derive(Debug, Deserialize)]
struct RawConfig {
    tracker: Option<RawTracker>,
    runner: Option<RawRunner>,
}

/// Build ServiceConfig from workflow front matter (e.g. `WorkflowDefinition.config`).
/// Applies env resolution to `tracker.api_key`, then validates.
pub fn from_workflow_config(value: &serde_json::Value) -> Result<ServiceConfig, ConfigError> {
    let raw: RawConfig = serde_json::from_value(value.clone())
        .map_err(|e| ConfigError::Deserialize(e.to_string()))?;

    let tracker = raw
        .tracker
        .ok_or_else(|| ConfigError::MissingKey("tracker".to_string()))?;
    let repo = tracker
        .repo
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let api_key_raw = tracker.api_key.unwrap_or_default();
    let api_key = resolve_var(&api_key_raw).trim().to_string();

    let tracker_config = TrackerConfig {
        repo,
        api_key,
        endpoint: tracker.endpoint,
        active_states: tracker.active_states.or_else(|| Some(vec!["open".to_string()])),
        terminal_states: tracker.terminal_states.or_else(|| Some(vec!["closed".to_string()])),
    };

    let runner_raw = raw
        .runner
        .ok_or_else(|| ConfigError::MissingKey("runner".to_string()))?;
    let command = runner_raw
        .command
        .map(|s| resolve_var(&s).trim().to_string())
        .unwrap_or_default();

    let runner_config = RunnerConfig {
        command,
        turn_timeout_ms: runner_raw.turn_timeout_ms.or(Some(3_600_000)),
        read_timeout_ms: runner_raw.read_timeout_ms.or(Some(5_000)),
        stall_timeout_ms: runner_raw.stall_timeout_ms.or(Some(300_000)),
    };

    let service = ServiceConfig {
        tracker: tracker_config,
        runner: runner_config,
    };
    service.validate_dispatch().map_err(ConfigError::Validation)?;
    Ok(service)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_workflow_config_success() {
        let value = serde_json::json!({
            "tracker": { "repo": "owner/repo", "api_key": "test-key" },
            "runner": { "command": "codex app-server" }
        });
        let config = from_workflow_config(&value).unwrap();
        assert_eq!(config.tracker.repo, "owner/repo");
        assert_eq!(config.tracker.api_key, "test-key");
        assert_eq!(config.runner.command, "codex app-server");
    }

    #[test]
    fn from_workflow_config_missing_tracker() {
        let value = serde_json::json!({ "runner": { "command": "cmd" } });
        let r = from_workflow_config(&value);
        assert!(matches!(r, Err(ConfigError::MissingKey(_))));
    }

    #[test]
    fn from_workflow_config_empty_api_key_fails_validation() {
        let value = serde_json::json!({
            "tracker": { "repo": "r", "api_key": "" },
            "runner": { "command": "c" }
        });
        let r = from_workflow_config(&value);
        assert!(matches!(r, Err(ConfigError::Validation(_))));
    }

    #[test]
    fn from_workflow_config_resolves_api_key_var() {
        std::env::set_var("TEST_GH_KEY", "resolved-secret");
        let value = serde_json::json!({
            "tracker": { "repo": "r", "api_key": "$TEST_GH_KEY" },
            "runner": { "command": "c" }
        });
        let config = from_workflow_config(&value).unwrap();
        std::env::remove_var("TEST_GH_KEY");
        assert_eq!(config.tracker.api_key, "resolved-secret");
    }
}
