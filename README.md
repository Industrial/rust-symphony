# RustSymphony

A Rust implementation of the **Symphony** orchestrator: a long-running service that reads work from GitHub Issues, creates an isolated git worktree per issue, and runs a coding agent session for that issue inside the git worktree.

## What Symphony Does

- **Polls** the issue tracker on a fixed cadence and dispatches work with bounded concurrency.
- **Maintains** a single authoritative orchestrator state for dispatch, retries, and reconciliation.
- **Creates** deterministic per-issue git worktrees and preserves them across runs.
- **Stops** active runs when issue state changes make them ineligible (reconciliation).
- **Recovers** from transient failures with configurable exponential backoff.
- **Loads** runtime behavior from a repository-owned `WORKFLOW.md` (YAML front matter + prompt body).
- **Exposes** operator-visible observability (e.g. structured logs).

Symphony is a **scheduler/runner and tracker reader**. Ticket writes (state transitions, comments, PR links) are performed by the coding agent using tools in its runtime environment.

## Crates

| Crate | Purpose |
|-------|---------|
| `symphony-domain` | Core domain types: Issue, WorkflowDefinition, Worktree, RunAttempt, Session, RetryEntry, OrchestratorState (SPEC §4). |
| `symphony-workflow` | Workflow loader: path resolution, YAML front matter parsing, prompt body (SPEC §5). |
| `symphony-config` | Typed config: defaults, `$VAR` resolution (shellexpand), dispatch validation (SPEC §6). |
| `symphony-orchestration` | Orchestrator messages, claim state, scheduling (sort, retry delay), runtime snapshot (SPEC §7, §8, §12). |
| `symphony-workspace` | Git worktree path resolution and safety checks (SPEC §9). |
| `symphony-agent` | Agent runner protocol: NDJSON-over-stdio message parsing (SPEC §10). |
| `symphony-tracker` | GitHub issue tracker: normalization to domain Issue, error types (SPEC §11). |
| `symphony-prompt` | Prompt construction: Liquid templating from Issue + attempt (SPEC §12). |
| `symphony-runner` | Main binary: poll loop, orchestrator, workflow load, tracker and agent integration. |

## Quick Start

### Install the CLI (no Rust required)

```bash
curl -fsSL https://raw.githubusercontent.com/Industrial/rust-symphony/main/install.sh | sh
```

Optional: set `SYMPHONY_VERSION=vX.Y.Z` to pin a release, or `SYMPHONY_INSTALL_DIR=/path` to choose the install directory (default: `~/.local/bin`, or `/usr/local/bin` when run as root). Add the install directory to your `PATH` if needed.

Alternatively, with a Rust toolchain: `cargo install symphony-runner`.

### Development

