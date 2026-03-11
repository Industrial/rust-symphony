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

# Run all tests (Moon)
devenv shell -- moon run :test

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
- **Tasks:** [Moon](https://moonrepo.dev/) is used for workspace tasks. Each crate has `check` and `test`; run e.g. `devenv shell -- moon run symphony-domain:test`.
- **Environment:** Use [devenv](https://devenv.sh/) 2.x and `devenv shell --` for all commands (see `.cursor/rules/shell.mdc`). Install with `nix profile install github:cachix/devenv#default`. On first run after config changes, use `Ctrl+Alt+R` in the shell to reload (2.0 native reloading).
- **Quality:** Unit tests are required for all code (see [docs/16-testing.md](docs/16-testing.md)); implementation is not complete without them.

## License

See [LICENSE](LICENSE) for terms applicable to this repository.
