# m6 commits.md round-4 pi review

> Verdict: blocking.
> Counts: B/2 M/3 N/2

Reviewed round-4 `commits.md` at `70e4352`, `commits-pi-review-3.md`, ratified `scope.md`, owner ratification commit `a0764b3`, and the live code paths named in the prompt (`LoadPolicy`, lockin public command API, and the openai-stub serve/handle split). Round 4 cleans up several mechanical round-3 issues, especially c10's public API use and c14's `std::process::exit(1)` approach.

It does not converge. The lazy-load pivot is not defensible against ratified scope §I2: scope explicitly requires an end-to-end spawn-on-first-call integration test, not parser-only coverage. There is also a concrete macOS CI regression left in c18: the body still uses GNU `find -printf` despite the changelog claiming it was replaced.

## Round-3 follow-up

| prior id | status | round-4 result |
|---|---|---|
| B-1 c10 private `SandboxedCommand.command` | closed | c10 now uses `cmd.spawn()?; child.wait()?` public API |
| B-2 c14 panic isolated in tokio task | closed | c14 now uses `std::process::exit(1)` in `handle()` after deterministic stderr |
| B-3 c16/c18 GNU `find -printf` | partially open | c16 fixed; c18 code block still uses `-printf` |
| B-4 c24a spawn_on_demand API lacks state | argued/pivoted, not closed | runtime surface dropped, but parser-only pivot violates scope §I2 |
| B-5 c24b child-process observability | argued/pivoted, not closed | observability issue vacated only by the invalid parser-only pivot |
| M-1 c24a dispatch hook imprecise | vacated | runtime dispatch hook removed by pivot |
| M-2 fake-syd tests Linux gating | closed | c10 states all three lockin fake-syd tests are `#[cfg(target_os = "linux")]` |
| M-3 c28 decision row numbering | partially open | title/appendix say 59–69, but c28 prose still says placeholders 59–68 per scope |
| M-4 c14 stale size text | closed | c14 size text says exhaustion-panic-and-process-exit |
| N-1 c09 count | closed | c09 says five coordinated edits |
| N-2 c04 `/usr/bin/true` | closed | synthetic binary no longer hard-codes `/usr/bin/true` |
| N-3 c28 stale round/budget text | mostly closed | acceptance says 27 impl + c28 retro = 28 total |

## Lazy-load pivot legitimacy

**Verdict: no — not defensible without owner/scope re-ratification.**

Ratified scope §I2 is not ambiguous parser-only language. It says the `load.triggers.kind = "tool"` field is plumbed but “never exercised end-to-end,” and that m6 lands “a fixture lock that uses the trigger + an integration test at `rafaello/tests/lazy_load_tool_trigger_spawns_on_first_call.rs`.” The owner ratification item 6 says lazy-load coverage is in scope; scope §I2 defines that coverage as spawn-on-first-call observability.

Round 4's c24 parser-validation test is useful, but it proves only deserialization of `LoadPolicy::Lazy { command }`. It does not exercise lazy loading, does not prove not-spawned-at-startup, and does not prove spawn-on-first-call. The row-68 deferral is a reasonable proposal, but it changes the ratified acceptance and needs an explicit scope/owner revision rather than a `commits.md` reinterpretation.

## Blockers

### B-1. c24 parser-validation-only does not satisfy ratified scope §I2

**Anchor:** commits.md c24 / scope.md §I2 / owner item 6

**Issue:** c24 replaces the scoped integration test with `lazy_load_command_trigger_parses_from_lock.rs`, a parser/validation regression. Scope §I2 explicitly requires a fixture lock plus an integration test named around `lazy_load_tool_trigger_spawns_on_first_call.rs`; it calls out that the field was never exercised **end-to-end**. The new c24 acceptance even asserts `rg "spawn_on_demand" ...` returns zero hits, confirming the runtime half is intentionally absent.

