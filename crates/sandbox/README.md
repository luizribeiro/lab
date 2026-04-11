# capsa-sandbox

Build and run a child process inside an OS sandbox.

- Linux backend: `syd`
- macOS backend: `sandbox-exec`

`Sandbox::builder()` provides one cross-platform API.

## Quick start

```rust
use std::path::Path;
use capsa_sandbox::Sandbox;

let status = Sandbox::builder()
    .command(Path::new("/usr/bin/env"))?
    .status()?;
assert!(status.success());
# Ok::<(), anyhow::Error>(())
```

## Design principles

- **Policy compiler, not enforcer**: translates your policy to native backend rules.
- **Secure defaults**: deny-all filesystem/network; explicit allowlists only.
- **Explicit relaxation**: access is granted only through builder methods.
- **No privileged runtime**: works as a regular user.
- **Fail closed**: unsupported OS/backend/configuration returns errors.

## syd path (Linux)

On Linux the sandbox delegates enforcement to `syd`. The caller must
supply the absolute path to the `syd` binary via `.syd_path()`:

```rust,ignore
use std::path::Path;
use capsa_sandbox::Sandbox;

let status = Sandbox::builder()
    .syd_path("/nix/store/.../bin/syd")
    .command(Path::new("/usr/bin/env"))?
    .status()?;
# Ok::<(), anyhow::Error>(())
```

Callers typically read an environment variable (e.g. `CAPSA_SYD_PATH`)
and pass it through the builder.

## Library paths

Dynamically linked binaries need their library directories
allowlisted. Use `.library_path()` to grant read (and on Linux,
exec) access:

```rust,ignore
use std::path::Path;
use capsa_sandbox::Sandbox;

let status = Sandbox::builder()
    .library_path("/usr/lib")
    .command(Path::new("/usr/bin/env"))?
    .status()?;
# Ok::<(), anyhow::Error>(())
```

## Filesystem policy

A private `$TMPDIR` is created for the child and removed when
the sandbox is dropped.

```rust
use std::path::Path;
use capsa_sandbox::Sandbox;

let status = Sandbox::builder()
    .read_only_dir("/usr")
    .read_only_dir("/etc")
    .command(Path::new("/usr/bin/env"))?
    .status()?;
assert!(status.success());
# Ok::<(), anyhow::Error>(())
```

## Network policy

```rust
use std::path::Path;
use capsa_sandbox::Sandbox;

let status = Sandbox::builder()
    .allow_network(true)
    .command(Path::new("/usr/bin/env"))?
    .status()?;
assert!(status.success());
# Ok::<(), anyhow::Error>(())
```

## File descriptor policy

Pass only the fds the child needs; all other fds `>= 3` are sealed
at exec time (via [`capsa-process`](../capsa-process)).

```rust
use std::os::fd::AsRawFd;
use std::path::Path;
use capsa_sandbox::Sandbox;

let (reader, _writer) = std::os::unix::net::UnixDatagram::pair()?;
let fd = reader.as_raw_fd();

let mut builder = Sandbox::builder();
builder.inherit_fd(reader.into());

let status = builder
    .command(Path::new("/usr/bin/env"))?
    .status()?;
assert!(status.success());
# Ok::<(), anyhow::Error>(())
```

## Resource limits

```rust
use std::path::Path;
use capsa_sandbox::Sandbox;

let status = Sandbox::builder()
    .max_open_files(64)
    .max_cpu_time(30)
    .max_processes(32)
    .disable_core_dumps()
    .command(Path::new("/usr/bin/env"))?
    .status()?;
assert!(status.success());
# Ok::<(), anyhow::Error>(())
```

## Tokio support

Enable with `--features tokio`.

```rust,ignore
use std::path::Path;
use capsa_sandbox::Sandbox;

let status = Sandbox::builder()
    .allow_network(true)
    .tokio_command(Path::new("/usr/bin/env"))?
    .status()
    .await?;
assert!(status.success());
# Ok::<(), anyhow::Error>(())
```
