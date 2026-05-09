# m1 — manifest / lock / grant / compiler foundation — retrospective

> **Status:** ratified by owner 2026-05-08 after four pi review
> rounds (`retrospective-pi-review.md` through
> `retrospective-pi-review-4.md`). Owner waiver of the
> fittings flake (m0-known, self-resolves on `mcpfit-v0.1`
> merge) recorded in §"Acceptance summary check". m1 closes;
> m2 scoping starts when the owner says go.
>
> Originally drafted 2026-05-08 after all 36 m1 git commits
> (`ba66f05` → `c8cd1af`) landed on `rafaello-v0.1` (worktree
> `/home/luiz/lab-wt/m1-retro-claude`); this revision folds in
> the four pi review iterations + the doc-drift follow-up
> commits.

This is the milestone-level review against `scope.md` (round 7,
ratified) and `commits.md` (round 4, ratified) per
`plans/README.md` Phase 3. It complements `manual-validation.md`,
which captures the c37 evidence; this file answers the five
retrospective questions and proposes the deltas to
`overview.md` / `decisions.md` / `glossary.md` and the deltas
to `streams/f-manifest/rfc-manifest-schema.md` that
`milestones/README.md` §"Stream RFC drift" specifically assigns
to m1's retrospective.

The five sections below match the questions the milestone driver
was asked to answer.

> **Numbering note.** `commits.md` enumerates plan rows
> c01–c37. The git log carries 36 commits because plan rows
> c31 (`compile filesystem plan via carve-out`) and c32
> (`private-state grant`) bundled into a single git commit
> `14a1688` (see §3 below). From plan c33 onward the git
> commit number runs +1 behind the plan row: git c34 (`f280498`)
> = plan c35 (broker_acl); git c35 (`72d3a90`) = plan c36
> (fittings W cutover); git c36 (`c8cd1af`) = plan c37
> (manual-validation). Earlier plan rows c01–c30 map 1:1.

---

## 1. Coverage

Every named test in `scope.md` §"Demo bar" — both the positive
and the negative integration matrices — landed under
`rafaello/crates/rafaello-core/tests/` (or `fittings/tests/` for
§W). The trace tables in `commits.md` were verified
mechanically against the on-disk listing
(`ls rafaello/crates/rafaello-core/tests/`); every scope-named
file is present. No test from the matrix was dropped or silently
substituted.

### Positive matrix verification

