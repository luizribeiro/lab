# capsa-sandbox

Builds a `Command` that runs a child process inside an OS sandbox
(Linux: `syd`, macOS: `sandbox-exec`).

## Design principles

- **Policy compiler, not enforcer**: translates `SandboxSpec` into
  native rules (`syd` on Linux, `sandbox-exec` on macOS) and relies
  on those backends for enforcement.
- **Cross-platform by default**: `Sandbox::builder()` works on both
  Linux and macOS with the same API surface.
- **Embeddable crate, not a service**: no daemon, no IPC — `build()`
  returns a configured `(std::process::Command, Sandbox)`.
- **Hardened by default**: deny-all filesystem/network posture,
  fd sealing via `capsa-process`, and privilege hardening handled
  automatically by the sandbox backend.
- **Relaxed explicitly, not implicitly**: callers opt into
  network/path/ioctl access.
- **No privileges required**: runs as a regular unprivileged user on
  both Linux and macOS. No root, no setuid, no ambient capabilities.
- **Fails fast on unsupported setups**: missing `syd`, unsupported OS,
  invalid fd inheritance — all return errors, never silently continue.

## Basic usage

```rust,no_run
use std::path::Path;
use capsa_sandbox::Sandbox;

let (mut cmd, _sandbox) = Sandbox::builder().build(Path::new("/bin/true"))?;
let status = cmd.status()?;
assert!(status.success());
# Ok::<(), anyhow::Error>(())
```

## Filesystem access

Control which paths the child can read, write, or ioctl. A private
`$TMPDIR` is created automatically and cleaned up when `Sandbox` drops.

```rust,no_run
use std::path::Path;
use capsa_sandbox::Sandbox;

let (mut cmd, _sandbox) = Sandbox::builder()
    .read_only_path("/usr")
    .read_only_path("/etc")
    .read_write_path("/var/data")
    .ioctl_path("/dev/kvm")
    .build(Path::new("/usr/bin/env"))?;
# Ok::<(), anyhow::Error>(())
```

## Network

```rust,no_run
use std::path::Path;
use capsa_sandbox::Sandbox;

let (mut cmd, _sandbox) = Sandbox::builder()
    .allow_network(true)
    .build(Path::new("/usr/bin/curl"))?;
# Ok::<(), anyhow::Error>(())
```

## File descriptors

Pass fds into the child via `inherit_fd`. All other fds `>= 3` are
sealed (`FD_CLOEXEC`) at exec time to prevent leaking privileged
handles.

```rust,no_run
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::Path;
use capsa_sandbox::Sandbox;

let (read_end, mut write_end) = std::io::pipe()?;
writeln!(write_end, "hello")?;
drop(write_end);

let mut builder = Sandbox::builder();
let fd = builder.inherit_fd(read_end.into());

let (mut cmd, _sandbox) = builder.build(Path::new("/bin/sh"))?;
cmd.arg("-c").arg(format!("IFS= read -r line <&{fd}; [ \"$line\" = \"hello\" ]"));
assert!(cmd.status()?.success());
# Ok::<(), anyhow::Error>(())
```

## Resource limits

Enforce POSIX rlimits in the child. On Linux, rlimits are enforced by
syd natively via its `rlimit/` directives. On macOS, they are applied
via `setrlimit` in a `pre_exec` hook.

```rust,no_run
use std::path::Path;
use capsa_sandbox::Sandbox;

let (mut cmd, _sandbox) = Sandbox::builder()
    .max_open_files(64)
    .max_address_space(512 * 1024 * 1024) // 512 MiB
    .max_cpu_time(30)                     // seconds
    .max_processes(32)
    .disable_core_dumps()
    .build(Path::new("/usr/bin/myservice"))?;
# Ok::<(), anyhow::Error>(())
```

## Privilege controls

On Linux, syd drops all capabilities and sets `NO_NEW_PRIVS`
automatically. On macOS, `sandbox-exec` provides equivalent process
isolation. These protections are not configurable through the builder.

## Async (tokio)

Requires `--features tokio`.

```rust,no_run
use std::path::Path;
use capsa_sandbox::{Sandbox, tokio as sandbox_tokio};

let builder = Sandbox::builder().allow_network(true);
let (mut cmd, _sandbox) = sandbox_tokio::build(builder, Path::new("/bin/true"))?;
# Ok::<(), anyhow::Error>(())
```

Platform support: Linux (via `syd`) and macOS (via `sandbox-exec`).
