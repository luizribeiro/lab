# m3 — manual validation evidence

> Companion to `retrospective.md`. Records the exact
> acceptance-gate transcripts and out-of-band evidence
> that `scope.md` §"Acceptance summary" requires before
> ratification. Status: round 1, 2026-05-10.

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
failed / 0 ignored.

**Source:** `/tmp/m3-acceptance.log` (transient; the
aggregate above was extracted via `grep "test result:"
/tmp/m3-acceptance.log | awk '{p+=$4; f+=$6; i+=$8; c+=1}
END{print c, p, f, i}'`).

The captured run includes Linux-only tests gated on
`#[cfg(target_os = "linux")]` (e.g.
`frontend_handle_drop_does_not_leak_zombie.rs`,
`supervisor_spawn_post_register_reaps_child.rs`); macOS
will skip these and the count there is expected to be
slightly lower.

> Per `scope.md` round-9 polish: the headline
> `rfl_chat_demo_bar.rs` test ran in under 30 s under
> `cargo test`, well within the m2-set budget.

## 2. Linux build gate — `cargo build --workspace --bins`

**Command** (verbatim from `scope.md` §"Acceptance summary"):
```
nix develop --impure --command cargo build --manifest-path rafaello/Cargo.toml --workspace --bins --features rafaello-core/test-fixture
```

**Status:** ✅ green (captured 2026-05-10).

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

No `private_intra_doc_links` warnings (m2 retro §5.2's
fix held). No new doc warnings introduced under m3's
expanded surface (rafaello-tui, rafaello/lib).

**Source:** `/tmp/m3-doc.log`.

## 4. macOS CI gate — hard ratification gate

**Status:** ⏳ pending the post-retrospective branch
push. Per `scope.md` round-6 tightening, macOS CI green
is a hard ratification gate and not deferrable.

**Capture procedure** (matches m2 manual-validation §5.7):

1. Push `rafaello-v0.1` to GitHub: `git push origin
   rafaello-v0.1`.
2. The existing `.github/workflows/rafaello.yml`
   workflow auto-triggers (m2 commit `7b0daf4` enabled
   `rafaello-v0.1` push triggers and `test-fixture`
   feature; m2 retro §5.7 captured run `25623373610`
   for the macOS-CI green gate at m2 close).
3. Capture the `macos-latest` job URL into this section.
4. Confirm the same 516 (Linux-counted) less the
   `#[cfg(target_os = "linux")]` exemptions tests pass
   on `macos-latest`.
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
