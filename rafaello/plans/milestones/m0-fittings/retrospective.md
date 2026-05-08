# m0 — fittings v1 — retrospective

> Written 2026-05-08, after all 32 m0 commits (`43f3a75` →
> `24c3438`) landed on `rafaello-v0.1`. Worktree:
> `/home/luiz/lab-wt/m0-retro-claude` on
> `agents/m0-retro/claude`.

This is the milestone-level review against `scope.md` and
`commits.md` per `plans/README.md` Phase-3 step 1. It complements
`manual-validation.md`, which captures the c32 evidence; this file
answers the five retrospective questions and proposes any deltas
to `overview.md` / `decisions.md` / stream RFCs that the m0
implementation invalidated.

The five sections below match the questions the milestone driver
was asked to answer.

---

## 1. Coverage

Every named test in `scope.md` §"Demo bar" — both the positive and
the negative integration matrices — landed under
`fittings/tests/` (or `fittings/examples/mcp-server/tests/` for the
mcp-server-specific cases). No test from the matrix was dropped or
silently substituted.

### Positive matrix → landing commit

| scope.md test file | Landed in | Notes |
|--------------------|-----------|-------|
| `peerhandle_bidirectional.rs` | `c8e4ee1` (c14) | Both directions exercised within one connection, as scope requires. |
| `peerhandle_outside_handler.rs` | `63b9f9d` (c10) server side; extended in `46fc5f2` (c19) client side | Two-stage landing matches the c10/c19 split agreed in `commits-pi-review-2.md`. |
| `service_context_peer_call.rs` | `57579c5` (c15) | Handler exercises both `ctx.peer().notify` and `ctx.peer().call`. |
| `peerhandle_dropped_future_cancels.rs` | `8db2c73` (c18) | LSP default + MCP override both covered. |
| `peerhandle_close_drain.rs` | `2bef8a2` (c16) | Pending `peer.call`s resolve `Transport`; `peer.closed()` resolves on both sides. |
| `service_context_notify.rs` | `ee15f31` (c09) | Five notifications mid-request, ordered before response. |
| `service_context_cancelled_by_token.rs` | `4a78e7a` (c22) | Token-fired suppression. |
| `service_context_cancelled_by_handler.rs` | `4a78e7a` (c22) | Handler-returned `Err(Cancelled)` suppression without token. |
| `error_preservation_round_trip.rs` | `404eb9e` (c08) | Table-driven across the five predefined codes; full server→client path. |
| `error_marker_round_trip.rs` | `51e594a` (c04) | `Transport` + `Panic` markers via the codec; end-to-end Panic via dispatcher proven separately by `handler_panic_maps_to_panic.rs` in c08. |
| `service_code_ranges.rs` | `c07317d` (c05) | Includes the round-1 additions: above-reserved-negative (`-31999`) and code `0` (negative case). |
| `id_null_explicit_request.rs` | `ee25b5f` (c20) | Promoted out of Group 4 per pi round-1 decision. |
| `id_null_concurrent_rejected.rs` | `ee25b5f` (c20) | Same commit; second-concurrent-null rejection. |
| `bounded_notify_drop.rs` | `ee15f31` (c09) initial; `c8e4ee1` (c14) extended with post-flood `peer.call` | Two-stage landing agreed in commits-pi-review-2 finding 1. |
| `cancellation_outside_semaphore.rs` | `7205dc6` (c21) | `with_max_in_flight(1)` saturated; cancellation observed outside the permit pool. |
| `batch_cancellation_partial_suppression.rs` | `98d992a` (c24) | Both partial-suppression and all-suppressed (no batch response) cases. |
| `id_namespace_isolation.rs` | `c8e4ee1` (c14) | 100 concurrent calls per direction; no `s_<n>`/`c_<n>` collisions. |

### Negative matrix → landing commit

| scope.md test file | Landed in | Notes |
|--------------------|-----------|-------|
| `malformed_cancellation.rs` | `a2d3e63` (c23) | Table-driven across LSP-default and MCP-override. |
| `notification_handler_panic.rs` | `46fc5f2` (c19) | Panic in client-side handler doesn't kill subsequent traffic. |
| `inbound_request_no_service.rs` | `23b4122` (c13) | Mirrors `inbound_request_with_service.rs` in the same commit. |
| `peer_gone_during_notify.rs` | `f8a7ef0` (c25) | Asserts `peer.closed()` + pending-`peer.call`-Transport contract; does **not** assert synchronous `notify` failure. |
| `invalid_service_code_marker.rs` | `c07317d` (c05) | Includes code `0`, code `-32700` (reserved), and a code conflicting with a predefined variant. |

