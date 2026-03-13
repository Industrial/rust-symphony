# Symphony Testing Strategy: QA Perspective

**Purpose:** Define how we write tests, which layer each kind of test belongs to, whether we chase “every use case” or “all permutations,” and how to set it up. This extends and operationalizes `docs/SPEC/16-testing.md` and SPEC §17.

---

## 1. The Three Layers (and what belongs where)

| Layer | Scope | Boundaries | Speed | Flakiness | When to use |
|-------|--------|------------|--------|-----------|-------------|
| **Unit** | Single crate/module/function | No I/O, no real network, no real FS (or temp dirs only). Dependencies mocked or in-memory. | Fast (ms) | Low | Pure logic, parsing, validation, domain rules, sorting, retry math. |
| **Integration** | Multiple crates or one crate + real-ish I/O | Real temp dirs, mock HTTP (wiremock), mock subprocess (script/binary), or in-process orchestrator with mocked tracker/workspace/agent. **No** live GitHub, **no** real agent. | Medium (seconds) | Low–medium | Tracker client vs wiremock, config load from fixture, workspace lifecycle, orchestrator state machine with mocks, agent protocol parsing against a fake stdin/stdout. |
| **E2E** | Whole system | Real binary, real (or test) repo, optional real agent or stub. Optional `GITHUB_TOKEN`; often `#[ignore]` in CI. | Slow (tens of seconds+) | Higher | Smoke: “runner starts, one poll, exits”; optional real-API smoke. |

**Rule of thumb:** If it needs the real GitHub API or a real coding agent run, it’s E2E (or “real integration” per SPEC §17.8). If it can be satisfied with wiremock + mock traits, it’s integration. If it’s pure logic and data, it’s unit.

---

## 2. Can we cover every possible use case? Should we cover all permutations?

**Short answer:** No to “all permutations”; yes to “every **meaningful** use case” if we define “meaningful” by risk and spec.

- **Every use case (scenario):** We **should** aim to cover every scenario that the spec and product care about (e.g. “reconciliation stops worker when issue goes terminal,” “retry backoff cap,” “PRs excluded from candidates”). That’s a finite list from SPEC §17 and the implementation notes.
- **All permutations:** We **should not** exhaustively combine every parameter (e.g. every label set × every state × every error code). That’s combinatorially explosive and doesn’t scale.

**Practical approach:**

1. **Equivalence classes:** Group inputs that should behave the same (e.g. “any 4xx” vs “any 5xx” for tracker errors). One test per class (or a small number) is enough.
2. **Boundary / one-of-each:** For enums or small sets (e.g. `active_states`, terminal vs non-terminal), one test per important value or one test that exercises “all supported values” is enough; we don’t need every combination with other dimensions.
3. **Risk-based:** More tests for failure paths and reconciliation/retry logic (where bugs are costly); fewer for “happy path” variations that are structurally the same.
4. **Spec as checklist:** Use SPEC §17 (and `docs/SPEC/16-testing.md`) as the scenario checklist. Each bullet is a “use case”; we don’t need multiple tests per bullet unless the bullet clearly has sub-cases (e.g. 401 vs 404 vs 500).

So: **cover every spec-relevant scenario (use case), but not every permutation.** Use equivalence classes and boundaries to keep the suite finite and maintainable.

---

## 3. Which tests belong in which layer?

### 3.1 Unit tests (in-crate, `#[cfg(test)]` or `tests/` next to code)

- **symphony-domain:** Ordering (priority, created_at, identifier), any pure domain helpers.
- **symphony-workflow:** Parse front matter, split body, error on invalid YAML or non-map.
- **symphony-config:** Defaults, `$VAR` resolution (with env set in test), validation rules (required fields, dispatch validation). Use fixture paths or in-memory strings; no real file I/O required if we pass `&str` or `Path`.
- **symphony-orchestration:** Sort order, `can_dispatch`, retry delay math, `apply_agent_update` / `apply_worker_exit` state transitions with in-memory state.
- **symphony-workspace:** Sanitize identifier → path (deterministic); “path under root” checks with temp dir or mock root.
- **symphony-tracker:** **Parsing/normalization only** (e.g. JSON → `Issue`, exclude PRs) with inline JSON; no HTTP.
- **symphony-prompt:** Render Liquid with `issue` + `attempt`; unknown variable → error.
- **symphony-agent:** Parse NDJSON lines, handshake state machine, timeout logic (with fake time if possible). No real subprocess.

**Location:** `mod tests` in the same file or `crates/<crate>/src/<module>/tests.rs` (or similar). Run with `cargo test` / nextest per crate.

### 3.2 Integration tests (cross-crate or I/O with mocks)

- **Workflow + config:** Load real `WORKFLOW.md` from a **fixture file** in repo (e.g. `tests/fixtures/WORKFLOW.md`); assert parsed config and prompt body. Invalid YAML, missing file → error. **Belongs in symphony-config or symphony-workflow** in `crates/<crate>/tests/` (Rust integration tests).
- **Tracker client:** **wiremock** in front of GitHub-shaped endpoints; assert client parses and normalizes, pagination, 401/404/500 handling. **Belongs in symphony-tracker** `tests/` (e.g. `tracker_wiremock.rs`).
- **Workspace manager:** **tempfile** for root; create worktree, call hooks (maybe no-op or script), assert path under root, reuse (created_now = false). **Belongs in symphony-workspace** `tests/`.
- **Orchestrator:** **mockall** (or manual mocks) for `Tracker` and workspace/agent; feed messages (PollTick, WorkerExit, AgentUpdate, Reconcile); assert state transitions and that “terminate worker” or “clean workspace” is requested. **Belongs in symphony-runner** (or a shared test harness) because the loop owns the state machine; use a test channel for `OrchestratorMessage`.
- **Agent runner protocol:** Subprocess = **small script or test binary** that writes NDJSON to stdout; assert handshake and turn/start sent, session_id and completion parsed. Timeout = script that sleeps; assert timeout path. **Belongs in symphony-agent** `tests/`.

