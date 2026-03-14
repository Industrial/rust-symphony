# 05 — Configuration Specification

Rust implementation notes for **SPEC §6**. Uses **shellexpand** for `$VAR` and path expansion; **validator** for validation after resolution.

**Deliverable:** Unit tests must be written for all code; implementation is not complete without them. See [16-testing.md](16-testing.md).

---

## Crates

```toml
[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
shellexpand = "3"
validator = { version = "0.18", features = ["derive"] }
# Optional: for home dir in path expansion
dirs = "5"
```

- **shellexpand**: Expands `$VAR_NAME` and `${VAR_NAME}` from the environment; use for config values that may contain `$VAR` (e.g. `tracker.api_key`, `worktree.root`). Also supports `~` for home directory when used with a context that provides it.
- **validator**: Validate typed config after resolution (required fields, non-empty strings, numeric ranges). Use `#[validate(...)]` and `Validate::validate()` before dispatch (see §6.3).
- **dirs**: Optional; use `dirs::home_dir()` to resolve `~` in paths when not using shellexpand’s default (shellexpand can use `~` if given a home callback).

---

## 6.1 Source Precedence and Resolution (SPEC §6.1)

1. Workflow file path (runtime/CLI).
2. YAML front matter values (from [04-workflow-spec.md](04-workflow-spec.md)).
3. Environment indirection: replace `$VAR_NAME` (and `${VAR_NAME}`) in selected string values with `std::env::var("VAR_NAME")`. Empty env → treat as missing where required (e.g. `api_key`).
4. Built-in defaults.

**Path/command semantics** (SPEC):

- Expand `~` (home) and `$VAR` in **path-like** config values (e.g. `worktree.root`).
- Do not rewrite URIs or arbitrary shell command strings (e.g. `runner.command` may contain spaces; expand only if you explicitly define that it supports `$VAR`).

### Resolving `$VAR` with shellexpand

```rust
use shellexpand::env::full_with_context_no_errors;

fn resolve_var(s: &str) -> String {
    full_with_context_no_errors(s, |key| std::env::var(key).ok()).into_owned()
}
```

- Use `resolve_var` for values that are defined to support indirection (e.g. `tracker.api_key`, `worktree.root`). If the result is empty and the field is required (e.g. `api_key`), validation fails.

### Path expansion

- **Home (`~`)**: Use `shellexpand::tilde()` with a home dir from `dirs::home_dir()`, or a custom context that provides `HOME`.
- **Path separators**: SPEC says paths with `~` or path separators are expanded; bare strings without separators can be preserved (e.g. relative roots). Normalize to absolute when storing `worktree.root` for safety (e.g. `std::fs::canonicalize` or prepend current dir for relative paths).

---

## 6.2 Dynamic Reload (SPEC §6.2)

- Config is re-read when `WORKFLOW.md` changes (see [04-workflow-spec.md](04-workflow-spec.md) file watching).
- On reload: re-parse front matter, re-apply defaults, re-run env resolution, re-validate. Use the new typed config for future dispatch, retry scheduling, reconciliation, hooks, and runner launches.
- In-flight agent sessions are not restarted by config change.
- Invalid reload: keep last known good config, log/emit error, do not crash.

---

## 6.3 Dispatch Preflight Validation (SPEC §6.3)

Run before startup and before each dispatch cycle. Use **validator** on the resolved config struct.

**Checks**:

1. Workflow file can be loaded and parsed.
2. `tracker.repo` is present (non-empty after trim).
3. `tracker.api_key` is present after `$VAR` resolution (non-empty).
4. `tracker.claim_label`, `tracker.pr_open_label`, and `tracker.pr_base_branch` are required (non-empty after trim); config load fails if any is missing.
5. `runner.command` is present and non-empty.
6. `worktree.root` is required (non-empty after resolution); config load fails if missing.
7. `worktree.main_repo_path` is present and non-empty after resolution (required for worker development in a git worktree and branch).

Example (conceptual):

