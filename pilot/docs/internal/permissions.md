# Permissions: design doc for `Event::PermissionRequest`

Phase 3.2. Goal: normalize per-tool permission prompts into a structured
event, and let callers approve / deny inline. This doc is a research +
design pass — no Rust changes land here.

## TL;DR

The phase-kickoff premise — "pilot keeps the child's stdin open through
the turn and `Session::respond_to_permission` writes the response there"
— does not match what any of the four CLIs actually do in their
non-interactive modes. Empirically (CLI versions in the per-driver
sections below):

- **claude** never blocks on stdin for approval in `-p`. It silently
  auto-denies and continues. The only headless approval hook is
  `--permission-prompt-tool <mcp_tool_name>`, which delegates approval
  to an MCP tool — pilot would have to host an MCP server.
- **codex** `exec --json` is effectively pinned to approval policy
  `never`. The agent is told no escalation channel exists and answers
  in-band.
- **gemini** in default approval mode does not register write /
  shell-exec tools in the model's tool set at all. There is no
  permission event because the tool is treated as nonexistent.
- **pi** has no permission protocol. Tools either exist (via `--tools`)
  or don't.

Given that, this doc proposes:

1. Still define `Event::PermissionRequest { call_id, tool, args }` —
   the variant is forward-compatible and lets us model claude's
   `permission_denials` retrospectively.
2. Implement claude support via an in-process MCP bridge, not stdin.
3. Return `Err(Error::Unsupported)` from `Session::respond_to_permission`
   for the other three drivers.

The rest of this doc backs that up with the raw observations and the
exact Rust shapes.

## 1. Per-CLI runtime behavior

Probes ran under `/tmp/pilot-perm-probe`. Each invocation matched the
driver's default argv composition (see `src/driver/<name>.rs`) with the
approval-bypass flag removed (per the investigation brief).

### 1.1 claude — version `2.1.143`

Driver source: `src/driver/claude.rs`. Default argv (today):

```
claude -p --verbose --output-format stream-json --session-id <uuid> -- <prompt>
```

`PermissionMode::Default` omits the `--permission-mode` flag entirely
(`src/driver/claude.rs:88-96`), so the CLI runs in its default approval
mode.

**Probe.** `claude -p --verbose --output-format stream-json --session-id <UUID> -- "Write 'test' to /tmp/perm-probe-marker-claude.txt"`.

**Observed.** The CLI runs to completion in ~6s, exits 0, and the marker
file is *not* written. The relevant slice of stdout
(`/tmp/pilot-perm-probe/claude.out`, formatting trimmed):

```jsonc
// 1. The model issues the tool call as a normal Event::ToolCall candidate.
{"type":"assistant","message":{"role":"assistant","content":[
  {"type":"tool_use","id":"toolu_01PABwuKaNwxLzBZYqjPYqtP","name":"Write",
   "input":{"file_path":"/tmp/perm-probe-marker-claude.txt","content":"test"},
   "caller":{"type":"direct"}}],...}}

// 2. Immediately afterwards (same stream, no stdin read in between),
//    a synthetic user/tool_result that auto-denies.
{"type":"user","message":{"role":"user","content":[
  {"type":"tool_result",
   "content":"Claude requested permissions to write to /tmp/perm-probe-marker-claude.txt, but you haven't granted it yet.",
   "is_error":true,
   "tool_use_id":"toolu_01PABwuKaNwxLzBZYqjPYqtP"}]},...}

// 3. Assistant follow-up acknowledging the denial.
{"type":"assistant","message":{"content":[
  {"type":"text","text":"Permission to write that file wasn't granted. Let me know if you'd like to approve it and retry."}],...}}

// 4. Final result. Note `permission_denials`.
{"type":"result","subtype":"success","is_error":false,...,
 "permission_denials":[{"tool_name":"Write",
   "tool_use_id":"toolu_01PABwuKaNwxLzBZYqjPYqtP",
   "tool_input":{"file_path":"/tmp/perm-probe-marker-claude.txt","content":"test"}}],...}
```

