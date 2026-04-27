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
- **Secure defaults**: deny-all filesystem/network; explicit allowlists only.
- **Explicit relaxation**: access is granted only through builder methods.
- **No privileged runtime**: works as a regular user.
- **Fail closed**: unsupported OS/backend/configuration returns errors.

## Platform support

The deny-all default is fully realized on **Linux** via `syd` plus
landlock: nothing outside the structured allowlists is reachable.

**macOS** support is best-effort. The Seatbelt backend imports
Apple's `system.sb` baseline as the starting profile (see
`crates/sandbox/src/darwin/policy.rs`); that baseline is a
pragmatic, non-empty set of rules required for normal Mach / IPC /
loader operation. macOS deny-all is therefore not byte-equivalent
to Linux deny-all — treat the macOS backend as a dev-time
convenience and Linux as the production target.
