# Pi review round 8 — m2 runtime-extensibility verification

Scope reviewed: `rafaello/plans/milestones/m2-broker-spawn/scope.md`

Milestone: `m2 — rafaello-core broker + locked plugin spawn`

Review stance: targeted verification after the runtime-extensibility discussion tweaks. Focus: confirm the four agreed framing changes are reflected without introducing implementation blockers or new contradictions.

## Verdict

**Ratifiable.**

The runtime-extensibility edits are present and coherent. They keep m2 concrete around lockin/outpost-proxy/socketpair/fittings while documenting the actual long-term seam as authenticated peer admission into the broker. I found no new blocking issues.

During this verification I applied two tiny wording cleanups in `scope.md` so the text consistently names `SandboxBuild` and `tokio_command(...)` after the rename.

## Runtime-extensibility closure check

### 1. Peer admission vs fd inheritance

**Closed.**

The supervisor section now explicitly frames lockin + socketpair + `RFL_BUS_FD` as the v1 backend implementation, not the architectural invariant. The invariant is that core binds an authenticated bus connection to a principal it spawned or attached. The text also correctly defers any `RuntimeBackend`/`SandboxBackend` trait until a second concrete backend exists.

### 2. `SpawnError::Lockin` renamed to `SandboxBuild`

**Closed.**

The public `SpawnError` enum now uses:

```rust
SandboxBuild { canonical: CanonicalId, source: anyhow::Error }
```

with comments explaining that the backend-specific source is lockin for m2 but the variant name is backend-neutral. The spawn sequence maps `builder.tokio_command(...)` failures to `SandboxBuild`.

### 3. Outpost policy vocabulary vs enforcement backend

**Closed.**

The Inputs section now distinguishes:

- `outpost::NetworkPolicy` as the durable shared host-policy vocabulary used by manifests / m1 compilation / future runtimes.
- `outpost-proxy` as m2's lockin-specific enforcement backend.

This avoids baking HTTP CONNECT proxy enforcement into the long-term policy model.

### 4. TMPDIR behavior is not plugin ABI

**Closed.**

The lockin env-clear deviation now says plugins should not treat absent `TMPDIR` / `TMP` / `TEMP` as a portable ABI guarantee. m2 directs plugins that need durable scratch/state to `RFL_PRIVATE_STATE_DIR`, and leaves a future portable `RFL_TEMP_DIR` contract open if needed.

## New blocking findings

None.

## New non-blocking findings

None remaining after the two wording cleanups above.

## Notes on wording cleanup applied

- Replaced the dependency note's stale `SandboxBuilder::command` reference with `SandboxBuilder::tokio_command`.
- Replaced the SP3 stale `Lockin.source` reference with `SandboxBuild.source` and `tokio_command(...)`.
