# 17 — Implementation Checklist (Definition of Done)

Checklist for **SPEC §18**. Use for conformance and release validation. Core = §18.1; Extensions = §18.2; Operational = §18.3.

---

## 18.1 Required for Conformance (Core)

- [ ] **Unit tests for all code:** Every module and crate must have unit tests as a deliverable; code is not done until tests are written ([16-testing.md](16-testing.md)).
- [ ] Workflow path: explicit runtime path and cwd default (`WORKFLOW.md`)
- [ ] WORKFLOW.md loader: YAML front matter + prompt body split ([04-workflow-spec.md](04-workflow-spec.md))
- [ ] Typed config layer: defaults and `$VAR` resolution ([05-configuration.md](05-configuration.md))
- [ ] Dynamic WORKFLOW.md watch/reload for config and prompt
- [ ] Polling orchestrator with single-authority mutable state ([06-orchestration.md](06-orchestration.md))
- [ ] Issue tracker client: candidate fetch, state refresh, terminal fetch ([10-github-tracker.md](10-github-tracker.md))
- [ ] Workspace manager: sanitized per-issue workspaces ([08-workspace-management.md](08-workspace-management.md))
- [ ] Hooks: after_create, before_run, after_run, before_remove; timeout config (default 60000 ms)
- [ ] Coding-agent subprocess client: JSON line protocol ([09-agent-runner.md](09-agent-runner.md))
- [ ] Runner command config: `runner.command` required
- [ ] Strict prompt rendering: `issue` and `attempt` variables ([11-prompt-construction.md](11-prompt-construction.md))
- [ ] Exponential retry queue with continuation retries after normal exit ([07-polling-scheduling.md](07-polling-scheduling.md))
- [ ] Retry backoff cap: `agent.max_retry_backoff_ms` (default 5m)
- [ ] Reconciliation: stop runs on terminal/non-active tracker states; workspace cleanup for terminal (startup + on transition)
- [ ] Structured logs: `issue_id`, `issue_identifier`, `session_id` ([12-logging-observability.md](12-logging-observability.md))
- [ ] Operator-visible observability (logs; optional snapshot/status surface)

---

## 18.2 Recommended Extensions (Not Required)

- [ ] Optional HTTP server: CLI `--port` overrides `server.port`; safe bind host; endpoints per §13.7 ([12-logging-observability.md](12-logging-observability.md))
- [ ] Optional GitHub client-side tool: repo-scoped API access from agent session
- [ ] TODO: Persist retry queue and session metadata across restarts
- [ ] TODO: Observability settings in workflow front matter
- [ ] TODO: First-class tracker write APIs in orchestrator

---

## 18.3 Operational Validation Before Production

- [ ] Run real integration profile (§17.8) with credentials and network
- [ ] Verify hooks and workflow path resolution on target OS/shell
- [ ] If HTTP server shipped: verify port and loopback bind

---

## References

- [SPEC.md](SPEC.md) §18 — Implementation Checklist
