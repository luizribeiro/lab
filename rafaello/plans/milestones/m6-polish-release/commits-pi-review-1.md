# m6 commits.md round-1 pi review

> Verdict: blocking.
> Counts: B/6 M/5 N/4

Reviewed `commits.md` round 1 (`5538f5e` draft text in this worktree), ratified `scope.md` round 5, the owner ratification commit `a0764b3`, `plans/README.md` Phase 2 / prior-milestone patterns, the m5b commits review precedent, and live source under `rafaello/crates/` plus `lockin/crates/sandbox/`.

The draft has good high-level coverage of phases A–J and most owner defaults are visibly represented, but it is not yet handable to per-commit agents. The blocking pattern is row-local green-bar mechanics: several rows cite private or nonexistent live APIs, one row builds a test binary before its source exists, the openai-stub plan is for a bus-event dispatcher while the live binary is an HTTP Chat Completions stub, the release package cannot actually include the stub binary with the current feature gate, and two regression-anchor rows are test-only while live runtime support/hooks are absent.

## Blockers

### B-1. c09 cannot pass its own fake-syd acceptance before c10 exists

**Anchor:** commits.md c09/c10 / scope.md §C2–§C3 / live `lockin/crates/sandbox/Cargo.toml`

**Issue:** c09 adds the `[[bin]] fake-syd` target but explicitly says the source file `lockin/crates/sandbox/tests/bin/fake_syd.rs` ships in c10. c09 acceptance then runs `cargo build -p lockin-sandbox --features test-fixture --bin fake-syd`. That is not row-local green: the target path is missing in c09, and the live package name is `lockin`, not `lockin-sandbox`.

**Recommendation:** Move `tests/bin/fake_syd.rs` into c09 with the `[[bin]]` entry, or defer the `--bin fake-syd` build acceptance to c10. Also use the live package name/manifest invocation (`cargo build --manifest-path lockin/Cargo.toml -p lockin ...`) unless c09 renames the package, which it should not.

### B-2. c09/c10 tests require private lockin internals and a nonexistent typed error surface

**Anchor:** commits.md c09/c10 / live `lockin/crates/sandbox/src/lib.rs:116-123`, `:209-232`

**Issue:** c10 says integration tests construct `SandboxSpec { syd_path, syd_pty_path }` and assert `Err(SandboxError::SydPtyNotFound { … })`. Live `SandboxSpec` is `pub(crate)`, `resolve_syd_path` is private, and there is no `SandboxError` enum at all; lockin currently returns `anyhow::Result`. c09 adds `syd_pty_path` only to `SandboxSpec`, not to the public `SandboxBuilder`, so external tests cannot drive the explicit-path arm.

**Recommendation:** Make c09 define the public testable surface it expects: a `SandboxBuilder::syd_pty_path(...)` method and either a real typed error API or acceptance that matches the existing `anyhow` channel. Put private helper assertions in in-module unit tests if needed; keep integration tests on public builder APIs.

### B-3. c14/c15 design the stub as a bus-event dispatcher, but the live stub is an HTTP API server

**Anchor:** commits.md c14/c15 / scope.md §E1–§E2 / live `rafaello/crates/rafaello-openai-stub/src/bin/rfl_openai_stub.rs`

**Issue:** c14 says the stub handles incoming `user_message` / `tool_result` events and emits canonical `tool_request` / `assistant_message` events. Live `rfl-openai-stub` binds `127.0.0.1:0`, serves `POST /v1/chat/completions`, and returns JSON responses from `--response` / `RFL_OPENAI_STUB_RESPONSE`. It has no bus connection, no canonical event IDs, and no event dispatcher. c15's “feed a fixture `user_message`” acceptance is therefore not implementable against the live binary without rewriting its role.

**Recommendation:** Reshape E1/E2 around the actual HTTP stub: parse `RFL_OPENAI_STUB_SCRIPTED_TURNS`, inspect Chat Completions request messages/tool results, and return scripted ChatCompletionResponse JSON. Name the singular env as `RFL_OPENAI_STUB_RESPONSE` and test by making HTTP requests to the stub, not by feeding bus events.

### B-4. c16/c17 cannot include `rfl-openai-stub` in the release tree as drafted

