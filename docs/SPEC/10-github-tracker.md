# 10 — Issue Tracker Integration (GitHub Issues)

Rust implementation notes for **SPEC §11**. Uses **reqwest** for HTTP and **serde** / **chrono** for GitHub API responses and normalization into the domain `Issue` model.

**Deliverable:** Unit tests must be written for all code; implementation is not complete without them. See [16-testing.md](16-testing.md).

---

## Crates

```toml
[dependencies]
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
```

- **reqwest**: Async HTTP client; `get()`, query params, `Authorization` header, timeout (e.g. 30s), pagination via `Link` or `page`.
- **serde_json**: Deserialize GitHub API JSON into structs; then map to domain `Issue` ([03-domain-model.md](03-domain-model.md)).

---

## 11.1 Required Operations (SPEC §11.1)

1. **fetch_candidate_issues()** — Open (or configured active) issues for the repo; exclude PRs; paginate.
2. **fetch_issues_by_states(state_names)** — For startup cleanup; e.g. `state=closed` when terminal includes closed.
3. **fetch_issue_states_by_ids(issue_ids)** — Current state for given IDs (reconciliation); map `node_id` or numeric id to repo + number, then `GET /repos/{owner}/{repo}/issues/{number}`.

---

## 11.2 API Semantics (SPEC §11.2)

- **Base**: `tracker.endpoint` (default `https://api.github.com`). Auth: `Authorization: Bearer <token>` or `Authorization: token <token>`.
- **List issues**: `GET /repos/{owner}/{repo}/issues?state=open&per_page=100&sort=created&direction=asc&page={n}`. Parse `Link` for next page; repeat until no next.
- **Single issue**: `GET /repos/{owner}/{repo}/issues/{issue_number}`. For reconciliation, maintain a map `node_id` or numeric `id` → `(owner, repo, number)` or fetch by number if you store number in the issue id.
- **Exclude PRs**: GitHub list endpoint returns both; filter items where `pull_request` is absent (or use a dedicated issues endpoint if available).
- **Timeout**: `reqwest::Client::builder().timeout(Duration::from_secs(30))`.

### Tracker is read-only; agent closes issues (SPEC §1, §7–8)

The **orchestrator** only reads the tracker (list issues, single issue for reconciliation). It does not close issues or add comments. When a worker exits normally, the orchestrator schedules a short **continuation retry**; when that runs, it **re-fetches the current state** of those issues. If an issue is now in a **terminal state** (e.g. `closed`), the orchestrator releases the claim and does **not** re-dispatch. So to stop the runner from re-picking the same issue, the **coding agent** (or a human) must **close the issue** when done, using whatever tools the agent has (e.g. GitHub CLI in the workspace, or a comment for a maintainer to close). See WORKFLOW.md prompt instructions.

### Token permissions (SPEC §11.1–11.5)

The orchestrator **only reads** issues (no writes). Required API calls:

| Operation | Endpoint |
|-----------|----------|
| List issues (candidates, terminal cleanup) | `GET /repos/{owner}/{repo}/issues?state=...&per_page=100&page={n}` |
| Single issue (reconciliation) | `GET /repos/{owner}/{repo}/issues/{issue_number}` |

Use the **minimum** scope that allows these:

- **Fine-grained PAT**: Repository permissions → **Issues: Read-only** for the target repo(s). (Use "Read and write" only if another part of the system, e.g. the agent, will use the same token to update issues.)
- **Classic PAT**: For a **public** repo, **`public_repo`** is enough. For a **private** repo, **`repo`** (full control of private repositories) is required to read issues.

No other scopes (workflow, packages, org, etc.) are needed for the tracker.

---

## 11.3 Normalization (SPEC §11.3)

Map each GitHub issue to [Issue](03-domain-model.md):

| Field | Source |
|-------|--------|
| `id` | `node_id` (preferred) or `id.to_string()` |
| `identifier` | `format!("{}/{}#{}", owner, repo, number)` |
| `title` | `title` |
| `description` | `body` (Option) |
| `priority` | None (or from labels if extended) |
| `state` | `state` (open/closed), store lowercase |
| `branch_name` | None unless extended |
| `url` | `html_url` |
| `labels` | `labels.iter().map(|l| l.name.to_lowercase()).collect()` |
| `blocked_by` | `[]` (or labels/convention if extended) |
| `created_at` | `chrono::DateTime::parse_from_rfc3339` then convert to `Utc` |
| `updated_at` | same |

Use [sanitize_workspace_key](03-domain-model.md) for workspace paths.

---

## 11.4 Error Handling (SPEC §11.4)

Typed errors (e.g. **thiserror**):

- `MissingTrackerApiKey`, `MissingTrackerRepo` (config validation).
- `GitHubApiRequest` (transport).
- `GitHubApiStatus(u16)` (non-2xx).
- `GitHubUnknownPayload` (parse/deserialize failure).

Orchestrator behavior: candidate fetch failure → log, skip dispatch this tick. State refresh failure → log, keep workers. Terminal fetch failure → log warning, continue startup.

---

## 11.5 Tracker Writes

Orchestrator does not perform tracker writes. Agent can use an optional GitHub tool (e.g. `github_api`) for comments/state changes; see SPEC §11.5.

---

## References

- [SPEC.md](SPEC.md) §11 — Issue Tracker Integration  
- [03-domain-model.md](03-domain-model.md) — Issue, sanitize_workspace_key
