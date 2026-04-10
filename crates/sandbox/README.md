# capsa-sandbox

Builds a `Command` that runs a child process inside an OS sandbox
(Linux: `syd`, macOS: `sandbox-exec`).

## Design principles

- **Policy compiler, not enforcer**: translates sandbox policy into
  native rules (`syd` on Linux, `sandbox-exec` on macOS) and relies
  on those backends for enforcement.
- **Cross-platform by default**: `Sandbox::builder()` works on both
  Linux and macOS with the same API surface.
- **Embeddable crate, not a service**: no daemon, no IPC — `build()`
  returns a configured `(std::process::Command, Sandbox)`.
- **Hardened by default**: deny-all filesystem/network posture, fd
  sealing via [`capsa-process`](../capsa-process), and privilege
  hardening handled automatically by the sandbox backend.
- **Relaxed explicitly, not implicitly**: callers opt into
  network/path/ioctl access.
- **No privileges required**: runs as a regular unprivileged user on
  both Linux and macOS. No root, no setuid, no ambient capabilities.
- **Fails fast on unsupported setups**: missing `syd`, unsupported OS,
  or invalid configuration — all return errors, never silently
  continue.

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

Register fds via `inherit_fd` so the child can access them inside the
sandbox. All non-inherited fds `>= 3` are sealed at exec time (via
[`capsa-process`](../capsa-process)) to prevent leaking privileged
handles.

```rust,no_run
use std::os::fd::AsRawFd;
use std::path::Path;
use capsa_sandbox::Sandbox;

let (reader, writer) = std::os::unix::net::UnixDatagram::pair()?;
let reader_fd = reader.as_raw_fd();

let mut builder = Sandbox::builder();
builder.inherit_fd(reader.into());

let (mut cmd, _sandbox) = builder.build(Path::new("/usr/bin/myworker"))?;
cmd.arg("--fd").arg(reader_fd.to_string());
# Ok::<(), anyhow::Error>(())
```

## Resource limits

Enforce POSIX rlimits in the child. On Linux these are passed to syd
as native `rlimit/` directives. On macOS they are applied via
`setrlimit` before exec.

```rust,no_run
use std::path::Path;
use capsa_sandbox::Sandbox;

let (mut cmd, _sandbox) = Sandbox::builder()
    .max_open_files(64)
    .max_address_space(512 * 1024 * 1024)
    .max_cpu_time(30)
    .max_processes(32)
    .disable_core_dumps()
    .build(Path::new("/usr/bin/myservice"))?;
# Ok::<(), anyhow::Error>(())
```

## Privilege controls

On Linux, syd drops all capabilities and sets `NO_NEW_PRIVS`
automatically. On macOS, `sandbox-exec` provides equivalent process
isolation. These protections are handled by the sandbox backend and
are not configurable through the builder.

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