**Anchor:** commits.md c06/c16/c17 / owner item 13 / live `rafaello/crates/rafaello-openai-stub/Cargo.toml`

**Issue:** Owner item 13 requires `rafaello-openai-stub` in the release package. c16 adds `-p rafaello-openai-stub`, but the live binary target is `required-features = ["test-fixture"]`; a normal Nix package build will not produce `$out/bin/rfl-openai-stub`. c17 then tries to move that missing binary into `share/rafaello/plugins/rfl-openai-stub/bin/`. Separately, c06's inventory and acceptance add manifests/openrpc tests for five plugin crates and omit `rafaello-openai-stub`, even though c17 needs a stub plugin tree to copy.

**Recommendation:** In the same Phase F/B2 plan, either remove the stub binary's release-blocking feature gate or make the Nix build enable the needed feature intentionally. Also add the stub `rafaello.toml`/`openrpc.json` source-tree work and test coverage to c06 if c17 installs it as a plugin tree.

### B-5. c23 is test-only but the cited `core.tools_list` registration seam does not exist

**Anchor:** commits.md c23 / scope.md §I1 / live `rafaello/crates/rafaello/src/chat/test_ordering_hook.rs`, `rafaello-core/src/supervisor/core_service.rs`

**Issue:** c23 promises one new test file asserting `Broker::register_rpc("core.tools_list", _)` precedes provider spawn, using an “existing m5b broker test-ordering hook.” Live code has no `Broker::register_rpc` symbol; `core.tools_list` is served by supervisor/core-service code. The existing startup hook records only `SetAuditWriter` and `PluginSupervisorSpawn`, so a new test file alone cannot observe the claimed ordering.

**Recommendation:** Either make c23 add the minimal production/test instrumentation needed to record the real invariant (for example `ToolSchemaCatalogBuilt` before first `PluginSupervisorSpawn`), or rewrite the acceptance to assert an existing observable path. Do not leave it as a test-only row against nonexistent broker APIs.

### B-6. c24 is test-only, but live `run_chat` still eagerly spawns every tool plugin

**Anchor:** commits.md c24 / scope.md §I2 / live `rafaello/crates/rafaello/src/lib.rs:416-465`

**Issue:** c24 touches only a fixture lock, sidecar manifest, and one test. The test asserts a `load.triggers.kind = "tool"` plugin is not spawned at startup and spawns on first tool call. Live `run_chat` ignores `entry.bindings.load` and eagerly spawns every non-provider plugin with tools in the loop at `lib.rs:455-465`. The planned test will fail unless the same row implements lazy-load runtime behavior or the runtime already exists, which it does not.

**Recommendation:** Promote c24 from “fixture + test” to an implementation row that wires the lazy tool-trigger into the supervisor/chat path, with the fixture test in the same commit. If full lazy spawning is out of m6, change scope/commits to a parser/validation-only regression instead of claiming spawn-on-first-call.

## Major

### M-1. PP1 acceptance cites private/nonexistent helpers from external tests

**Anchor:** commits.md c02/c05/c17 / scope.md PP1 / live `rafaello-core/src/compile.rs:440`, `digest.rs`

**Issue:** c02, c05, and c17 ask tests under `rafaello/crates/rafaello/tests/` to call `compile::resolve_entry(...)`, but live `resolve_entry` is a private function inside `rafaello_core::compile` and is not re-exported. c02 also cites `rafaello_core::digest::recompute`, which does not exist; live helpers are `content_digest` and `manifest_digest`.

**Recommendation:** Either make c02 explicitly expose a small public/test helper in `rafaello-core`, or assert PP1 through public APIs (`validate::lock` / `compile_plugin`) and use the real digest helpers. Update every PP1 row consistently.

### M-2. Dependency lines omit surfaces consumed by later-row acceptance

**Anchor:** commits.md c07, c11, c27

**Issue:** c07's init/install smoke consumes c02/c03 but lists only c05/c06 plus prose “baselining.” c11's empty-DB test says “fresh `rfl init` lock” while declaring only baseline. c27 fills `manual-validation.md` §5 created by c26 but declares `Depends on. c01–c25`, omitting c26.

**Recommendation:** Make the `Depends on.` lines mechanical: list c02/c03 where `rfl init` is invoked, or remove those invocations from the row-local acceptance; add c26 to c27.