All 37 scope-named positive tests (counted via `awk '/^### Positive integration tests/,/^### Negative/'  scope.md | grep -c '^|.*\.rs.*|'` — pi review-1 corrected the round-1 draft's false "47" count) are present and pass under the
c37 capture (`manual-validation.md` §1: 269 tests / 0 failed).
Spot checks against the trace table:

| scope.md test file | Landed in | Notes |
|--------------------|-----------|-------|
| `manifest_parse_minimal.rs` | `1d84452` (plan c11) | Lifted from c04 per `commits-pi-review-2.md` finding 2 — needs `canonical_bytes()`. |
| `manifest_parse_tool_example.rs` / `manifest_parse_provider_example.rs` / `manifest_parse_renderer_example.rs` | `1d84452` (c11) | Post-simplification rewrites of RFC §9.x examples per scope's "fixture provenance" note. |
| `manifest_placeholder_expansion.rs` | `0cc3034` (c03) | Lives at c03 with the placeholder expander (foundation). |
| `tool_meta_always_confirm_round_trip.rs` | `4cd51f5` (c14) lock-side; `aec82b4` (plan c34) extension | Two-stage per m0 pattern §4.3 — c14 lands lock round-trip; the `CompiledPlugin.tool_meta` half rides the same file in plan c34's compile commit. |
| `lock_parse_round_trip.rs`, `lock_load_policy_round_trip.rs`, `lock_load_policy_eager_string.rs`, `sinks_inferred_flag_round_trips.rs` | `e8159a4` (c13) | All four lock-schema round-trip positives in one commit. |
| `topic_id_derivation.rs`, `topic_id_collision_detection.rs` | `0b32402` (c15) | T1/T3 land together. |
| `compile_default_bundle.rs`, `compile_scoped_bundle_union.rs` | `b71f8f6` (c30) | Bundle-flatten union per row 17. |
| `compile_placeholder_resolves_to_absolute.rs`, `compile_capability_path_nonexistent_write_leaf.rs` | `14a1688` (plan c31+c32 bundle) | Resolver positives. |
| `compile_private_state_grant.rs`, `compile_private_state_excluded_from_workspace_write.rs` | `14a1688` | Topic-id form per pi-3 finding 5; the trifecta-side assertion second-stages the c19 unit baseline. |
| `compile_resource_limit_defaults.rs`, `compile_digest_match.rs` | `aec82b4` (plan c34) | Limits + digest gating. |
| `compile_network_proxy_plan.rs`, `compile_network_proxy_allow_hosts_validates.rs`, `compile_env_set_passes_through.rs` | `f5c6ff8` (plan c33) | NetworkPlan + outpost dry-run + EnvPlan. |
| `broker_acl_extraction.rs`, `broker_acl_tool_owner_resolves_routing.rs` | `f280498` (plan c35) | G1/G2 positives. |
| `digest_content_deterministic.rs`, `digest_distinct_paths_same_target.rs` | `f13b4a3` (c17) | Recursion-stack cycle detection per pi-6 finding 6. |
| `trifecta_two_plugins_one_hop.rs`, `trifecta_iknowwhatimdoing_bypass.rs` | `bee83f2` (c19) | One-hop graph + override. |
| `sinks_infer_defaults.rs`, `sinks_infer_from_named_bundle.rs`, `tool_table_omitted_uses_defaults.rs` | `4a8e618` (c18) | Si1 + the §15.1 default-fill round-trip. |
| `manifest_load_event_pattern_match.rs`, `manifest_validate_load_trigger_cross_refs.rs`, `manifest_grant_match_present.rs`, `manifest_openrpc_sibling_present.rs`, `manifest_canonical_bytes_stable.rs` | `05f8fac` (c10) / `1d84452` (c11) | V1 + validate-with-package positives. |
| `validate_lock_multiplugin_context.rs` | `75427c2` (c22) | `LockValidationContext.plugin_dirs` per pi-6 finding 1. |
| `env_scrubber_strips_known_secrets.rs` | `2ffe3e6` (c21) | Sc2 happy path. |
| `method_not_found_typed_method_round_trip.rs` (in `fittings/tests/`) | `72d3a90` (plan c36) | §W5 table-driven; `cargo test --manifest-path fittings/Cargo.toml --workspace` confirms blast radius is clean for every consumer except the pre-existing m0 flake (§5.1 below). |

### Negative matrix verification

All 86 scope-named negative tests (counted via `awk '/^### Negative integration tests/,/^### Manual/' scope.md | grep -c '^|.*\.rs.*|'` — pi review-1 corrected the round-1 draft's false "71" count; some rows under the negative heading are positive-behaviour assertions of refusal-with-override or pass-on-positive variants of negative fixtures, so the file count is lower than 86 but every named row maps to assertions in landed test files) are present and pass. Spot
checks across the four security-sensitive layers — manifest
grammar, lock-side mirrors, carve-outs, compile-time path
resolver — confirm the trace table maps onto landed git
commits without slips.

### Trace-table caveat (pi review-2 finding 5)

`commits.md`'s trace tables have a small handful of stale
commit-row mappings (e.g. `manifest_grant_match_present.rs`
appears at both c11 and c10 in the positive table) — artefacts
of the round-2 → round-4 renumbering when the c10 V1 commit was
inserted. The coverage conclusion below is scoped to **file
presence + green test runs**, which were verified mechanically;
exact commit-row mappings should be cross-checked against
`git log --oneline -- rafaello/crates/rafaello-core/tests/`
rather than against the commits.md tables. Acceptable for
v1 archive purposes; not blocking ratification.

### Documented alias

`scope.md`'s positive matrix names two tests on adjacent rows
with near-identical descriptions: `compile_digest_match.rs`
(scope row 1136) and `digest_match_compiles.rs` (scope row
1140). `commits.md` resolves this as a documented alias in the
trace table at line 1070: "`digest_match_compiles.rs` | c34
(alias for `compile_digest_match.rs` per scope round-3 wording —
single file under canonical scope name)". Only
`compile_digest_match.rs` landed
(`rafaello/crates/rafaello-core/tests/compile_digest_match.rs` —
the doc comment cites c34 / §D3). This is the canonical scope
name; the duplicate row in scope.md is leftover from the round-1
draft that pi-1 commits-finding 6 had asked to canonicalise. Not
a coverage gap, but the `digest_match_compiles.rs` row should
be dropped from scope.md in a retrospective sweep so future
readers don't think it's a missing test (see §2.6 below).

### Tests added beyond the matrix

These weren't named in `scope.md`'s matrices but were required
by individual `commits.md` acceptance criteria (or by the
"every commit lands green" rule):

- `error_surface_compiles.rs` (c02) — build-only compile
  assertion for the `Error` re-export.
- `safepath_parse.rs`, `capability_path_template_parse.rs` (c03)
  — `manifest::SafePath` / `CapabilityPathTemplate` parse-time
  unit positives + negatives. Foundational for M11.
- `manifest_top_level_parse.rs` (c04) — top-level decode unit
  positive, after pi-3 commits-finding 3 split
  `manifest_parse_minimal.rs` to c11.
- `manifest_provides_parse.rs`, `manifest_bus_parse.rs`,
  `manifest_capabilities_parse.rs`, `manifest_load_parse.rs`,
  `manifest_renderers_parse.rs` (c05–c09) — per-block parse
  units, one per scope §M3–M7. Each commit lands green with a
  basic decode positive.
- `manifest_malformed_sinks_type.rs` (c05) — the type-mismatch
  half of `manifest_malformed_sinks.rs` per pi-3
  commits-finding 6. The grammar half lives at c10 under the
  scope-named file. Two assertion blocks split across two test
  binaries (the round-3 plan was "single file, two
  assertions"; the implementation chose two files because
  `manifest_malformed_sinks.rs` raises `ValidationError` while
  `manifest_malformed_sinks_type.rs` raises `ManifestError`,
  and the test bodies wanted distinct error matches). Both are
  exercised by the same `cargo test` invocation; coverage of
  the scope-named single-file requirement is met.
- `manifest_unknown_tool_field.rs` (c05) — supplementary parse
  negative covering `[provides.tool.<name>]`-table unknown-key
  rejection (`#[serde(deny_unknown_fields)]`) since the
  scope-named `manifest_unknown_field.rs` was reframed in c11
  to cover top-level + nested-under-`[provides]` only.
- `compile_types_compile.rs` (c28) — build-only assertion for
  the `CompiledPlugin` plan type surface.
- `validate_lock_full_pass.rs` (c23) — multi-plugin V3 happy
  path (commits.md c23 acceptance line).

All extras are additive; none replace a scope-named test.

### Coverage verdict

**No gaps.** Every scope-named positive (37 rows) and negative (86 rows)
test is implemented and passes; the c37 capture reports 269
`rafaello-core` tests passing. The §W cutover tests pass against
the full fittings workspace (one pre-existing m0 flake aside,
§5.1). The c37 acceptance gate is met.

---

## 2. Drift against overview / decisions / stream RFCs

The drift items below were **known going in**: scope §"Acceptance
summary" lists them under the retrospective bullet, and
`milestones/README.md` §"Stream RFC drift" assigns them to m1.
This section catalogs them with the canonical fix for each, plus
two undocumented drift items surfaced during review of the
landed code. Three categories:

1. Stream F manifest RFC body — patched in this branch (the
   one stream-RFC patch the README's authoring convention
   carves out for m1).
2. `overview.md` / `decisions.md` / `glossary.md` text — patched
   in this branch.
3. Stream A security RFC drift — recorded only; per
   `plans/README.md` §"Authoring conventions" stream RFCs are
   not retroactively rewritten, and Stream A is not Stream F.

### 2.1 Stream F manifest RFC body — items 1–4 of overview §15.1

`streams/f-manifest/rfc-manifest-schema.md` still ships the
pre-simplification schema:

- §2 (`:39`) declares `runtime = "subprocess"` as a top-level
  field.
- §3 (`:62-83`) describes `[rpc]` with `openrpc =
  "openrpc.json"` *or* inline `[[rpc.methods]]`.
- §6 / §9 worked examples reuse `runtime` and `[rpc]`.
- The `[provides]` block from overview §15.1 item 1 (`tools`,
  `provider`, `[provides.tool.<name>]` with `sinks`,
  `grant_match`, `always_confirm`) does not exist in the RFC
  body — it lives only in the overview's normative-delta
  table.
- `helper_for` is in the RFC's open-question list (§11);
  overview §15.1 item 2 had committed to landing it as a
  top-level field, then `decisions.md` row 26 deferred it to
  v2.
- `eager_failure` (overview §15.1 item 4 STRIKE) was never in
  the RFC body, but the RFC also doesn't carry the
  fail-closed-only commitment that replaced it.
- The validation rules from overview §15.1 item 3 (provider
  network-warning, `[provides.tool.<name>]` table-defaults,
  `helper_for` lock-list cross-check, `always_confirm`
  enforcement semantics) are not in the RFC body.
- `decisions.md` row 31 mandates the `openrpc.json` sibling
  for every plugin; the RFC body does not yet carry that text
  (and §3's "or inline `[[rpc.methods]]`" branch contradicts
  it).

**Canonical fix.** Per `milestones/README.md` §"Stream RFC drift",
m1's retrospective owns the §15.1 items 1–4 patches into the
Stream F RFC body. Land as a separate follow-up commit on this
branch:

> `docs(rafaello-stream-f): fold §15.1 items 1–4 into manifest RFC`

The patch:

1. **Strikes `runtime` from §2.** Replaces the worked
   top-level shape with `schema`, `name`, `version`, `entry`,
   `rafaello`, plus optional metadata. Notes that `runtime` is
   a v2 reservation consumed by the capsa swap (`decisions.md`
   row 30).
2. **Replaces §3 `[rpc]` with the `openrpc.json` sibling
   requirement.** New text: every plugin ships an
   `openrpc.json` sibling at install time
   (`decisions.md` row 31); the manifest does not list
   methods. The inline-methods branch is removed.
3. **Adds a new §"`[provides]` block"** mirroring overview
   §15.1 item 1: `tools`, `provider`,
   `[provides.tool.<name>]` (`sinks`, `grant_match`,
   `always_confirm`).
4. **Adds the validation rules** from §15.1 item 3 (`always_confirm`
   enforced semantics, missing-table defaults aligned with
   m1's pi-6 finding 5 implementation, `helper_for` deferral
   note pointing at row 26).
5. **Updates §9 worked examples** (`rust-tools`,
   `markdown-pretty`, `anthropic`) to drop `runtime` / `[rpc]`
   and add the `[provides]` block, matching the
   post-simplification fixtures m1's tests already use
   (scope's "fixture provenance" note).
6. **Strikes `helper_for` from §11 open questions** and links
   to `decisions.md` row 26 (v2 deferral).
7. **Strikes the eager-failure speculation** (was never in
   the RFC; no diff, but a one-liner pointing at row 26's
   sibling row pinning fail-closed-only is added under §7
   "Lazy-loading").

Per `plans/README.md` §"Authoring conventions" the RFC normally
"stays as a historical artefact of the ratification round" and
drift gets resolved in `decisions.md`. m1 is the documented
exception precisely because the §15.1 deltas are *implementation
contracts*, not historical drift, and m1 is the implementation.

### 2.2 Security RFC §9 — `requires_confirmation` rename

`streams/a-security/rfc-security-model.md` §9 item 2 (`:1195-1197`)
still says:

```
`always_confirm` (per-tool, enforced UX gate; see
 `overview.md` §15.1 row 3 — replaces the earlier
 advisory `requires_confirmation`).
```

The forward-pointing wording is correct (it tells the reader
`always_confirm` is the live name and `requires_confirmation`
is dead). No code references the dead name (`grep -rn
requires_confirmation rafaello/crates fittings/crates fittings/tests
fittings/examples` is empty); the remaining textual occurrences
in `rafaello/plans/...` are rename-note context, m1 process
artefacts, and prior overview-review snapshots — see the
canonical-fix paragraph below for the full list.

**Canonical fix.** No RFC patch needed for the rename itself —
the RFC's §9 #2 mention is in "X replaces Y" rename-note
framing, not as a live field name; the live name is already
`always_confirm`. No code reference exists (`grep -rn
requires_confirmation rafaello/crates fittings` is empty).
The remaining textual occurrences in the plans tree
(`rfc-security-model.md:1197` rename-note; `rfc-camel-on-v1.md:229`
CaMeL-v2 territory; `milestones/README.md`,
`m1-manifest/{scope,commits,driver-notes}.md`, prior overview
reviews — all rename-note context or m1 process artefacts) are
deliberate historical notes and process trail, not drift.
Recorded as resolved-by-design; the m1-retrospective Stream A
patches landed at §2.3 / §2.4 below cover the genuine drift
(helper plugins + external attach).

### 2.3 Security RFC §7.4.1 — helper plugins drift

`streams/a-security/rfc-security-model.md` §7.4.1 (`:1076`)
still describes the full helper-plugin design (`bindings.helper_for`,
`RFL_HELPER_FD`, helper-channel framing). §9 item 5
(`:1211-1215`) accepts it as "v1". `decisions.md` row 26
(2026-05-08) defers helper plugins to v2 and reverses
decision #14.

**Canonical fix.** Patch the RFC body with a v1-status
deferral banner at the top of §7.4.1 pointing at
`decisions.md` row 26 (pi review-1 of this retrospective:
the round-1 "no Stream A patch" wording contradicted
`milestones/README.md` §"Stream RFC drift" which explicitly
assigns this drift to m1's retrospective). The body is
preserved as the historical-as-of-2026-05-07 design; the
banner makes the v1 reader unambiguous. m1's manifest-side
enforcement is already in place — `M2` rejects `helper_for`
at parse time as `ManifestError::ReservedField`
(`tests/manifest_helper_for_field.rs`); `L4` lock schema has
no `helpers` / `helper_for` fields and rejects them
(`tests/lock_helper_field_rejected.rs`). Banner landed in
commit `8d0a28c`.

### 2.4 Security RFC §5.7 — external attach drift

`streams/a-security/rfc-security-model.md` §5.7 still
describes external UDS-attached frontend principals (the
`frontend.<attach-id>.*` external-attach branch).
`decisions.md` row 27 defers external attach to v2 (TUI is the
only frontend in v1; the `frontend.<attach-id>.*` namespace is
"reserved" only in the external-attach sense — the TUI itself
uses it).

**Canonical fix.** Patch the RFC body with a v1-status
deferral banner at the top of §5.7 pointing at `decisions.md`
row 27 (same reasoning as §2.3: pi review-1 caught the
README's explicit assignment of this drift to m1). The body
is preserved as historical-as-of-2026-05-07; the banner
clarifies that v1 ships TUI-only (local-spawned per §5.7.1's
first bullet). m3 owns the TUI implementation per row 34.
Banner landed in commit `8d0a28c` (same commit as §2.3).

### 2.5 Private-state path-key — `<plugin-id>` → `<topic-id>` clarification

`overview.md` §5.5 (`:584-592`), `decisions.md` row 16, and
`glossary.md`'s "Per-plugin private state" entry all say:

```
${PROJECT_ROOT}/.rafaello-plugin-data/<plugin-id>/
```

`<plugin-id>` is ambiguous between the canonical-id form
(`<source>:<name>@<version>`, which contains `:` / `/` / `@`
and is not a safe filename) and the topic-id form
(`id_<base32-no-pad-lower(sha256(canonical_id))[0..16]>`,
which is path-safe by construction). The canonical-id form
cannot be used as a filesystem segment without escaping, so
m1's compiler picked the topic-id form per scope §C5 and
`commits-pi-review-2.md` finding 1, with the matching test
being `tests/compile_private_state_grant.rs`.

**Canonical fix.** Patch all three docs to spell out
"topic-id" as the path segment. Land as a separate follow-up
commit on this branch:

> `docs(rafaello): clarify private-state path key as topic-id`

Three text edits:

- `overview.md` §5.5: `<plugin-id>` → `<topic-id>` with a
  parenthetical "(the hashed form per `decisions.md` row 5;
  the raw `<source>:<name>@<version>` canonical id is not
  path-safe)".
- `decisions.md` row 16: same change, same parenthetical.
- `glossary.md` "Per-plugin private state": same change.

This was named in scope §"Acceptance summary"'s retrospective
bullet (pi-4 finding 4) and is m1's territory because m1's C5
landed the topic-id choice in code.

### 2.6 Scope.md `digest_match_compiles.rs` duplicate row

`scope.md` row 1140 names `digest_match_compiles.rs` as a
positive test. `commits.md` trace table line 1070 documents it
as an alias for `compile_digest_match.rs` (scope row 1136).
Only the canonical name landed (§1 above). Cosmetic drift; if
left in scope.md a future reader counting the matrix will
think a test is missing.

**Canonical fix.** Strike the duplicate row from `scope.md`'s
positive matrix in the same follow-up commit as §2.5, with a
one-line note in the commit message that it's a documented
alias resolved during implementation. Trivial doc cleanup; not
worth a standalone commit.

### 2.7 Verdict

Five pieces of action-required drift, **all landed as
follow-up commits on this branch BEFORE the retrospective
ratifies** (per pi review-1 of this retrospective: the README
+ scope require Stream A patches too — the round-1 draft
incorrectly punted them as "no RFC patch"):

1. ✅ `streams/a-security/rfc-security-model.md` §5.7 + §7.4.1
   — v1-status deferral banners pointing at `decisions.md` rows
   26 / 27 (commit `8d0a28c`).
2. ✅ `streams/f-manifest/rfc-manifest-schema.md` — top-of-RFC
   v1-status banner mapping body sections to live position
   (overview §15.1 + decisions rows 26/30/31/32/17/24); commit
   `5677dae`. Per the README's "RFCs are historical artefacts"
   policy, the body is preserved with a banner rather than
   rewritten section-by-section. The live schema is the union
   of (RFC body) MINUS deferrals MINUS overrides PLUS overview
   §15.1 normative-delta items 1–4.
3. ✅ `overview.md` §5.5 + `glossary.md` "Per-plugin private
   state" — `<plugin-id>` → `<topic-id>` clarification;
   `decisions.md` row 37 added (refines row 16, append-only
   per the decisions log preamble); commit `93761c8`.
4. ✅ `scope.md` duplicate `digest_match_compiles.rs` row
   struck; commit `823e8bb`.
5. ✅ `cargo doc` broken-intra-doc-link on `Error` fixed in
   `rafaello/crates/rafaello-core/src/error.rs`; clippy
   `derivable_impls` (3 sites) + `manual_contains` (1 site)
   cleaned up; rustfmt re-applied across rafaello + fittings;
   commit `823e8bb`.

The `requires_confirmation` rename (§2.2 below) is
resolved-by-design — the RFC's only mention is in §9 #2 in
"X replaces Y" rename-note framing, not as a live field name.

m1 added one new `decisions.md` row (37, refining row 16) for
the private-state path-key clarification; everything else was
already pinned in an existing decisions row or overview §15.1
item before m1 started.

---

## 3. Slipped or cut

### 3.1 Nothing slipped from the scope item lists

No item from scope's `S` / `M` / `L` / `T` / `V` / `C` / `D` /
`G` / `Tr` / `K` / `Sc` / `Si` / `W` / `E` lists was deferred,
dropped, or downgraded. Every public API surface and every
named test landed.

### 3.2 Plan c31 + c32 bundled into git commit `14a1688`

`commits.md` has two adjacent rows in Group 11 (compiler core):

- **plan c31** — "compile path resolver (existing-ancestor
  canonical + lexical suffix + containment) + placeholder
  application" (§C3).
- **plan c32** — "compile filesystem plan via carve-out +
  private-state grant (C5)" (§C5 + §K integration).

Both landed in a single git commit:

```
14a1688 feat(rafaello-core): compile path resolver + carve-out
        + private-state grant (C3, C5, K)
```

This is the only deviation from the 1:1 plan-row → git-commit
mapping in m1. The c31 acceptance tests
(`compile_placeholder_resolves_to_absolute.rs`,
`compile_capability_path_nonexistent_write_leaf.rs`,
`compile_unknown_placeholder.rs`,
`compile_path_escape_after_expansion.rs`,
`compile_capability_path_symlink_ancestor_escape.rs`) and the
c32 acceptance tests
(`compile_private_state_grant.rs`,
`compile_private_state_excluded_from_workspace_write.rs`) all
land in this single commit.

**Why bundled.** The plan c32 commit's body is a 4-line
addition on top of c31's resolver — once the resolver exists
and the post-flatten reads/writes pass through it, wiring
`carveout::compile_against` and injecting the private-state
dir is a thin hookup rather than a standalone idea. The
per-commit agent prompt for c31 read both rows
(commits-md row text inline) and judged the increment too
small to warrant two commits within the workspace-greenness
rule. The bundled commit is ~250 lines across resolver + carve-out
hookup + private-state injection + 7 tests.

This is the sort of judgement call m0 retrospective §4.1
contemplated for "trait changes that don't admit a smaller
increment" — here the increment exists but is small enough
that splitting would have produced a c32 commit of ~30 lines
of glue plus 2 tests. m0's pattern says fewer larger commits
beat more tiny ones when the tiny-commit boundary is
unmotivated. **From plan c33 onward git numbers run +1
behind plan numbers** — see the numbering note above.

The bundling did not change any acceptance behaviour, did not
re-order any external dependency, and did not skip any test.
It is recorded here for traceability rather than as a
slip-or-cut.

### 3.3 No m1a / m1b split was triggered

`scope.md` §"Internal split" + `commits.md` §"m1a / m1b
checkpoint" both flagged a possible split after c18 (sink
default inference + digests + topic-id derivation +
single-plugin validation all landed; no V3 / trifecta /
carve-out / compiler / broker ACL yet). The driver landed c19
through c37 cleanly on top of c01–c18 without forcing a
split. Default ("ship m1 as one milestone") prevailed. The
split boundary remains documented for any future reader who
needs to bisect or pull a partial slice.

### 3.4 No extra-scope features added during implementation

Every test and code path landed in m1 traces back to a row in
`scope.md` or to a `commits.md` per-commit acceptance line. No
agent-introduced "while I'm in here" features. The extras
named in §1.4 (build-only assertions, per-block parse units,
the type-mismatch test split for `manifest_malformed_sinks*`)
are all `commits.md`-ratified or per-commit-greenness
forced — none represent feature creep.

### 3.5 §W landed at the documented spot

The fittings `MethodNotFound` typed-`method` cutover (plan
c36, scope §W) landed as `72d3a90` in the second-to-last
position, after the `rafaello-core` work and before
`manual-validation.md`. `commits.md` allowed late or early
placement; the late slot keeps the cutover isolated from the
new-crate work and matches m0's "one consolidated cutover
commit" pattern (m0 retrospective §4.1). The full fittings
workspace `cargo test` passes (`manual-validation.md` §2);
blast radius across `mcp-server` and `examples/*` is clean.

---

## 4. Process notes for the next milestone driver

These are sharp edges m1 hit that aren't already in
`plans/README.md` §"Recurring operational gotchas" or §"Patterns
from prior milestones". File future gotchas there as they're
learned.

### 4.1 Six pi rounds on `commits.md` is a lot — and most of the deltas were phase-boundary discoveries, not bugs

m1's commit plan went through **six** pi review rounds (`commits-pi-review-1.md` through `commits-pi-review-3.md`
on commits.md plus `pi-review-1.md` through `pi-review-6.md`
on scope.md), versus m0's three on commits.md. Why the increase?

- **Surface size.** m1's scope is a brand-new ~6k-LoC crate
  with 14 module groups, ~120 tests, and dual-validation
  (manifest-side V1+V2 plus lock-side V3 mirroring). m0
  refactored an existing crate in place. More surface →
  more rules → more rounds to converge on consistent rules.
- **Phase-boundary discovery.** `commits-pi-review-2.md`
  finding 5 reorganised the parse-vs-validate boundary
  (round-2 plan had grammar checks at parse time raising
  `ManifestError`; round-3 moved them to V1 raising
  `ValidationError`). This shifted ~14 negative tests from
  c04–c09 into a new c10. The reorganisation was correct —
  two-phase structure is what the implementation actually
  wants — but it took two rounds of churn before it
  settled, because round-1 didn't notice scope.md already
  named the errors as `ValidationError`.
- **Lock-side mirror surface.** Pi-5 finding 1, 2, 3, 6 all
  added new V3 rules mirroring V1 / V2 / parse-time checks
  on the runtime-authority side. Each finding added a
  separate tranche of negative tests. The rule "every
  install-time check the manifest enforces, V3 must enforce
  on the lock too" was new in m1 (m0 didn't have a runtime
  authority); discovery happened in pieces.
