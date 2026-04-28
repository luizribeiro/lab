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

## Filesystem policy

Each builder method grants one capability — `read`, `write`, `exec`,
or `ioctl` — on the listed path or directory:

- `.read_path(p)` / `.read_dir(d)` — readable input.
- `.write_path(p)` / `.write_dir(d)` — readable + writable.
- `.exec_path(p)` / `.exec_dir(d)` — `execve` / `posix_spawn`-able,
  plus implied read. `exec_dir` is recursive on both platforms.
- `.ioctl_path(p)` / `.ioctl_dir(d)` — `ioctl`-able, plus implied
  read.

`write`, `exec`, and `ioctl` each imply `read` on the same path, so
nothing needs to be added to `read_*` to mirror them. `read` does
not imply `exec`: readable inputs are not automatically launchable
as new processes. On macOS, `read` does grant `file-map-executable`
so `dlopen` works for any readable shared library, matching the
Linux baseline where `mmap(PROT_EXEC)` of a readable file is not
sandbox-mediated.

A private `$TMPDIR` is created for the child and removed when the
sandbox is dropped.

```rust,no_run
use std::path::Path;
use lockin::Sandbox;

let status = Sandbox::builder()
    .read_dir("/usr")
    .read_path("/etc/ssl/cert.pem")
    .write_dir("/tmp/output")
    .write_path("/var/log/app.log")
    .exec_path("/bin/sh")
    .exec_dir("/nix/store/abc-glibc/lib")
    .ioctl_path("/dev/null")
    .ioctl_dir("/dev/dri")
    .command(Path::new("/usr/bin/env"))?
    .status()?;
# let _ = status;
# Ok::<(), anyhow::Error>(())
```

## Network policy

```rust
use std::path::Path;
use lockin::Sandbox;

let status = Sandbox::builder()
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
use lockin::Sandbox;

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
    .network_allow_all()
    .tokio_command(Path::new("/usr/bin/env"))?
    .status()
    .await?;
assert!(status.success());
# Ok::<(), anyhow::Error>(())
```
