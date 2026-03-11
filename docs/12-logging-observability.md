# 12 — Logging, Status, and Observability

Rust implementation notes for **SPEC §13**. Uses **tracing** for structured logging; optional **axum** for the HTTP API and dashboard (extension).

---

## Crates

```toml
[dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
# Optional: HTTP server (extension)
axum = { version = "0.7", optional = true }
tokio = { version = "1", features = ["net", "time"], optional = true }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
```

- **tracing** / **tracing-subscriber**: Spans and events with `issue_id`, `issue_identifier`, `session_id`; key=value style; env filter for level.
- **axum**: Optional; serve `GET /`, `GET /api/v1/state`, `GET /api/v1/:issue_identifier`, `POST /api/v1/refresh` when `server.port` or CLI `--port` is set.

---

## 13.1 Logging Conventions (SPEC §13.1)

- **Issue-related logs**: Include `issue_id` and `issue_identifier` (e.g. `tracing::Span::current().record("issue_id", &issue.id)` or event fields).
- **Session lifecycle**: Include `session_id`.
- **Format**: Stable key=value; outcome (completed, failed, retrying); concise error reason; avoid logging large payloads.

---

## 13.2 Outputs and Sinks (SPEC §13.2)

- Default: subscriber writing to stderr (e.g. `tracing_subscriber::fmt`). Optional: file, OpenTelemetry, etc.
- If a sink fails: log a warning and continue; do not crash the orchestrator.

---

## 13.3 Runtime Snapshot (SPEC §13.3)

Expose a snapshot type (or closure) that returns: `running` (list of session rows with `turn_count`), `retrying`, `agent_totals` (input/output/total tokens, seconds_running), `rate_limits`. Run inside the orchestrator task or with a channel request/response. On timeout or unavailable, return an error variant (e.g. `SnapshotError::Timeout`).

---

## 13.4 Optional Status Surface (SPEC §13.4)

Terminal output or dashboard is optional; draw only from orchestrator state. No impact on correctness.

---

## 13.5 Session Metrics and Token Accounting (SPEC §13.5)

- Prefer absolute thread totals from agent events; ignore delta-only payloads for totals.
- Track deltas from last reported totals to avoid double-counting.
- Add run duration to cumulative `agent_totals.seconds_running` when a session ends. Optionally add active-session elapsed time when producing a snapshot.
- Store latest rate-limit payload in orchestrator state.

---

## 13.6 Humanized Event Summaries (SPEC §13.6)

Optional; observability only. Do not drive orchestrator logic from humanized strings.

---

## 13.7 Optional HTTP Server (SPEC §13.7)

When enabled (CLI `--port` or `server.port` in workflow):

- **Bind**: Loopback (e.g. `127.0.0.1`); port from config or CLI (CLI overrides). Port `0` = ephemeral.
- **GET /** — Human-readable dashboard (server-rendered HTML or static SPA consuming the API).
- **GET /api/v1/state** — JSON: generated_at, counts, running, retrying, agent_totals, rate_limits (see SPEC for shape).
- **GET /api/v1/:issue_identifier** — JSON: issue-specific debug info; 404 if unknown.
- **POST /api/v1/refresh** — Trigger poll + reconcile (best-effort); respond 202 with queued/coalesced.
- **Errors**: 405 for wrong method; JSON envelope `{"error":{"code":"...","message":"..."}}` for 404 etc.

Serve from a separate tokio task; snapshot data provided by the orchestrator (e.g. via shared state or request channel).

---

## References

- [SPEC.md](SPEC.md) §13 — Logging, Status, and Observability
