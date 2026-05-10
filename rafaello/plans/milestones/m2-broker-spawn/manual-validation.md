# m2 — broker + spawn — manual validation

> Captured 2026-05-09 on the `agents/m2/c31` worktree off
> `rafaello-v0.1`, after c01–c30 had landed.

This document records the milestone-level manual validation called
out in `scope.md` §"Manual validation in `manual-validation.md`".
Per scope, the macOS leg is delegated to CI under the m0/m1
precedent — see `## macOS CI` below.

## Environment

| Item | Value |
|------|-------|
| Host OS | `Linux 6.12.84 x86_64` (NixOS) |
| Rust | `rustc 1.94.0 (4a4ef493e 2026-03-02)` (workspace `rust-toolchain.toml`, observed both bare and inside `nix develop --impure`) |
| Branch | `agents/m2/c31` (off `rafaello-v0.1`) |
| HEAD at capture | `4b415b1 test(rafaello-core): canonical happy-path + cross-plugin round-trip + private-state + taint + plugin-to-core peer call` (c30 — c31 is this commit) |

## 1. `cargo test -p rafaello-core --features test-fixture` green

```
$ nix develop --impure --command cargo test \
    --manifest-path rafaello/Cargo.toml -p rafaello-core \
    --features test-fixture
... (last 20 lines) ...
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/validate_lock_full_pass.rs (rafaello/target/debug/deps/validate_lock_full_pass-02c402d430a86a93)

running 3 tests
test carveout_refusal_surfaces_through_v3 ... ok
test trifecta_failing_plugin_is_refused ... ok
test multi_plugin_fixture_passes_v3 ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/validate_lock_multiplugin_context.rs (rafaello/target/debug/deps/validate_lock_multiplugin_context-54433fa13cb58f05)

running 2 tests
test missing_plugin_dir_for_installed_plugin_is_rejected ... ok
test passes_with_two_plugins_and_distinct_plugin_dirs ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests rafaello_core

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Cargo prints one `test result:` line per test binary rather than a
single milestone-wide total; aggregating across every binary in this
run yields **357 tests passed; 0 failed; 0 ignored**. This includes
the m1 surface (manifest / lock / digest / scrubber / sinks /
trifecta / topic_id / validate / broker_acl) carried forward
unchanged plus the m2 additions (bus + supervisor + fixture
integration tests). `--features test-fixture` is required to compile
the spawn-fixture integration tests; without it, `cargo test`
silently builds a smaller surface and the m2 spawn tests are not
exercised. Run completes in well under the 30 s scope budget.

## 2. Happy-path supervisor spawn fixture with `RUST_LOG=rafaello_core=debug`

```
$ RUST_LOG=rafaello_core=debug nix develop --impure --command \
    cargo test --manifest-path rafaello/Cargo.toml -p rafaello-core \
    --features test-fixture --test supervisor_spawn_fixture_happy_path \
    -- --nocapture
...
     Running tests/supervisor_spawn_fixture_happy_path.rs (rafaello/target/debug/deps/supervisor_spawn_fixture_happy_path-428872710fa15696)

