# SPEC Addendum 2 — Implementation steps

This directory holds **step documents** for implementing [SPEC_ADDENDUM_2.md](../SPEC_ADDENDUM_2.md) (fix-PR: re-dispatch when checks fail or when someone mentions the bot; orchestrator read-only, single polling loop).

Each step maps to an addendum section and includes implementation notes; tests must be written and all must pass for the step to be complete.

| Step | Document | Addendum section |
|------|----------|------------------|
| 01 | [01-scope-and-opt-in.md](01-scope-and-opt-in.md) | B.1 fix_pr opt-in |
| 02 | [02-fix-pr-candidate-set.md](02-fix-pr-candidate-set.md) | B.2 Fix-PR candidate set, issue→PR resolution |
| 03 | [03-single-polling-loop.md](03-single-polling-loop.md) | B.3 Single polling loop, check status and mentions |
| 04 | [04-dispatch-condition.md](04-dispatch-condition.md) | B.4 Dispatch condition (check failed OR mention) |
| 05 | [05-mention-trigger.md](05-mention-trigger.md) | B.5 mention_handle, B.5.1 Newness rule |
| 06 | [06-definition-of-checks.md](06-definition-of-checks.md) | B.6 Definition of “checks” |
| 07 | [07-agent-fix-pr-behaviour.md](07-agent-fix-pr-behaviour.md) | B.7 Agent behaviour when dispatched for fix-PR |
| 08 | [08-integration-with-base-spec.md](08-integration-with-base-spec.md) | B.8 Interaction with base SPEC and Addendum 1 |
| 09 | [09-config-summary.md](09-config-summary.md) | B.9 Config key summary |

See [09-config-summary.md](09-config-summary.md) for the definition-of-done note and config reference.