**Interpretation.**

- There is no `permission_request`-typed event. The CLI's headless
  approval flow is to immediately deny and emit a `tool_result` with
  `is_error: true` and a hard-coded English string starting with
  `"Claude requested permissions to "`.
- The CLI does not pause for stdin input. Probes with
  `--input-format stream-json --include-hook-events` (designed to expose
  hook lifecycle events) confirm the same behavior: auto-deny, no event
  type that looks like a permission request, no waiting.
- The final `result` event carries a `permission_denials` array — a
  retrospective summary. That is what pilot can use to *report*
  permission gating ex post.
- The hidden `--permission-prompt-tool <mcp_tool>` flag is the only
  documented headless approval channel. Probing
  `claude -p --verbose --permission-prompt-tool dummy_tool ...` (with
  `--output-format stream-json`) returns:
  ```
  Error: MCP tool dummy_tool (passed via --permission-prompt-tool) not found.
    Available MCP tools: mcp__claude_ai_Google_Calendar__authenticate, ...
  ```
  i.e. the flag is real, but it expects an MCP tool name that the
  parent has registered via `--mcp-config`. This is the official
  approval-interception mechanism for Claude Code in non-interactive
  mode. The `canUseTool` callback in the Claude Code SDK
  (`@anthropic-ai/claude-agent-sdk`) is the JS-side counterpart of the
  same hook.

**Response shape, if pilot bridges via MCP.** When the model wants to
call a gated tool, the CLI invokes the named MCP tool with arguments
shaped like `{ tool_name: string, input: object, tool_use_id: string }`
(the public SDK docs spell this out). The MCP tool's *return value* is
the decision: either `{ "behavior": "allow", "updatedInput": {...} }`
or `{ "behavior": "deny", "message": "..." }`. So claude does not
"accept the response on stdin"; the response is the MCP tool's reply,
delivered over the JSON-RPC stream of pilot's MCP server.

### 1.2 codex — version `codex-cli 0.130.0`

Driver source: `src/driver/codex.rs`. Default argv:

```
codex exec --json --sandbox <mode> --skip-git-repo-check <prompt>
```

The brief asked us to drop the bypass flag — `Codex::new()` already
does that (`SandboxMode::ReadOnly` is the default, and no
`--dangerously-bypass-approvals-and-sandbox` flag is emitted). Read-only
won't reach an approval gate (writes just fail), so we additionally
probed `--sandbox workspace-write` (the realistic write scenario).

**Probe A.** `codex exec --json --sandbox workspace-write --skip-git-repo-check "Write 'test' to /tmp/perm-probe-marker-codex.txt"`.

**Observed.** Exit 0; marker file written without prompting. Codex's
`workspace-write` policy already allows `/tmp` (the cwd was under
`/tmp`), so no escalation was needed. Stdout contained the usual
`item.started`/`item.completed` events for the `command_execution`
item, no permission-related events:

```jsonc
{"type":"item.started","item":{"id":"item_1","type":"command_execution",
  "command":"/bin/zsh -lc \"printf 'test' > /tmp/perm-probe-marker-codex.txt && ls -l /tmp/perm-probe-marker-codex.txt\"","status":"in_progress"}}
{"type":"item.completed","item":{... "exit_code":0,"status":"completed"}}
```

**Probe B.** Force an out-of-workspace write to provoke escalation:
`codex exec --json --sandbox workspace-write -c approval_policy="on-request" --skip-git-repo-check "Write 'test' to /etc/perm-probe-marker-codex.txt"`.

**Observed.** Exit 0, marker file *not* written, only an in-band
assistant message:

```
"I can't write to /etc/perm-probe-marker-codex.txt in this session
 because /etc is outside the writable workspace and approvals/escalation
 are disabled."
```

