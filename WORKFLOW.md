---
# GitHub issue tracker (required).
# Set GITHUB_TOKEN in the environment; workflow resolves $GITHUB_TOKEN at runtime.
# Token needs read access to Issues only: fine-grained "Issues: Read-only", or classic "public_repo" (public) / "repo" (private). See docs/10-github-tracker.md.
tracker:
  repo: "Industrial/rust-symphony"
  api_key: "$GITHUB_TOKEN"
  active_states: ["open"]
  terminal_states: ["closed"]
  include_labels: ["symphony", "bot"]
  exclude_labels: ["symphony-claimed", "wip"]

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

1. Read the issue and the codebase in this workspace.
2. Implement or fix what the issue asks for.
3. Run tests and fix any failures (`devenv shell -- moon run :test` as appropriate).
4. Follow project conventions (see `.cursor/rules`, `docs/`, and existing code).
5. When done, summarize changes and any follow-ups in a comment or handoff as defined by the project's workflow.
6. **Close this issue** when the work is complete (e.g. via `gh issue close` or the GitHub UI), so the runner stops picking it up. If you cannot close issues, add a clear "ready to close" comment so a maintainer can close it.
