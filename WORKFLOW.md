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
