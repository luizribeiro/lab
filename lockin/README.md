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

### Exit codes

| Code | Meaning |
|---|---|
| `0`–`255` | Child's own exit code. |
| `128 + N` | Child was killed by signal `N` (e.g. `137` = SIGKILL, `143` = SIGTERM). |
| `125` | lockin itself failed (config parse, path resolution, sandbox setup). |

### Config reference

All fields are optional. Everything defaults to deny/false/empty.

| Field | Type | Description |
|---|---|---|
| `command` | `[string, ...]` | Base command (argv prefix). CLI args are appended. |
| `sandbox.allow_network` | `bool` | Allow outbound/inbound networking. |
| `sandbox.allow_kvm` | `bool` | Allow `/dev/kvm` access. Linux only; ignored on macOS. |
| `sandbox.allow_interactive_tty` | `bool` | Allow controlling terminal access. |
| `filesystem.read_only_paths` | `[path, ...]` | Individual read-only file paths. |
| `filesystem.read_only_dirs` | `[path, ...]` | Recursive read-only directories. |
| `filesystem.read_write_paths` | `[path, ...]` | Individual read-write file paths. |
| `filesystem.read_write_dirs` | `[path, ...]` | Recursive read-write directories. |
| `filesystem.ioctl_paths` | `[path, ...]` | ioctl-allowed file paths. |
| `filesystem.ioctl_dirs` | `[path, ...]` | ioctl-allowed directories. |
| `filesystem.library_paths` | `[path, ...]` | Dynamic linker library directories. |
| `limits.max_open_files` | `int` | `RLIMIT_NOFILE` |
| `limits.max_address_space` | `int` | `RLIMIT_AS` (bytes) |
| `limits.max_cpu_time` | `int` | `RLIMIT_CPU` (seconds) |
| `limits.max_processes` | `int` | `RLIMIT_NPROC` |
| `limits.disable_core_dumps` | `bool` | Set `RLIMIT_CORE` to 0. |
| `env.inherit` | `bool` | Pass parent env to the child (default `true`). Set `false` to start with an empty env. |
| `env.block` | `[string, ...]` | Shell-glob patterns (`*`, `?`, `[...]`, case-sensitive). Matching env keys are always stripped from the child. |

The CLI also reads `LOCKIN_LIBRARY_DIRS` (colon-separated absolute
paths) and adds each directory to `filesystem.library_paths`.

### Environment variables

By default, lockin passes every parent environment variable through
to the sandboxed child, with two exceptions:

- **Built-in blocklist** (always stripped): dynamic-linker vars that
  bypass the filesystem sandbox by loading arbitrary code.
  - Linux: `LD_PRELOAD`, `LD_LIBRARY_PATH`, `LD_AUDIT`
  - macOS: `DYLD_INSERT_LIBRARIES`, `DYLD_LIBRARY_PATH`, `DYLD_FRAMEWORK_PATH`.
    On macOS these matter for non-SIP-hardened binaries; SIP-protected
    binaries have them removed by the OS regardless.
- **User blocklist** (`env.block`): glob patterns you specify in the
  config. Useful for stripping known credential vars before they
  reach the sandbox:

  ```toml
  [env]
  block = ["AWS_*", "*_TOKEN", "*_SECRET", "GITHUB_TOKEN"]
  ```

Setting `env.inherit = false` starts the child with an empty
environment. The sandbox library still injects a private
`TMPDIR`/`TMP`/`TEMP` pointing to a temp directory it creates for
the child.

#### What this does not protect against

The default (`inherit = true` + `block`) is a tool for stripping
*specific, known* variables. It is not a general-purpose credential
hygiene default:

- A credential not matched by any `block` pattern is passed through.
  If you set `SNOWFLAKE_PASSWORD` in your shell and don't add
  `SNOWFLAKE_*` to `block`, the child sees it.
- Variables not in `block` still reveal their existence to the child.
  `SSH_AUTH_SOCK` being set reveals an agent is running, even if the
  socket itself is unreachable under the filesystem policy.

For stronger hygiene, either use `inherit = false` and manage the env
explicitly, or unset sensitive variables in the parent shell before
invoking lockin.

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
