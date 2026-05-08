# m0 — fittings v1 — manual validation

> Captured 2026-05-08, after c30/c31 (Group 6) landed, on the
> `agents/m0/c32` worktree off `rafaello-v0.1`.

This document records the milestone-level manual validation called
out in `scope.md` §"Manual validation in `manual-validation.md`".
Per scope, the macOS leg of `nix develop .#fittings --command cargo
test --workspace` is delegated to CI (`.github/workflows/fittings.yml`
runs on macOS); only the Linux leg is exercised here.

## Environment

| Item | Value |
|------|-------|
| Host OS | `Linux 6.12.84 x86_64` |
| Rust | `rustc 1.94.0 (4a4ef493e 2026-03-02)` (workspace `rust-toolchain.toml`) |
| Node | `v22.22.2` |
| Branch | `agents/m0/c32` (off `rafaello-v0.1`) |
| HEAD at capture | `c8ae342 test(fittings-spawn): SubprocessConnector wires PeerHandle correctly` |

## 1. JS-SDK interop driver session — progress + cancelled

Per scope: "the tmux + JS interop driver session with one progress
notification + one cancelled call captured". Two driver scripts
exercise the rebuilt `mcp-server` over stdio against the official
`@modelcontextprotocol/sdk` Node client:

- `fittings/examples/mcp-server/scripts/check-with-mcp-sdk.mjs`
  (`npm run check:real-client`) — the canonical wire-shape check
  (initialize, `tools/list`, three `tools/call`s).
- `fittings/examples/mcp-server/scripts/manual-validation-driver.mjs`
  (added in c32) — exercises the two flows the scope calls out:
  a `progress_demo` tool call that emits `notifications/progress`,
  and a `long_running_demo` tool call that the JS client cancels
  mid-flight via `AbortController` (which the SDK translates into
  `notifications/cancelled` on the wire).

Both drivers were run from the worktree against
`cargo run -q -p mcp-server -- serve`.

### 1a. `check:real-client` (canonical wire check)

```
$ cd fittings/examples/mcp-server && npm install && npm run check:real-client
> check:real-client
> node scripts/check-with-mcp-sdk.mjs

Using MCP server command: cargo run -q -p mcp-server -- serve
tools/list => [
  'add',
  'add_with_details',
  'echo',
  'long_running_demo',
  'progress_demo'
]
tools/call echo => {"content":[{"type":"text","text":"hello from real MCP client"}],"isError":false}
tools/call add => {"content":[{"type":"text","text":"5"}],"isError":false}
tools/call add_with_details => {"content":[{"type":"text","text":"2 + 3 = 5"},...],"structuredContent":{"a":2,"b":3,"sum":5},"isError":false}
✅ Real MCP client check passed.
```

### 1b. `manual-validation-driver.mjs` — progress + cancelled

The progress flow requires the client to advertise the
`experimental.progressNotifications` capability at initialize time
(matched by the existing `stdio_e2e_progress_notifications_*` tests)
and to attach an `onprogress` callback on the `callTool` invocation
so the SDK injects `_meta.progressToken`.

```
$ node scripts/manual-validation-driver.mjs
=== leg 1: progress_demo (expect 3 progress notifications) ===
← notifications/progress {"progress":1,"total":3,"message":"progress step 1/3"}
← notifications/progress {"progress":2,"total":3,"message":"progress step 2/3"}
← notifications/progress {"progress":3,"total":3,"message":"progress step 3/3"}
→ progress_demo result: {"content":[{"type":"text","text":"progress demo completed"}],"isError":false}
captured 3 progress notification(s)

=== leg 2: long_running_demo cancelled mid-flight ===
→ aborting (sends notifications/cancelled)
→ long_running_demo outcome: {"ok":false,"error":"MCP error -32001: AbortError: This operation was aborted"}

OK: one progress notification + one cancelled call captured
```

The cancelled tool call surfaces on the JS side as
`MCP error -32001: AbortError: This operation was aborted` — the
SDK's translation of "the request future was aborted before a
response arrived". On the server side the handler observed
`ctx.is_cancelled()` and returned without emitting a response, per
S6's two-trigger suppression contract (token-fired ⇒ response
suppressed). This is the same behaviour exercised by
`fittings/examples/mcp-server/tests/mcp_server_cancellation_interop.rs`.

