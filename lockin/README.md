# lockin

Build and run a child process inside an OS sandbox.

- Linux backend: `syd`
- macOS backend: `sandbox-exec`

## CLI

The `lockin` command runs any program inside a sandbox configured
by a TOML file:

```sh
lockin [-c <config>] [--] <program> [args...]
```

Config resolution: if `-c` is given, that file is used (error if
missing). Otherwise `./lockin.toml` is used if it exists. If
neither is found, a deny-all default policy applies.

### Example

```toml
# lockin.toml
command = ["/usr/bin/python3"]

[sandbox]
allow_network = false

[filesystem]
read_only_dirs = ["/usr/lib/python3.11", "/etc/ssl/certs"]
library_dirs_from_env = true

[limits]
max_open_files = 1024
max_cpu_time = 60
```

```sh
lockin -- script.py --verbose
lockin -c sandbox.toml -- myapp --flag
```

### Shebang support

The CLI supports Linux-portable shebangs via the `-c` short flag
with an attached value:

```python
#!/usr/bin/lockin -c/etc/lockin/python3.toml

import json, sys
print(json.load(sys.stdin)["name"])
```

The config's `command` field specifies the interpreter. Trailing
arguments from the command line are appended to it.

### Config reference

All fields are optional. Everything defaults to deny/false/empty.

| Field | Type | Description |
|---|---|---|
| `command` | `[string, ...]` | Base command (argv prefix). CLI args are appended. |
| `sandbox.allow_network` | `bool` | Allow outbound/inbound networking. |
| `sandbox.allow_kvm` | `bool` | Allow `/dev/kvm` access. |
| `sandbox.allow_interactive_tty` | `bool` | Allow controlling terminal access. |
| `sandbox.syd_path` | `string` | Explicit path to `syd` (Linux). |
| `filesystem.read_only` | `[path, ...]` | Individual read-only file paths. |
| `filesystem.read_only_dirs` | `[path, ...]` | Recursive read-only directories. |
| `filesystem.read_write` | `[path, ...]` | Individual read-write file paths. |
| `filesystem.read_write_dirs` | `[path, ...]` | Recursive read-write directories. |
| `filesystem.ioctl` | `[path, ...]` | ioctl-allowed file paths. |
| `filesystem.ioctl_dirs` | `[path, ...]` | ioctl-allowed directories. |
| `filesystem.library_dirs` | `[path, ...]` | Dynamic linker library directories. |
| `filesystem.library_dirs_from_env` | `bool` | Read library dirs from `LOCKIN_LIBRARY_DIRS`. |
| `limits.max_open_files` | `int` | `RLIMIT_NOFILE` |
| `limits.max_address_space` | `int` | `RLIMIT_AS` (bytes) |
| `limits.max_cpu_time` | `int` | `RLIMIT_CPU` (seconds) |
| `limits.max_processes` | `int` | `RLIMIT_NPROC` |
| `limits.disable_core_dumps` | `bool` | Set `RLIMIT_CORE` to 0. |

## Rust API

`Sandbox::builder()` provides the same cross-platform API for use
as a library.

### Quick start

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

## Design principles

- **Policy compiler, not enforcer**: translates your policy to native backend rules.
- **Secure defaults**: deny-all filesystem/network; explicit allowlists only.
- **Explicit relaxation**: access is granted only through builder methods.
- **No privileged runtime**: works as a regular user.
- **Fail closed**: unsupported OS/backend/configuration returns errors.

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
at exec time (via [`lockin-process`](../lockin-process)).

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
