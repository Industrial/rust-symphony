# 13 — Failure Model and Recovery Strategy

Rust implementation notes for **SPEC §14**. Typed errors (e.g. **thiserror**); recovery behavior and operator intervention points; no new crates required beyond what’s already used.

---

## Crates

```toml
[dependencies]
thiserror = "2"
anyhow = "2"  # optional, for context in app code
```

- **thiserror**: Define error enums per failure class; implement `From` and `Display`; use in orchestrator and workers so recovery is explicit.

---

## 14.1 Failure Classes (SPEC §14.1)

1. **Workflow/Config**: `MissingWorkflowFile`, `WorkflowParseError`, `WorkflowFrontMatterNotAMap`, `ConfigValidationError` (missing repo, api_key, runner.command), `TemplateParseError`, `TemplateRenderError`.
2. **Workspace**: `WorkspaceCreateFailed`, `WorkspacePathInvalid`, `HookFailed`, `HookTimeout`.
3. **Agent session**: `RunnerNotFound`, `InvalidWorkspaceCwd`, `ResponseTimeout`, `TurnTimeout`, `TurnFailed`, `TurnCancelled`, `TurnInputRequired`, `StartupFailed`.
4. **Tracker**: `MissingTrackerApiKey`, `MissingTrackerRepo`, `GitHubApiRequest`, `GitHubApiStatus`, `GitHubUnknownPayload`.
5. **Observability**: `SnapshotTimeout`, `SnapshotUnavailable`, log sink errors (log and continue).

Group into an app-level enum (e.g. `SymphonyError`) or keep per-module; ensure orchestrator and workers map to retry vs skip-dispatch vs shutdown as below.

---

## 14.2 Recovery Behavior (SPEC §14.2)

- **Dispatch validation failure**: Skip new dispatches this tick; keep service running; run reconciliation; log and notify.
- **Worker failure**: Remove from running; add runtime/tokens to totals; schedule retry with exponential backoff (see [07-polling-scheduling.md](07-polling-scheduling.md)).
- **Tracker candidate fetch failure**: Log; skip dispatch this tick; next tick retries.
- **Tracker state refresh failure**: Log; keep current workers; retry refresh next tick.
- **Dashboard / log failure**: Do not crash orchestrator; emit warning if possible.

---

## 14.3 Partial State Recovery / Restart (SPEC §14.3)

No persistent orchestrator state. On restart:

- Run startup terminal workspace cleanup ([07-polling-scheduling.md](07-polling-scheduling.md)).
- Start poll loop; fetch candidates and dispatch. No restoration of retry timers or running sessions.

---

## 14.4 Operator Intervention (SPEC §14.4)

- **Edit WORKFLOW.md**: Reload detected; re-apply config and prompt; no restart required for most settings.
- **Change issue state in tracker**: Reconciliation stops runs (terminal → cleanup workspace; non-active → stop without cleanup).
- **Restart process**: For deployment or full recovery; state is rebuilt from tracker and filesystem.

---

## References

- [SPEC.md](SPEC.md) §14 — Failure Model and Recovery  
- [07-polling-scheduling.md](07-polling-scheduling.md) — Retry and startup cleanup
