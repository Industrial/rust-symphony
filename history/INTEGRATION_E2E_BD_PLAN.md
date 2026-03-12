# Integration and E2E Testing — bd Swarm Plan

**Epic:** `rust-symphony-fv0` — Integration and E2E testing (no unit tests)

Unit tests are out of scope (handled elsewhere). This plan covers only **integration tests** (wiremock, tempfile, fixtures, mock orchestrator, fake agent subprocess) and **E2E** (CLI, optional real tracker smoke). Dependencies are minimized so **bd swarm** can maximize parallel work.

---

## Dependency tiers

### Tier 0 — No dependencies (6 tasks, all parallel)

| ID | Title |
|----|--------|
| `rust-symphony-fv0.1` | Add dev-deps to symphony-tracker (wiremock, tokio) |
| `rust-symphony-fv0.2` | Add dev-deps to symphony-workspace (tempfile, tokio) |
| `rust-symphony-fv0.3` | Add dev-deps to symphony-config for integration tests (tokio) |
| `rust-symphony-fv0.4` | Add dev-dependencies to symphony-runner (mockall, tokio) |
| `rust-symphony-fv0.5` | Add dev-deps to symphony-agent for protocol integration tests (tokio) |
| `rust-symphony-fv0.6` | Create tests fixtures dir and sample WORKFLOW.md for config tests |

**Swarm:** All 6 can be claimed and completed in parallel.

---

### Tier 1 — One or two blockers (5 integration-test tasks)

Each task depends only on the prerequisites it needs (no cross-deps between these).

| ID | Title | Blocked by |
|----|--------|------------|
| `rust-symphony-fv0.7` | symphony-config: integration test load WORKFLOW from fixture | fv0.6 (fixtures), fv0.3 (config dev-deps) |
| `rust-symphony-fv0.8` | symphony-tracker: wiremock integration test fetch candidates | fv0.1 |
| `rust-symphony-fv0.9` | symphony-workspace: tempfile integration test worktree create and path under root | fv0.2 |
| `rust-symphony-fv0.10` | symphony-agent: integration test fake subprocess NDJSON handshake | fv0.5 |
| `rust-symphony-fv0.11` | symphony-runner: orchestrator integration test with mock tracker and workspace | fv0.4 |

**Swarm:** After Tier 0 is done, all 5 become ready and can be worked in parallel.

---

### Tier 2 — Runner E2E (2 tasks, same blocker)

Both depend only on runner dev-deps, so they become ready with the other Tier 1 tasks (no extra chain).

| ID | Title | Blocked by |
|----|--------|------------|
| `rust-symphony-fv0.12` | symphony-runner: CLI integration test workflow path and exit codes | fv0.4 |
| `rust-symphony-fv0.13` | symphony-runner: integration feature and real tracker smoke test (ignored) | fv0.4 |

**Swarm:** Ready as soon as fv0.4 is closed; can run in parallel with fv0.11 and each other.

---

## Summary

- **Total tasks:** 1 epic + 13 tasks (all under epic `rust-symphony-fv0`).
- **Max parallel at start:** 6 (Tier 0).
- **Max parallel after Tier 0:** 7 (fv0.7, fv0.8, fv0.9, fv0.10, fv0.11, fv0.12, fv0.13).
- **No task** depends on another integration-test task; only on dev-deps or fixtures.

---

## bd commands (for swarm)

```bash
# See ready work (no blocking deps)
devenv shell -- bd ready --json

# Claim a task
devenv shell -- bd update <id> --status in_progress --claim

# Complete
devenv shell -- bd close <id> --reason "Done"
```

---

## Reference

- **Strategy:** [history/TESTING_STRATEGY.md](TESTING_STRATEGY.md)
- **Spec:** [docs/SPEC/16-testing.md](../docs/SPEC/16-testing.md), SPEC §17
