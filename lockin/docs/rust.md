# lockin Rust API

`Sandbox::builder()` provides the same cross-platform API for use
as a library.

## Quick start

```rust
use std::path::Path;
use lockin::Sandbox;

let status = Sandbox::builder()
    // Reads LOCKIN_LIBRARY_DIRS (set by `nix develop` or your own
    // tooling) so dynamically linked binaries can load under syd.
    .library_paths_from_env()
    .command(Path::new("/usr/bin/env"))?
    .status()?;
assert!(status.success());
# Ok::<(), anyhow::Error>(())
```

## syd path (Linux)

On Linux the sandbox delegates enforcement to `syd`. The library
resolves the `syd` binary automatically using this fallback chain:

1. Explicit `.syd_path()` on the builder
2. `LOCKIN_SYD_PATH` environment variable
3. `syd` found in `PATH`

```rust,no_run,ignore
use std::path::Path;
use lockin::Sandbox;

let status = Sandbox::builder()
    .syd_path("/nix/store/.../bin/syd")
    .command(Path::new("/usr/bin/env"))?
    .status()?;
# Ok::<(), anyhow::Error>(())
```

## Library paths

Dynamically linked binaries need their library directories
allowlisted. Use `.library_path()` to grant read (and on Linux,
exec) access:

```rust,no_run,ignore
use std::path::Path;
use lockin::Sandbox;

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
use lockin::Sandbox;

let status = Sandbox::builder()
    .library_paths_from_env()
    .read_only_dir("/usr")
    .read_only_dir("/etc")
    .read_only_path("/dev/null")
    .command(Path::new("/usr/bin/env"))?
    .status()?;
assert!(status.success());
# Ok::<(), anyhow::Error>(())
```

## Network policy

```rust
use std::path::Path;
use lockin::Sandbox;

let status = Sandbox::builder()
    .library_paths_from_env()
    .allow_network(true)
    .command(Path::new("/usr/bin/env"))?
    .status()?;
assert!(status.success());
# Ok::<(), anyhow::Error>(())
```

## File descriptor policy

Pass only the fds the child needs; all other fds `>= 3` are sealed
at exec time (via [`lockin-process`](../crates/process)).

```rust
use std::os::fd::AsRawFd;
use std::path::Path;
use lockin::Sandbox;

let (reader, _writer) = std::os::unix::net::UnixDatagram::pair()?;
let fd = reader.as_raw_fd();

let mut builder = Sandbox::builder()
    .library_paths_from_env();
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
use lockin::Sandbox;

let status = Sandbox::builder()
    .library_paths_from_env()
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
use lockin::Sandbox;

let status = Sandbox::builder()
    .library_paths_from_env()
    .allow_network(true)
    .tokio_command(Path::new("/usr/bin/env"))?
    .status()
    .await?;
assert!(status.success());
# Ok::<(), anyhow::Error>(())
```
