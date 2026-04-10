# capsa-sandbox

Builds a `Command` that runs a child process inside an OS sandbox
(Linux: `syd`, macOS: `sandbox-exec`).

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

Pass fds into the child via `inherit_fd`. By default, all other fds
`>= 3` get `FD_CLOEXEC` at exec time to prevent leaking privileged
handles. Disable with `close_non_inherited_fds(false)`.

```rust,no_run
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::Path;
use capsa_sandbox::Sandbox;

let (read_end, mut write_end) = std::io::pipe()?;
writeln!(write_end, "hello")?;
drop(write_end);

let mut builder = Sandbox::builder();
let fd = builder.inherit_fd(read_end.into())?;

let (mut cmd, _sandbox) = builder.build(Path::new("/bin/sh"))?;
cmd.arg("-c").arg(format!("IFS= read -r line <&{fd}; [ \"$line\" = \"hello\" ]"));
assert!(cmd.status()?.success());
# Ok::<(), anyhow::Error>(())
```

## Resource limits

Enforce POSIX rlimits in the child via `setrlimit`:

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

## Privilege controls (Linux)

These methods only exist on Linux (`cfg(target_os = "linux")`).

- `no_new_privs(bool)` — enabled by default. Calls
  `prctl(PR_SET_NO_NEW_PRIVS)` to block setuid/file-capability
  escalation.
- `allow_capability(Capability)` — by default all Linux capabilities
  are dropped. Call this for each capability the child needs.

```rust,no_run
# #[cfg(target_os = "linux")]
# {
use std::path::Path;
use capsa_sandbox::{Capability, Sandbox};

let (mut cmd, _sandbox) = Sandbox::builder()
    .allow_capability(Capability::NetBindService)
    .build(Path::new("/usr/bin/myservice"))?;
# }
# Ok::<(), anyhow::Error>(())
```

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
