# capsa-sandbox

Build and run a child process inside an OS sandbox.

- Linux backend: `syd`
- macOS backend: `sandbox-exec`

`Sandbox::builder()` provides one cross-platform API and returns
`(std::process::Command, Sandbox)`.

## Quick start

```rust,no_run
use std::path::Path;
use capsa_sandbox::Sandbox;

let (mut cmd, _sandbox) = Sandbox::builder()
    .build(Path::new("/bin/true"))?;

let status = cmd.status()?;
assert!(status.success());
# Ok::<(), anyhow::Error>(())
```

## Design principles

- **Policy compiler, not enforcer**: translates your policy to native backend rules.
- **Secure defaults**: deny-all filesystem/network; explicit allowlists only.
- **Explicit relaxation**: access is granted only through builder methods.
- **No privileged runtime**: works as a regular user.
- **Fail closed**: unsupported OS/backend/configuration returns errors.

## Filesystem policy

A private `$TMPDIR` is created for the child and removed when
`Sandbox` is dropped.

```rust,no_run
use std::path::Path;
use capsa_sandbox::Sandbox;

let (_cmd, _sandbox) = Sandbox::builder()
    .read_only_path("/usr")
    .read_only_path("/etc")
    .read_write_path("/var/data")
    .ioctl_path("/dev/kvm")
    .build(Path::new("/usr/bin/env"))?;
# Ok::<(), anyhow::Error>(())
```

## Network policy

```rust,no_run
use std::path::Path;
use capsa_sandbox::Sandbox;

let (_cmd, _sandbox) = Sandbox::builder()
    .allow_network(true)
    .build(Path::new("/usr/bin/curl"))?;
# Ok::<(), anyhow::Error>(())
```

## File descriptor policy

Pass only the fds the child needs; all other fds `>= 3` are sealed
at exec time (via [`capsa-process`](../capsa-process)).

```rust,no_run
use std::os::fd::AsRawFd;
use std::path::Path;
use capsa_sandbox::Sandbox;

let (reader, _writer) = std::os::unix::net::UnixDatagram::pair()?;
let fd = reader.as_raw_fd();

let mut builder = Sandbox::builder();
builder.inherit_fd(reader.into());

let (mut cmd, _sandbox) = builder.build(Path::new("/usr/bin/myworker"))?;
cmd.arg("--fd").arg(fd.to_string());
# Ok::<(), anyhow::Error>(())
```

## Resource limits

```rust,no_run
use std::path::Path;
use capsa_sandbox::Sandbox;

let (_cmd, _sandbox) = Sandbox::builder()
    .max_open_files(64)
    .max_address_space(512 * 1024 * 1024)
    .max_cpu_time(30)
    .max_processes(32)
    .disable_core_dumps()
    .build(Path::new("/usr/bin/myservice"))?;
# Ok::<(), anyhow::Error>(())
```

## Tokio support

Enable with `--features tokio`.

```rust,no_run
use std::path::Path;
use capsa_sandbox::{Sandbox, tokio as sandbox_tokio};

let builder = Sandbox::builder().allow_network(true);
let (_cmd, _sandbox) = sandbox_tokio::build(builder, Path::new("/bin/true"))?;
# Ok::<(), anyhow::Error>(())
```
