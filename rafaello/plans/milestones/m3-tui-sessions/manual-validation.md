# m3 — manual validation evidence

> Companion to `retrospective.md`. Records the exact
> acceptance-gate transcripts and out-of-band evidence
> that `scope.md` §"Acceptance summary" requires before
> ratification. Status: round 2, 2026-05-10 (round 1
> archived only the cargo aggregate counts; round 2
> inlines tail snippets durably and corrects the §5.8
> root-cause attribution).

The scope's acceptance summary names four cargo-driven
gates and one out-of-band manual smoke. This document
captures each.

## 1. Linux acceptance gate — `cargo test --workspace --features test-fixture`

**Command** (verbatim from `scope.md` §"Acceptance summary"):
```
nix develop --impure --command cargo test --manifest-path rafaello/Cargo.toml --workspace --features test-fixture
```

**Status:** ✅ green (captured 2026-05-10 inside the
devshell on Linux x86_64 / 6.12.84).

**Aggregate:** 308 test binaries / 516 tests passed / 0
failed / 0 ignored. Re-captured 2026-05-10 after the
§5.8 macOS un-gating commit (`1e839b3`); the five
newly-un-gated rafaello-tui integration tests continue
to pass on Linux post-removal.

**Source:** `/tmp/m3-acceptance.log` (~284 KB transient
log). The aggregate was extracted via `grep "test
result:" /tmp/m3-acceptance.log | awk '{p+=$4; f+=$6;
i+=$8; c+=1} END{print c, p, f, i}'`. Tail of the log
archived inline below for durability:

```
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.00s
... (308 binaries total; doc-tests for the three workspace
crates each report `0 passed; 0 failed; 0 ignored`)
```

The captured run includes Linux-only tests gated on
`#[cfg(target_os = "linux")]`. The current Linux-only
gates split into two classes:

- **Platform-inherent** (cannot be fixed for macOS in
  m3 without scope expansion): `frontend_handle_drop_does_not_leak_zombie.rs`
  (`/proc/<pid>` zombie observation), `supervisor_spawn_post_register_reaps_child.rs`
  (Linux-specific reap timing), `supervisor_spawn_unwinds_*_fd_baseline.rs`
  (`/proc/self/fd` count baseline assertions).
- **Harness gating that is NOT platform-inherent**:
  every `rafaello-tui/tests/*.rs` integration test
  (`tui_handler_calls_frontend_ready.rs`,
  `tui_test_mode_logs_bus_events_to_stderr.rs`,
  `tui_test_mode_exits_on_test_done.rs`,
  `tui_test_mode_self_timeout_exits_zero.rs`,
  `tui_sends_frontend_ready_after_handler_registration.rs`)
  is currently `#![cfg(target_os = "linux")]` because
  the c25 agent applied a defensive Linux gate without
  a real macOS try-and-fail run. The harness already
  uses the m2-pattern `SockFlag::empty()` +
  `fcntl(F_SETFD, FD_CLOEXEC)` (no SOCK_CLOEXEC), so
  there is no reason to expect a macOS-specific
  socketpair failure. **This is not an inherent macOS
  limitation**; the gate is overcautious. Filed in
  `retrospective.md` §5.8 as a required follow-up code
  commit before ratification — the commit drops the
  gate, pushes, and lets `macos-latest` CI prove (or
  surface a real unanticipated issue).

> Per `scope.md` round-9 polish: the headline
> `rfl_chat_demo_bar.rs` test ran in under 30 s under
> `cargo test`, well within the m2-set budget.

## 2. Linux build gate — `cargo build --workspace --bins`

**Command** (verbatim from `scope.md` §"Acceptance summary"):
```
nix develop --impure --command cargo build --manifest-path rafaello/Cargo.toml --workspace --bins --features rafaello-core/test-fixture
```

**Status:** ✅ green (captured 2026-05-10). Tail of the
build log archived inline:

```
Running           devenv:enterShell
Succeeded         devenv:enterShell (4.47ms)
Running           devenv:enterTest
No command        devenv:enterTest
1 Skipped, 4 Succeeded
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.07s
```

(The 0.07s figure reflects an incremental build over an
already-warm `target/` from the preceding test run.)

The explicit `--features rafaello-core/test-fixture` is
required because `rfl-bus-fixture` is gated by
`required-features = ["test-fixture"]` on the
`rafaello-core` crate; `--workspace --bins` alone would
skip it. This verifies all three bins build:

- `rfl` (`rafaello/crates/rafaello`)
- `rfl-tui` (`rafaello/crates/rafaello-tui`)
- `rfl-bus-fixture` (`rafaello/crates/rafaello-core`,
  feature-gated)

**Source:** `/tmp/m3-build.log`.

## 3. Linux doc gate — `cargo doc --workspace --no-deps`

**Command** (verbatim from `scope.md` §"Acceptance summary"):
```
nix develop --impure --command cargo doc --manifest-path rafaello/Cargo.toml --workspace --no-deps
```

