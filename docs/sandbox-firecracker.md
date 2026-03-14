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

The **symphony-sandbox** crate provides the API and process-like handle; config and agent integration are in place. Full VM lifecycle (fctools `Vm::prepare`/start, worktree drive, vsock connection to the guest agent-runner) is implemented behind the `firecracker` feature and currently returns a clear error until a compatible guest rootfs and fctools wiring are completed. Use `runner.sandbox: none` for host process runs.