Stderr contained `"Reading additional input from stdin..."` but no JSON
event indicated an escalation request, and the CLI exited cleanly without
ever blocking on stdin.

**Interpretation.**

- `codex exec --json` is the only codex sub-mode pilot drives, and it
  effectively pins `approval_policy = never` regardless of what
  `-c approval_policy="on-request"` says — the
  CLI's own `--help` even tells you so:
  *"Prefer `on-request` for interactive runs or `never` for
   non-interactive runs."*
- There is no `permission_request`-shaped event. There is no
  response protocol on stdin.
- The CLI's *other* subcommands — `mcp-server`, `app-server`,
  `remote-control` — implement a richer JSON-RPC protocol that
  includes approval RPCs, but pilot does not invoke those today.
  Adopting one of them would be a separate, larger driver rewrite.

### 1.3 gemini — version `0.42.0`

Driver source: `src/driver/gemini.rs`. Default argv (with
`Gemini::new()` and `--yolo` dropped per the brief):

```
gemini -p <prompt> --output-format stream-json --session-id <uuid> --skip-trust
```

`ApprovalMode::Default` omits the `--approval-mode` flag.

**Probe.** Same prompt as the others; output saved to
`/tmp/pilot-perm-probe/gemini.out`.

**Observed.** The model attempts a `run_shell_command` and then a
`write_file`, and *every* attempt comes back with:

```jsonc
{"type":"tool_result","tool_id":"run_shell_command_...","status":"error",
 "output":"Tool \"run_shell_command\" not found. Did you mean one of: \"update_topic\", \"grep_search\", \"invoke_agent\"?",
 "error":{"type":"tool_not_registered","message":"Tool \"run_shell_command\" not found. ..."}}
```

Same for `write_file`. The CLI exits 0 after the model wanders
through `list_directory` / `google_web_search` / `invoke_agent`
trying to find a way around the missing tools.

**Interpretation.**

- In default approval mode under `-p`, gemini does not surface a
  permission request — instead it removes the gated tools from the
  registry altogether. The model receives `tool_not_registered`
  errors and the run continues.
- The only way to enable writes/exec is `--approval-mode yolo`
  (or `auto_edit` for edits), which pilot's `Yolo` variant already
  sets. That is the inverse of the design we want: permission is
  binary at process spawn, not per-call.
- There is, therefore, no event for pilot to normalize and no
  response protocol to speak.

### 1.4 pi — version `0.73.1`

Driver source: `src/driver/pi.rs`. Default argv:

```
pi -p --mode json --session-dir <dir> [--provider <p>] [--model <m>] [--thinking <l>] <prompt>
```

**Probe.** `pi -p --mode json --session-dir /tmp/pilot-perm-probe/pi-session "Write 'test' to /tmp/perm-probe-marker-pi.txt"`.

**Observed.** Exit 0; marker file written. No approval prompt, no
permission event. The full event-type set observed across the
~40-event stream:

```
agent_end, agent_start, message_end, message_start, message_update,
session, text, text_delta, text_end, text_start,
tool_execution_end, tool_execution_start, tool_execution_update,
toolcall_delta, toolcall_end, toolcall_start, turn_end, turn_start
```

Nothing approval-shaped. The `tool_execution_start` for the
`bash printf > /tmp/...` command is emitted *after* the agent has
already decided to run it, with no preceding gate event.

**Interpretation.**

- Pi has no permission protocol whatsoever. Access is controlled
  out-of-band via `--tools <allowlist>` / `--no-tools` /
  `--no-builtin-tools`.
- There is nothing for pilot to normalize, and no channel on which a
  response could be delivered.

### 1.5 Summary table