### Tests added beyond the matrix

These weren't named in `scope.md` but were required by individual
commit acceptance criteria in `commits.md` (and by the
"every commit lands green" rule):

- `core_request_id_shape.rs` (c01) — three decode paths.
- `core_predefined_error_data.rs` (c02) — variant construction.
- `wire_outbound_error_round_trip.rs` (c03), `wire_inbound_error_round_trip.rs` (c04) — codec-level round-trip distinct from the dispatcher-level `error_preservation_round_trip.rs`.
- `cancelled_no_wire_mapping.rs` (c06) — confirms `Cancelled` has no outbound code.
- `service_context_basic.rs` (c08) — `ctx.request_id()` / `ctx.is_cancelled()` smoke from inside a handler.
- `handler_panic_maps_to_panic.rs` (c08) — proves the `Panic` variant + `fittingsKind = "panic"` marker end-to-end through the dispatcher's `catch_unwind` path.
- `peerhandle_server_initiated_call.rs` (c12) — hand-rolled echo client; precedes the full `Service`-backed bidirectional test in c14.
- `inbound_request_with_service.rs` (c13) — mirror of the negative `no_service` test.
- `transport_bidirectional_regression.rs` (c30) — T1.
- `spawn_peerhandle_round_trip.rs` (c31) — P1.
- `mcp_server_cancellation_config.rs` (c27), `mcp_server_cancellation_interop.rs` (c28) — mcp-server side of the cancellation cutover.

All extra tests are additive; none replace a scope-named test.

### Coverage verdict

**No gaps.** All 17 positive + 5 negative scope-named tests are
implemented and pass; the manual-validation `cargo test
--workspace` run reports 226 tests passing on Linux
(`manual-validation.md` §2). The c32 acceptance gate is met.

---

## 2. Drift against overview / decisions / stream RFCs

### 2.1 Real drift: cancellation builder API shape

**What the RFC says.**
`streams/b-fittings/rfc-fittings-notifications.md:251-256` and
`:951-952` describe two builder methods on `Server`:

```
Server::new(service, transport)
    .with_cancellation_method("notifications/cancelled")
    .with_cancellation_id_extractor(|params| /* fn returning JsonRpcId */)
```

The id extractor is documented as a *closure* `|params| -> JsonRpcId`,
explicitly so the LSP-default extractor reads `params.id` and the
MCP-style override reads `params.requestId` via custom logic.

**What landed.**
`fittings::server::Server::with_cancellation` in
`fittings/crates/server/src/server.rs:203` is a single method
taking two `&str` arguments — the method name and the id field
name:

```rust
pub fn with_cancellation(self, method: &str, id_field: &str) -> Self
```

The id extractor is no longer a closure; it's the field name to
read from the JSON `params` object, with the JSON-RPC `JsonRpcId`
parsed from whatever value lives at that field.

**Origin.** This is the API shape that `commits.md` c17 already
specified (`Server::with_cancellation(method: &str, id_field: &str)`).
It's not an implementation accident; it's a deliberate
simplification ratified at commit-plan time. The drift is in the
RFC text, which still shows the closure API.

**Impact.** Zero functional gap — both the LSP default and the MCP
override are configurable, the malformed-payload test
(`malformed_cancellation.rs` in c23) covers both extractors, and
the two-trigger suppression rule in S6 is unaffected. The closure
form is strictly more powerful (could synthesise an id from
multiple fields), but no scope test or downstream consumer needs
that power. If a future consumer does, it's an additive RFC
amendment to add a closure-accepting variant.

**Proposed fix.** Append a new row to `decisions.md` recording
the simpler builder shape and the rationale for not amending the
RFC text. The README's drift policy ("Stream RFCs in `streams/`
are not retroactively rewritten when `overview.md` evolves. Drift
gets resolved in `overview.md` and called out in the next
milestone retrospective.") applies here. Since `overview.md`
itself does not name `with_cancellation_method` —
`grep -n "with_cancellation" overview.md` is empty — there's no
overview text to fix; the `decisions.md` row is sufficient as the
canonical record.

Landed as commit `ad22eee` on this branch.

### 2.2 Drift the README pre-existing-knew-about

