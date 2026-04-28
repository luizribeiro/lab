# lockin CLI

The `lockin` command runs any program inside a sandbox configured
by a TOML file:

```sh
lockin [-c <config>] [--] <program> [args...]
```

Config resolution: if `-c` is given, that file is used (error if
missing). Otherwise `./lockin.toml` is used if it exists. If
neither is found, a deny-all default policy applies.

`<program>` (and `command[0]` from the TOML) must be an absolute
path (`/usr/bin/python3`) or a relative path containing a `/`
(`./script.py`). Bare names with no slash are rejected — lockin
intentionally does not perform `PATH` lookup, because the resolved
binary determines the sandbox's exec allowlist and silent `PATH`
search would produce a misleading policy. The path itself is not
otherwise normalized or authenticated: `..` segments and setuid bits
on the resolved binary are not rejected by lockin (the kernel's
`no-new-privs` on Linux and Seatbelt on macOS strip suid in the
sandboxed-child path, but the policy author is responsible for the
identity of the binary they name).

## Backend requirements

- **Linux**: requires the `syd` binary (sydbox-rs). lockin resolves
  it via the explicit `--` flag in the API, the `LOCKIN_SYD_PATH`
  environment variable, or `PATH`. For production use, pin via
  `LOCKIN_SYD_PATH` or the Nix wrapper (`wrapWithLockin` sets
  `LOCKIN_SYD_PATH` automatically); `PATH` lookup is a development
  convenience only. Pinning is a security requirement anywhere `PATH`
  is not fully trusted: an attacker who controls any earlier `PATH`
  directory can substitute the `syd` binary and silently disable the
  sandbox. The version pinned in this repo's Nix toolchain
  is **sydbox 3.49.1**; that is the documented baseline this
  release is tested against.