- **Numbering churn.** Pi-3 finding 1 collapsed a duplicate
  `c12` (the round-2 commits.md had two commits both
  numbered c12). Each round of structural reorg cascaded
  +1 / +2 renumbers across the trace tables; round-2's
  +1 cascade and round-3's collapse each absorbed a full
  pi pass on their own.

**Recommendation for m2.** Expect five rounds on commits.md if
m2's scope is comparable in size. Plan calendar for it. The
surplus rounds are not wasted — they catch the structural bugs
that wouldn't survive implementation (phase-boundary leaks,
duplicate numbers, missing dep edges). The m0 lesson "if
commits.md looks obviously right after one pass, that's
suspicious" extends: "if it looks right after three passes
on a milestone the size of m1+, that's also suspicious".

### 4.2 Per-commit agent prompts must inline the row text, not cite by row number

The plan c31 + c32 bundling (§3.2 above) traces back to a
prompt-shape problem. The orchestrator's per-commit prompt
read (paraphrased):

> "Read `commits.md`. Implement the next unlanded row in the
> sequence."

The agent on the c31 worktree opened `commits.md`, saw c31 and
c32 sitting adjacent in Group 11, judged the increment between
them small, and bundled both. The bundling itself was
defensible (§3.2) but the **decision happened inside the
agent**, not in the orchestrator's prompt design. Had the
prompt been:

