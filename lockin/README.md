# lockin

Build and run a child process inside an OS sandbox.

- Linux backend: `syd`
- macOS backend: `sandbox-exec`

## Three ways to use it

| | What it does | Docs |
|---|---|---|
| **CLI** | Run any program under a TOML-declared policy. | [docs/cli.md](docs/cli.md) |
| **Rust API** | Embed the sandbox in your own program via `Sandbox::builder()`. | [docs/rust.md](docs/rust.md) |
| **Nix** | `wrapWithLockin` produces a derivation whose `bin/*` runs under lockin. | [docs/nix.md](docs/nix.md) |

All three share the same policy model — the TOML schema, the Rust
builder methods, and the Nix attrset are three surfaces over the
same set of knobs.

## Design principles

- **Policy compiler, not enforcer**: translates your policy to native backend rules.
- **Deny by default**: filesystem, network, and exec are denied unless explicitly allowed.
- **Explicit relaxation**: access is granted only through builder methods.
- **No privileged runtime**: works as a regular user.
- **Fail closed**: unsupported OS/backend/configuration returns errors.

## What's enforced

The same builder methods produce the same enforcement on both
backends. A program inside a default-policy lockin sandbox cannot:

- read or write files outside the configured allowlists,
- exec any binary other than the one named in `.command(...)` —
  except, on Linux, binaries inside any directory passed to
  `library_path`, which are recursively exec-able because the
  dynamic linker (`ld-linux*.so.*`) lives there and must be
  launchable for dynamically linked programs to start. macOS does
  not have this exception (dyld is loaded by the kernel, not via
  `execve`),
- open IP sockets (TCP or UDP) when network mode is `deny`,
- connect AF_UNIX sockets to paths outside the allowlists,
- bind or listen on a network port,
- inherit dynamic-linker variables (`LD_PRELOAD`,
  `DYLD_INSERT_LIBRARIES`, and siblings — always stripped),
- inherit file descriptors `>= 3` that weren't explicitly passed
  through `inherit_fd` / `map_fd` / `keep_fd` — fds in that range
  are marked `FD_CLOEXEC` (on Linux ≥ 5.11 the full range in one
  `close_range` syscall; on older Linux and on macOS, fds up to the
  process's `RLIMIT_NOFILE` capped at 65 536) and the kernel closes
  them at `execve`.

A private `$TMPDIR` is mounted/exposed for the child and removed
when the sandbox is dropped. `RLIMIT_*` and core-dump suppression
apply identically on both backends.

### Platform specifics

**Linux** uses [`syd`](https://git.sr.ht/~alip/syd) (sydbox-rs) plus
Landlock. Nothing outside the structured allowlists is reachable.
`library_path` directories are recursively exec-able so the
dynamic linker can launch the configured command.

**macOS** uses the system `sandbox-exec` (Seatbelt). Apple-shipped
read-only system paths — system frameworks under `/System` and
`/Library/Apple`, `/usr/lib`, `/usr/share`, timezone data, the
random/null/zero/fd device nodes, and the `/private/etc/{passwd,
master.passwd,protocols,services}` lookup files — remain readable
so dynamically linked programs can start. A small set of Apple
system services (cfprefsd, trustd, logd, the Darwin notification
center, OpenDirectory libinfo, and a few peers) remains reachable
over Mach for the same reason. User data, application data, and
arbitrary system state are denied unless allowlisted. Egress to the
syslog Unix socket, `mach-register`, the XPC service-name lookup
namespace, and writes under `/cores` are denied. `process-exec` is
granted only for the command path; `library_path` directories are
mappable as code (`file-map-executable`) but not `execve()`-able.
