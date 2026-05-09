# m2-broker-spawn commits.md — pi review round 3

> Review target: `rafaello/plans/milestones/m2-broker-spawn/commits.md`
> round-3 draft, reviewed against ratified `scope.md` and prior
> `commits-pi-review-{1,2}.md`.
>
> Verdict: **not quite ready to ratify**. Round 3 closes the
> round-2 blockers, but I found two small c20 executable traps in
> the fixture init text. These are localized wording/implementation
> directives, not structural replanning. After those edits, the
> remaining findings are polish-only.

## Round-2 closure check

- **c19 `Response` shape:** closed. The plan now uses
  `Response { id: JsonRpcId::Null, result: Value::Null,
  metadata: Default::default() }`.
- **Reaper/watcher `JoinHandle` ownership:** closed. c14/c21/c25
  now store only `watcher_join`; the watcher owns/awaits the reaper
  handle.
- **c25 mutex-across-await:** closed. c25 drains `managed` into a
  local `Vec` before awaiting.
- **c20 mode-before-fd:** closed for `scaffold_only` and unknown mode.
- **Canonical headline test:** closed. The canonical
  `supervisor_spawn_fixture_happy_path.rs` is now the c30
  publish/observer demo-bar test, not the c21 lifecycle test.
- **c23 `SafePath` / `LockFlags`:** closed.
- **c19 post-register reaper observability:** closed via a test hook.
- **c22 `call_core_then_exit` start gate:** closed.

## Blocking findings

### B1. c20 fixture fd setup omits `set_nonblocking(true)` before Tokio wrapping

**Where:** c20 fixture wire setup, around the `OwnedFd::from_raw_fd(fd)`
step.

**Problem:** c20 says to convert the inherited fd and wrap it directly as
`tokio::net::UnixStream::from_std(...)`. Scope §F3 requires the exact
sequence:

```rust
let std = std::os::unix::net::UnixStream::from(owned);
std.set_nonblocking(true)?;
let stream = tokio::net::UnixStream::from_std(std)?;
```

The inherited child fd is not guaranteed to be nonblocking: supervisor
socketpair creation uses `SOCK_CLOEXEC`, not `SOCK_NONBLOCK`, and the
direct c20 fixture test also inherits a normal Unix fd unless it does extra
work. Tokio `from_std` requires a nonblocking std stream.

**Why it matters:** the first real fixture transport test in c20 can fail or
hang, and every later supervisor fixture test depends on the same code path.

**Fix:** in c20, explicitly require wrapping the `OwnedFd` as a std
`UnixStream`, calling `set_nonblocking(true)`, then passing it to
`tokio::net::UnixStream::from_std(...)`.

### B2. c20 readiness is described before registering the service handlers

**Where:** c20 `respond_peer_call` mode text.

**Problem:** the text currently says the fixture calls
`core.fixture.ready` and then registers the service handling
`core.fixture.start` / `core.fixture.echo`. Scope §H5/§F2 says readiness is
emitted **after** `Client::connect`, `with_service`, and
`with_notification_handler` are installed, specifically so harness calls do
not race into `MethodNotFound`.

**Why it matters:** c20's own direct test sends `core.fixture.start` and
`core.fixture.echo` after observing readiness; c21's peer-call test relies
on the same guarantee. If an agent follows the current order literally,
these tests become racy.

**Fix:** reorder the c20 prose to: build/connect client → install the
mode's service/notification handler → call `core.fixture.ready` → wait for
`core.fixture.start` / serve `core.fixture.echo` → hold until SIGTERM.

## Non-blocking polish

### N1. c30 says four tests but lists five

**Where:** c30.

The row says "Four integration tests" / "owns all four" but lists five
bullets, including `supervisor_peer_call_plugin_to_core.rs`. Change to
"Five integration tests" / "owns all five".

### N2. Unknown-mode fixture test filename drift

**Where:** c03 vs c20.

c03 creates `tests/fixture_binary_unknown_mode_exits_64.rs`; c20 refers to
`tests/fixture_unknown_mode_exits_64.rs` as already existing. Normalize the
name so the c20 prompt does not invite an accidental duplicate or needless
rename.

### N3. Tiny pseudo-code polish in c25

`managed.registered.take().drop()` is not Rust syntax. Prefer prose or
`drop(managed.registered.take())` to avoid prompt-copy confusion.

## Verdict

Round 3 is very close: no remaining structural reorganization, no obvious
scope coverage gap, and the prior round's major blockers are closed. Fix B1
and B2 in c20, then this should be ratifiable with only N-level polish left.