| CLI    | Emits structured permission event? | Accepts response on stdin? | Mechanism actually available                                              |
|--------|------------------------------------|----------------------------|---------------------------------------------------------------------------|
| claude | No — auto-denies, emits `tool_result is_error:true` and `result.permission_denials` | No  | `--permission-prompt-tool <mcp_tool>` delegated through an MCP server     |
| codex  | No — escalation disabled in `exec --json` | No                  | None within `codex exec` (would require `mcp-server` / `app-server` mode) |
| gemini | No — gated tools are simply unregistered | No                   | None within `-p` default approval mode (write/exec require `--yolo` etc.) |
| pi     | No — no approval protocol           | No                         | None                                                                      |

## 2. Proposed `Event::PermissionRequest`

The phase brief picked the field set (`call_id`, `tool`, `args`). The
empirical data does not push back — the only data we can plausibly
fill is exactly those three plus driver provenance. Concretely:

```rust
// in src/event.rs, alongside the existing variants
#[non_exhaustive]
pub enum Event {
    // ...existing variants...

    /// The underlying agent asked for approval before invoking `tool`.
    /// Callers wishing to approve / deny respond via
    /// [`Session::respond_to_permission`].
    ///
    /// `call_id` matches the `call_id` of the subsequent
    /// [`Event::ToolCall`] (claude) — drivers that synthesize this
    /// event from after-the-fact signals MUST preserve that
    /// correspondence so callers can join the two.
    PermissionRequest {
        call_id: String,
        tool: String,
        args: serde_json::Value,
    },
}
```

**Justification of the field set.**

- `call_id`: every CLI that surfaces tool invocations gives them an id
  (claude `tool_use.id`, codex `item.id`, gemini `tool_id`, pi
  `toolCall.id`). Reusing the same id lets callers correlate a
  `PermissionRequest` with the eventual `ToolCall` / `ToolResult` pair.
- `tool`: matches the eventual `ToolCall.name`. For claude this is
  `tool_use.name` (e.g. `"Write"`).
