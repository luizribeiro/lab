# lockin CLI

The `lockin` command runs any program inside a sandbox configured
by a TOML file:

```sh
lockin [-c <config>] [--] <program> [args...]
```

Config resolution: if `-c` is given, that file is used (error if
missing). Otherwise `./lockin.toml` is used if it exists. If
neither is found, a deny-all default policy applies.

## Example

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

## Config reference

All fields are optional. Everything defaults to deny/false/empty.

| Field | Type | Description |
|---|---|---|
| `command` | `[string, ...]` | Base command (argv prefix). CLI args are appended. |
| `sandbox.allow_network` | `bool` | Allow outbound/inbound networking. |
| `sandbox.allow_kvm` | `bool` | Allow `/dev/kvm` access. Linux only; ignored on macOS. |
| `sandbox.allow_interactive_tty` | `bool` | Allow controlling terminal access. |
| `sandbox.allow_non_pie_exec` | `bool` | Permit exec of non-PIE binaries. Needed for compiler toolchains built without `-fPIE` (notably `gcc`/`rustc` on Nix). Linux only; ignored on macOS. |
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
| `env.inherit` | `bool` | Pass parent env to the child (default `false`). Set `true` to inherit everything and use `env.block` to strip. |
| `env.pass` | `[string, ...]` | Shell-glob patterns. Parent env keys matching any pattern are imported (only when `inherit = false`). |
| `env.set` | `{ key = "value", ... }` | Hardcoded env values. Applied after `pass`; overrides on collision. |
| `env.block` | `[string, ...]` | Shell-glob patterns (`*`, `?`, `[...]`, case-sensitive). Matching env keys are always stripped, even from `set`. |
| `darwin.raw_seatbelt_rules` | `[string, ...]` | Raw sandbox-exec S-expression rules appended verbatim to the generated profile. Escape hatch for darwin operations not expressible structurally (`iokit-open`, `mach-lookup`, `sysctl-read`, etc.). macOS only; ignored on Linux. Malformed rules cause `sandbox-exec` to reject the profile at spawn (exit 125). |

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
