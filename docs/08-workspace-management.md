# 08 — Workspace Management and Safety

Rust implementation notes for **SPEC §9**. Uses **async** I/O (`tokio::fs`), a **subprocess crate** (or **tokio::process**) for hooks with **timeout and kill**, and **non-blocking** execution so the runtime is not blocked.

---

## Design Choices

| Choice | Decision | Rationale |
|--------|----------|-----------|
| Directory I/O | **Async** (`tokio::fs`) | Keeps workspace create/remove on the async executor; consistent with Tokio runtime. |
| Hooks | **Subprocess with timeout** | Run each hook via `sh -lc <script>` in the workspace dir; enforce `hooks.timeout_ms` and kill on timeout. Use **tokio::process::Command** + **tokio::time::timeout** so the runtime is not blocked. |
| Don't block runtime | **Spawn hook in a task** | Run the hook (and its timeout) in a dedicated async task or `tokio::spawn`; send result back on a channel. The orchestrator/worker awaits the result without blocking the executor. Optionally use **spawn_blocking** only if a sync process crate is used. |

---

## Crates

```toml
[dependencies]
tokio = { version = "1", features = ["fs", "process", "time"] }
# Optional: higher-level subprocess crate with timeout/kill (e.g. async-process, command_timeout)
# If not used: tokio::process::Command + tokio::time::timeout + child.kill() is sufficient.
```

- **tokio::fs**: `create_dir_all`, `remove_dir_all`, `metadata` (to detect existing dir).
- **tokio::process**: `Command::new("sh").args(["-lc", script]).current_dir(&workspace_path)`. Spawn child, then `tokio::time::timeout(Duration::from_millis(timeout_ms), child.wait()).await`; on timeout call `child.kill().await` and return error.

---

## 9.1 Workspace Layout (SPEC §9.1)

- **Root**: From config `workspace.root` (resolved and absolute). Per-issue path: `root.join(sanitize_workspace_key(identifier))`.
- **Sanitize**: Replace any char not in `[A-Za-z0-9._-]` with `_` (see [03-domain-model.md](03-domain-model.md)).
- Workspaces are reused; successful runs do not delete them.

---

## 9.2 Workspace Creation and Reuse (SPEC §9.2)

1. **Sanitize** `issue.identifier` → `workspace_key`.
2. **Path** = `workspace_root.join(workspace_key)`.
3. **Ensure dir exists**: `tokio::fs::create_dir(path).await` — if error is "already exists", treat as reuse (`created_now = false`); else `created_now = true`. Alternatively use `metadata(path).await`; if `is_dir()` then reuse else create.
4. If **created_now** and `hooks.after_create` is set, run the hook (see §9.4); on failure return error and optionally remove the new dir.
5. Return `Workspace { path, workspace_key, created_now }`.

---

## 9.3 Optional Workspace Population (SPEC §9.3)

Implementation-defined (e.g. in `after_create`: git clone, copy template). Failures surface as errors; new workspace creation failure may remove the partial dir.

---

## 9.4 Workspace Hooks (SPEC §9.4)

**Contract**: Execute with `cwd` = workspace path; shell = `sh -lc <script>` (or `bash -lc`); timeout = `hooks.timeout_ms` (default 60000).

**Run without blocking the runtime**:

- Spawn a task that runs the hook and applies the timeout:
  - `let mut child = Command::new("sh").args(["-lc", script]).current_dir(&path).spawn()?;`
  - `match timeout(Duration::from_millis(timeout_ms), child.wait()).await { Ok(Ok(status)) => ... Ok(Err(e)) => ... Err(_) => { child.kill().await; ... timeout error } }`
- The caller (orchestrator or agent runner) `await`s the task result. No blocking of the executor.
- Log start, failure, and timeout.

**Semantics**:

- **after_create** failure/timeout → return error from workspace creation (caller may remove dir).
- **before_run** failure/timeout → return error from run attempt.
- **after_run** / **before_remove** failure/timeout → log and ignore (do not fail the caller).

---

## 9.5 Safety Invariants (SPEC §9.5)

1. **Agent cwd**: Before launching the agent subprocess, set `Command::current_dir(&workspace_path)` and assert (or validate) that `workspace_path` is canonical and equals the intended path.
2. **Path under root**: Normalize `workspace_root` and `workspace_path` to absolute; require `workspace_path.starts_with(workspace_root)` (with path component semantics). Reject if not.
3. **Workspace key**: Use only the sanitized key (chars in `[A-Za-z0-9._-]`) in the directory name.

---

## Removal (terminal cleanup, reconciliation)

Use **tokio::fs::remove_dir_all(path).await** for workspace deletion. Run from the orchestrator task or a dedicated task; do not block the main loop. On failure log and continue (SPEC: before_remove failure is ignored).

---

## References

- [SPEC.md](SPEC.md) §9 — Workspace Management and Safety  
- [03-domain-model.md](03-domain-model.md) — Workspace, sanitize_workspace_key