> "Implement EXACTLY plan row c31. The full row text is:
>  [paste]. Acceptance: [paste each bullet]. Do not implement
>  any other row in this commit."

…the agent would have landed c31 alone and the c32 agent would
have layered c32 on top. Either outcome is acceptable, but the
inlined-row form is *predictable*: the orchestrator knows what
will land. The cite-by-row form delegates the granularity
decision to the agent, which is a category of choice that
should sit with the orchestrator (because only the orchestrator
sees the scope-vs-implementation trade-off for the milestone
as a whole).

**Recommendation for m2.** Always paste the full commit row
text + acceptance bullets into the per-commit prompt. Cite
`commits.md` only as a reference, not as the authoritative
source the agent re-reads. This also defends against
mid-implementation `commits.md` drift: if the orchestrator
later patches `commits.md` (e.g. to re-number after a
checkpoint reorg), an in-flight per-commit agent that reads
the doc fresh would see different text than the prompt-time
text.

### 4.3 Two-stage tests landed cleanly using m0's pattern

m1 used the m0 retrospective §4.3 two-stage pattern in two
places without friction:

- `compile_private_state_excluded_from_workspace_write.rs` —
  c19 lands a unit-style trifecta-sanity assertion against a
  pre-built programmatic lock; c32 (bundled into git
  `14a1688`) extends the same file with the
  compiler-injection assertion against a CompiledPlugin.
