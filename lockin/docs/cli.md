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
search would produce a misleading policy.

## Backend requirements

- **Linux**: requires the `syd` binary (sydbox-rs). lockin resolves
  it via the explicit `--` flag in the API, the `LOCKIN_SYD_PATH`
  environment variable, or `PATH`. For production use, pin via
  `LOCKIN_SYD_PATH` or the Nix wrapper (`wrapWithLockin` sets
  `LOCKIN_SYD_PATH` automatically); `PATH` lookup is a development
  convenience only. The version pinned in this repo's Nix toolchain
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

## Config reference

All fields are optional. Everything defaults to deny/false/empty.

| Field | Type | Description |
|---|---|---|
| `command` | `[string, ...]` | Base command (argv prefix). CLI args are appended. Must be non-empty if present (omit the field entirely to use CLI args alone). |
| `sandbox.network.mode` | `"deny"` \| `"allow_all"` \| `"proxy"` | Network enforcement strategy (default `"deny"`). `deny` blocks IP networking (TCP/UDP, v4 and v6), inbound bind/listen, and AF_UNIX outbound to arbitrary paths; on macOS a small set of Apple system services required for normal program startup remains reachable, but programs cannot register new Mach names, look up arbitrary XPC services, write to `/cores`, or connect to the syslog Unix socket. `allow_all` removes all restrictions. `proxy` spawns an HTTP CONNECT proxy on loopback, sets `HTTP_PROXY`/`HTTPS_PROXY`/`ALL_PROXY` (and lowercase variants), clears `NO_PROXY`/`no_proxy`, and restricts OS-level outbound to the proxy port only — traffic that bypasses the proxy env fails closed. |
| `sandbox.network.allow_hosts` | `[string, ...]` | Host allowlist for `mode = "proxy"`. Exact hostnames (`"api.example.com"`) or wildcard patterns (`"*.cdn.example.com"`). Must be empty for `deny` / `allow_all` modes. |
| `sandbox.allow_kvm` | `bool` | Allow `/dev/kvm` access. Linux only; ignored on macOS. |
| `sandbox.allow_interactive_tty` | `bool` | Allow controlling terminal access. |
| `sandbox.allow_non_pie_exec` | `bool` | Permit exec of non-PIE binaries. Needed for compiler toolchains built without `-fPIE` (notably `gcc`/`rustc` on Nix). Linux only; ignored on macOS. |
| `filesystem.read_paths` | `[path, ...]` | Individual read-only file paths. |
| `filesystem.read_dirs` | `[path, ...]` | Recursive read-only directories. |
| `filesystem.write_paths` | `[path, ...]` | Individual read-write file paths. |
| `filesystem.write_dirs` | `[path, ...]` | Recursive read-write directories. |
| `filesystem.ioctl_paths` | `[path, ...]` | ioctl-allowed file paths. |
| `filesystem.ioctl_dirs` | `[path, ...]` | ioctl-allowed directories. |
| `filesystem.library_paths` | `[path, ...]` | Dynamic linker library directories. On Linux, binaries inside these directories are also exec-able (required for the dynamic linker). |
| `limits.max_open_files` | `int` | `RLIMIT_NOFILE` |
| `limits.max_address_space` | `int` | `RLIMIT_AS` (bytes) |
| `limits.max_cpu_time` | `int` | `RLIMIT_CPU` (seconds) |
| `limits.max_processes` | `int` | `RLIMIT_NPROC` |
| `limits.disable_core_dumps` | `bool` | Set `RLIMIT_CORE` to 0. |
| `env.inherit` | `bool` | Pass parent env to the child (default `false`). Set `true` to inherit everything and use `env.block` to strip. |
| `env.pass` | `[string, ...]` | Shell-glob patterns. Parent env keys matching any pattern are imported (only when `inherit = false`). |
| `env.set` | `{ key = "value", ... }` | Hardcoded env values. Applied after `pass`; overrides on collision. |
| `env.block` | `[string, ...]` | Shell-glob patterns (`*`, `?`, `[...]`, case-sensitive). Matching env keys are always stripped, even from `set`. |
| `darwin.raw_seatbelt_rules` | `[string, ...]` | Raw sandbox-exec S-expression rules appended verbatim to the generated profile. Raw rules can broaden sandbox authority, including invoking named bundles (`system-graphics`, `system-network`) defined by the macOS system profile but not enabled by default — a single `(system-network)` token unlocks routing-socket egress, mDNS, and the network-extension service surface. Treat as a trusted-policy escape hatch; the caller owns the safety of every rule. Intended for darwin operations not expressible structurally (`iokit-open`, `mach-lookup`, `sysctl-read`, etc.). macOS only; ignored on Linux. Malformed rules cause `sandbox-exec` to reject the profile at spawn; the child exits with `sandbox-exec`'s failure status. |

The CLI also reads `LOCKIN_LIBRARY_DIRS` (colon-separated absolute
paths) and adds each directory to `filesystem.library_paths`.

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