running 1 test
test supervisor_spawn_fixture_happy_path ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.26s
```

`--test <name>` precedes `--`, per pi-2 §14 (a Cargo arg, not a test
arg). The test exercises the canonical headline path: lock load →
supervisor spawn of two `respond_peer_call` fixture plugins
(publisher + watcher) under lockin + outpost-proxy → broker
registration of both → cross-plugin event publish + fan-out →
graceful shutdown.

The non-`test result:` stderr seen during this run is `lockin`
sandbox advisory output (deny-then-permit syscall traces emitted by
the embedded sandbox when the fixture child touches `/proc/self/cgroup`,
`/sys/devices/system/cpu/online`, `/proc/stat` — all expected,
not blocking the run) — those lines are well-formed JSON tagged
`"act":"deny"` and identify the fixture binary as the originating
`cmd`. The test's own broker-registration / fan-out *tracing*
emissions (the `tracing::debug!` call at
`crates/rafaello-core/src/bus.rs:245` and others) are not visible
in this capture because the integration test does not initialise a
`tracing-subscriber` — see follow-up F1.

## 3. Source-module inventory

```
$ find rafaello/crates/rafaello-core/src -name '*.rs' | sort
rafaello/crates/rafaello-core/src/bin/rfl_bus_fixture.rs
rafaello/crates/rafaello-core/src/broker_acl.rs
rafaello/crates/rafaello-core/src/bus.rs
rafaello/crates/rafaello-core/src/carveout.rs
rafaello/crates/rafaello-core/src/compile.rs
rafaello/crates/rafaello-core/src/digest.rs
rafaello/crates/rafaello-core/src/error.rs
rafaello/crates/rafaello-core/src/lib.rs
rafaello/crates/rafaello-core/src/lock/bindings.rs
rafaello/crates/rafaello-core/src/lock/canonical_id.rs
rafaello/crates/rafaello-core/src/lock/flags.rs
rafaello/crates/rafaello-core/src/lock/grant.rs
rafaello/crates/rafaello-core/src/lock/load_policy.rs
rafaello/crates/rafaello-core/src/lock/lock_file.rs
rafaello/crates/rafaello-core/src/lock/mod.rs
rafaello/crates/rafaello-core/src/lock/session.rs
rafaello/crates/rafaello-core/src/manifest/bus.rs
rafaello/crates/rafaello-core/src/manifest/capabilities.rs
rafaello/crates/rafaello-core/src/manifest/capability_path_template.rs
rafaello/crates/rafaello-core/src/manifest/load.rs
rafaello/crates/rafaello-core/src/manifest/mod.rs
rafaello/crates/rafaello-core/src/manifest/placeholders.rs
rafaello/crates/rafaello-core/src/manifest/provides.rs
rafaello/crates/rafaello-core/src/manifest/renderers.rs
rafaello/crates/rafaello-core/src/manifest/safepath.rs
rafaello/crates/rafaello-core/src/manifest/top_level.rs
rafaello/crates/rafaello-core/src/manifest/validate_with_package.rs
rafaello/crates/rafaello-core/src/paths.rs
rafaello/crates/rafaello-core/src/scrubber.rs
rafaello/crates/rafaello-core/src/sinks.rs
rafaello/crates/rafaello-core/src/supervisor.rs
rafaello/crates/rafaello-core/src/topic_id.rs
rafaello/crates/rafaello-core/src/trifecta.rs
rafaello/crates/rafaello-core/src/validate/mod.rs
rafaello/crates/rafaello-core/src/validate/topic.rs
```

m2-introduced files:

- `bus.rs` (443 lines) — broker service: registration, subscribe
  derivation from lock, publish_msg / publish_core / fan-out, §B7
  result-routing protection. The c06/c07 plan-of-record described a
  `bus/` directory with a `publish_msg.rs` submodule; the actual
  landed shape per c06 is a single-file module that internally
  carries the publish path, since there's only one public type
  (`Broker`) and the publish helpers are private. See follow-up F2.
- `supervisor.rs` (925 lines) — `PluginSupervisor`,
  `ManagedSpawn` / `SpawnObservation` / `SpawnHandle` split from
  pi commits round-1 B2, lockin + outpost-proxy + reaper +
  `Drop`-time SIGKILL handoff. As with `bus.rs`, the original
  c14/c20 sketch named a `supervisor/lifecycle.rs` submodule;
  the implementation kept it inline. See follow-up F2.
- `bin/rfl_bus_fixture.rs` (342 lines) — the multi-mode fixture
  binary (`scaffold_only`, `respond_peer_call`, `observer`,
  `call_core_then_exit`, plus the publish_bad_* negative modes).
- `error.rs` — augmented in m2 with `BrokerError`,
  `SpawnError`, and `InvalidPlanReason` (§E in scope), all
  re-exported from `lib.rs`.

## 4. `/proc/<fixture-pid>/fd/` snapshot during a `respond_peer_call` run

Captured by spawning the standalone `rfl-bus-fixture` binary in
`respond_peer_call` mode under a Python harness that mimics the
supervisor wiring: it builds a Unix socketpair, hands the child
half to the fixture as fd 3 via `dup2`, marks it `inheritable`,
and `exec`s the binary with `RFL_BUS_FD=3` and
`RFL_FIXTURE_MODE=respond_peer_call`. The python harness keeps its
own (parent-side) socket half on its own fd 3, so the inode-overlap
check below is meaningful.

```
$ ls -la /proc/3440098/fd/      # child (fixture)
total 0
dr-x------ 2 luiz users 10 May  9 22:44 .
dr-xr-xr-x 9 luiz users  0 May  9 22:44 ..
lr-x------ 1 luiz users 64 May  9 22:44 0 -> /dev/null
l-wx------ 1 luiz users 64 May  9 22:44 1 -> /tmp/fixture_out.txt
l-wx------ 1 luiz users 64 May  9 22:44 2 -> /tmp/fixture_out.txt
lrwx------ 1 luiz users 64 May  9 22:44 3 -> socket:[324029719]
lrwx------ 1 luiz users 64 May  9 22:44 4 -> anon_inode:[eventpoll]
lrwx------ 1 luiz users 64 May  9 22:44 5 -> anon_inode:[eventfd]
lrwx------ 1 luiz users 64 May  9 22:44 6 -> anon_inode:[eventpoll]
lrwx------ 1 luiz users 64 May  9 22:44 7 -> socket:[324023058]
lrwx------ 1 luiz users 64 May  9 22:44 8 -> socket:[324023059]
lrwx------ 1 luiz users 64 May  9 22:44 9 -> socket:[324023058]

