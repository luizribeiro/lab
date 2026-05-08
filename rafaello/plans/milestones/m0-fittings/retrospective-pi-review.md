Adversarial review of `rafaello/plans/milestones/m0-fittings/retrospective.md`:

## Findings

### 1. **Blocking: malformed cancellation coverage is overclaimed**

Retrospective says “No gaps” and that `malformed_cancellation.rs` covers id-type mismatch.

But scope requires:

- `scope.md:163-168`: id-type mismatch = “string id sent for a numeric in-flight key or vice versa”.

Current test does not do that:

- `fittings/tests/malformed_cancellation.rs:121-122` starts in-flight request id `"hold-1"` string.
- `fittings/tests/malformed_cancellation.rs:105-109` sends cancellation id `42` numeric.

That is not a type mismatch for the same logical id; it is just an unknown id. The implementation’s mismatch detector only reports mismatch when `key.to_string() == id.to_string()` but kinds differ:

- `fittings/crates/server/src/server.rs:82-87`

So the test never exercises the intended mismatch path. Fix: add cases like in-flight id `42` + cancellation id `"42"` and/or in-flight id `"42"` + cancellation id `42`, for both LSP and MCP configs.

### 2. **High: errors RFC drift is missed**

Retrospective says `rfc-fittings-errors.md` “matches the implementation byte-for-byte” (`retrospective.md:195-202`). It does not.

The RFC requires `MethodNotFound` to carry `method: Option<String>` and to extract / synthesize method data:

- `rfc-fittings-errors.md:72-80`
- `rfc-fittings-errors.md:202`
- `rfc-fittings-errors.md:239-244`

Implementation has only:

```rust
MethodNotFound { message: String, data: Option<Value> }
```

in `fittings/crates/core/src/error.rs:18-22`, and `error_map.rs` does not implement the RFC’s method extraction/fallback behavior.

Either this is an implementation gap, or it was intentionally cut by `scope.md`/`commits.md`; either way the retrospective must record it as drift/cut, not claim byte-for-byte match.

### 3. **Medium: overview drift is missed**

Retrospective says no drift against `overview.md` (`retrospective.md:173-193`), but `overview.md:313-315` still says:

```text
Client::new(transport, service) constructor binds an inbound Service impl
```

Actual landed API is:

- `Client::connect(connector)` with no service at construction: `fittings/crates/client/src/lib.rs:66`
- service registration via `Client::with_service(svc)`: `fittings/crates/client/src/lib.rs:122`

`overview.md` §15.6 later says `Client::with_service(svc)`, so the overview is internally inconsistent. The retro should not say “no overview text to fix.”

### 4. **Low: `originalCode` claim appears false**

Retrospective says implementation includes `originalCode` alongside `fittingsKind: "invalidServiceCode"` in some paths (`retrospective.md:204-210`).

I found no `originalCode` in code:

```bash
rg originalCode fittings
# no hits
```

Current invalid-code mapping emits only:

```rust
{"fittingsKind": "invalidServiceCode"}
```

in `fittings/crates/wire/src/error_map.rs`. Remove or correct that paragraph.

### 5. **Low: final status text is stale/overconfident**

Current branch already has the follow-up decisions row (`ad22eee`), but retrospective still says “one decisions.md row to follow” (`retrospective.md:456-488`). If the final reviewed artifact includes both commits, this reads stale.

Also, macOS validation is “delegated to CI” without a run/link. That may be acceptable by process, but the retro’s checkmark wording is stronger than the evidence shown.

## Verdict

I would not sign off on the retrospective as written. The main blockers are the untested malformed cancellation type-mismatch case and the missed `MethodNotFound` RFC drift.