- `tool_meta_always_confirm_round_trip.rs` — c14 lands the
  lock-side round-trip; plan c34 (`aec82b4`) extends with the
  `CompiledPlugin.tool_meta` half.

Both extensions edit the existing test file rather than create
a new one, per m0's "extend, not duplicate" rule. The pattern
is now well-understood; future drivers should reach for it
explicitly when a scope-named test depends on multiple commits.
Recording here as confirmation that m0's lesson applied
cleanly, not as new guidance.

### 4.4 Pi-review-6 on scope.md was the last possible round

Scope.md ratified at round 7 — six rounds of pi review.
`pi-review-6.md` produced findings (lock-side bundle key /
capability path / tool-name grammar mirrors; exec-paths under
project; `LoadPolicy` snapshot in lock; `compile_plugin`
precondition contract). All landed in scope.md round 7 and
implementation followed. The signal that scope was converged
was that round-6 findings were all **completeness**
(things-V3-should-also-check) rather than **correctness**
(things-V3-says-but-shouldn't); when a review round only adds
to the rule list rather than reshuffling existing rules, the
plan is structurally stable and ratification can land.

**Recommendation for m2.** Track this signal explicitly during
review. If round-N is still reshuffling rules (changing
errors, moving checks between V1/V2/V3, renaming fields),
plan another round. If round-N only adds items to lists, that
round is the last.

### 4.5 `manifest_malformed_sinks.rs` ended up as two files

`commits-pi-review-3.md` finding 6 specified the test as
"single file, two assertion blocks" split across c05 (parse
type-mismatch raising `ManifestError`) and c10 (grammar
raising `ValidationError`). The c05 agent landed
`manifest_malformed_sinks_type.rs` instead — a second file,
not a second `#[test]` in the same file. The c10 agent then
landed `manifest_malformed_sinks.rs` with the grammar half.
Both are exercised by `cargo test`; coverage is met. But it
deviates from the explicit ratified shape.

**Why it happened.** The c05 prompt likely cited "[the malformed
sinks parse case]" without inlining the
"single file, two assertion blocks" instruction. The agent
followed the convention "one test file per scenario" that
holds elsewhere in the suite.

