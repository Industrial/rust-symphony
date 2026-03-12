# RustSymphony

A Rust implementation of the **Symphony** orchestrator: a long-running service that reads work from GitHub Issues, creates an isolated workspace per issue, and runs a coding agent session for that issue inside the workspace.

## What Symphony Does

- **Polls** the issue tracker on a fixed cadence and dispatches work with bounded concurrency.
- **Maintains** a single authoritative orchestrator state for dispatch, retries, and reconciliation.
- **Creates** deterministic per-issue workspaces and preserves them across runs.
- **Stops** active runs when issue state changes make them ineligible (reconciliation).
- **Recovers** from transient failures with configurable exponential backoff.
- **Loads** runtime behavior from a repository-owned `WORKFLOW.md` (YAML front matter + prompt body).
- **Exposes** operator-visible observability (e.g. structured logs).

Symphony is a **scheduler/runner and tracker reader**. Ticket writes (state transitions, comments, PR links) are performed by the coding agent using tools in its runtime environment.

## Crates

| Crate | Purpose |
|-------|---------|
| `symphony-domain` | Core domain types: Issue, WorkflowDefinition, Workspace, RunAttempt, Session, RetryEntry, OrchestratorState (SPEC §4). |
| `symphony-workflow` | Workflow loader: path resolution, YAML front matter parsing, prompt body (SPEC §5). |
| `symphony-config` | Typed config: defaults, `$VAR` resolution (shellexpand), dispatch validation (SPEC §6). |
| `symphony-orchestration` | Orchestrator messages, claim state, scheduling (sort, retry delay), runtime snapshot (SPEC §7, §8, §12). |
| `symphony-workspace` | Workspace path resolution and safety checks (SPEC §9). |
| `symphony-agent` | Agent runner protocol: NDJSON-over-stdio message parsing (SPEC §10). |
| `symphony-tracker` | GitHub issue tracker: normalization to domain Issue, error types (SPEC §11). |
| `symphony-prompt` | Prompt construction: Liquid templating from Issue + attempt (SPEC §12). |
| `symphony-runner` | Main binary: poll loop, orchestrator, workflow load, tracker and agent integration. |

## Quick Start