`plans/README.md:73-75` already calls out that stream RFCs are
not retroactively rewritten and drift surfaces in the next
retrospective. The cancellation API shape above is the only
concrete RFC↔code mismatch m0 introduced, but for completeness a
few minor wording mismatches that fall under the same policy (no
patch needed) are worth recording here so a future reader knows
they are *known* drift, not undiscovered:

- `rfc-fittings-notifications.md:552-557` discusses cancellation
  method *defaults* in question form ("`notifications/cancelled`
  as the default? Or LSP's `$/cancelRequest`?"). m0 picked LSP
  default per scope §S7; the RFC question text was not updated.
- `rfc-fittings-notifications.md:608-621` describes the
  configured extractor flow in terms of a closure
  (`cancellation_id_extractor` and "the extractor's target field
  (`id` / `requestId`)"). The latter half ("target field") matches
  the field-name implementation; the former half ("extractor")
  hints at a closure that no longer exists. Reads coherently if
  you treat "extractor" as "the configured field-name lookup".

Neither warrants an RFC patch.

### 2.3 Drift against `overview.md` §3 (Client API surface)

> Updated 2026-05-08 in response to pi review of this
> retrospective (finding #3). The original draft of this section
> claimed "no drift against `overview.md`"; that was wrong.

`overview.md:313-318` (in §3) described the client constructor
as `Client::new(transport, service)`. The landed API is
`Client::connect(connector)` for construction plus
`Client::with_service(svc)` for inbound-service registration —
verified in `fittings/crates/client/src/lib.rs:66` and `:122`,
and consistent with `overview.md` §15.6 which already used the
landed shape. The overview was internally inconsistent.

**Fix.** A follow-up commit on this branch
(`docs(rafaello): update overview §3 to match landed Client
API`) rewrites the §3 paragraph to match §15.6 / the landed
code.

The rest of `overview.md` lines up against the landed shape:

- §15.6 names `Server::peer() -> PeerHandle` and `Client::peer() ->
  PeerHandle` exposing notify+call+closed — matches the c10/c14/c16
  surface.
- The `JsonRpcId` keying for `pending_outbound`, prefixed-string
  per-direction id allocation, drop-on-full bounded notification
  channel, and "connection close → all pending `peer.call`s see
  `Transport`" contract all match decision rows 18 and 22.
- `ServiceContext` (`notify`, `cancelled`, `is_cancelled`,
  `request_id`, `peer`) matches `fittings/crates/core/src/context.rs`.

`decisions.md` rows 18 and 22 are accurate against the landed
implementation. No new architectural decision row is required;
the additions are the tactical rows in §2.1 (cancellation
builder shape) and §2.4 (`MethodNotFound` typed-method-field
deferral).

### 2.4 Drift against `rfc-fittings-errors.md` (`MethodNotFound` typed `method` field)

> Updated 2026-05-08 in response to pi review of this
> retrospective (finding #2). The original draft of this section
> claimed the errors RFC "matches the implementation
> byte-for-byte"; that was wrong.

`rfc-fittings-errors.md:71-79` (the `FittingsError` struct
definition), `:202` (the encoder table), and `:239-244` (the
decoder match) all require `MethodNotFound` to carry a typed
`method: Option<String>` and the codec to extract / synthesise
`data.method` from it. The landed shape in
`fittings/crates/core/src/error.rs:18-22` is

```rust
MethodNotFound { message: String, data: Option<Value> }
```

— same shape as the other four predefined variants, no typed
`method` field, and `error_map.rs` has no
`extract_method` / fallback path.

**Origin of the gap.** `scope.md` §W2 lists the five predefined
variants as carrying `data: Option<Value>` and `message: String`
only. The typed `method` field on `MethodNotFound` was in the
RFC but was not pulled into `scope.md`'s in-scope item set; the
implementation followed the scope. Pi review of m0 caught the
RFC↔scope inconsistency that was carried into the landed code.

**Fix: defer to m1.** No v1 consumer needs the typed field;
producers that want to surface the unknown method name can
populate `data.method` today (the existing
`FittingsError::method_not_found(msg)` constructor leaves
`data: None`, so this is opt-in, not automatic). The deferral
landed as commit `0b61bdd` appending `decisions.md` row 36;
pi round-2 review then corrected an earlier "purely additive"
framing in row 36 — adding `method: Option<String>` to a
public Rust enum struct variant is **source-breaking**
(direct struct-literal constructors break unconditionally;
exhaustive pattern matches that bind fields by name break
depending on the bind list), not purely additive. The
deferral is a deferred API-shape cutover acceptable before
fittings hits a public v1 stability boundary, recorded as
commit `b36e690` revising row 36's wording.

The other four predefined variants (`Parse`, `InvalidRequest`,
`InvalidParams`, `Internal`), the `Transport` / `Panic` /
`invalidServiceCode` markers, and the code-range expansion all
match the RFC byte-for-byte — proven empirically by
`error_preservation_round_trip.rs`,
`error_marker_round_trip.rs`, `service_code_ranges.rs`, and
`invalid_service_code_marker.rs`. The drift is localised to the
`MethodNotFound` variant shape.

### 2.5 Verdict

Three pieces of drift require action, all landing as follow-up
commits on this branch **after** retrospective.md:

1. `decisions.md` row 35 — cancellation builder API shape (§2.1).
2. `decisions.md` row 36 — `MethodNotFound` typed-method-field
   deferral (§2.4).
3. `overview.md` §3 — Client API surface (§2.3).

No stream RFC patches required (per
`plans/README.md` §"Authoring conventions" stream RFCs are not
retroactively rewritten; drift gets resolved in `overview.md`
plus `decisions.md` rows).

---

## 3. Slipped or cut

### 3.1 Nothing slipped from the matrix

No scope-named test was deferred, dropped, or downgraded. No
piece of the W/C/S/K/T/P/M/E item lists in `scope.md` "In scope"
was bundled into m1 or beyond.

### 3.2 Bundled additions

These landed inside m0 commits without being named in
`scope.md`'s "In scope" list, but each is either an obvious
support-test or a commit-plan-ratified addition:

- **`FittingsError::Panic { message }`** as an explicit variant
  (commits.md c02) — *not* in `scope.md`'s C-layer list. Pi
  round-2 finding 4 added it so c04's marker round-trip could
  prove the `fittingsKind = "panic"` contract end-to-end via
  c08's dispatcher. Recorded in `commits.md` "What changed from
  the first draft" §1.
- **Server panic mapping** (c08) routing handler panics into
  `FittingsError::Panic` instead of the previous flat
  `Internal("request handler panicked")`. Same round-2 finding 4
  origin.
- **Existing one-arg constructors preserved on the predefined
  variants** (`FittingsError::method_not_found(msg)` etc., setting
  `data: None`). Not in scope; pi round-2 finding 8 added it to
  spare existing call sites the churn.
- **`Client::dropped_notifications()` counter** (c19) — scope
  §K2 explicitly carved this as an "implementation convenience,
  not RFC-mandated" allowance; landed as opt-in.
- **`spawn_peerhandle_round_trip.rs` upgraded to require
  call+notify** (c31) — scope §P1 said "verify against the new
  shape and update its tests if needed; no public API change",
  pi round-1 escalated this to require the full bidirectional
  surface (c14/c16 deps), not just notify.
- **`manual-validation-driver.mjs`** (c32) — a new JS driver
  script in `fittings/examples/mcp-server/scripts/`. Not in
  scope, but the c32 evidence required two flows
  (`progress_demo` + cancelled `long_running_demo`) that
  `check:real-client` does not exercise. The driver doubles as
  reproducible regression evidence.

### 3.3 Commit-plan additions vs the original scope wording

The "What changed from the first draft" section of `commits.md`
itself records the round-1 and round-2 additions; they are all
strictly additive against scope and were ratified by the owner
on 2026-05-08. Nothing in m0's *implementation* added a feature
beyond what `commits.md` already named.

### 3.4 No m0a/m0b split was triggered

`scope.md` and `commits.md` both flagged a possible split after
c10. Group 3 (bidirectional `PeerHandle`) landed cleanly on top
of Group 2 without forcing a partial-fittings-v0.X merge to
`main`. Default ("ship m0 as one milestone") prevailed. The
optional `fittings-v0.X` carve-out in `decisions.md` row 33 and
README §Phase 3 remains available for a future fittings consumer
but was not exercised.

---

## 4. Process notes for the next milestone driver

These are sharp edges m0 hit that aren't already in
`plans/README.md` §"Recurring operational gotchas". File future
gotchas there as they're learned.

### 4.1 Workspace-wide cutover commits are unavoidable for breaking trait changes

`commits.md` c08 is the only commit in m0 that touches every
crate at once (`fittings-core` + `fittings-server` +
`fittings-macros` + every example) — because the
`Service::call` signature changed and the workspace-green-per-commit
rule is non-negotiable. Pi round-1 finding 1 established that
no smaller increment keeps the workspace compiling.

For m1 and beyond: if a stream RFC requires a breaking trait
change with multiple consumers, plan one consolidated cutover
commit up front in `commits.md`. Trying to stage it across
2–3 commits will fail the per-commit greenness gate. Document
the size in the commit body so reviewers know it's intentional.

### 4.2 Pi review iterations on `commits.md` are worth the wall-clock

m0's commit plan went through three pi review rounds before
ratification. Round-1 caught the API-cutover-must-be-one-commit
issue, the wire-vs-core layering bug (errors live in
`fittings-core`, not `fittings-wire`), and stale internal
references. Round-2 caught the `Panic`-variant-must-be-explicit
issue, the `c10`-needs-a-raw-harness issue, and the
`manual-validation.md`-is-a-milestone-deliverable-not-c08
finding. Each round reshuffled commit dependencies meaningfully.

If `commits.md` looks "obviously right" after one pi pass, that's
suspicious. The next milestone should expect at least two rounds.

### 4.3 Two-stage tests are the right way to ladder API-surface dependencies

Several scope-named tests landed in two stages —
`peerhandle_outside_handler.rs` (c10 server, c19 client),
`bounded_notify_drop.rs` (c09 base, c14 post-flood `peer.call`
extension). Pattern: land the test exercising whatever surface
exists at the earlier commit, then *extend* it (not duplicate it)
when the rest of the surface arrives. Keeps every commit green
without leaving placeholder `#[ignore]` tests scattered.

The next milestone should reach for this pattern explicitly when
a single scope test depends on multiple commits. The trap to
avoid is "punt the whole test to the last commit" — that grows
the late commit out of the size budget and makes the early commits
look untested.

### 4.4 RFC↔code drift policy: pin once, don't keep editing

`plans/README.md:73-75` is explicit that stream RFCs are not
retroactively rewritten. m0 hit this with the cancellation
builder API (§2.1 above): the right move is a tactical
`decisions.md` row, not an RFC patch. For future drivers: if you
catch yourself wanting to edit a stream RFC mid-implementation,
stop — that's `decisions.md` territory. The RFC stays as a
historical artefact of the ratification round.

### 4.5 Pre-commit symlink in retrospective worktrees

The retrospective worktree (`/home/luiz/lab-wt/m0-retro-claude`)
needs the same `.pre-commit-config.yaml` symlink workaround the
per-commit agent worktrees needed:

```
ln -s /nix/store/5c1w75c9icw1z6rngplsvbb3n3b8fqdx-pre-commit-config.json \
    .pre-commit-config.yaml
```

This is *adjacent* to the existing `PREK_ALLOW_NO_CONFIG=1`
gotcha in README — the gotcha covers orchestrator-side commits;
this covers in-worktree retro commits. Worth folding into the
README's recurring-gotchas list as a one-liner. Not a blocker for
m0 sign-off, just a paper cut for whoever runs m1's retro.

### 4.6 `nix develop` invocation needs `--impure`

`scope.md` §"Manual validation" specifies
`nix develop .#fittings --command cargo test --workspace`. As
captured in `manual-validation.md` §4, this fails without
`--impure` due to a `devenv` "current directory" assertion; CI
already passes `--impure`. Future scope docs should encode
`--impure` directly. Worth a one-line scope-template note for
m1.

---

## 5. Known issues to track

These are pre-existing bugs surfaced during m0 implementation but
**not introduced by m0**. Recording them here so they don't get
forgotten; m0 is not the right milestone to fix them.

### 5.1 `fittings-client::tests::fatal_send_error_is_propagated_to_queued_calls` is ~20% flaky

**Where.** `fittings/crates/client/src/lib.rs:937` — an in-crate
unit test (`#[tokio::test]` block in the `tests` module).

**Symptom.** Race between the send-task fatal-error path and the
queued-call slot vacating. Roughly 1 in 5 `cargo test` runs across
m0 saw it fail; never caused a commit to fail to land because
re-running cleared it.

**Origin.** Pre-dates m0. The send-task / queued-call architecture
in `fittings-client` is unchanged by m0 (m0 *added* surfaces —
`peer().notify`, `peer().call`, notification handler — but did
not refactor the send-task lifecycle). `git log -p
fittings/crates/client/src/lib.rs` confirms the test predates
`rafaello-design`.

**Proposed fix location.** `fittings-client` send-task close
sequencing — add an explicit synchronization between
fatal-send-error broadcast and pending-call slot release so the
queued-call observer sees the slot transition before the result.
This is a `fittings-client` library change, not a test-harness
change; the test is asserting a real contract. Out of scope for
m0; track as a `fittings` follow-up regardless of milestone.

### 5.2 `mcp-server` `stdio_e2e_runtime_registry_mutation_emits_list_changed_and_updates_tools_list` flake

**Where.**
`fittings/examples/mcp-server/tests/stdio_e2e.rs ::
stdio_e2e_runtime_registry_mutation_emits_list_changed_and_updates_tools_list`.

**Symptom.** The `tools/list` request is racing the
`tools/register` request that precedes it on stdin. When the
server processes them out of order, `tools/list` returns the
pre-registration tool set:

```
left:  ["add", "add_with_details", "echo", "long_running_demo", "progress_demo"]
right: ["add", "add_with_details", "echo", "long_running_demo", "progress_demo", "runtime_tool"]
```

Flaked in 2/5 back-to-back runs during c32 manual validation.

**Origin.** Pre-dates m0; the test harness's "write all stdin
inputs then read all responses" pattern predates the m0 mcp-server
refactor (c26/c27/c28 changed the *server* side, not the test
harness). c32's commit message records this as a "pre-existing
mcp-server stdio_e2e flake flagged as a retrospective follow-up".

**Confirmed test name.** From `manual-validation.md` §2 capture
during c32:
`stdio_e2e_runtime_registry_mutation_emits_list_changed_and_updates_tools_list`.
Same file (`stdio_e2e.rs`) holds several other stable
`stdio_e2e_*` tests; only the `runtime_registry_mutation` variant
flakes.

**Proposed fix location.** `mcp-server` test harness — pump inputs
synchronously after each response (read-then-write) instead of
write-all-then-read. This is an `mcp-server` test-harness change,
not a fittings library change.

### 5.3 No new flakes introduced by m0

Outside the two pre-existing flakes above, the
`manual-validation.md` 226-test pass is reproducibly green on
Linux. No m0-introduced test was flaky during the milestone.

### 5.4 Where to file these

Both flakes are repo-internal (no external bug tracker for
`fittings`). The current convention is to track in this
`retrospective.md` and via the commit log; if `fittings` ever
gets its own issue tracker, port these. Until then, the milestone
driver for whoever picks them up has the context here.

---

## Follow-up commits on this branch

Drift fixes land as separate commits after this retrospective.
Status as of 2026-05-08 (post pi review of the retrospective):

1. ✅ `docs(rafaello): record fittings cancellation builder API
   shape` (`ad22eee`) — `decisions.md` row 35. (§2.1.)
2. ✅ `docs(rafaello): defer MethodNotFound method-field shape
   to m1` (`0b61bdd`) — `decisions.md` row 36. (§2.4.)
3. ✅ `docs(rafaello): update overview §3 to match landed
   Client API` (`f1a070f`) — `overview.md` §3 rewrite. (§2.3.)
4. ✅ `test(fittings-server): cover same-id type-mismatch in
   malformed_cancellation` (`3380a28`) — addresses pi review
   finding #1.

No stream RFC patches required.

---

## Acceptance summary check

`scope.md` §"Acceptance summary" requires:

- ✅ Every named test in positive + negative matrices implemented
  and passing. (§1 above; `manual-validation.md` §2; the
  malformed-cancellation type-mismatch coverage gap caught by
  pi review of this retrospective is closed by the follow-up
  test commit listed above.)
- ✅ `cargo test -p fittings -p fittings-{wire,core,server,client,
  transport,spawn,macros,testkit} -p mcp-server` green on Linux.
  (`manual-validation.md` §2.) The macOS leg is **delegated to CI
  per scope** — the authoritative cross-platform signal is
  `.github/workflows/fittings.yml` running on `macos-latest`,
  not a local capture; nothing in this retrospective was
  validated on a macOS host. If the CI run for the
  `rafaello-v0.1` tip fails on macOS, m0's acceptance flips red.
- ✅ JS-SDK interop check passes. (`manual-validation.md` §1a.)
- ✅ `mcp-server/src/serve_stdio` no longer contains the manual
  notification-draining loop. (c26 — verified in
  `manual-validation.md`.)
- ✅ `manual-validation.md` records the items in scope. (Landed
  c32 / `24c3438`.)
- ✅ `retrospective.md` written, with drift surfaced as deltas.
  (This file; the three `decisions.md`/`overview.md` follow-up
  commits listed above are landed.)

m0 is done.
