Reviewed `agents/m0-retro/claude` through `4c5fae7`.

I also ran:

```bash
cd /home/luiz/lab-wt/m0-retro-claude/fittings
cargo test --test malformed_cancellation malformed_cancellation_is_logged_and_dropped
```

It passed.

## Round 2 findings

### 1. Medium: `MethodNotFound` deferral is documented as “purely additive”, but it is not

`decisions.md` row 36 and `retrospective.md` §2.4 say adding `method: Option<String>` later is “purely additive” because it “defaults to None”.

That is false for a public Rust enum struct variant. Changing:

```rust
MethodNotFound { message, data }
```

to:

```rust
MethodNotFound { method, message, data }
```

is source-breaking for direct constructors and exhaustive pattern matches. There is no automatic default for a new enum-variant field.

Suggested fix: call it a deferred breaking/API-shape cutover that is acceptable before a public fittings v1 boundary, not “purely additive”. Also soften “callers can read `data.method` directly” because current normal `method_not_found(...)` paths can have `data: None`.

### 2. Low: malformed-cancellation test now sends the right mismatch, but still does not assert the WARN classification

The new test now correctly exercises `Str("42")` vs `Num(42)` in both directions. That closes my original coverage blocker.

Residual nit: the test still only asserts “dropped / connection survives / requests complete”; it does not verify the mismatch-specific WARN path versus a generic “no matching in-flight request” WARN. If log text is considered part of S7.1 acceptance, capture tracing output. If not, this is fine.

### 3. Low: minor stale prose remains in §2.1

`retrospective.md` §2.1 still says “A separate follow-up commit on this branch lands that row,” even though the follow-up section later says it already landed. Not harmful, but slightly stale.

## Verdict

The 5 original findings are substantively addressed. I would only request the `MethodNotFound` “purely additive” wording correction before final sign-off.
