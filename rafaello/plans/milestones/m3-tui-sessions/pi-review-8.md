# Round-8 review — `scope.md` at `70591a2`

## Verdict

Needs revision before commit planning. The round-8 draft is close, but it still has three blocking issues that would either make specified tests impossible or introduce process-lifecycle hazards, plus five medium issues that should be fixed to keep implementation/test work deterministic.

## Blocking findings

### Blocking #1 — M1 namespace patch conflicts with current validator and provider-manifest model

Refs: `scope.md:1729-1754`; current `validate/mod.rs:55-78,359-375`; `rfc-manifest-schema.md:487-498`.

`§M1` says every plugin manifest publish must start with `plugin` and should reject `provider.*` as forbidden. That conflicts with existing provider plugin manifests (`[provides] provider = ...`, publishes under `provider.<id>.*`) and with the current `manifest_with_id` logic that validates provider namespace ownership.

It also names new `ManifestError::*` variants, but the path being extended is `validate::manifest_standalone`, which returns `ValidationError`, later wrapped as `ManifestError::Validation`.

**Fix:** make M1 add `ValidationError` variants (or wrapped assertions) and preserve/explicitly stage provider publish semantics rather than blanket-forbidding `provider.*`.

---

### Blocking #2 — `shutdown()` after `wait()` can signal a recycled PID

Refs: `scope.md:689-716`, `1578-1598`.

`rfl chat` waits for TUI exit, then calls `frontend_handle.shutdown()`. But `shutdown()` is specified to send SIGTERM first. Once the reaper has observed exit, the PID may already be reusable; SIGTERM/SIGKILL could hit an unrelated process.

Same hazard exists if a caller does `wait().await` then drops the handle.

**Fix:** `shutdown()`/`Drop` must check cached reaper outcome before signaling, or `wait()` must disarm `child_pid` once exit is observed. If outcome is already terminal, only drain/abort serve and drop registration.

---

### Blocking #3 — `resolve_tui_path` pure-function tests are not importable

Refs: `scope.md:339-354`, `1623-1647`; current `crates/rafaello` has only `src/main.rs`.

The plan defines `pub fn resolve_tui_path(...)` and puts integration tests in `rafaello/tests/`, but binary-crate public items are not importable by integration tests unless the package also has a library target.

**Fix:** add a `crates/rafaello/src/lib.rs` / `[lib]` surface for CLI helpers, with `main.rs` delegating to it; or move the pure-function tests into unit tests inside the binary module.

## Medium findings

### Medium #1 — invalid attach-id spawn test is unconstructable

Refs: `scope.md:435-437`, `582-600`, `2058-2061`.

`CompiledFrontend.attach_id` is `AttachId`, and `AttachId::new` validates. Therefore `frontend_spawn_invalid_attach_id_rejected.rs` cannot construct an invalid plan through public API. `AttachIdInvalid` in `FrontendSpawnError` is likewise unreachable unless an unchecked constructor exists.

**Fix:** either keep `attach_id: String` and validate in `spawn`, add a cfg-test unchecked constructor, or drop the invalid-attach-id spawn test/error reason and rely on `AttachId::new` tests.

---

### Medium #2 — replay-readiness stderr assertion is underspecified/flaky

Refs: `scope.md:1999-2013`, spawn contract around `614-627`.

The test says it captures `rfl chat` stderr and `rfl-tui` stderr separately. But `rfl chat` spawns the TUI; unless the supervisor explicitly pipes/forwards child stderr, the test only sees inherited combined stderr, or no child stderr at all. If two pipes are used, read-time `Instant` ordering can invert under scheduling jitter.

**Fix:** specify stderr handling. Easiest: inherit child stderr into parent stderr and assert line order in the single pipe. Or explicitly pipe/tee child stderr with sequence markers rather than read-time timestamps.

---

### Medium #3 — frontend spawn failure cleanup is not specified after child spawn

Refs: `scope.md:608-627`, `632-648`, `580-593`.

The plan spawns the child before server/register/serve setup. If a later step fails (`Transport`, `BrokerRegister`, race after `try_reserve_frontend_registration`), the spec does not require killing/reaping the child or closing resources.

**Fix:** add m2-style unwind rules for every post-spawn failure: drop registration if acquired, abort serve if spawned, SIGKILL + reap child, close fds.

---

### Medium #4 — non-`Exited` `ReaperOutcome` cases are not handled

Refs: `scope.md:1578-1596`; current `error.rs` includes `Exited`, `WaitFailed`, `ReaperPanicked`.

`§C2` only maps `ReaperOutcome::Exited(status)`. Current m2 shape also has `WaitFailed` and `ReaperPanicked`; F4 even mentions “pre-wait outcome”.

**Fix:** specify both as abnormal frontend exits and ensure cleanup runs before returning non-zero.

---

### Medium #5 — macOS fd-inheritance test relies on `lsof`

Refs: `scope.md:1045-1050`.

`lsof` availability/output on macOS CI is an avoidable external dependency, while macOS CI is a hard gate.

**Fix:** use a probe child that receives the lock fd number and calls `fcntl(F_GETFD)` / equivalent, expecting `EBADF` after exec.

## Summary

The main architectural pieces are converging, but round 8 should patch the M1 provider/validator mismatch, make frontend shutdown safe after observed exit, expose CLI helper code in an importable library surface, and tighten the affected tests/cleanup semantics before `commits.md` is drafted.