**Prerequisites:** Rust toolchain, [devenv](https://devenv.sh/) 2.x for the development environment.

```bash
# Build
devenv shell -- cargo build

# Run code coverage (llvm-cov; enforces 95% for symphony-agent and symphony-domain)
devenv shell -- moon run :test-coverage

# Run the orchestrator (optional workflow path; config via env)
devenv shell -- cargo run -p symphony-runner -- /path/to/WORKFLOW.md

# Dry-run: one poll cycle, log candidates and what would be dispatched, then exit (no workers or tracker writes)
devenv shell -- cargo run -p symphony-runner -- --dry-run /path/to/WORKFLOW.md
```

## Documentation

- **Specification:** [docs/SPEC.md](docs/SPEC.md) — language-agnostic Symphony service spec.
- **Addendum 1:** [docs/SPEC_ADDENDUM_1.md](docs/SPEC_ADDENDUM_1.md) — label filtering, durable claim, PR-driven workflow.
- **Addendum 2:** [docs/SPEC_ADDENDUM_2.md](docs/SPEC_ADDENDUM_2.md) — fix-PR: re-dispatch when checks fail or when someone mentions the bot (e.g. `@symphony`). Top-level **`fix_pr`** in workflow front matter (default: `false`) opts in; when `true`, the orchestrator applies fix-PR logic for issues with `pr_open_label`. When `false` or omitted, no check-status or mention polling occurs. This repo sets `fix_pr: true` in `WORKFLOW.md`.
- **Rust implementation notes:** `docs/SPEC/` — problem statement, domain model, workflow, config, orchestration, polling, git worktree, agent runner, tracker, prompt construction, logging, failure recovery, security, reference algorithms, testing, checklist.

## Development

- **Rust:** 2021 edition. Format with `cargo fmt`, lint with `cargo clippy`.
- **CI / Cachix:** Nix-based CI (e.g. `setup-nix-devenv`) can use a [Cachix](https://www.cachix.org/) binary cache to speed up builds. The cache name is set in the workflow (e.g. `rust-symphony`). **Read-only:** If the cache is public, no secrets are required; CI only pulls from the cache. **Read + write:** To push new store paths to the cache, add a Cachix auth token to GitHub Secrets as `CACHIX_AUTH_TOKEN` and ensure the job that runs the Nix/Cachix steps passes it in `env`. The reusable action skips pushing on PRs from forks to avoid leaking write access.
- **Tasks:** [Moon](https://moonrepo.dev/) is used for workspace tasks. Each crate has `check` and `test`; run e.g. `devenv shell -- moon run symphony-domain:test`.
- **Tests:** Use [cargo-nextest](https://nexte.st/) for faster, parallel test runs: Per-crate `cargo test` remains available via `moon run <crate>:test`. **Sandbox/E2E (x86_64-linux):** `devenv shell -- ./bin/run-sandbox-e2e` builds kernel and rootfs with Nix and runs the Firecracker integration tests; see [docs/SPEC/16-testing.md](docs/SPEC/16-testing.md) §17.9.
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
# Token: for orchestrator-only, Issues: Read-only (or classic public_repo/repo). For PR-driven workflow the agent needs write: Issues, Pull requests, Contents. See docs/SPEC/10-github-tracker.md.
# Addendum 1 (docs/SPEC_ADDENDUM_1.md): include_labels / exclude_labels filter candidates; claim_label is auto-excluded so the agent can "claim" an issue; pr_base_branch (default main) for worker branches and PR target; pr_open_label optional for PR-driven flow.
# Addendum 2 (docs/SPEC_ADDENDUM_2.md): fix_pr re-dispatches the agent when the PR has failing checks or when someone mentions the configured handle (e.g. @symphony).
fix_pr: true
tracker:
  repo: "Industrial/rust-symphony"
  api_key: "$GITHUB_TOKEN"
  active_states: ["open"]
  terminal_states: ["closed"]
  include_labels: ["symphony", "bot"]
  exclude_labels: ["symphony-claimed", "wip"]
  claim_label: "symphony-claimed"
  pr_open_label: "pr-open"
  mention_handle: "symphony"

# Command to run the coding agent in each git worktree (required).
# Change this to use a different agent; it is run with cwd = per-issue git worktree.
# type: "codex" (default) | "acp" | "cli"
#   codex = Codex-style protocol (thread/start, turn/start, turn/completed).
#   acp   = Cursor ACP (agent acp). Command: "agent acp".
#   cli   = Cursor non-interactive: prompt as argument, parse stream-json. Use when "agent acp" is not available (e.g. NixOS).
# Optional: set SYMPHONY_EXIT_ON_WORKER_FAILURE=1 to exit with code 1 on first worker failure.
# Debug: RUST_LOG=debug to see agent_direction=send|recv and agent_line.
runner:
  type: cli
  # command: "/run/current-system/sw/bin/cursor-agent --force --approve-mcps --model auto --force --workspace . --print --output-format stream-json --stream-partial-output"
  command: "/run/current-system/sw/bin/cursor-agent --force --approve-mcps --model auto --force --workspace . --print --output-format text"
  turn_timeout_ms: 3600000
  read_timeout_ms: 60000
  stall_timeout_ms: 300000

# How often to poll the tracker (default 30_000 ms).
polling:
  interval_ms: 60000

# Root directory for per-issue git worktrees. Supports $VAR and ~.
# Default if omitted: system temp dir / symphony_worktrees.
worktree:
  root: "./.symphony_worktrees"

# Optional: agent concurrency and retry.
agent:
  max_concurrent_agents: 3
  max_turns: 20
  max_retry_backoff_ms: 300000
---

# Prompt template (how the agent receives the ticket)

The text below is the prompt sent to the agent for each issue. Edit it to change how the ticket is presented.
Liquid variables: `issue` (title, identifier, state, description, url, labels), optional `attempt` (retry number), optional `workflow` (when set: `workflow.pr_base_branch` — base branch for worker branches and PR target, e.g. main or develop).
See https://shopify.github.io/liquid/ for syntax.

**Tracker and completion:** The runner only **reads** the tracker (no write access). When the agent is done, it should **close the issue** (or move it to a terminal state) using its own tools (e.g. GitHub CLI, API, or a comment for a human to close). Once the issue is closed, the runner will stop re-dispatching it on the next poll.

**Claim label (Addendum 1):** Add the claim label `symphony-claimed` to this issue as your first step (e.g. `gh issue edit … --add-label symphony-claimed`). That prevents other workers from picking the same issue and survives restarts.

**PR-driven handoff (Addendum 1):** Worker branches MUST be based off the configured base branch (e.g. fetch and checkout `main` or the branch in `workflow.pr_base_branch`, then create `symphony/issue-<number>` from it). When opening a PR, target that same base (e.g. `gh pr create --base {{ workflow.pr_base_branch | default: "main" }} --body "Fixes #N"`). You MUST (1) push your branch, (2) open a PR with body containing "Fixes #N", (3) post a comment on this issue with the PR URL. Do not consider the task complete until all three are done. Use `gh pr create` and `gh issue comment <number> --body "PR: <url>"` if available. Do **not** merge the PR—a human merges; closing the issue happens when the PR is merged. Optionally add the label `pr-open` to the issue when the PR is open.

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

This is **attempt {{ attempt }}**. A previous run may have been interrupted or failed; continue from the current state of the git worktree.
{% endif %}

## Instructions

1. **Claim the issue** (if the workflow uses a claim label): Add the claim label to this issue first (e.g. `gh issue edit … --add-label symphony-claimed`) so no other worker picks it up.
2. Read the issue and the codebase in this git worktree.
3. **Fix-PR runs (SPEC_ADDENDUM_2 B.7):** If this issue already has an open PR (e.g. you were re-dispatched because CI failed or someone requested changes): use the **current branch**, pull or rebase as needed, make changes to address the failure or review feedback, commit and push. Do **not** open a new PR. When done, you may post a short comment on the issue or PR (e.g. "Pushed fixes for CI.").
4. **New work:** Otherwise, implement what the issue asks for. Use a single branch per issue (e.g. `symphony/issue-<number>`), created from the base branch (e.g. `main` or `{{ workflow.pr_base_branch | default: "main" }}`), if opening a PR.
5. Run tests and fix any failures (`devenv shell -- moon run :test` as appropriate).
6. Follow project conventions (see `.cursor/rules`, `docs/`, and existing code).
7. When done, either:
   - **PR-driven:** You MUST complete all of: (1) push your branch, (2) open a PR with body "Fixes #N" targeting the base branch (e.g. `gh pr create --base main --body "Fixes #N"` or use `--base {{ workflow.pr_base_branch | default: "main" }}` when the variable is set), (3) post a comment on this issue with the PR link (e.g. `gh issue comment <issue-number> --body "PR: https://github.com/…"`). Do not exit until all three are done. Do not merge the PR—a human merges. Optionally add the `pr_open_label` to the issue when the PR is open. **Or**
   - **Direct close:** Summarize in a comment, then **close this issue** (e.g. `gh issue close` or GitHub UI) so the runner stops picking it up. If you cannot close issues, add a clear "ready to close" comment for a maintainer.

## Before you're done (required for PR-driven flow)

If you are using the PR-driven handoff, you must complete every item before exiting:

- [ ] Branch pushed to origin
- [ ] PR opened with body containing "Fixes #N" (replace N with the issue number from this ticket)
- [ ] Comment on this issue with the PR link
```