**Status:** ✅ green and warning-free (captured 2026-05-10).
Tail of the doc log archived inline:

```
    Checking rafaello-core v0.0.0 (/home/luiz/lab/rafaello/crates/rafaello-core)
 Documenting rafaello-core v0.0.0 (/home/luiz/lab/rafaello/crates/rafaello-core)
    Checking rafaello v0.0.0 (/home/luiz/lab/rafaello/crates/rafaello)
    Checking rafaello-tui v0.0.0 (/home/luiz/lab/rafaello/crates/rafaello-tui)
 Documenting rafaello-tui v0.0.0 (/home/luiz/lab/rafaello/crates/rafaello-tui)
 Documenting rafaello v0.0.0 (/home/luiz/lab/rafaello/crates/rafaello)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.59s
   Generated /home/luiz/lab/rafaello/target/doc/rafaello/index.html and 4 other files
```

No `private_intra_doc_links` warnings (m2 retro §5.2's
fix held). No new doc warnings introduced under m3's
expanded surface (rafaello-tui, rafaello/lib).

## 4. macOS CI gate — hard ratification gate

**Status:** ⏳ pending the post-retrospective branch
push. Per `scope.md` round-6 tightening, macOS CI green
is a hard ratification gate and not deferrable.

**Push attempt 2026-05-10 14:22:** the driver attempted
`git push origin rafaello-v0.1` directly after
landing the §5.8 macOS un-gating commit (`1e839b3`).
The push hung for ~20 minutes, then on retry failed
with `sign_and_send_pubkey: signing failed for RSA
".../cardno:27_054_871" from agent: agent refused
operation` — the SSH agent is locked / the
hardware-token is not authorising signatures.
Resolution requires the user to unlock the agent
interactively (e.g. `! ssh-add -l` then unlock the
hardware-key PIN); the driver cannot complete this
step autonomously. The eight commits queued for
push (the six follow-ups plus the post-§5.8 Linux
re-run capture and the round-3/round-4 retrospective
revisions) are all on the local `rafaello-v0.1` HEAD
at `21d2ceb` and will land on origin once the user
re-runs the push.

**Capture procedure** (matches m2 manual-validation §5.7):

1. Push `rafaello-v0.1` to GitHub: `git push origin
   rafaello-v0.1`.
2. The existing `.github/workflows/rafaello.yml`
   workflow auto-triggers (m2 commit `7b0daf4` enabled
   `rafaello-v0.1` push triggers and `test-fixture`
   feature; m2 retro §5.7 captured run `25623373610`
   for the macOS-CI green gate at m2 close).
3. Capture the `macos-latest` job URL into this section.
4. Confirm a green run on `macos-latest`. The expected
   pass count is the Linux 516 minus all
   `#[cfg(target_os = "linux")]`-gated tests; an exact
   arithmetic is not pre-named here because the
   exemption inventory depends on whether the
   §5.8-filed rafaello-tui-harness macOS port lands
   before the macOS CI run captures.
5. Any non-platform-inherent failure must be fixed in
   m3 before ratification (per scope round-6: macOS
   failures are NOT retrospective-time follow-ups).

**To be filled in** with the macOS run URL once the
push completes.

## 5. Real interactive `rfl chat` smoke

`scope.md` §"Manual validation" requires a real
interactive `rfl chat` session against the in-test
fixture-entry harness (`RFL_HARNESS_FIXTURES=1`),
screen-recorded, demonstrating:

1. Eight built-in kinds render readably.
2. Unknown-kind falls back to author-supplied fallback
   text.
3. `q` quits cleanly, restoring the terminal.
4. Second `rfl chat` in the same project errors with
   the holder pid (lock contention path).

**Status:** ⏳ pending.

The headless `rfl_chat_demo_bar.rs` (c31) and the
`rfl_chat_locked_session_errors_with_holder_pid.rs`
(c29) integration tests cover the same code paths
mechanically; the manual smoke verifies human-facing
terminal restoration + readability that automation
cannot. Recording lands here once captured.

## 6. CI green — Linux + macOS workflow run URL

**Status:** ⏳ pending the post-retrospective push (same
push as §4).

Both `ubuntu-latest` and `macos-latest` jobs in the
`rafaello.yml` workflow must be green. Workflow run
URL captured here once available.

---

## Acceptance-summary cross-reference

| `scope.md` §"Acceptance summary" bullet | Section | Status |
|-----------------------------------------|---------|--------|
| `cargo test --workspace --features test-fixture` green on Linux | §1 | ✅ |
| **macOS CI green** (hard gate) | §4 | ⏳ pending push |
| `cargo build --workspace --bins --features rafaello-core/test-fixture` green | §2 | ✅ |
| `cargo doc --workspace --no-deps` warning-free | §3 | ✅ |
| Real interactive `rfl chat` recording | §5 | ⏳ pending capture |
| CI green Linux + macOS | §6 | ⏳ pending push |

The retrospective remains pre-evidence on §4 / §5 / §6;
ratification waits on those captures plus pi
ratification of `retrospective.md` round 2+.
