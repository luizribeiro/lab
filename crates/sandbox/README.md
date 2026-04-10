# capsa-sandbox

`capsa-sandbox` builds a `Command` that runs a child process inside an OS sandbox (Linux: `syd`, macOS: `sandbox-exec`) with explicit filesystem, network, and fd controls.

## Usage

```rust,no_run
use std::path::Path;
use capsa_sandbox::Sandbox;

let (mut cmd, _sandbox) = Sandbox::builder().build(Path::new("/bin/true"))?;
let status = cmd.status()?;
assert!(status.success());
# Ok::<(), anyhow::Error>(())
```

### Path allowlists + network policy

```rust,no_run
use std::path::Path;
use capsa_sandbox::Sandbox;

let (mut cmd, _sandbox) = Sandbox::builder()
    .allow_network(false)
    .read_only_path("/usr")
    .read_only_path("/etc")
    .read_write_path("/tmp")
    .build(Path::new("/usr/bin/env"))?;

let _child = cmd.spawn()?;
# Ok::<(), anyhow::Error>(())
```

### FD inheritance

```rust,no_run
use std::io::Write;
use std::os::fd::AsRawFd;
use std::path::Path;
use capsa_sandbox::Sandbox;

let (read_end, mut write_end) = std::io::pipe()?;
writeln!(write_end, "hello")?;
drop(write_end);

let mut builder = Sandbox::builder();
let fd = builder.inherit_fd(read_end.into())?; // child sees the same fd number

let (mut cmd, _sandbox) = builder.build(Path::new("/bin/sh"))?;
cmd.arg("-c").arg(format!("IFS= read -r line <&{fd}; [ \"$line\" = \"hello\" ]"));
assert!(cmd.status()?.success());
# Ok::<(), anyhow::Error>(())
```

### Security hardening

**FD sealing** — by default, `SandboxBuilder` sets `FD_CLOEXEC` on
every fd `>= 3` not registered via `inherit_fd`, so leaked privileged
fds are closed at exec time. Disable with `close_non_inherited_fds(false)`.

**Resource limits** — enforce POSIX rlimits in the child via `setrlimit`:

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

**Privilege hardening** (Linux only; no-op on macOS):

- `no_new_privs(bool)` — enabled by default. Calls
  `prctl(PR_SET_NO_NEW_PRIVS)` to block setuid/file-capability escalation.
- `drop_all_capabilities()` — clears all Linux capability sets
  (effective, permitted, inheritable, ambient, bounding).

### Tokio async (`--features tokio`)

```rust,no_run
use std::path::Path;
use capsa_sandbox::{Sandbox, tokio as sandbox_tokio};

let builder = Sandbox::builder().allow_network(true);
let (mut cmd, _sandbox) = sandbox_tokio::build(builder, Path::new("/bin/true"))?;
let status = cmd.spawn()?.wait().await?;
assert!(status.success());
# Ok::<(), anyhow::Error>(())
```

Platform support: Linux (via `syd`) and macOS (via `sandbox-exec`).
