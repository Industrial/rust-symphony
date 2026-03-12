# 09 — Agent Runner Protocol (Coding Agent Integration)

Rust implementation notes for **SPEC §10**. **Minimal protocol**: line-delimited JSON over stdio, **no full JSON-RPC crate**. Use **tokio::process** (or a subprocess crate) for the agent process; **BufReader + line reading** and **serde_json** for parsing; **ad-hoc message enum** for known methods and events.

**Deliverable:** Unit tests must be written for all code; implementation is not complete without them. See [16-testing.md](16-testing.md).

---

## Protocol Summary

The agent is a **subprocess**; communication is **stdio** only:

- **Stdin**: Symphony sends one JSON object per line (requests: initialize, thread/start, turn/start).
- **Stdout**: Agent sends one JSON object per line (responses with `id`/`result` or `error`; notifications like `turn/completed` without `id`).

Framing is **newline-delimited JSON** (NDJSON). No full JSON-RPC library: parse each line with **serde_json**, then match on a `method` field (or `result`/`error`) and extract the few fields we need (thread_id, turn_id, token usage, etc.).

---

## Crates

```toml
[dependencies]
tokio = { version = "1", features = ["process", "io-util", "time"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- **tokio::process::Command**: Launch `bash -lc <runner.command>` with `current_dir(worktree_path)`, `stdin(Stdio::piped())`, `stdout(Stdio::piped())`, `stderr(Stdio::piped())`.
- **Line reading**: Use **tokio::io::BufReader** on `child.stdout.take().unwrap()`; read lines in a loop (e.g. `AsyncBufReadExt::read_line` or manual buffer until `\n`). Max line length ~10 MB (SPEC); reject or truncate if exceeded.
- **JSON**: **serde_json::from_str::<Value>(line)** (or a minimal struct with `method`, `id`, `params`, `result`, `error`). Then match and extract; unknown methods/fields map to `AgentMessage::Other` or similar and are ignored or forwarded.

---

## 10.1 Launch Contract (SPEC §10.1)

- **Command**: `runner.command` (e.g. `codex app-server`, `cursor`, `claude`, `opencode`).
- **Invocation**: `Command::new("bash").args(["-lc", &config.runner.command]).current_dir(worktree_path)`.
- **Streams**: Pipe stdin/stdout/stderr; protocol is on **stdout** only; stderr log as diagnostics.

---

## 10.2 Session Startup Handshake (SPEC §10.2)

Send in order (each line one JSON object):

1. `{"id":1,"method":"initialize","params":{"clientInfo":{"name":"symphony","version":"1.0"},"capabilities":{}}}`
2. Wait for response line; parse `id: 1`, `result` or `error`; on error or timeout (`runner.read_timeout_ms`) fail startup.
3. `{"method":"initialized","params":{}}` (notification; no response expected).
4. `{"id":2,"method":"thread/start","params":{"approvalPolicy":...,"sandbox":...,"cwd":<abs_worktree>}}`
5. Wait for response; from `result.thread.id` (or equivalent) take `thread_id`.
6. `{"id":3,"method":"turn/start","params":{"threadId":<thread_id>,"input":[{"type":"text","text":<prompt>}],"cwd":...,"title":"<issue.identifier>: <issue.title>",...}}`
7. Wait for response; from `result.turn.id` take `turn_id`; emit `session_id = format!("{}-{}", thread_id, turn_id)`.

Use **tokio::time::timeout** for each request/response pair so a stuck agent does not hang the worker.

---

## 10.3 Streaming Turn Processing (SPEC §10.3)

- **Loop**: Read lines from stdout (BufReader). For each line, `serde_json::from_str` into a flexible value or ad-hoc struct.
- **Completion**: On notification `method == "turn/completed"` → success; `turn/failed` or `turn/cancelled` → failure. Also treat **turn timeout** (`runner.turn_timeout_ms` from first byte of turn) and **subprocess exit** as failure.
- **Continuation**: After `turn/completed`, if issue still active and turns left, send another `turn/start` with same `threadId` and continuation text; do not restart the process.
- **Stderr**: Spawn a task that reads stderr and logs lines; do not parse as JSON.

---

## Minimal Message Handling (No Full JSON-RPC)

Define an enum for **incoming** messages (responses and notifications) and parse per line:

```rust
#[derive(Debug)]
pub enum AgentMessage {
    Response { id: Option<u64>, result: Option<serde_json::Value>, error: Option<serde_json::Value> },
    Notification { method: String, params: Option<serde_json::Value> },
    Malformed(String),
}