**Recommendation for m2.** When `commits.md` specifies an
unusual test layout (single file with multiple
phase-bound blocks; two-stage tests; single file under a
non-canonical name), the per-commit prompt must state that
shape explicitly. Generic "implement the malformed sinks
test" prompts produce the generic layout. This compounds with
§4.2: cite-by-row hides the explicit layout instruction in
the row body that the agent may not re-read with intent.

---

## 5. Known issues to track

These are pre-existing bugs surfaced during m1 implementation
or new low-severity issues introduced by m1 that don't block
the milestone. Recording here so they don't get forgotten.

### 5.1 Pre-existing m0 flake: `stdio_e2e_runtime_registry_mutation_emits_list_changed_and_updates_tools_list`

Already in `m0-fittings/retrospective.md` §5.2 and
`m0-fittings/manual-validation.md` §2 / "Known follow-up". m1
hit it again during the §W cutover validation
(`manual-validation.md` §2): one failure out of 230 tests in
the fittings workspace, same race as m0. Not introduced by
m1; not in m1's acceptance matrix; carrying the reference
forward so future m1 readers don't re-debug it. Fix lives in
the `mcp-server` test harness (read-then-write instead of
write-all-then-read), per m0 retrospective §5.2's "proposed
fix location".

### 5.2 ✅ Resolved: `cargo doc` broken-intra-doc-link in `rafaello-core` error module

