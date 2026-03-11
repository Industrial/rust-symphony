---
# GitHub issue tracker (required).
# Set GITHUB_TOKEN in the environment; workflow resolves $GITHUB_TOKEN at runtime.
tracker:
  repo: "Industrial/rust-symphony"
  api_key: "$GITHUB_TOKEN"
  active_states: ["open"]
  terminal_states: ["closed"]

# Command to run the coding agent in each workspace (required).
# Runs with cwd = per-issue workspace. Use an agent that speaks the runner protocol
# (e.g. Cursor Agent, or another NDJSON-over-stdio compatible runner).
runner:
  command: "cursor agent"
  turn_timeout_ms: 3600000
  read_timeout_ms: 5000
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
3. Run tests and fix any failures (`devenv shell -- cargo test`, `devenv shell -- moon run :test` as appropriate).
4. Follow project conventions (see `.cursor/rules`, `docs/`, and existing code).
5. When done, summarize changes and any follow-ups in a comment or handoff as defined by the project's workflow.