```rust
use validator::Validate;

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

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct RunnerConfig {
    #[validate(length(min = 1, message = "runner.command required"))]
    pub command: String,
    pub turn_timeout_ms: Option<u64>,
    pub read_timeout_ms: Option<u64>,
    pub stall_timeout_ms: Option<u64>,
}

pub struct ServiceConfig {
    pub tracker: TrackerConfig,
    pub runner: RunnerConfig,
    // ... polling, workspace, hooks, agent
}

impl ServiceConfig {
    pub fn validate_dispatch(&self) -> Result<(), ConfigValidationError> {
        self.tracker.validate().map_err(ConfigValidationError::Tracker)?;
        self.runner.validate().map_err(ConfigValidationError::Runner)?;
        Ok(())
    }
}
```

- Resolve `api_key` with shellexpand (or custom `$VAR` replacement) **before** building `TrackerConfig` and running `validate()`. If resolution yields empty and the spec requires the key, validation fails.

---

## 6.4 Config Fields Summary (SPEC §6.4)

Typed getters should expose (after resolution and defaults):

| Key | Type | Default / note |
|-----|------|----------------|
| `tracker.repo` | `String` | required |
| `tracker.api_key` | `String` | required (after `$VAR`) |
| `tracker.claim_label` | `String` | **required**; label agent adds when claiming |
| `tracker.pr_open_label` | `String` | **required**; label for PR-open visibility/filtering |
| `tracker.pr_base_branch` | `String` | **required**; base branch for worker branches and PR target |
| `tracker.endpoint` | `String` | `"https://api.github.com"` |
| `tracker.active_states` | `Vec<String>` | `["open"]` |
| `tracker.terminal_states` | `Vec<String>` | `["closed"]` |
| `polling.interval_ms` | `u64` | `30000` |
| `worktree.root` | `PathBuf` | **required**; root for per-issue worktrees; expand `~` and `$VAR` |
| `worktree.main_repo_path` | `PathBuf` | **required**; path to main git repo; expand `~` and `$VAR` |
| `hooks.after_create` / `before_run` / `after_run` / `before_remove` | `Option<String>` | optional script |
| `hooks.timeout_ms` | `u64` | `60000` |
| `agent.max_concurrent_agents` | `u32` | `10` |
| `agent.max_turns` | `u32` | `20` |
| `agent.max_retry_backoff_ms` | `u64` | `300000` |
| `agent.max_concurrent_agents_by_state` | `HashMap<String, u32>` | empty; keys normalized lowercase |
| `runner.command` | `String` | required |
| `runner.turn_timeout_ms` | `u64` | `3600000` |
| `runner.read_timeout_ms` | `u64` | `5000` |
| `runner.stall_timeout_ms` | `u64` | `300000` |
| `server.port` (extension) | `Option<u16>` | optional |

- **runner.***: Optional provider-specific keys (e.g. approval_policy, sandbox) can be stored as a pass-through map or extra fields and ignored by validation except where the implementation defines them.

---

## Implementation Notes

1. **Deserialization**: From the workflow loader you get a map (`serde_json::Value` or similar). Deserialize into a flattened or nested config struct; use `serde(default)` and `Option` for optional fields so missing keys get defaults.
2. **Env resolution**: After deserializing, walk string fields that support `$VAR` (or only those you list, e.g. `tracker.api_key`, `worktree.root`) and replace with `shellexpand` (or equivalent). Then validate.
3. **Path expansion**: For `worktree.root`, after `$VAR` resolution expand `~` and convert to absolute path; store as `PathBuf`.
4. **Validation**: Run `validate_dispatch()` at startup and on each poll tick before dispatch; on failure skip dispatch and emit an operator-visible error.

---

## References

- [SPEC.md](SPEC.md) §6 — Configuration Specification  
- [04-workflow-spec.md](04-workflow-spec.md) — Workflow loader output (config + prompt_template)  
- [03-domain-model.md](03-domain-model.md) — Service config typed view