Captured as `manual-validation.md` §3 + §"Follow-ups" F1.
`crates/rafaello-core/src/error.rs:6` originally wrote
``[`Error`]``; rustdoc couldn't disambiguate between the
`Error` enum and the `thiserror::Error` derive-macro re-export
in scope. Fixed by changing to ``[`enum@Error`]``. Landed in
commit `823e8bb` (alongside the §2 doc patches and the
clippy-derivable_impls / manual_contains cleanups).
`cargo doc --manifest-path rafaello/Cargo.toml -p
rafaello-core --no-deps` now warning-free; the scope
§"Acceptance summary" doc bullet is met.

### 5.3 ✅ Resolved: scope wording on the fixtures directory

Scope §"Manual validation" originally called for `tree
rafaello/crates/rafaello-core/tests/fixtures`, but the
directory doesn't exist — m1 fixtures are inline / programmatic
via `tempfile::tempdir()` inside each test file. The
scope bullet now reads `find rafaello/crates/rafaello-core/tests
-type f -name '*.rs' | sort` (the test surface as it actually
exists) and explicitly notes the "no separate fixtures/
directory in v1" intent. Stream F RFC banner also updated to
match. Pi review-2 of this retrospective caught both stale
references.

### 5.4 No new flakes introduced by m1 in `rafaello-core`

Outside §5.1 (pre-existing m0 flake hit during fittings
validation, unrelated to `rafaello-core`), the `rafaello-core`
269-test suite is reproducibly green on Linux across the four
captures in `manual-validation.md` (debug, debug under nix
develop, release, single-binary nix-develop). No m1-introduced
test was flaky during the milestone.

### 5.5 No clippy warnings suppressed by agents

Spot-check across `git log -p ba66f05..c8cd1af |
grep -E "#\[allow\(clippy"` shows no clippy-allow attributes
landed by m1. The pre-commit hook sequence (rustfmt + clippy
+ test) gates every commit, so any agent-suppressed warning
would be visible in the diff.

### 5.6 Where to file these

All four are repo-internal. The current convention is to track
in this `retrospective.md` and via the commit log; if rafaello
ever gets its own issue tracker, port these. Until then, the
milestone driver for m2 has the context here.

---

## Follow-up commits on this branch

