# 14 — Security and Operational Safety

Rust implementation notes for **SPEC §15**. No new crates; document trust boundary, filesystem and secret handling, hook safety, and hardening. Implementation of secrets and paths uses **shellexpand** (config) and path checks (workspace manager) already described in earlier docs.

**Deliverable:** Unit tests must be written for all code; implementation is not complete without them. See [16-testing.md](16-testing.md).

---

## 15.1 Trust Boundary (SPEC §15.1)

Document in the implementation:

- **Environment**: Trusted only, or restrictive/multi-tenant; what operators and code can do.
- **Approval/sandbox**: Whether the runner uses auto-approve, operator approval, or stricter sandbox; document the chosen policy.

---

## 15.2 Filesystem Safety (SPEC §15.2)

**Mandatory** (already in [08-worktree-management.md](08-worktree-management.md)):

- Workspace path is under `workspace.root`; validate with normalized absolute paths and `path.starts_with(root)`.
- Agent subprocess is launched with `current_dir(workspace_path)` only.
- Workspace directory names use sanitized identifiers only (`[A-Za-z0-9._-]`).

**Recommended**: Dedicated OS user; restrict workspace root permissions; separate volume if possible.

---

## 15.3 Secret Handling (SPEC §15.3)

- **$VAR resolution**: Use shellexpand (or equivalent) in config layer ([05-configuration.md](05-configuration.md)); do not log resolved tokens.
- **Validation**: Check presence of required secrets (e.g. `tracker.api_key`) after resolution without printing values; fail validation with a generic message.

---

## 15.4 Hook Script Safety (SPEC §15.4)

- Hooks are trusted configuration from WORKFLOW.md; they run in the workspace dir with full shell.
- Truncate hook output in logs; enforce timeout ([08-worktree-management.md](08-worktree-management.md)).

---

## 15.5 Harness Hardening (SPEC §15.5)

- Tighten runner approval/sandbox where supported.
- Add isolation (containers, VM, network limits) if needed.
- Filter which issues/labels/repos are eligible for dispatch.
- Scope any GitHub client-side tool to the configured repo.
- Reduce tools and credentials to the minimum required.

Document chosen measures and treat hardening as part of the core safety model.

---

## References

- [SPEC.md](SPEC.md) §15 — Security and Operational Safety  
- [05-configuration.md](05-configuration.md) — Env resolution  
- [08-worktree-management.md](08-worktree-management.md) — Path and cwd invariants