- **macOS**: uses the system `sandbox-exec` (Seatbelt). No extra
  dependencies. The full set of paths and Mach services that the
  Seatbelt baseline (`system.sb`) leaves reachable on top of your
  allowlists — system frameworks, `/usr/lib`, `/usr/share`,
  timezone data, several `/private/etc` lookup files, a few
  `/private/var/db` reads, broad `sysctl-read`, read+write on
  `/dev/null`/`/dev/zero`/`/dev/fd`, plus a fixed set of Apple
  Mach services — is enumerated in the [top-level README's
  Platform specifics](../README.md#platform-specifics). Most user
  data, application data, and arbitrary system state are denied
  unless allowlisted.

## Example

```toml
# lockin.toml
command = ["/usr/bin/python3"]

[sandbox.network]
mode = "proxy"
allow_hosts = ["huggingface.co", "*.hf.co"]

[filesystem]
read_dirs = ["/usr/lib/python3.11", "/etc/ssl/certs"]

[limits]
max_open_files = 1024
max_cpu_time = 60
```

```sh
lockin -- script.py --verbose
lockin -c sandbox.toml -- myapp --flag
```

## Shebang support

The CLI supports Linux-portable shebangs via the `-c` short flag
with an attached value:

```python
#!/usr/bin/lockin -c/etc/lockin/python3.toml

import json, sys
print(json.load(sys.stdin)["name"])
```

The config's `command` field specifies the interpreter. Trailing
arguments from the command line are appended to it.

## Exit codes

| Code | Meaning |
|---|---|
| `0`–`255` | Child's own exit code. |
| `128 + N` | Child was killed by signal `N` (e.g. `137` = SIGKILL, `143` = SIGTERM). |
| `125` | lockin itself failed (config parse, path resolution, sandbox setup). |

A child process that exits with code `125` of its own is
indistinguishable from a lockin-side `125` error.

## Filesystem capability model

Every `filesystem.*` entry grants one capability — `read`, `write`,
or `exec` — on the listed path or directory. The capabilities
are nested: `write` and `exec` each imply `read` on the same
path or directory (so a `write_dir` does not also need to be listed in
`read_dirs`). The reverse is not true: `read` does not imply `exec`,
so readable inputs are not silently executable as new processes.

On macOS, `read` additionally grants `file-map-executable` on the same
path, so the child can `dlopen`/mmap shared libraries that live in any
readable directory. This is required for cross-platform parity: on
Linux, `mmap(PROT_EXEC)` of a readable file is not sandbox-mediated,
so a writable directory there can already host code that gets mapped
executable. The macOS rule mirrors that, and as a result a writable
directory can host code that the child mmaps as executable on either
platform. Process-level exec (`execve` / `posix_spawn`) remains gated
separately — only `exec_paths` / `exec_dirs` grant it. Both grant
recursive exec on Linux and macOS.

## Path resolution

Relative paths inside the TOML — every `filesystem.*` entry and a
relative `command[0]` — resolve against the directory containing the
config file, not the caller's CWD. So `read_dirs = ["./data"]` in
`/etc/lockin/foo.toml` always means `/etc/lockin/data`, regardless of
where `lockin` was invoked from. Absolute paths pass through unchanged.

Relative program paths typed on the command line (the argument after
`--`) keep their normal shell semantics and resolve against the
caller's CWD.

## Config reference

All fields are optional. Everything defaults to deny/false/empty.

`ioctl` access is currently controlled by `sandbox.allow_kvm` and
`sandbox.allow_interactive_tty`; finer-grained ioctl policy is not
exposed.

| Field | Type | Description |
|---|---|---|
| `command` | `[string, ...]` | Base command (argv prefix). CLI args are appended. Must be non-empty if present (omit the field entirely to use CLI args alone). |
| `sandbox.network.mode` | `"deny"` \| `"allow_all"` \| `"proxy"` | Network enforcement strategy (default `"deny"`). `deny` blocks IP networking (TCP/UDP, v4 and v6), inbound bind/listen, and AF_UNIX outbound to arbitrary paths; on macOS a small set of Apple system services required for normal program startup remains reachable, but programs cannot register new Mach names, look up arbitrary XPC services, write to `/cores`, or connect to the syslog Unix socket. `allow_all` removes all restrictions. `proxy` spawns an HTTP CONNECT proxy on loopback, sets `HTTP_PROXY`/`HTTPS_PROXY`/`ALL_PROXY` (and lowercase variants), clears `NO_PROXY`/`no_proxy`, and restricts OS-level outbound to the proxy port only — traffic that bypasses the proxy env fails closed. The proxy only handles HTTPS via the `CONNECT` method; plain-HTTP forwarding is not implemented, so `http://` URLs through the proxy will fail. |
| `sandbox.network.allow_hosts` | `[string, ...]` | Host allowlist for `mode = "proxy"`. Exact hostnames (`"api.example.com"`) or wildcard patterns (`"*.cdn.example.com"`). Must be empty for `deny` / `allow_all` modes. See [allow_hosts trust model](#allow_hosts-trust-model) for what allowing a host implies. |
| `sandbox.allow_kvm` | `bool` | Allow `/dev/kvm` access. Linux only; ignored on macOS. |
| `sandbox.allow_interactive_tty` | `bool` | Allow controlling terminal access. |
| `sandbox.allow_non_pie_exec` | `bool` | Permit exec of non-PIE binaries. Needed for compiler toolchains built without `-fPIE` (notably `gcc`/`rustc` on Nix). Linux only; ignored on macOS. |
| `filesystem.read_paths` | `[path, ...]` | Files the child can read. |
| `filesystem.read_dirs` | `[path, ...]` | Directories the child can read recursively. |
| `filesystem.write_paths` | `[path, ...]` | Files the child can write. Implies read on the same path. |
| `filesystem.write_dirs` | `[path, ...]` | Directories the child can write recursively. Implies recursive read. |
| `filesystem.exec_paths` | `[path, ...]` | Binaries the child can `execve` / `posix_spawn`. Implies read. |
| `filesystem.exec_dirs` | `[path, ...]` | Directories whose contents the child can exec recursively. Implies recursive read. |
| `limits.max_open_files` | `int` | `RLIMIT_NOFILE` |
| `limits.max_address_space` | `int` | `RLIMIT_AS` (bytes). On macOS the limit is inherited by `sandbox-exec` itself, which runs before the user program; values too tight to fit `sandbox-exec`'s own footprint will fail the spawn. |
| `limits.max_cpu_time` | `int` | `RLIMIT_CPU` (seconds) |
| `limits.max_processes` | `int` | `RLIMIT_NPROC` |
| `limits.disable_core_dumps` | `bool` | Set `RLIMIT_CORE` to 0. |
| `env.inherit` | `bool` | Pass parent env to the child (default `false`). Set `true` to inherit everything and use `env.block` to strip. |
| `env.pass` | `[string, ...]` | Shell-glob patterns. Parent env keys matching any pattern are imported (only when `inherit = false`). |
| `env.set` | `{ key = "value", ... }` | Hardcoded env values. Applied after `pass`; overrides on collision. |
| `env.block` | `[string, ...]` | Shell-glob patterns (`*`, `?`, `[...]`, case-sensitive). Matching env keys are always stripped, even from `set`. |
| `darwin.raw_seatbelt_rules` | `[string, ...]` | Raw sandbox-exec S-expression rules appended verbatim to the generated profile. Raw rules can broaden sandbox authority, including invoking named bundles (`system-graphics`, `system-network`) defined by the macOS system profile but not enabled by default — a single `(system-network)` token unlocks routing-socket egress, mDNS, and the network-extension service surface. Re-enabling `network-outbound` via raw rules also re-enables AF_UNIX outbound — the implicit `network-outbound` deny that the deny-by-default mode installs covers both IP and Unix-domain egress, so a raw `(allow network-outbound ...)` lifts it for both. Treat as a trusted-policy escape hatch; the caller owns the safety of every rule. Intended for darwin operations not expressible structurally (`iokit-open`, `mach-lookup`, `sysctl-read`, etc.); process-exec is not one of them — use `filesystem.exec_paths` / `filesystem.exec_dirs` instead. macOS only; ignored on Linux. Malformed rules cause `sandbox-exec` to reject the profile at spawn; the child exits with `sandbox-exec`'s failure status. |

## allow_hosts trust model

`allow_hosts` is a *hostname* allowlist, not an *address* allowlist.
The proxy resolves each connection's target hostname at request time
and admits the connection if the hostname matches an allowlist entry.
Whatever IP that name resolves to — now or in the future — becomes
reachable through the proxy.

A few consequences worth understanding before writing an entry:

- **DNS controls the address set.** Resolution happens per-request, so
  the set of reachable IPs can shift between requests if the
  authoritative records change (DNS rebinding). Allowing a domain
  implicitly trusts whoever controls that domain's authoritative DNS.
- **Names that resolve to host-local or internal addresses give the
  sandboxed program reach into the host network.** This includes
  loopback (`127.0.0.0/8`, `::1`), private RFC1918 ranges
  (`10/8`, `172.16/12`, `192.168/16`), link-local (`169.254.0.0/16`,
  `fe80::/10`), and cloud metadata endpoints such as
  `169.254.169.254`. That is sometimes intentional — sandboxing a
  tool that talks to a local dev server, or scoped corp-internal
  access — but it is reach the deny-by-default mode does not grant,
  so be sure that's what you want.
- **Wildcards extend trust to every subdomain controller.** A pattern
  like `*.example.com` admits whatever `anything.example.com`
  resolves to, for any subdomain anyone with control of
  `example.com`'s zone chooses to publish. Avoid wildcards on domains
  you do not own or fully control.
- **The policy author owns the allowlist's attack surface.** lockin
  enforces the contract literally — once a host is named, the proxy
  admits connections to it. lockin does not second-guess intent by
  filtering address classes after resolution; that would silently
  break the legitimate uses above.

If the program only needs a fixed set of public APIs, prefer listing
exact hostnames over wildcards, and avoid entries that resolve into
the host's private network unless that reach is the point.

## Environment variables

By default, lockin starts the child with an empty environment
(`inherit = false`). You add what the child needs via `env.pass`
(import from parent) and `env.set` (hardcoded values).

Unconditional built-in blocklist — dynamic-linker vars that would
bypass the filesystem sandbox by loading arbitrary code — is always
stripped, even from `env.set`:

- Linux: `LD_PRELOAD`, `LD_LIBRARY_PATH`, `LD_AUDIT`
- macOS: `DYLD_INSERT_LIBRARIES`, `DYLD_LIBRARY_PATH`, `DYLD_FRAMEWORK_PATH`.
  On macOS these matter for non-SIP-hardened binaries; SIP-protected
  binaries have them removed by the OS regardless.

Example (deny-by-default with explicit pass/set):

```toml
[env]
pass = ["PATH", "HOME", "USER", "TERM", "LANG", "LC_*", "NIX_*"]
set = { RUST_LOG = "info" }
```

The sandbox library also always injects `TMPDIR`/`TMP`/`TEMP`
pointing to a private temp directory; these survive `inherit = false`.

### Inherit-mode escape hatch

For workflows where you trust the parent env and only want to strip
a few specific vars, set `inherit = true`:

```toml
[env]
inherit = true
block = ["AWS_*", "*_TOKEN", "*_SECRET", "GITHUB_TOKEN"]
```

In this mode, `pass` is ignored (everything already inherits) and
`block` acts as the sole filter. This is the enumerate-or-leak mode:
any credential not listed in `block` leaks. Prefer deny-by-default
for stronger isolation guarantees.

### `env.block` precedence

`env.block` is a *filter*, not a stage. It strips matching keys no
matter where they came from — `inherit`, `pass`, or `set`. That
means `set.LD_PRELOAD = "/path"` does not override the built-in
blocklist: the assignment is silently dropped.