All drift fixes and cleanups landed BEFORE the retrospective
ratifies (per pi review-1 + 2 of this retrospective: an
acceptance-owned follow-up isn't optional cleanup). Final state
as of 2026-05-08:

1. ✅ Stream A v1-status banners (§5.7 + §7.4.1) — commit
   `8d0a28c` (§2.3 + §2.4).
2. ✅ Stream F top-of-RFC v1-status banner — commit `5677dae`
   (§2.1).
3. ✅ Private-state topic-id clarification (overview §5.5 +
   glossary + new `decisions.md` row 37 refining row 16) —
   commit `93761c8` (§2.5).
4. ✅ `cargo doc` warning fix + clippy `derivable_impls` /
   `manual_contains` cleanups + rustfmt re-apply across
   rafaello + fittings + scope.md duplicate-row strike — commit
   `823e8bb` (§2.6 + §5.2).
5. ✅ `lockin.toml` overview/glossary drift patched
   (`overview.md` §6 + `glossary.md` Lockin / Sandbox-policy
   entries point at `CompiledPlugin` plan + `decisions.md`
   row 32) — pi review-2 finding 2 caught it after the round-1
   retrospective missed; commit landing this batch.
6. ✅ Scope manual-validation fixtures bullet reworded; Stream
   F banner updated to drop the non-existent `tests/fixtures/`
   reference — pi review-2 finding 3; same commit as #5.

**One** `decisions.md` row added (37, refining row 16 for the
private-state path-key clarification). **Two** stream-RFC
patches landed: Stream A v1-status banners on §5.7 + §7.4.1
+ §9 #2 / #5 strikethroughs (commits `8d0a28c` + the pi-3
fix landing alongside this retrospective revision); Stream F
top-of-RFC v1-status banner (commit `5677dae`). The
README's authoring-conventions rule (RFCs are historical
artefacts) is honoured — bodies are preserved with banners
rather than section-by-section rewrites.

---

## Acceptance summary check

`scope.md` §"Acceptance summary" requires:

- ✅ Every named test in the positive + negative matrices
  implemented and passing. (§1 above; `manual-validation.md`
  §1: 269 `rafaello-core` tests passing on Linux.)
- ✅ `cargo test --manifest-path rafaello/Cargo.toml -p
  rafaello-core` green on Linux. The macOS leg is delegated
  to CI per scope's "no platform-specific code" carve-out;
  if the CI run for the `rafaello-v0.1` tip fails on macOS,
  m1's acceptance flips red.
- ✅ `cargo test --manifest-path fittings/Cargo.toml
  --workspace` — **owner-approved waiver 2026-05-08** for the
  pre-existing m0 flake `mcp-server::stdio_e2e::stdio_e2e_runtime_registry_mutation_emits_list_changed_and_updates_tools_list`.
  m1 retrospective re-run: 3 attempts FAIL, FAIL, PASS — same
  race as m0 retrospective §5.2 (write-all-then-read on
  `tools/list` after `tools/register`). The §W cutover doesn't
  touch the test or its harness; the flake is a pre-existing
  harness bug owned by mcp-server's test infrastructure, not
  the fittings library. **Self-resolves on `mcpfit-v0.1` merge**:
  the in-flight `mcpfit-v0.1` branch (95 commits of mcpfit
  reimplementation work; merge base `0d4ab4a` predates m0)
  moves the mcp-server example out of the fittings workspace
  to `mcpfit/example/`; on `mcpfit-v0.1` the same test passes
  reliably (5/5 in a worktree run during this retrospective)
  because the mcpfit reimplementation processes stdin requests
  in strict order. After `mcpfit-v0.1` merges, the flaky test
  is no longer in the fittings workspace AND no longer flakes
  in its new home — the m1 acceptance gate becomes naturally
  green. The 228 other fittings tests (including the §W
  targeted regression test) all pass at current HEAD.
- ✅ `cargo doc --manifest-path rafaello/Cargo.toml -p
  rafaello-core --no-deps` warning-free (§5.2 — fixed in
  commit `823e8bb`).
- ✅ `manual-validation.md` records the items in scope's
  Manual-validation list (`c8cd1af` — landed at end of
  Phase 3).
- ✅ `retrospective.md` written with drift surfaced as
  deltas. (This file.)
- ✅ Stream F RFC body — v1-status banner landed (§2.1 + commit
  `5677dae`). Per `plans/README.md` "RFCs are historical
  artefacts" policy, the body itself is not section-rewritten;
  the banner maps every RFC text to the live position.
- ✅ Security RFC `requires_confirmation` rename — resolved
  by design (§2.2).
- ✅ Helper plugins / external-attach drift — v1-status
  banners landed in §7.4.1 + §5.7 of the security RFC
  (§2.3 + §2.4 + commit `8d0a28c`).
- ✅ Private-state `<plugin-id>` → `<topic-id>` clarification
  landed in overview §5.5 + glossary + new decisions row 37
  (refines row 16) (§2.5 + commit `93761c8`).

m1 is **done.** Owner approved the fittings-flake waiver
2026-05-08 with the rationale that `mcpfit-v0.1` merges
self-resolve the flake (the test moves out of the fittings
workspace AND no longer races under the mcpfit
reimplementation). All other acceptance gates are met. The
core deliverable (the `rafaello-core` crate with 269 tests
green) has landed; the §W fittings cutover has landed (228
fittings tests green); all documentation reconciliation
listed in §"Follow-up commits on this branch" has landed;
security RFC §7.5 private-state path-key banner landed
alongside the final revision. No open architectural-doc
rough edges currently named.