**Location:** `crates/<crate>/tests/*.rs`. These are the “integration” tests that SPEC §17.1–17.5 describe. They do not call real GitHub or run a real agent.

### 3.3 E2E / real integration (optional, often skipped in CI)

- **CLI:** Run `symphony-runner` binary with valid/missing workflow path; assert exit code and stderr. Can be a small Rust integration test that runs the binary, or a script. **Can live in symphony-runner** `tests/` or `bin/` script.
- **Real tracker (SPEC §17.8):** Requires `GITHUB_TOKEN`; create/close a test issue or use a dedicated test repo; mark `#[ignore]` or gate on `--features integration` + env. **Single smoke test** is enough.
- **Full runner smoke:** Optional: run one poll (or dry-run) against real API; or run with a “no-op” agent that exits immediately. Heavy; only for release validation or nightly.

**Location:** `symphony-runner/tests/` or top-level `bin/`; run with `cargo test --features integration` or `cargo test --ignored` when token is set.

---

## 4. Mapping SPEC §17 to layers

| SPEC §17 section | Primary layer | Notes |
|------------------|---------------|--------|
| 17.1 Workflow and config parsing | Unit + integration | Unit: defaults, validation, `$VAR`. Integration: load from fixture file, error cases. |
| 17.2 Workspace manager | Unit + integration | Unit: sanitize path, “under root” logic. Integration: tempfile, create/hooks/reuse. |
| 17.3 Issue tracker client | Unit + integration | Unit: parse JSON → Issue (no HTTP). Integration: wiremock for list/issues, pagination, errors. |
| 17.4 Orchestrator dispatch, reconciliation, retry | Unit + integration | Unit: sort, can_dispatch, retry math. Integration: mock tracker/workspace/agent, drive loop or transition API. |
| 17.5 Agent runner | Unit + integration | Unit: parse NDJSON, timeouts (mock time). Integration: fake subprocess script. |
| 17.6 Observability | Unit or integration | Log snapshot or inject failure; assert process continues. |
| 17.7 CLI and lifecycle | Integration (or E2E) | Invoke binary; missing path, startup failure, clean exit. |
| 17.8 Real integration | E2E | Optional; `#[ignore]` or feature-flagged; smoke only. |

---

## 5. Setup: how to implement this

### 5.1 Dev-dependencies (per crate that needs them)

Add only where needed:

```toml
[dev-dependencies]
tokio = { version = "1", features = ["test-util", "rt", "macros"] }   # async tests
mockall = "0.13"   # symphony-runner (orchestrator), optionally agent/workspace
wiremock = "0.6"   # symphony-tracker
tempfile = "3"     # symphony-workspace
```

- **symphony-domain, symphony-config, symphony-prompt, symphony-orchestration:** minimal (maybe only `tokio` for any async helpers used in tests).
- **symphony-tracker:** `tokio`, `wiremock`.
- **symphony-workspace:** `tokio`, `tempfile`.
- **symphony-agent:** `tokio`, optionally `mockall` for protocol tests.
- **symphony-runner:** `tokio`, `mockall`, and possibly shared test utilities.

### 5.2 Where integration tests live

- **Rust integration tests:** `crates/<crate>/tests/*.rs`. Each file is a separate crate; use the crate under test as a dependency. Good for: tracker+wiremock, workspace+tempfile, config+fixture, agent+fake subprocess, orchestrator+mocks.
- **Fixtures:** `crates/<crate>/tests/fixtures/` or repo-root `tests/fixtures/` (e.g. `WORKFLOW.md`, sample JSON for tracker).
- **E2E / real integration:** `symphony-runner/tests/` for CLI tests; optional `tests/real_integration.rs` with `#[ignore]` and env check for `GITHUB_TOKEN`.

### 5.3 Running tests

- **Real integration:**  
  - `cargo test --features integration` and skip if `GITHUB_TOKEN` unset, or  
  - `cargo test --ignored` when token is set (e.g. in release workflow or manually).

### 5.4 Optional feature flag for real integration

In `symphony-runner` (or a dedicated integration crate):

```toml
[features]
integration = []   # Enables tests that require network/token
```

Then in test:

```rust
#[cfg_attr(not(feature = "integration"), ignore)]
#[tokio::test]
async fn real_tracker_smoke() {
  if std::env::var("GITHUB_TOKEN").is_err() {
    eprintln!("SKIP: GITHUB_TOKEN not set");
    return;
  }
  // ...
}
```

CI runs without `--features integration` (and without token), so real integration is skipped; release or manual runs can enable it.

---

## 6. Summary: decision guide

- **Pure function or in-memory logic?** → **Unit** (same crate, `#[cfg(test)]` or adjacent test module).
- **Real file system (temp dir) or real HTTP shape (mock server) or real subprocess (fake binary)?** → **Integration** (`crates/<crate>/tests/*.rs`).
- **Real GitHub API or real agent run?** → **E2E / real integration** (optional, `#[ignore]` or feature-gated).
- **“Should this scenario be tested?”** → If it’s in SPEC §17 or a clear product requirement, yes; one (or a few) tests per scenario.
- **“Should we test every combination of X and Y?”** → No; use equivalence classes and boundaries, and add combinations only when risk justifies it.

This gives a clear, maintainable split: **unit tests for logic, integration tests for I/O and cross-crate behavior with mocks/fakes, E2E for rare smoke checks**, and no attempt to cover every permutation—only every meaningful use case from the spec and risk.