**Prerequisites:** Rust toolchain, [devenv](https://devenv.sh/) 2.x for the development environment.

```bash
# Build
devenv shell -- cargo build

# Run all tests (recommended: nextest, via Moon)
devenv shell -- moon run :test-nextest

# Run code coverage (llvm-cov; enforces 95% for symphony-agent and symphony-domain)
devenv shell -- moon run :test-coverage

# Run the orchestrator (optional workflow path; config via env)
devenv shell -- cargo run -p symphony-runner -- /path/to/WORKFLOW.md

# Dry-run: one poll cycle, log candidates and what would be dispatched, then exit (no workers or tracker writes)
devenv shell -- cargo run -p symphony-runner -- --dry-run /path/to/WORKFLOW.md
```

## Documentation

- **Specification:** [docs/SPEC.md](docs/SPEC.md) — language-agnostic Symphony service spec.
- **Rust implementation notes:** `docs/01-problem-and-goals.md` through `docs/17-implementation-checklist.md` — problem statement, domain model, workflow, config, orchestration, polling, workspace, agent runner, tracker, prompt construction, logging, failure recovery, security, reference algorithms, testing, checklist.

## Development

- **Rust:** 2021 edition. Format with `cargo fmt`, lint with `cargo clippy`.
- **CI / Cachix:** Nix-based CI (e.g. `setup-nix-devenv`) can use a [Cachix](https://www.cachix.org/) binary cache to speed up builds. The cache name is set in the workflow (e.g. `rust-symphony`). **Read-only:** If the cache is public, no secrets are required; CI only pulls from the cache. **Read + write:** To push new store paths to the cache, add a Cachix auth token to GitHub Secrets as `CACHIX_AUTH_TOKEN` and ensure the job that runs the Nix/Cachix steps passes it in `env`. The reusable action skips pushing on PRs from forks to avoid leaking write access.
- **Tasks:** [Moon](https://moonrepo.dev/) is used for workspace tasks. Each crate has `check` and `test`; run e.g. `devenv shell -- moon run symphony-domain:test`.
- **Tests:** Use [cargo-nextest](https://nexte.st/) for faster, parallel test runs: `devenv shell -- moon run :test-nextest` (CI uses this). Per-crate `cargo test` remains available via `moon run <crate>:test`.
- **Coverage:** [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) runs tests with coverage and enforces 95% thresholds for `symphony-agent` and `symphony-domain`: `devenv shell -- moon run :test-coverage`. Thresholds and package list are defined in `bin/test-coverage`.
- **Environment:** Use [devenv](https://devenv.sh/) 2.x and `devenv shell --` for all commands (see `.cursor/rules/shell.mdc`). Install with `nix profile install github:cachix/devenv#default`. On first run after config changes, use `Ctrl+Alt+R` in the shell to reload (2.0 native reloading).
- **Quality:** Unit tests are required for all code (see [docs/16-testing.md](docs/16-testing.md)); implementation is not complete without them.

## License

See [LICENSE](LICENSE) for terms applicable to this repository.

## [WORKFLOW.md](WORKFLOW.md)
```
---
# GitHub issue tracker (required).
# Set GITHUB_TOKEN in the environment; workflow resolves $GITHUB_TOKEN at runtime.
# Token needs read access to Issues only: fine-grained "Issues: Read-only", or classic "public_repo" (public) / "repo" (private). See docs/10-github-tracker.md.
# Addendum 1 (docs/SPEC_ADDENDUM_1.md): include_labels / exclude_labels filter candidates; claim_label is auto-excluded so the agent can "claim" an issue; pr_open_label optional for PR-driven flow.
tracker:
  repo: "Industrial/rust-symphony"
  api_key: "$GITHUB_TOKEN"
  active_states: ["open"]
  terminal_states: ["closed"]
  include_labels: ["symphony", "bot"]
  exclude_labels: ["symphony-claimed", "wip"]
  claim_label: "symphony-claimed"
  pr_open_label: "pr-open"

# Command to run the coding agent in each workspace (required).
# Change this to use a different agent; it is run with cwd = per-issue workspace.
# type: "codex" (default) | "acp" | "cli"
#   codex = Codex-style protocol (thread/start, turn/start, turn/completed).
#   acp   = Cursor ACP (agent acp). Command: "agent acp".
#   cli   = Cursor non-interactive: prompt as argument, parse stream-json. Use when "agent acp" is not available (e.g. NixOS).
# Optional: set SYMPHONY_EXIT_ON_WORKER_FAILURE=1 to exit with code 1 on first worker failure.
# Debug: RUST_LOG=debug to see agent_direction=send|recv and agent_line.
runner:
  type: cli
  command: "/run/current-system/sw/bin/cursor-agent --force --approve-mcps --model auto --force --workspace . --print --output-format stream-json --stream-partial-output"
  turn_timeout_ms: 3600000
  read_timeout_ms: 60000
  stall_timeout_ms: 300000

# How often to poll the tracker (default 30_000 ms).
polling:
  interval_ms: 60000

# Root directory for per-issue workspaces. Supports $VAR and ~.
# Default if omitted: system temp dir / symphony_workspaces.
workspace:
  root: "./.symphony_workspaces"

# Optional: agent concurrency and retry.
agent:
  max_concurrent_agents: 3
  max_turns: 20
  max_retry_backoff_ms: 300000
---

# Prompt template (how the agent receives the ticket)

The text below is the prompt sent to the agent for each issue. Edit it to change how the ticket is presented.
Liquid variables: `issue` (title, identifier, state, description, url, labels), optional `attempt` (retry number).
See https://shopify.github.io/liquid/ for syntax.

**Tracker and completion:** The runner only **reads** the tracker (no write access). When the agent is done, it should **close the issue** (or move it to a terminal state) using its own tools (e.g. GitHub CLI, API, or a comment for a human to close). Once the issue is closed, the runner will stop re-dispatching it on the next poll.

**Claim label (Addendum 1):** Add the claim label `symphony-claimed` to this issue as your first step (e.g. `gh issue edit … --add-label symphony-claimed`). That prevents other workers from picking the same issue and survives restarts.

**PR-driven handoff (Addendum 1):** You may work on a single branch per issue (e.g. `symphony/issue-<number>`), open a PR with "Fixes #N" in the body, post a comment on the issue with the PR link, then exit. Do **not** merge the PR—a human merges; closing the issue happens when the PR is merged. Optionally add the label `pr-open` to the issue when the PR is open.

---

# Symphony workflow — RustSymphony

You are working on a GitHub issue for the **RustSymphony** project (a Rust implementation of the Symphony orchestrator).

## Issue

- **Title:** {{ issue.title }}
- **Identifier:** {{ issue.identifier }}
- **State:** {{ issue.state }}
{% if issue.url %}
- **URL:** {{ issue.url }}
{% endif %}
{% if issue.labels.size > 0 %}
- **Labels:** {{ issue.labels | join: ", " }}
{% endif %}

{% if issue.description %}
## Description

{{ issue.description }}
{% endif %}

{% if attempt %}
## Attempt

This is **attempt {{ attempt }}**. A previous run may have been interrupted or failed; continue from the current state of the workspace.
{% endif %}

## Instructions

1. **Claim the issue** (if the workflow uses a claim label): Add the claim label to this issue first (e.g. `gh issue edit … --add-label symphony-claimed`) so no other worker picks it up.
2. Read the issue and the codebase in this workspace.
3. Implement or fix what the issue asks for. Use a single branch per issue (e.g. `symphony/issue-<number>`) if opening a PR.
4. Run tests and fix any failures (`devenv shell -- moon run :test` as appropriate).
5. Follow project conventions (see `.cursor/rules`, `docs/`, and existing code).
6. When done, either:
   - **PR-driven:** Open a PR (body: "Fixes #N"), comment on the issue with the PR link, then exit. Do not merge; a human merges and the issue will close when the PR is merged. Optionally add the `pr_open_label` to the issue when the PR is open. **Or**
   - **Direct close:** Summarize in a comment, then **close this issue** (e.g. `gh issue close` or GitHub UI) so the runner stops picking it up. If you cannot close issues, add a clear "ready to close" comment for a maintainer.
```