fn parse_line(line: &str) -> AgentMessage {
    let v: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return AgentMessage::Malformed(line.to_string()),
    };
    let obj = match v.as_object() {
        Some(o) => o,
        None => return AgentMessage::Malformed(line.to_string()),
    };
    if obj.contains_key("method") && !obj.contains_key("id") {
        return AgentMessage::Notification {
            method: obj.get("method").and_then(|m| m.as_str()).unwrap_or("").to_string(),
            params: obj.get("params").cloned(),
        };
    }
    AgentMessage::Response {
        id: obj.get("id").and_then(|i| i.as_u64()),
        result: obj.get("result").cloned(),
        error: obj.get("error").cloned(),
    }
}
```

Then in the runner: match on `AgentMessage` to handle `initialize` result, `thread/start` result (extract `thread.id`), `turn/start` result (extract `turn.id`), and notifications `turn/completed`, `turn/failed`, `turn/cancelled`, token usage, etc. No need for a JSON-RPC client crate.

---

## 10.4 Emitted Runtime Events (SPEC §10.4)

For each parsed notification or relevant response, emit an event to the orchestrator (e.g. via channel): `AgentUpdate { issue_id, update: AgentUpdatePayload }`. Payload can include: `session_id`, `thread_id`, `turn_id`, `agent_pid`, `last_agent_event`, `last_agent_timestamp`, `last_agent_message`, token counts, rate limits. Map notification `method` to an event type (e.g. `turn_completed`, `turn_failed`, `notification`).

---

## 10.5 Approval, Tool Calls, User Input (SPEC §10.5)

Implementation-defined. If the protocol sends approval requests (e.g. tool call with id), the runner can auto-approve or fail; document the policy. Unsupported tool calls: respond with a failure result and continue. User-input-required: fail the run (SPEC).

---

## 10.6 Timeouts and Errors (SPEC §10.6)

- **runner.read_timeout_ms**: Per request/response during handshake and sync calls.
- **runner.turn_timeout_ms**: From start of turn until `turn/completed` or `turn/failed`/`turn/cancelled`.
- **runner.stall_timeout_ms**: Enforced by orchestrator (see [07-polling-scheduling.md](07-polling-scheduling.md)).

Map failures to normalized errors: `runner_not_found`, `invalid_workspace_cwd`, `response_timeout`, `turn_timeout`, `turn_failed`, `turn_cancelled`, `turn_input_required`, etc.

---

## Protocol selection: runner.type (codex | acp | cli)

In WORKFLOW.md front matter, set **`runner.type`** to choose the protocol:

- **`codex`** (default): Codex-style protocol (SPEC §10): `initialize` → `initialized` → `thread/start` → `turn/start`, with responses carrying `result.thread.id` and `result.turn.id`, and notifications `turn/completed`, `turn/failed`, `turn/cancelled`. Reference: [OpenAI Codex app-server](https://developers.openai.com/codex/app-server/).
- **`acp`**: ACP (Agent Client Protocol) for Cursor. Use **`runner.command: "agent acp"`**. Flow: `initialize` → `authenticate` → `session/new` → `session/prompt`; we handle `session/request_permission` with `allow-once`. See [Cursor ACP docs](https://cursor.com/docs/cli/acp). Pre-authenticate with `agent login` or `CURSOR_AUTH_TOKEN` / `CURSOR_API_KEY`.
- **`cli`**: Cursor CLI non-interactive mode when `agent acp` is not available (e.g. NixOS). The runner runs `command "$SYMPHONY_PROMPT"` (prompt passed as one argument), reads **stream-json** NDJSON from stdout, and treats completion when a line has `type: "result", subtype: "success"`. Use the same flags you would for a one-shot run: `--print --output-format stream-json` (and optionally `--stream-partial-output`). See [Cursor output format](https://cursor.com/docs/cli/reference/output-format).

Example for Cursor with ACP:

```yaml
runner:
  type: acp
  command: "agent acp"
  read_timeout_ms: 60000
  turn_timeout_ms: 3600000
```

Example for Cursor on NixOS (no `agent acp`):

```yaml
runner:
  type: cli
  command: "/run/current-system/sw/bin/cursor-agent --force --approve-mcps --model auto --force --workspace . --print --output-format stream-json --stream-partial-output"
  turn_timeout_ms: 3600000
  read_timeout_ms: 60000
```

## Debugging: what we send and receive

With **`RUST_LOG=debug`** (or `symphony_agent=debug`), the runner logs:

- **agent_direction=send**, **agent_line**: every JSON line written to the agent’s stdin.
- **agent_direction=recv**, **agent_line**: every JSON line read from the agent’s stdout (handshake and turn loop).
- **agent_stderr**: each line read from the agent’s stderr.

Use this to confirm whether the agent is receiving our messages and what (if anything) it sends back. If you see only “send” lines and no “recv” lines, the agent is not replying on stdout (wrong protocol or wrong command).

---

## References

- [SPEC.md](SPEC.md) §10 — Agent Runner Protocol  
- [07-polling-scheduling.md](07-polling-scheduling.md) — Stall timeout  
- [08-worktree-management.md](08-worktree-management.md) — Workspace path and cwd  
- [Cursor ACP](https://cursor.com/docs/cli/acp) — Cursor’s stdio protocol (different from Codex-style)