**Recommendation:** Restore an end-to-end I2 row (implementation + observable test) or take the pivot back through scope/owner ratification. If the owner accepts deferral, record it in a revised scope/ratification note; until then, parser-only c24 is a scope miss.

### B-2. c18 still uses GNU `find -printf` in the macOS CI matrix

**Anchor:** commits.md c18 / scope.md §F3 macOS CI hard gate

**Issue:** The round-4 changelog says both c16 and c18 use `find ... -exec basename {} \;`, but c18's workflow block still contains:

```sh
test "$(find ./result/bin -maxdepth 1 -type f -printf '%f\n' | sort | tr '\n' ' ')" = "rfl rfl-tui "
```

BSD `find` on `macos-latest` does not support `-printf`, so the macOS hard gate will fail.

**Recommendation:** Update the c18 workflow body to the portable form already used in c16, e.g. `find ./result/bin -maxdepth 1 -type f -exec basename {} \; | sort | tr '\n' ' '`.

## Major

### M-1. c28 decisions-row prose is still internally inconsistent

**Anchor:** commits.md c28 / acceptance appendix

**Issue:** The heading and appendix say `decisions.md` rows 59–69, and c28 lists row 69. But the prose introducing the list still says “row appends — placeholders 59–68 per scope §J3.” That is no longer accurate after adding row 68 for lazy-load deferral and moving m6 ratification to row 69.

**Recommendation:** If the lazy-load deferral survives owner review, say plainly that round 4 expands the retrospective placeholder range to 59–69. If the pivot is reverted, remove row 68 deferral and return to the ratified range.

### M-2. c14/c15 call the exit path a panic, but implement `process::exit(1)`

**Anchor:** commits.md c14/c15 / scope.md §E1–§E2

**Issue:** c14's mechanism is now viable against the live serve loop, but the wording still mixes “panic” and `std::process::exit(1)`. c15's test is named `exhaustion_panics_deterministically`, but it observes process exit, not a Rust panic. This is probably acceptable as an HTTP-stub analogue, but the row should be precise so agents do not try to combine both mechanisms.

**Recommendation:** Phrase the semantic as “fatal deterministic process exit with the scoped panic message” or actually use a panic propagated through a fatal channel. Keep test names aligned with the mechanism.

### M-3. c18's acceptance no longer mentions the F2 layout check it adds

**Anchor:** commits.md c18

**Issue:** c18's `What` block adds the F2 layout shell-step, but its `Acceptance` bullets only require the workflow to have ubuntu/macos jobs and to run `nix build`, plus a green CI URL. The observable acceptance should include the plugin-tree layout step, especially because c17 now delegates real Nix-layout verification to c18.

**Recommendation:** Add an acceptance bullet that the workflow contains and runs the portable F2 layout check (`$out/bin` exactly `rfl`, `rfl-tui`; plugin manifests/openrpc/binaries present and non-symlink).

## Nits

### N-1. c24's fixture TOML shorthand is imprecise

**Anchor:** commits.md c24

The row says the fixture lock uses `[bindings.load] command = ["read-file"]`. In the actual lock file this table must be nested under the plugin entry, e.g. `[plugin."local:readfile@0.0.0".bindings.load]`. The surrounding prose implies this, but the code block should be exact.

### N-2. c18 appendix text still names the old `find -printf` shape

**Anchor:** commits.md acceptance traceability appendix Phase F

The appendix says the F2 layout shell-step uses `find ./result/bin -maxdepth 1 -type f`; after fixing B-2, make it match the portable command or keep it generic.

## What's working

- c04/c07 now have a clean dependency order.
- c09/c10 are much closer to live lockin APIs; c10 uses the public spawn/wait path and Linux-gates the fake-syd tests.
- c14's `process::exit(1)` is a viable way to make fatal scripted-turn failures observable from the HTTP stub child process.
- The row count is mechanically back to 27 implementation rows plus c28 retro = 28 total slots.