### M-3. Decisions-row placeholders are centralized in c28 but missing from the load-bearing rows

**Anchor:** commits.md c02/c05/c09/c11-c12/c14/c16-c20/c25/c28 / scope.md §J3

**Issue:** The draft prompt expected commits that ratify owner judgments or load-bearing invariants to reference the relevant decisions row placeholder. c28 lists rows 59–68, but the rows that actually introduce the decisions mostly do not point at them: c02/c03 should point at row 60, c05 at row 61, c09/c10 at row 62, c11/c12 at row 63, c14/c15 at row 64, c16/c17 at row 65, and c19/c20 at row 66. c25 does mention row 67, which is the right pattern.

**Recommendation:** Add row-placeholder references to each load-bearing commit body/why section so the retrospective append is not a silent late addition. Also fix c03's “owner-judgment item 8” reference: ratified item 8 is `result_large_err`, not init decline semantics.

### M-4. Several live paths in row file lists are wrong or too imprecise

**Anchor:** commits.md c10/c17/c19 / live tree

**Issue:** c10 says to extend `rafaello/crates/rafaello-bus-fixture/src/bin/`, but live `rfl-bus-fixture` is a binary target at `rafaello/crates/rafaello-core/src/bin/rfl_bus_fixture.rs`; there is no `rafaello-bus-fixture` crate. c17 names `rafaello/tests/nix_build_layout.rs`, but runnable rafaello package integration tests live under `rafaello/crates/rafaello/tests/`. The traceability appendix says `Formula/rafaello.rb` while the row creates `homebrew/rafaello.rb`.

**Recommendation:** Normalize every precise file path to the live workspace path before per-commit prompts are generated. If `rafaello/tests` is intended as shorthand, do not use it in row-level `Files touched` lines.

### M-5. Sizing and inventory summaries are not mechanically trustworthy

**Anchor:** commits.md sizing summary / c25 / live `rg result_large_err`

**Issue:** The sizing summary first totals 26 implementation commits, then retallies; c05 appears both as `medium` and as the named `medium-to-large` candidate, while the second bucket says `medium-to-large (2): not-named`. c25 says 2–4 source files for `result_large_err`, but live non-test module-level allows are already five source files (`bus.rs`, `session/mod.rs`, `supervisor.rs`, `reemit/mod.rs`, `agent/mod.rs`) plus one test allow.

**Recommendation:** Recompute the sizing table from the final row list and make c25's inventory acceptance match live `rg` output. If c25 intentionally excludes test-file allows, say so explicitly.

## Nits

### N-1. c04's bundled-plugin fixture mirror creates an unscoped drift surface

**Anchor:** commits.md c04 / scope.md §A4 + PP1

c04 introduces `rafaello/fixtures/m6-bundled-plugins/rfl-openai/` with a placeholder `bin/rfl-openai` while claiming it contains the same files Phase F2 copies. c17/c18 do not actually compare it to the Phase F source tree. Either drop the mirror or add a sync assertion.

### N-2. c10 says “four tests” but enumerates five numbered items

**Anchor:** commits.md c10

The first item is the fake-syd binary, not a test. Reword the count to avoid per-agent confusion.

### N-3. c16's `nix-store --query --references .../bin` acceptance does not list installed binaries

**Anchor:** commits.md c16

`nix-store --query --references` reports store references, not the contents of `$out/bin`. Use `find ./result/bin -maxdepth 1 -type f -printf '%f\n'` or equivalent for the flat pre-c17 layout assertion.

### N-4. Some subjects are broader than one idea

**Anchor:** commits.md c05/c09/c17

The forced-monolithic rows are justified, but their subjects read like long “and/plus” bundles. Keep the justifications, but consider shorter subjects with the bundle described in the body.

## What's working

- Phase A/B/F all carry the PP1 copy/real-file/containment invariant, which is the right load-bearing thread.
- The three required regression anchors from scope §I are present as rows.
- Owner defaults for G.β, no `pty:off` fallback, bus-fixture exclusion, openai-stub inclusion, manual-only Homebrew validation, and Ctrl-C quit are visible.
- The acceptance-traceability appendix is useful; after the row-local mechanics are fixed, it should make the next review much narrower.
