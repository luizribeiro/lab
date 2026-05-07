# Architecture decision log

Append-only. Reversals add a new row that references the reversed
decision; do not delete or edit prior rows.

Status values:

- `locked` — committed, irreversible without re-opening v1 scope.
- `ratified` — agreed by owner + claude + pi after debate.
- `deferred-to-vN` — not in v1; revisit at the named version.
- `reversed` — superseded by a later row (see Reverses column).

| # | Date | Decision | Rationale | Status | Reverses |
|---|------|----------|-----------|--------|----------|