$ ls -la /proc/3440096/fd/      # parent (harness mimicking cargo test)
total 0
dr-x------ 2 luiz users  4 May  9 22:44 .
dr-xr-xr-x 9 luiz users  0 May  9 22:44 ..
lr-x------ 1 luiz users 64 May  9 22:44 0 -> /dev/null
l-wx------ 1 luiz users 64 May  9 22:44 1 -> /tmp/fixture_out.txt
l-wx------ 1 luiz users 64 May  9 22:44 2 -> /tmp/fixture_out.txt
lrwx------ 1 luiz users 64 May  9 22:44 3 -> socket:[324029718]
```

Documented invariants per scope §"Manual validation"
(*qualitative* invariants, not exact-count assertions per pi-1
§32, §553):

- ✅ **fd 3 exists in the child and is `socket:[…]`.**
  `3 -> socket:[324029719]`.
- ✅ **The parent does not have a duplicate of the same socket
  inode in its `/proc/self/fd` snapshot.** Parent fd 3 is
  `socket:[324029718]` — the *peer* half of the socketpair, with a
  distinct inode. No parent fd resolves to inode `324029719`. The
  child-side half has not leaked back into the parent — exactly the
  invariant the supervisor relies on (the supervisor closes its
  copy of the child-side fd immediately after `exec` so only the
  child holds it).
- ✅ **Tokio-runtime fds present (eventfd, epoll) are expected;
  not enumerated.** `4` (eventpoll), `5` (eventfd), `6` (eventpoll)
  are the multi-thread runtime's wakeup machinery; `7` / `8` /
  `9` are the connector / peer / dup pair internal to
  `fittings-client`. Counts are not asserted on.

## 5. `cargo doc -p rafaello-core --no-deps`

```
$ nix develop --impure --command cargo doc \
    --manifest-path rafaello/Cargo.toml -p rafaello-core --no-deps
...
warning: public documentation for `SpawnHandle` links to private item `ManagedSpawn`
   --> crates/rafaello-core/src/supervisor.rs:108:37
    |
108 | /// supervisor owns the child via [`ManagedSpawn`] and kills
    |                                     ^^^^^^^^^^^^ this item is private
    |
    = note: this link will resolve properly if you pass `--document-private-items`
    = note: `#[warn(rustdoc::private_intra_doc_links)]` on by default

warning: `rafaello-core` (lib doc) generated 1 warning
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.87s
   Generated /home/luiz/lab-wt/m2-c31/rafaello/target/doc/rafaello_core/index.html
```

**Not warning-free** — exactly one `rustdoc::private_intra_doc_links`
warning on the `SpawnHandle` doc comment, because `ManagedSpawn` is
private and a public docstring intra-doc-links into it. This mirrors
m1's F1 (`Error` link disambiguation): a one-character / one-line
fix at `crates/rafaello-core/src/supervisor.rs:108` that wasn't
caught during c14 review. Recorded as F3 below — the m2
retrospective sweep is the natural place for it, matching the m1
precedent (F1 fixed in retrospective, not in the original commit).

## 6. `cargo build --features test-fixture --bin rfl-bus-fixture`

```
$ nix develop --impure --command cargo build \
    --manifest-path rafaello/Cargo.toml -p rafaello-core \
    --features test-fixture --bin rfl-bus-fixture
