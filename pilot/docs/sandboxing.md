# Sandboxing

## What pilot does

Pilot is a transport between your Rust code and an AI agent CLI's stream-json interface. It spawns processes; it does not isolate them. The spawned agent inherits the calling process's user, working directory, environment, and filesystem access. Pilot is not a security boundary, and it does not attempt to be one.

If you need to confine what the agent can read, write, exec, or reach over the network, that confinement has to come from outside pilot — either the agent CLI's own per-tool config knobs (limited), or a dedicated sandboxing layer wrapped around the agent binary (recommended for real workloads).

## Defaults are permissive

Each driver ships with its CLI's safety rails turned off:

- **Claude:** `PermissionMode::BypassPermissions` — no per-tool approval prompts.
- **Codex:** `SandboxMode::DangerFullAccess` plus `dangerously_bypass_approvals: true` — no sandbox, and the approval gate is bypassed via `--dangerously-bypass-approvals-and-sandbox`.
- **Gemini:** `ApprovalMode::Yolo` — no per-tool approval prompts. Also `skip_trust: true` so gemini does not block on its per-folder trust prompt.
- **Pi:** no per-tool approval protocol in the wire format; pi runs with whatever permissions its provider config grants it.

The reason is that pilot is headless. Per-tool approval prompts assume an interactive terminal and a human to say yes — in pilot's spawn-per-turn model, there is no such loop. Leaving the safety rails on by default would mean the smallest example silently does nothing the first time the agent decides to call a tool. Permissive defaults make `Session::new(driver, workdir).send(...)` actually work out of the box; they also mean you should not point pilot at a working directory you care about without thinking about isolation first.

## Restricting via driver config

If you want to re-enable the CLI's own approval gating without bringing in external tooling, each driver exposes typed knobs. See the per-driver docs for the full set; the short version:

For Claude — see [docs/claude.md](claude.md):

```rust
use pilot::{Claude, ClaudeConfig, PermissionMode};

let config = ClaudeConfig {
    permission_mode: PermissionMode::Default,
    ..ClaudeConfig::default()
};
let driver = Claude::with_config(config);
```

For Codex — see [docs/codex.md](codex.md):

```rust
use pilot::{Codex, CodexConfig, SandboxMode};

let config = CodexConfig {
    sandbox: SandboxMode::ReadOnly,
    dangerously_bypass_approvals: false,
    ..CodexConfig::default()
};
let driver = Codex::with_config(config);
```

For Gemini — see [docs/gemini.md](gemini.md):

```rust
use pilot::{ApprovalMode, Gemini, GeminiConfig};

let config = GeminiConfig {
    approval_mode: ApprovalMode::Default,
    ..GeminiConfig::default()
};
let driver = Gemini::with_config(config);
```

For Pi — see [docs/pi.md](pi.md). Pi's permission model is provider-specific and does not surface through pilot the way the other three do.

Caveat: "approval prompts" in a headless transport usually do not produce a callable surface in pilot. In practice, the agent either silently skips the tool call or stalls waiting for input that never arrives. See [docs/internal/permissions.md](internal/permissions.md) for the empirical investigation behind this — the gist is that the CLI knobs above are useful for restricting what the agent *attempts* in a headless run, but they are not a substitute for process-level isolation.

## Restricting via external sandboxing (recommended for real workloads)

For production use cases where the agent must not have unrestricted disk or network access, wrap the agent process in a real sandbox. Two options live in this monorepo. The examples below are illustrative — read each tool's README for the authoritative invocation.

### lockin

`lockin` is an OS-level sandbox: it compiles a TOML policy down to `syd` + Landlock on Linux or `sandbox-exec` (Seatbelt) on macOS, then execs your program under that policy. Filesystem reads, writes, exec, and network are deny-by-default; you grant access through allowlists.

Because pilot's per-driver `binary` field is a single `PathBuf`, the cleanest way to slot lockin in is a small wrapper script that invokes lockin with the right policy and then forwards to the real agent CLI. Point the driver at that wrapper:

```rust
use std::path::PathBuf;
use pilot::{Claude, ClaudeConfig};

let config = ClaudeConfig {
    binary: Some(PathBuf::from("/usr/local/bin/claude-locked")),
    ..ClaudeConfig::default()
};
let driver = Claude::with_config(config);
```

Where `claude-locked` is something like:

```sh
#!/bin/sh
exec lockin -c /etc/pilot/claude.toml -- /usr/local/bin/claude "$@"
```

The same pattern works for `codex`, `gemini`, and `pi` — set the `binary` field on the relevant `*Config` to a wrapper that execs lockin around the real CLI. `lockin infer` can bootstrap the TOML by recording what the agent actually touched on a representative run.

### capsa

`capsa` is a different shape of sandbox: it builds and runs lightweight VMs via libkrun, with deny-by-default network policy (hostname allowlists) and per-attachment port forwards. It is heavier than lockin — a full guest kernel, not a syscall filter — and the integration pattern is different. Rather than wrapping the agent binary on the host, you typically run pilot *itself* inside a capsa VM, with the agent CLI installed in the guest. The host-side capsa builder configures the VM's network policy and forwards; pilot inside the VM then spawns the agent CLI with permissive defaults, which is fine because the VM is the boundary.

See capsa's README for the builder API and the `capsa-cli` invocation pattern.

## Why pilot does not bundle sandboxing

Two reasons. First, real process-level sandboxing is OS-specific and hard to do well: `lockin` exists because of that, and bundling that complexity into a transport library would either reinvent it badly or pull in a heavy dependency that most pilot consumers do not need. Second, sandboxing is a deployment concern, not a transport concern. The right policy depends on the workload, the host, and the threat model, none of which pilot can see from where it sits. Keeping pilot thin lets callers compose it with whatever sandbox suits their deployment instead of inheriting pilot's choices.

## See also

- [docs/claude.md](claude.md) — claude driver config reference
- [docs/codex.md](codex.md) — codex driver config reference
- [docs/gemini.md](gemini.md) — gemini driver config reference
- [docs/pi.md](pi.md) — pi driver config reference
- [docs/internal/permissions.md](internal/permissions.md) — empirical research on per-CLI permission models
