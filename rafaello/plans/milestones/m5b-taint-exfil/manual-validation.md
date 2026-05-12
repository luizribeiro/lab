# M5b — Manual Validation Notes

## §3 Wire shapes

- `core.session.confirm_request` `details.taint` is serialised as
  `Vec<TaintEntry>` — a JSON array of `{source, detail?}` objects
  (`detail` omitted when `None`). When the inbound
  `core.session.tool_request` envelope has no taint, the array
  renders as `[]` (an empty array), never `null`. §CD1 / §CD3.

## §4 Manual validation bullets

Six ratified scope §"Manual validation" bullets (verbatim from
`scope.md` round 7). Each is a manual-run transcript to record
before m5b → rafaello-v0.1 final acceptance.

1. **Verbatim-exfil walkthrough** against the m5b fixture with the
   file-backed `rafaello-fetch` (via `RFL_FETCH_TEST_BODY_PATH`);
   demonstrate the `provenance:` block rendering on the send-mail
   modal, operator denies, `mailcat.log` confirmed empty,
   `audit_events` carries the `confirm_request_taint_attached` row
   for the send-mail correlation id (the c23 deny-arm trajectory
   recorded manually).

   *Status*: ⏳ pending — the c23 e2e variant did not fire the
   value-match chain end-to-end (stub limitation per commit
   `86d6124`); the c23b harness sibling closes the chain at the
   ReemitRouter + ConfirmationGate seam. Manual validation needs a
   multi-turn rfl-openai-stub or a real-provider walkthrough to
   demonstrate the chain interactively.

2. **Allow-arm audit trail** — same fixture, operator allows both
   modals; `mailcat.log` receives the verbatim send-mail entry;
   `audit_events` carries the `confirm_request_taint_attached` row
   containing the fetch `{source: "tool", detail: "<rafaello-fetch
   canonical>"}` entry alongside `confirm_allowed` rows for both
   modals (the c24 allow-arm trajectory recorded manually).

   *Status*: ⏳ pending — c24's e2e variant deviated on the
   audit-row primitive (path-3 per commit body); manual validation
   is the canonical surface for the joined allow-arm audit trail.

3. **Overlay rendering plus terminal-clipping/ellipsis** — the c16
   `provenance:` block render exercised at multiple terminal
   widths; capture a screenshot or text dump at 80×24 demonstrating
   the ellipsis behaviour for long taint vectors (scope §"Risks"
   #7 ratifies the audit-row carries the full vector; the overlay
   clips).

   *Status*: ⏳ pending — c16's `tui_confirm_overlay_taint_clipping`
   unit test covers the ellipsis logic; a manual screenshot at
   80×24 is the operator-facing anchor.

4. **macOS CI URL** — the run URL after branch push (m3 / m4 / m5a
   carryover hard gate).

   *Status*: ⏳ pending — fills in after rafaello-v0.1 push and the
   GitHub Actions macOS workflow completes.

5. **Audit-log inspection** — dump `audit_events` from
   `<project_root>/.rafaello/state/session.sqlite` (m5a §2.4 pinned
   the path); assert the three new m5b kinds surface alongside
   m5a's `confirm_request` / `confirm_allowed` / `confirm_denied`
   etc.

   *Status*: ⏳ pending — depends on the bullet-1 / bullet-2 run.
   Example query: `SELECT seq, kind, request_id FROM audit_events
   ORDER BY seq;` after the walkthrough.

6. **No-match / provider-only path** — drive a turn whose
   `tool_request` args don't match any prior tool_result substring;
   the modal fires with provider-only canonical taint; observe
   **no** `confirm_request_taint_attached` row in `audit_events`
   (the §AL1 predicate fails for provider-only taint; §EXFIL3 / c25
   is the mechanical anchor).

   *Status*: ⏳ pending — c25 is the regression anchor; manual run
   confirms the operator-visible behaviour.

If a LiteLLM-proxy-driven run is included alongside the above,
label it explicitly **"Real-provider walkthrough (file-backed
fetch via `RFL_FETCH_TEST_BODY_PATH`)"** — the provider is real
(LiteLLM proxy) but the fetch is still file-backed per scope §A6 /
owner-judgment item 3. No real-network claim. (Round-1 / round-2
retro drafts framed bullet 1 as "Real-network demo" — pi-1 M-6 /
pi-2 B-1 caught the misframing.)

## §5 Extras (not scope bullets)

- **§PT1 violation demo** — drive a plugin that publishes
  `plugin.<id>.tool_result` with a deliberately narrowed `taint`
  claim; observe the `core.lifecycle.publish_rejected` emission
  with `code = "taint_superset_violated"`, the synthetic deny-shaped
  result, and the `plugin_publish_rejected_taint_superset` audit
  row. The c22 fixture's `RFL_FETCH_TEST_TAINT_OVERRIDE` is the
  load-bearing mechanism (m5b retro §3.3).

  *Status*: ⏳ pending — useful for operator confidence in the §PT1
  enforcement path; not scope-mandated.
