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

The same builder methods are available on both backends; the
guarantees below hold under the default policy. Backend-specific
differences (what the OS sandbox layer leaves reachable on top of
your allowlists) are enumerated under [Platform
specifics](#platform-specifics).

A program inside a default-policy lockin sandbox cannot:

- read or write files outside the configured allowlists, except
  for a small, enumerated set of paths each backend's OS sandbox
  leaves reachable so dynamically linked programs can start (see
  Platform specifics);
- exec any binary other than the one named in `.command(...)` —
  except, on Linux, binaries inside any directory passed to
  `library_path` are recursively exec-able because the dynamic
  linker (`ld-linux*.so.*`) lives there and must be launchable.
  macOS does not have this exception (dyld is loaded by the
  kernel, not via `execve`);
- when network mode is `deny` (the default): open IP sockets (TCP
  or UDP), bind/listen on a port, or connect AF_UNIX sockets to
  paths outside the allowlists. `proxy` mode permits TCP egress
  only to the configured loopback proxy port; `allow_all` removes
  all of these network restrictions;
- inherit dynamic-linker variables (`LD_PRELOAD`,
  `LD_LIBRARY_PATH`, `LD_AUDIT`, `DYLD_INSERT_LIBRARIES`,
  `DYLD_LIBRARY_PATH`, `DYLD_FRAMEWORK_PATH`) — these are stripped
  at the library layer regardless of how a caller tries to set
  them (explicit `env()`, batched `envs()`, or inheritance from
  the parent process's environment);
- inherit file descriptors `>= 3` that weren't explicitly passed
  through `inherit_fd` / `map_fd` / `keep_fd` — fds in that range
  are marked `FD_CLOEXEC` (on Linux ≥ 5.11 the full range in one
  `close_range` syscall; on older Linux and on macOS, fds up to
  the process's `RLIMIT_NOFILE` capped at 65 536) and the kernel
  closes them at `execve`.

A private `$TMPDIR` is created on the host filesystem, allowlisted
for the child as `read_write` recursive, and exposed via the
`TMPDIR` env var. It is removed when the `Sandbox` value is
dropped on normal exit; abnormal termination (SIGKILL of the
parent, `abort`, `mem::forget`) may leave it behind.

Resource limits set via the builder (`max_open_files`,
`max_cpu_time`, `max_address_space`, `max_processes`) are applied
on both backends, via syd directives on Linux and a `setrlimit`
`pre_exec` hook on macOS. `disable_core_dumps()` is opt-in.

### Platform specifics

**Linux** uses [`syd`](https://git.sr.ht/~alip/syd) (sydbox-rs)
plus Landlock. On top of the configured allowlists the backend
reads a few paths the loader and runtime need: `/proc/self/maps`,
`/etc/ld.so.cache`, `/etc/ld.so.preload`, the TTY paths backing
stdio (resolved via `/proc/self/fd`), and the ancestor directories
of the program path (stat-only). `library_path` directories are
recursively read+exec so the dynamic linker can launch the
configured command — every binary inside them is therefore
exec-able from the sandbox. The `syd` binary itself is resolved
via `.syd_path()`, `LOCKIN_SYD_PATH`, or `PATH`; lockin verifies
the path is absolute but does not authenticate that it is in fact
syd, so pin a known-good path in production (the Nix wrapper does
this automatically).

**macOS** uses the system `sandbox-exec` (Seatbelt). The Seatbelt
baseline (`system.sb`) leaves the following reachable on top of
the configured allowlists:

- read on Apple-shipped system paths: frameworks under `/System`
  and `/Library/Apple`, `/usr/lib`, `/usr/share`, timezone data,
  the `/private/etc/{passwd, master.passwd, protocols, services}`
  lookup files, `/private/var/db/eligibilityd/eligibility.plist`,
  and `/private/var/db/DarwinDirectory`;
- read+write on `/dev/null` and `/dev/zero`, read+write on
  `/dev/fd`, read+write+ioctl on `/dev/dtracehelper`, and read on
  the random/zero/null/fd device nodes;
- broad `sysctl-read`;
- Mach service lookups for a fixed set of Apple system services
  needed for normal program startup: `cfprefsd` (agent + daemon),
  `trustd` (+ agent), `logd` (+ events), `analyticsd` (+
  `messagetracer`), `runningboard`, `secinitd`, `notification_center`,
  `diagnosticd`, `opendirectoryd` libinfo + membership,
  `DirectoryService` libinfo, `espd`, `bsd.dirhelper`,
  `dt.automationmode.reader`, `system.logger`,
  `xpc.activity.unmanaged`, and `appsleep`.

Beyond the Seatbelt baseline, lockin layers a hardening profile
that denies: `mach-register` (the sandboxed program cannot publish
new Mach names), XPC service-name lookups, connections to the
syslog Unix socket, and writes under `/cores`. `process-exec` is
granted only for the command path; `library_path` directories are
mappable as code (`file-map-executable`) but not `execve()`-able.

Because the macOS baseline grants broad sysctl-read and several
`/private/var/db` reads, "user data, application data, and
arbitrary system state are denied unless allowlisted" should be
read with that caveat in mind — most user data is denied, but
sysctl namespaces and a handful of system databases are not.

`darwin.raw_seatbelt_rules` (TOML) / `.raw_seatbelt_rule()` (Rust)
appends rules verbatim to the generated profile *after* the
hardening denies and can re-allow any of them, including reaching
named Apple bundles like `(system-network)`. Treat it as a
trusted-policy escape hatch: only pass rules you fully trust.
