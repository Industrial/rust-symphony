# Firecracker sandbox for agent runs

When `runner.sandbox: firecracker` is set, the agent runs inside a Firecracker microVM with the git worktree mounted, so LLM-generated code cannot affect the host.

## Configuration

In workflow front matter:

```yaml
runner:
  command: "sh -lc 'cursor-agent …'"
  sandbox: firecracker
  firecracker:
    kernel_path: "/path/to/vmlinux"   # or $VAR
    rootfs_path: "/path/to/rootfs.ext4"
    worktree_guest_path: "/worktree"  # optional, default /worktree
    vsock_port: 5000                  # optional, default 5000
```

- **kernel_path**, **rootfs_path**: Resolved with `$VAR` and `~` like `worktree.root`. Must point to a Linux kernel built for Firecracker and a rootfs image (e.g. ext4).
- **worktree_guest_path**: Path inside the guest where the worktree is mounted.
- **vsock_port**: Port the guest agent-runner listens on (host connects to `guest_cid:port`).

If `sandbox` is `firecracker` but `firecracker` (or valid kernel/rootfs paths) is missing, config load fails.

## Required assets

Operators must supply:

1. **Kernel**: A Linux kernel built for Firecracker (e.g. `vmlinux`). See [Firecracker kernel policy](https://github.com/firecracker-microvm/firecracker/blob/main/docs/kernel-policy.md).
2. **Rootfs**: A rootfs image (e.g. ext4) that includes:
   - The worktree mounted at `worktree_guest_path` (the host mounts it via a virtio-blk drive).
   - An **agent-runner** service that listens on vsock and implements the guest protocol below.

The repository does **not** bundle a default kernel or rootfs; paths are configurable only.

## Guest agent-runner protocol

The rootfs must run a service that:

1. Listens on the configured vsock port (e.g. 5000).
2. Accepts one connection per run.
3. Reads one JSON line: `{"command":"sh -lc …","cwd":"/worktree"}`.
4. Runs the command in the given `cwd` (worktree path in the guest).
5. Proxies stdin from the connection to the process; forwards process stdout and stderr to the host over framed messages.
6. Sends the process exit code when done.

**Frame format (guest → host):** `[u8 tag][u32 len_be][bytes]`  
- Tag `1`: stdout  
- Tag `2`: stderr  
- Tag `3`: exit (length 4, i32 exit code in big-endian)

**Host → guest:** Raw stdin bytes.

## Behaviour

- With `runner.sandbox: none` (or unset), the agent runs on the host as before (no VM).
- With `runner.sandbox: firecracker` and valid kernel/rootfs config, the runner uses the `symphony-sandbox` crate to start the microVM, run the agent command inside it, and expose a process-like handle (stdin/stdout/stderr, exit status) so the existing agent protocol layer is unchanged.
- VM is stopped and resources released when the run finishes (success, failure, or timeout).

## Implementation status

- **Config / wiring:** In place (`runner.sandbox`, `runner.firecracker`, agent passes config into spawn).
- **Guest runner:** Implemented in `crates/symphony-guest-runner`: vsock server, JSON line `{"command","cwd"}`, framed protocol (tags 1=stdout, 2=stderr, 3=exit). Build with `cargo build -p symphony-guest-runner`.
- **Kernel (Nix):** `linuxFirecracker` and `vmlinuxFirecracker` in the flake (x86_64-linux): `pkgs.linux.override` with `extraConfig` for VIRTIO_VSOCKETS, VIRTIO_BLK, etc. Use `nix build .#vmlinuxFirecracker`; kernel_path can point at `$(nix path-info .#vmlinuxFirecracker)/dev/vmlinux` or equivalent.
- **Guest rootfs (Nix):** `symphony-guest-rootfs-tree` and `symphony-guest-rootfs` (ext4 image) in the flake. Minimal rootfs with init (mount /proc, /sys, exec symphony-guest-runner), busybox, and the guest-runner binary. Build with `nix build .#symphony-guest-rootfs` or `nix run .#build-guest-rootfs` (writes to `./symphony-guest-rootfs.ext4` by default).
- **Host (symphony-sandbox, firecracker feature):** Implemented. Uses fctools (UnrestrictedVmmExecutor, ResourceSystem, VmmInstallation), Vm::prepare/start with kernel, rootfs, and worktree as second virtio-blk drive, vsock device (guest_cid=3), then host connects to vsock UDS, sends JSON `{"command","cwd"}`, demuxes framed stdout/stderr/exit. Firecracker binary discovered via `SYMPHONY_FIRECRACKER` or `which firecracker`. VM shutdown on SandboxChild drop or after wait(). Guest rootfs should mount the second block device at `worktree_guest_path` (e.g. `/worktree`) in init for the worktree to be available.

## Building kernel and rootfs (Nix)

On **x86_64-linux** the flake provides:

| Nix output | Description |
|------------|-------------|
| `vmlinuxFirecracker` | Kernel (use path to `dev/vmlinux` inside the store path) |
| `symphony-guest-rootfs` | ext4 rootfs image |
| `build-guest-rootfs` | App that writes `./symphony-guest-rootfs.ext4` |

**Build commands:**

```bash
# Kernel (path ends with .../dev/vmlinux)
nix build .#vmlinuxFirecracker
KERNEL_PATH="$(nix path-info .#vmlinuxFirecracker)/dev/vmlinux"

# Rootfs image
nix build .#symphony-guest-rootfs
ROOTFS_PATH="$(nix path-info .#symphony-guest-rootfs)"

# Or write rootfs to current directory
nix run .#build-guest-rootfs
# Uses ./symphony-guest-rootfs.ext4
```

**Config paths:** Set `kernel_path` and `rootfs_path` in workflow front matter to these paths (or to copies). Paths support `$VAR` and `~` expansion like `worktree.root`.

**Firecracker binary:** The host looks for the Firecracker binary via `SYMPHONY_FIRECRACKER` (env) or `which firecracker`. Install Firecracker on the host (e.g. from [releases](https://github.com/firecracker-microvm/firecracker/releases)) or set `SYMPHONY_FIRECRACKER` to the binary path.

**Reproducibility:** Kernel and rootfs are Nix derivations; the same flake inputs (e.g. `nixpkgs` revision) produce the same store paths and identical artifacts. Use `nix path-info .#vmlinuxFirecracker` and `nix path-info .#symphony-guest-rootfs` to get stable paths for config. Pinning the flake (e.g. in CI) ensures reproducible builds.

## Integration test

To run the sandbox integration test (run a command in the VM and assert on stdout/exit):

```bash
export SYMPHONY_SANDBOX_INTEGRATION=1
export SYMPHONY_KERNEL_PATH="/path/to/vmlinux"
export SYMPHONY_ROOTFS_PATH="/path/to/rootfs.ext4"
cargo test -p symphony-sandbox --features firecracker --test integration_firecracker run_command_in_vm_stdout_and_exit -- --ignored
```

The test is `#[ignore]` by default so it does not run in normal `cargo test`.
