---
tracker:
  repo: "owner/repo"
  api_key: "$GITHUB_TOKEN"
  claim_label: "symphony-claimed"
  pr_open_label: "pr-open"
  pr_base_branch: "main"
  active_states: ["open"]
  terminal_states: ["closed"]
runner:
  command: "echo agent"
polling:
  interval_ms: 30000
worktree:
  root: "./.worktrees"
  main_repo_path: "."
agent:
  max_concurrent_agents: 2
  max_turns: 10
---

# Fixture prompt

Minimal workflow for integration tests. Issue: {{ issue.identifier }}.
