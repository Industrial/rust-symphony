---
tracker:
  repo: "owner/repo"
  api_key: "$GITHUB_TOKEN"
  active_states: ["open"]
  terminal_states: ["closed"]
runner:
  command: "echo agent"
polling:
  interval_ms: 30000
worktree:
  root: "./.worktrees"
agent:
  max_concurrent_agents: 2
  max_turns: 10
---

# Fixture prompt

Minimal workflow for integration tests. Issue: {{ issue.identifier }}.