...
   Compiling rafaello-core v0.0.0 (/home/luiz/lab-wt/m2-c31/rafaello/crates/rafaello-core)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.90s
```

Green. Demonstrates that the fixture binary builds standalone (i.e.
not just as a side-effect of `cargo test --features test-fixture`),
which is the form the supervisor's spawn path relies on at runtime
when `RAFAELLO_FIXTURE_BINARY` points to it.

## macOS CI

Captured by milestone driver post-c31 (2026-05-10).

**Run:** https://github.com/luizribeiro/lab/actions/runs/25623373610
(commit `7db9da8` on `rafaello-v0.1`).

**Result:** ✅ both jobs green.

| Job | Conclusion |
|-----|------------|
| `test (ubuntu-latest)` | success |
| `test (macos-latest)` | success |

**Round-1 push (`7b0daf4`) failed** with two CI-environment issues
the local devshell didn't expose:

1. **macOS:** `error[E0599]: no associated item named SOCK_CLOEXEC found
   for struct SockFlag` at `supervisor.rs:394`. nix's `SockFlag` exposes
   `SOCK_CLOEXEC` only on Linux; macOS needs `fcntl(F_SETFD, FD_CLOEXEC)`
   after socketpair.
2. **Linux (CI):** `Linux sandbox requires syd but could not find it.
   Set LOCKIN_SYD_PATH, add syd to PATH, or call .syd_path() explicitly.`
   The `.#rafaello` devshell didn't export `LOCKIN_SYD_PATH` (the
   `lockin` devshell does — `lockin/nix/devenv.nix:16`).

**Round-2 fix (`7db9da8`):**
- `rafaello/crates/rafaello-core/src/supervisor.rs` — cfg-gated CLOEXEC
  fallback using `nix::fcntl::fcntl(F_SETFD(FD_CLOEXEC))` for non-Linux.
- `rafaello/nix/devenv.nix` — mirror lockin's `LOCKIN_SYD_PATH` export
  on Linux.

Per pi-1 B4 + pi-1 c29, branch push is a driver-owned action, not a
per-commit agent task. The driver pushed `rafaello-v0.1` to origin
**after** c31 landed; round-1 failure surfaced two cross-platform gaps
that landed as `7db9da8` and pushed for the round-2 green run above.
m2 retrospective §5.7 records the CI-discovered gaps + the fix.

## Follow-ups discovered while exercising this

1. **F1: `supervisor_spawn_fixture_happy_path` does not initialise
   `tracing-subscriber`.** Scope §"Manual validation" (and pi-1
   §197) calls for the test to use `tracing_test::traced_test` or
   explicit `tracing-subscriber` setup so that
   `RUST_LOG=rafaello_core=debug …` actually surfaces broker
   registration + fan-out traces; the landed test does not. The
   §B7 result-routing-protection `tracing::debug!` at
   `crates/rafaello-core/src/bus.rs:245` is the most visible
   victim. m2 retrospective territory; one-liner add to the test.
   Not a public-API regression; the broker / supervisor *emit*
   the traces correctly, only the consumer side is missing.

2. **F2: scope wording on the `bus/` and `supervisor/` submodule
   layout.** `scope.md` §"Manual validation" and the c06/c07/c14
   commit-row sketches name `bus/publish_msg.rs` and
   `supervisor/lifecycle.rs` submodules; the actual landed layout
   keeps both as single-file modules (`bus.rs`, `supervisor.rs`)
   because the publish / lifecycle helpers are private and there
   is exactly one public type per module. m2 retrospective should
   either reword the scope bullet or add a note pointing at the
   single-file rationale, matching the m1 F2 pattern.

3. **F3: `cargo doc` `private_intra_doc_links` warning on
   `crates/rafaello-core/src/supervisor.rs:108`.** Public
   `SpawnHandle` docstring links `[`ManagedSpawn`]` (private). One-
   line fix: replace with backtick-only inline-code, or move
   the link behind a `#[cfg(doc)]`-pub re-export. m2 retrospective
   sweep, not c31 — c31 is a docs-only capture commit and adding a
   source fix here would mix scopes. Direct analogue of m1's F1.

None of the above blocks the c31 acceptance; all three are recorded
for the m2 `retrospective.md`.