- `args`: free-form `serde_json::Value` — same as `ToolCall.args` and
  same justification (per-tool schemas vary, normalization beyond
  "preserve as-is" is not pilot's job).

**Why not more fields.**

- No `reason` / `message` field. Claude is the only driver that
  produces an explanatory string today
  (`"Claude requested permissions to write to <path>, ..."`), and it
  is fully derivable from `tool` + `args`. Adding a per-driver string
  here just pulls UI concerns into the core event.
- No `severity` / `category`. The CLIs don't classify; pilot would be
  inventing a taxonomy with no input data to fit it to.
- No `driver` field. Pilot does not stamp provenance on its other
  normalized variants (only `Event::Raw`); doing it here would be
  inconsistent.

**Where the variant actually fires.**

- **claude:** synthesized inside `Claude::parse`. When `parse_user`
  sees a `tool_result` whose `content` is a string starting with
  `"Claude requested permissions to "` and `is_error == true`, the
  driver emits *both* a `PermissionRequest` (with the matching
  `tool_use_id`) *and* the existing `ToolResult { ok: false, ... }`.
  The pair tells the caller "this tool was requested and immediately
  denied". This is retrospective, not live — callers cannot influence
  the outcome by the time the event arrives.
- **claude with MCP bridge (later commit):** when pilot spawns an MCP
  permission-bridge alongside the CLI, the bridge intercepts the
  approval call and converts it into a *live*
  `Event::PermissionRequest`, blocking the CLI on the MCP RPC until
  `Session::respond_to_permission` resolves. The variant is the same
  either way — only the timing semantics change.
- **codex / gemini / pi:** never emitted. The drivers' `parse`
  functions stay as they are.

## 3. Proposed response API

```rust
// in src/session.rs
impl Session {
    /// Approve or deny a permission request previously emitted as
    /// [`Event::PermissionRequest`].
    ///
    /// # Errors
    ///
    /// - [`Error::Unsupported`] if the underlying driver does not
    ///   expose a permission-response channel (currently: every
    ///   driver except claude with an MCP bridge configured).
    /// - [`Error::UnknownPermissionCall`] if `call_id` does not match
    ///   any outstanding request on this session.
    /// - [`Error::Io`] if the response could not be delivered (e.g.
    ///   the MCP bridge channel closed because the child exited).
    pub async fn respond_to_permission(
        &self,
        call_id: &str,
        decision: Decision,
    ) -> crate::Result<()>;
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum Decision {
    /// Allow this single call. `updated_args` lets the caller mutate
    /// the tool input before it runs (mirrors claude's
    /// `updatedInput` field in `canUseTool` responses); pass `None`
    /// to allow with the args the agent originally requested.
    Allow { updated_args: Option<serde_json::Value> },

    /// Deny this single call. `message` is shown back to the model
    /// as the reason (claude uses it verbatim in the synthetic
    /// `tool_result`).
    Deny { message: Option<String> },
}
```

**Scoping discussion: `call_id` vs session-wide.**

- Per-`call_id` is the natural unit because the CLIs themselves
  scope permission decisions to a single tool invocation. Claude's
  `canUseTool` callback resolves one tool call at a time; codex's
  `mcp-server` approval RPCs do the same; gemini and pi have no
  scope at all so the question doesn't apply.
- A session-wide "allow always" is tempting (mirrors interactive UX),
  but the underlying CLIs do not have a documented "remember this
  decision" hook in non-interactive mode. Synthesizing it inside
  pilot — i.e. caching `(tool, args-pattern) → Allow` and answering
  future requests without re-asking — is feature work that should
  live in the *caller* (a TUI / app), not in the core
  `Session::respond_to_permission` API. So this design intentionally
  keeps the API per-call.
- Consequently `Decision` has no `AllowAlways` / `DenyAlways`
  variants. They can be added later as `#[non_exhaustive]` lets the
  enum grow.

**Why not write to stdin?** The brief proposed
`Session::respond_to_permission(call_id, Decision)` writes a response
to the child's stdin. The empirical investigation shows no CLI reads
approval responses from stdin — claude's stdin in `-p` mode is for
the *prompt* (when `--input-format stream-json` is used), codex's
stdin in `exec --json` mode is for the *prompt* (the `"Reading
additional input from stdin..."` stderr line is unrelated to
approvals), and gemini/pi don't read any approval channel at all. So
the implementation must route through whatever each driver actually
supports — for claude, that means an MCP bridge; for the others, an
explicit `Err(Unsupported)`.

**Driver-trait surface needed.**

`Driver` (in `src/driver.rs`) does not currently expose any side
channel. The simplest surface is:

```rust
pub trait Driver: Send + Sync {
    // ...existing methods...

    /// Deliver a permission decision back to whatever channel this
    /// driver uses. Default impl: `Err(Error::Unsupported)`.
    async fn deliver_permission_decision(
        &self,
        _session_id: Uuid,
        _call_id: &str,
        _decision: Decision,
    ) -> crate::Result<()> {
        Err(crate::Error::Unsupported {
            driver: self.name(),
            feature: "permission responses",
        })
    }
}
```

`Session::respond_to_permission` is a thin pass-through that adds
"is there an outstanding request with this `call_id`?" bookkeeping.

## 4. Implementation plan

Ordered commit list. Each is sized to land in one PR-shaped step.
Tests + impl in the same commit (per `.claude/CLAUDE.md`).

1. **`event: add PermissionRequest variant`** — add the
   `Event::PermissionRequest { call_id, tool, args }` variant in
   `src/event.rs`; unit test that drivers can construct and pattern-
   match on it. No driver wiring yet.
2. **`error: add Unsupported and UnknownPermissionCall`** — extend
   `crate::Error` (`src/error.rs`) with the two new variants the API
   needs. Add `#[non_exhaustive]` if not already present.
3. **`driver: add Decision enum and deliver_permission_decision hook`** —
   `Decision` lives next to `Driver` in `src/driver.rs`. The trait
   method has a default `Err(Unsupported)` impl so existing drivers
   keep compiling.
4. **`session: add respond_to_permission with outstanding-call tracking`** —
   wire `Session::respond_to_permission` through to the driver hook.
   Maintain a `HashMap<call_id, ...>` of outstanding requests inside
   the session; populate it from a new observer hook on the turn
   stream. Default behavior — without driver changes — is that the
   map stays empty and every call returns `UnknownPermissionCall`,
   which is the correct fallback for codex / gemini / pi.
5. **`claude: synthesize PermissionRequest from auto-deny tool_result`** —
   in `parse_user_block`, detect `is_error: true` + content prefix
   `"Claude requested permissions to "` and emit
   `Event::PermissionRequest` alongside the existing
   `Event::ToolResult { ok: false, ... }`. Fixture + snapshot under
   `tests/fixtures/claude/`. Document that this is retrospective.
6. **`claude: MCP permission bridge`** — the heavy lift. Pilot
   spawns an in-process MCP server, binds it via `--mcp-config` +
   `--permission-prompt-tool`. Permission RPCs from the CLI become
   live `PermissionRequest` events, and `respond_to_permission`
   resolves the RPC. Opt-in via a new `ClaudeConfig` field
   (default off, because it spawns a side process). One commit if
   it stays small; if it grows beyond ~300 lines, split into
   "bridge transport" + "wire into Claude::command" sub-commits.
7. **`docs: per-driver permission notes`** — update
   `docs/{claude,codex,gemini,pi}.md` with the live behavior and
   pointers to this design doc. Add a section to
   `docs/agent-comparison.md` flagging which drivers support
   `respond_to_permission`.

Estimated total: **6–7 commits.** Commits 1–4 are mechanical and can
land in a single afternoon; commit 5 is small but needs a fixture
update; commit 6 is the real engineering work and is the gate on
shipping a useful Phase 3.2.

## 5. Known gaps

- **codex / gemini / pi will not support `respond_to_permission`** in
  any form pilot can drive today. Their `Session::respond_to_permission`
  calls return `Err(Error::Unsupported { driver, feature: "permission responses" })`.
  The drivers continue to surface permission-related signals as
  `Event::Raw` (codex: the in-band "approvals disabled" assistant
  message; gemini: `tool_not_registered` errors). The
  `Event::PermissionRequest` variant is never emitted for these.
- **claude without MCP bridge gets retrospective events only.** Commit
  5 (above) gives callers a normalized signal that a permission was
  denied, but it cannot reverse the denial — by the time pilot sees
  the `tool_result`, claude has already moved on. To actually approve
  a call, the caller must enable the MCP bridge (commit 6).
- **MCP bridge is non-trivial.** Hosting an MCP server inside pilot
  means picking a JSON-RPC implementation, choosing transport
  (stdio? unix socket?), serializing approval state across the
  pilot-side `Session` and the bridge-side JSON-RPC handlers, and
  surviving the child being killed mid-RPC. None of that is hidden in
  the design doc; we should expect commit 6 to be the dominant
  engineering cost of Phase 3.2.
- **Pattern-matching on a localized English string is brittle.** The
  `"Claude requested permissions to "` prefix used in commit 5 is the
  only reliable in-stream signal we have; a future Claude Code release
  could reword, localize, or restructure it. The synthesizer must
  degrade gracefully — when the prefix doesn't match, leave the
  existing `Event::ToolResult { ok: false }` untouched.
- **Codex's `mcp-server` / `app-server` modes are unexplored.** They
  do expose richer approval RPCs, but adopting one of them means a
  different driver entirely (different command, different event
  shapes). That is a Phase 4-ish question, not Phase 3.2.
- **No claim about gemini's enterprise / IDE channels.** Gemini's
  default approval mode is the only one this doc probed; the
  `--approval-mode plan` mode and gemini's IDE-side approval flows
  may have richer semantics, but those are not what pilot drives.

## Investigation artifacts

Probe stdout lives under `/tmp/pilot-perm-probe/` (not committed);
reproducible from the argv in section 1 against the named CLI versions.
