# lockin Rust API

`Sandbox::builder()` provides the same cross-platform API for use
as a library.

The `program` argument to `.command(...)` must be an absolute path
or a relative path with a `/`. lockin does not perform `PATH`
lookup: the sandbox's exec allowlist is derived from the resolved
binary, so callers must hand in an explicit path (the same
constraint the CLI enforces on `argv[0]`).

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
recursive exec) access. The Linux exec grant is required so the
dynamic linker (`ld-linux*.so.*`) inside the directory can launch
the configured command; it also makes every other binary in that
directory exec-able. macOS does not have this exception.

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
    .read_dir("/usr")
    .read_dir("/etc")
    .read_path("/dev/null")
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
    .network_allow_all()
    .command(Path::new("/usr/bin/env"))?
    .status()?;
assert!(status.success());
# Ok::<(), anyhow::Error>(())
```

## File descriptor policy

Pass only the fds the child needs. Any fd `>= 3` not explicitly
inherited via `inherit_fd` / `map_fd` / `keep_fd` is marked
`FD_CLOEXEC` inside the child immediately before exec, then closed
by the kernel at `execve`. The sweep covers the full fd range on
Linux ≥ 5.11 (single `close_range` syscall); on older Linux and on
macOS it iterates fds in `[3, min(RLIMIT_NOFILE, 65536))`. Fds
opened in the parent after `Sandbox::builder().command()` returns
are still covered. Implemented by [`lockin-process`](../crates/process).

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

## Raw sandbox-exec rules (macOS)

Trusted-policy escape hatch for darwin sandbox operations not
expressible through the structured API — `iokit-open`,
`mach-lookup`, `sysctl-read`, and the rest of the sandbox-exec
S-expression surface. Rules are appended verbatim after the
structured allows.

Raw rules can broaden sandbox authority arbitrarily. They can also
invoke named bundles defined by the macOS system profile but not
enabled by default: a single `(system-network)` token unlocks
routing-socket egress, mDNS, and the network-extension service
surface; `(system-graphics)` unlocks the IOKit GPU surface. The
caller is responsible for the safety of every rule passed in.
Ignored on non-darwin platforms.

```rust,no_run,ignore
use std::path::Path;
use lockin::Sandbox;

let status = Sandbox::builder()
    .library_paths_from_env()
    .raw_seatbelt_rule(
        r#"(allow iokit-open (iokit-user-client-class "AGXDeviceUserClient"))"#,
    )
    .raw_seatbelt_rule(
        r#"(allow mach-lookup (global-name "com.apple.windowserver.active"))"#,
    )
    .raw_seatbelt_rule("(allow sysctl-read)")
    .command(Path::new("/usr/bin/env"))?
    .status()?;
# Ok::<(), anyhow::Error>(())
```

Malformed rules cause `sandbox-exec` to reject the profile at spawn
time; the child exits with `sandbox-exec`'s failure status.

## Tokio support

Enable with `--features tokio`.

```rust,ignore
use std::path::Path;
use lockin::Sandbox;

let status = Sandbox::builder()
    .library_paths_from_env()
    .network_allow_all()
    .tokio_command(Path::new("/usr/bin/env"))?
    .status()
    .await?;
assert!(status.success());
# Ok::<(), anyhow::Error>(())
```
