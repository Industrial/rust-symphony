# 16 — Test and Validation Matrix

Rust implementation notes for **SPEC §17**. Use **tokio::test** for async tests, **mockall** (or similar) for trait mocks (tracker, workspace, agent runner), and **wiremock** for HTTP-based tracker tests when mocking the GitHub API.

---

## Deliverable: Unit tests for all code

**Unit tests must be written for all code as a deliverable.** For every module, crate, or component that is implemented, the implementation is not complete until unit tests are added (e.g. in `#[cfg(test)] mod tests` at the bottom of each file or in a dedicated test module). This applies to domain types, workflow loader, config layer, tracker client, workspace manager, orchestrator, agent runner, and any other implemented code. Integration and validation scenarios below are in addition to this requirement.

---

## Crates

```toml
[dev-dependencies]
tokio = { version = "1", features = ["test-util", "rt", "macros"] }
mockall = "0.13"
wiremock = "0.6"
# Optional: tempfile for workspace tests
tempfile = "3"
```

- **tokio::test**: Async test runtime; use for integration tests that run the orchestrator loop or workers.
- **mockall**: Implement mock traits for `Tracker`, `WorkspaceManager`, or agent runner so unit tests don’t hit real APIs or filesystems.
- **wiremock**: Mock HTTP server; respond to `GET /repos/.../issues` etc. so the GitHub client can be tested without credentials.
- **tempfile**: Temporary directories for workspace manager tests.

---

## 17.1 Workflow and Config Parsing (SPEC §17.1)

- Load WORKFLOW.md from a fixture path; assert config and prompt_template.
- Invalid YAML / non-map front matter → error.
- Missing file → `MissingWorkflowFile`.
- Defaults applied when keys omitted; `tracker.repo` and `runner.command` required after validation.
- `$VAR` resolution: set env, resolve, assert value; empty env for required key → validation error.
- Prompt template: render with `issue` and `attempt`; unknown variable → render error.

---

## 17.2 Workspace Manager and Safety (SPEC §17.2)

- Path from identifier is deterministic; sanitize produces safe directory name.
- Create dir once; second call reuses (created_now = false).
- after_create runs only on first create; before_run before each attempt; after_run / before_remove on exit/cleanup.
- Path under root: assert workspace path is under configured root; reject if not.
- Agent launch: assert cwd is workspace path (or mock Command and check args).

---

## 17.3 Issue Tracker Client (SPEC §17.3)

- **wiremock**: Stub `GET /repos/owner/repo/issues?state=open&...`; return JSON array; assert client parses and normalizes to `Issue`; exclude PRs (stub items with `pull_request` and assert they’re filtered).
- Pagination: stub multiple pages; assert all issues merged and order preserved.
- **fetch_issue_states_by_ids**: Stub `GET /repos/.../issues/{n}`; assert normalized state returned.
- Errors: stub 401, 404, 500; assert error type and that orchestrator skips dispatch or keeps workers as per spec.

---

## 17.4 Orchestrator Dispatch, Reconciliation, Retry (SPEC §17.4)

- Sort order: priority, created_at, identifier.
- Blocker rule: issue in primary active state with non-terminal blocker → not eligible.
- Reconciliation: mock tracker to return terminal state for a running issue; assert worker is terminated and workspace cleaned (or mock TerminateWorker).
- Retry: normal exit → retry with short delay; failure → backoff; slot full → requeue with error.
- Stall: mock no agent update for longer than stall_timeout; assert terminate and retry.

Use **mockall** (or manual mocks) for tracker and workspace; drive the orchestrator with a test channel and assert state transitions and outgoing messages.

---

## 17.5 Agent Runner / Coding-Agent Client (SPEC §17.5)

- Mock subprocess: script or test binary that writes JSON lines to stdout; assert handshake and turn/start sent, session_id and completion parsed.
- Timeout: mock no response; assert response_timeout or turn_timeout.
- Unsupported tool call: mock tool request; assert failure response and session continues.
- Token/usage: mock events; assert agent_totals updated.

---

## 17.6 Observability (SPEC §17.6)

- Validation failure logs (or snapshot) with expected fields.
- Logging sink failure: simulate; assert service keeps running.

---

## 17.7 CLI and Lifecycle (SPEC §17.7)

- CLI with workflow path: valid path loads; missing path fails with clear error.
- Startup validation failure: process exits non-zero.

---

## 17.8 Real Integration Profile (SPEC §17.8)

- Optional: tests that require `GITHUB_TOKEN`; skip in CI when unset; mark as `#[ignore]` or use `cargo test --release --features integration` with env check.
- Isolated repo/issue for smoke test; clean up created issues or use a dedicated test repo.

---

## References

- [SPEC.md](SPEC.md) §17 — Test and Validation Matrix