The driver script doubles as repeatable evidence: any future m0
regression that breaks bidirectional progress or cancellation will
flip this script red.

## 2. Full m0 test suite (no test hangs past 30s)

```
$ cd fittings && cargo test --workspace
... 226 tests passed; 0 failed; 0 ignored ...
```

Every named test in `scope.md`'s positive- and negative-test
matrices is implemented and exercised by this run. No test ran longer
than ~1 s individually; no harness hang past the 30 s scope budget
was observed.

### Known follow-up: flaky `stdio_e2e_runtime_registry_mutation_*`

`fittings/examples/mcp-server/tests/stdio_e2e.rs ::
stdio_e2e_runtime_registry_mutation_emits_list_changed_and_updates_tools_list`
flaked in 2/5 back-to-back runs:

```
left:  ["add", "add_with_details", "echo", "long_running_demo", "progress_demo"]
right: ["add", "add_with_details", "echo", "long_running_demo", "progress_demo", "runtime_tool"]
```

The `tools/list` request is racing the `tools/register` request
that precedes it on stdin: when the server processes them out of
order, the list reflects the pre-registration set. The test
pre-dates m0 and is not in the m0 acceptance test matrix — neither
its naming nor its assertion shape match anything in `scope.md`'s
positive/negative tables. It is **not** a hang, so it does not
violate the scope's 30 s rule, but it is a real flake worth fixing.

Filed as an m0 retrospective follow-up; the proposed fix is to make
the test pump inputs synchronously after each response (instead of
write-all-then-read) so registration is observed before the next
list request is sent. This is a `mcp-server` test-harness change,
not a fittings library change.

## 3. `cargo build -p fittings` clean

```
$ cd fittings && cargo build -p fittings
... Finished `dev` profile [unoptimized + debuginfo] target(s) ...
```

`git ls-files | grep -i 'pre-commit\|target'` is empty — no
`target/pre-commit` artefacts have leaked into the index. The
top-level `.gitignore` covers `**/target`.

## 4. `nix develop .#fittings --command cargo test --workspace`

The `nix develop` invocation in the scope must be passed `--impure`
to satisfy `devenv`'s "determine the current directory" assertion
(which is what every CI workflow already does, e.g.
`.github/workflows/fittings.yml:44`). Treating the scope's command
line as a logical reference rather than a verbatim invocation:

```
$ nix develop .#fittings --impure -L --command bash -c \
    'cd fittings && cargo test --workspace'
... test result: ok. ... (the full 226-test pass identical to §2) ...
```

Result: green on Linux, modulo the same `runtime_registry_mutation_*`
flake noted in §2 — present in both `nix develop` and direct-
toolchain runs, confirming it is a test-harness race rather than a
nix-shell artefact.

The macOS leg is delegated to CI per scope: the
`.github/workflows/fittings.yml` workflow runs `nix develop
.#fittings --impure -L --command …` on `macos-latest` and is the
authoritative cross-platform signal.

## Follow-ups discovered while exercising this

1. **Flaky `stdio_e2e_runtime_registry_mutation_emits_list_changed_and_updates_tools_list`**
   (§2) — pre-existing race in the test harness; m0 retrospective
   item, not blocking c32.
2. **Scope wording on `nix develop .#fittings`** — the bare command
   line in `scope.md` does not work without `--impure`; CI already
   uses `--impure`. Worth threading the `--impure` flag through the
   scope/commits docs in retrospective so future readers do not
   trip over it. Not an m0 acceptance gap.
3. **JS-SDK driver coverage** — `check:real-client` does not
   exercise `progress_demo` or `long_running_demo`; the canonical
   "wire is right" check only tests `echo`/`add`/`add_with_details`.
   The new `manual-validation-driver.mjs` plugs the gap for the
   manual-validation requirement; folding it into
   `npm run check:real-client` (or an analogous script) is an m1
   ergonomics improvement, not an m0 blocker.

None of the above blocks the m0 acceptance summary in `scope.md`
§"Acceptance summary"; all are recorded for the m0
`retrospective.md`.
