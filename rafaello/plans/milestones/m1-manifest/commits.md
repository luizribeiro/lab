# m1-manifest — commits

> **Status:** draft (round 3). Two pi rounds:
> `commits-pi-review-1.md` (6 blocking + 4 high + 4 minor, all
> resolved at round 2); `commits-pi-review-2.md` (6 blocking + 3
> high + 3 minor). This revision resolves all pi-2 findings,
> including a phase-boundary restructure (parse decodes raw,
> V1/V2/V3 own grammar + ACL refusals so `ValidationError`
> variants land at the right phase per scope's negative matrix).

Ordered commit list for m1, derived from `scope.md` (round 7).
Each commit is one logical idea **and leaves the workspace green**
— pre-commit hooks (rustfmt + clippy + test suites) gate every
commit; intermediate non-green states are not allowed. Commits
land sequentially on per-commit branches `agents/m1/c<NN>`
rebased onto `rafaello-v0.1`, no merge commits, no force pushes.
Tests land with the code that exercises them per
`~/.claude/CLAUDE.md`.

## Conventions

- Subject style `<type>(<scope>): <imperative>`. Scopes:
  `rafaello-core` (the new m1 crate), `fittings-core` /
  `fittings-wire` / `fittings` (for §W's workspace cutover),
  `rafaello` (workspace `Cargo.toml` changes), `rafaello-m1`
  (docs).
- "Acceptance" lists new tests + the pre-commit invariants the
  commit must keep green.
- "Depends on" cites the *lowest* commit numbers whose code or
  types this commit references. A commit only lands after every
  declared dependency has landed on `rafaello-v0.1`.
- Test files live under `rafaello/crates/rafaello-core/tests/`
  (or `rafaello/crates/rafaello-core/tests/fixtures/...` for
  fixture trees) unless otherwise noted; §W tests live under
  `fittings/tests/`.
- Per-commit agents pre-flight `cargo test --manifest-path
  rafaello/Cargo.toml -p rafaello-core` (and `cargo test
  --manifest-path fittings/Cargo.toml --workspace` for §W) until
  green before invoking pre-commit hooks.

## m1a / m1b checkpoint

Per scope §"Internal split" + pi review-2 finding 12: an explicit
go/no-go checkpoint sits **after c18** (parsers + lock schema +
canonical id + topic-id + digest + sink-default inference all
landed; no validation-as-orchestration / trifecta / carve-out /
compiler / broker ACL yet). The driver stops, re-evaluates, and
either continues with one milestone or opens an m1a (c01–c18) /
m1b (c19–c40) owner-ratification request.

The boundary matches the scope's natural data-vs-policy split:
m1a = "data layer" (parsers, schemas, derivations,
infrastructural typed errors, single-plugin validation,
fixtures); m1b = "policy + emission layer" (cross-plugin
validation orchestration, trifecta, carve-out, compiler core,
broker ACL, fittings W). Default: ship one milestone.

---

## Group 0 — Foundation: crate + workspace deps + typed errors + PathContext + SafePath + placeholders

### c01 — chore(rafaello): introduce `[workspace.dependencies]` and add `rafaello-core` crate skeleton

- **What.** Add a `[workspace.dependencies]` table to
  `rafaello/Cargo.toml` listing every crate scope §S2 names
  (`serde`, `toml`, `serde_json`, `sha2`, `data-encoding`,
  `thiserror`, `semver`, `chrono`, `outpost = { path =
  "../outpost/crates/outpost" }`, `tempfile` as a dev-dep). Add
  `crates/rafaello-core/` to `[workspace.members]`. Create
  `crates/rafaello-core/Cargo.toml` with `name = "rafaello-core"`,
  empty `[dependencies]` list (subsequent commits pull from
  workspace.dependencies as they need them) and an empty
  `[dev-dependencies]` block where `tempfile = { workspace =
  true }` will land when c10's fixture tests first need it (pi-2
  commits-minor: `[workspace.dependencies]` doesn't carry a
  dev-only flag — the workspace declaration is just the version
  pin; the dev-dep is added per-crate). `crates/rafaello-core/src/lib.rs`
  contains only `// crate doc placeholder; modules land in
  subsequent m1 commits.` so the crate compiles.
- **Why.** scope §S1, §S2.
- **Depends on.** baseline.
- **Acceptance.** `cargo test --manifest-path rafaello/Cargo.toml
  -p rafaello-core` green (no tests yet). `cargo doc
  --manifest-path rafaello/Cargo.toml -p rafaello-core --no-deps`
  warning-free. `cargo metadata --manifest-path
  rafaello/Cargo.toml --format-version 1` lists `outpost` as a
  workspace dep.

### c02 — feat(rafaello-core): typed-error enums + top-level `Error` (E1)

- **What.** New `rafaello_core::error` module landing every
  module-local error enum scope §E1 names: `ManifestError`,
  `LockError`, `ValidationError`, `CompileError`, `DigestError`,
  `CarveOutError`, `TrifectaError`. All `thiserror`-derived,
  `#[non_exhaustive]`, structured-context variants per scope's
  per-section error names. Top-level `rafaello_core::Error`
  unifies them via `#[from]`. Empty-shell variants are fine —
  the variant *names* are the contract; the constructing code
  lands in subsequent commits. Re-exports out of `lib.rs`:
  `pub use error::{Error, ManifestError, LockError,
  ValidationError, CompileError, DigestError, CarveOutError,
  TrifectaError}`.
- **Why.** scope §E1 (pi-1 commits-finding 7 — error surface had
  no explicit commit; landing it early lets every subsequent
  commit reference variant names without forward refs).
- **Depends on.** c01.
- **Acceptance.** `tests/error_surface_compiles.rs` is a
  build-only test asserting each module error enum is
  reachable through `rafaello_core::Error` and exposes the
  variant names scope §E1 enumerates. (Runtime behaviour:
  zero — variants are constructed by later commits.)

### c03 — feat(rafaello-core): infra — `PathContext`, `SafePath`, `CapabilityPathTemplate`, placeholder expander

- **What.** New `rafaello_core::paths` submodule (re-exported as
  needed) carrying the path infrastructure that several later
  commits (manifest validate-with-package, trifecta, carve-out,
  V3, compile) depend on. Scope §M11's two vocabularies land
  here (pi-1 commits-finding 4 — earlier draft introduced these
  inside c08/c29 and forced fragile dependency bullets):
  - `PathContext { project_root, home, plugin_dir, cache_dir,
    state_dir }` — the per-plugin context.
  - `manifest::SafePath::parse(s) -> Result<Self, ManifestError>`
    — relative-package paths only (no `..`, no `/`-leading, no
    empty segments, no control chars, no `\`).
  - `manifest::CapabilityPathTemplate::parse(s)` — accepts the
    closed M8 placeholder set as a prefix OR an absolute host
    path; rejects bare relative paths, control chars,
    non-UTF-8, `\`. `..` parser-allowed (compile-time
    containment check is c31's resolver).
  - `manifest::placeholders::expand(input, ctx) -> Result<String,
    ManifestError>` — the closed `${project}` / `${home}` /
    `${plugin}` / `${cache}` / `${state}` substitution.
  - `paths::resolve_under_root(template, ctx, root_kind) ->
    Result<PathBuf, PathError>` — pi-2 commits-finding 1 + 4
    pulled this resolver up from the round-2 c31: walk the
    post-expansion path component-by-component, canonicalise
    the longest existing ancestor (with symlink + escape
    checks), lexically join the non-existent suffix, final
    containment check against the named root (`Project` or
    `Plugin`). Used by V3's exec-path refusal (c27),
    `validate_with_package`'s exec-path refusal (c11), and
    the compiler (c31). Centralising the resolver here
    removes the round-2 c27-depends-on-c31 numeric back-edge.
- **Why.** scope §M8, §M11, §C3 resolver (pi-1 commits-finding
  4 + pi-2 commits-finding 1).
- **Depends on.** c01, c02.
- **Acceptance.** `tests/safepath_parse.rs` (positive +
  negatives: `..`, leading `/`, empty segment, `\`, control
  char). `tests/capability_path_template_parse.rs` (positive:
  each placeholder + absolute host path; negative: bare
  relative). `tests/manifest_placeholder_expansion.rs` —
  worked examples for every placeholder + nested mixes. (The
  placeholder test is named in scope's positive matrix; this
  is its home.)

---

## Group 1 — Manifest schema + parser (M1–M9 minus M8/M11 already in Group 0)

### c04 — feat(rafaello-core): `Manifest` top-level + reserved-field pre-scan + `name` grammar

- **What.** New `rafaello_core::manifest` module. Types:
  `Manifest { schema, name, version, entry, rafaello,
  description?, authors?, license?, homepage? }` per §M1.
  `entry` typed as `SafePath` (from c03). `name` validated
  against the topic-segment grammar `[a-z0-9_][a-z0-9_-]*` per
  §M1. `Manifest::parse(s) -> Result<Self, ManifestError>` runs
  a `toml::Table` pre-scan first per §M2 rejecting `runtime` /
  `rpc` / `helper_for` with `ManifestError::ReservedField`,
  then deserialises with `#[serde(deny_unknown_fields)]`.
- **Why.** scope §M1, §M2.
- **Depends on.** c02, c03.
- **Acceptance.** Positive: `tests/manifest_parse_minimal.rs`.
  Negatives: `tests/manifest_unknown_field.rs`,
  `tests/manifest_legacy_runtime_field.rs`,
  `tests/manifest_legacy_rpc_block.rs`,
  `tests/manifest_helper_for_field.rs`,
  `tests/manifest_invalid_name.rs`.

**Phase boundary** (pi-2 commits-finding 5 + 6): the parse
commits c05–c09 below decode TOML into typed structures
**without grammar enforcement on string fields** — tool names,
topic/pattern segments, sink classes, renderer kinds are
stored as raw `String` (or typed newtypes whose `try_from`
runs in V1). Parse-time errors are reserved for `ManifestError`:
TOML schema, `deny_unknown_fields`, `ReservedField`, M11 path
shape (`SafePath` / `CapabilityPathTemplate`), and serde
type-mismatch. **All grammar / cross-ref / namespace ACL
checks land in c10's `validate::manifest_standalone` (V1)
with the `ValidationError` variants scope's negative matrix
names.** This avoids the round-2 "scope says ValidationError
but parser raises ManifestError" inconsistency.

### c05 — feat(rafaello-core): manifest `[provides]` block raw decode

- **What.** Extend `manifest` with `Provides` typed struct:
  `tools: Vec<String>`, `provider: Option<String>`, and
  `tool: BTreeMap<String, ToolMetaManifest>` where
  `ToolMetaManifest = { sinks: Option<Vec<String>>,
  grant_match: Option<SafePath>, always_confirm: bool (default
  false) }`. SafePath does run at parse (path shape =
  `ManifestError`); tool-name and sink-class grammar checks
  are deferred to V1 (c10).
- **Why.** scope §M3 (raw decode half).
- **Depends on.** c04.
- **Acceptance.** Positive: `tests/manifest_provides_parse.rs`
  (basic decode of minimal `[provides]`). Negative:
  `tests/manifest_grant_match_traversal.rs` (this stays
  parse-time since `SafePath::parse` raises `ManifestError`).

### c06 — feat(rafaello-core): manifest `[bus]` block raw decode

- **What.** Extend `manifest` with `Bus { subscribes:
  Vec<String>, publishes: Vec<String> }` — strings only at
  parse time. Topic / pattern grammars + namespace ACL +
  pattern-vs-topic discipline checks land in V1 (c10).
- **Why.** scope §M4 (raw decode half).
- **Depends on.** c05.
- **Acceptance.** Positive: `tests/manifest_bus_parse.rs`
  (basic decode of a `[bus]` table). All bus-related
  ValidationError negatives move to c10.

### c07 — feat(rafaello-core): manifest `[capabilities]` block raw decode

- **What.** Extend `manifest` with `Capabilities` map:
  `BTreeMap<String, CapabilityBundle>` (bundle keys stored as
  `String`; `Default | Named(<n>)` resolution + tool-name
  cross-ref happens in V1). `CapabilityBundle` has
  `filesystem`, `network`, `env`, `limits` sub-tables. Path
  fields in `filesystem` typed as `CapabilityPathTemplate`
  (from c03 — that newtype's parse runs at decode, raising
  `ManifestError` for shape errors only). `network.allow_hosts`
  vs mode rule deferred to V1.
- **Why.** scope §M5 (raw decode half).
- **Depends on.** c05, c06.
- **Acceptance.** Positive: parse covered by c12's worked
  examples. Standalone:
  `tests/manifest_capabilities_parse.rs` (basic
  `[capabilities.default.filesystem]` decode).

### c08 — feat(rafaello-core): manifest `[load]` block raw decode

- **What.** Extend `manifest` with `Load` enum: `Eager | Boot |
  Manual | Lazy { event: Vec<String>, command: Vec<String>,
  kind: Vec<String> }`. Parser handles string-shorthand and
  table forms per §M6. Cross-ref checks against
  `provides.tools` / `bus.subscribes` patterns / renderer
  kinds defer to V1.
- **Why.** scope §M6 (raw decode half).
- **Depends on.** c06, c07.
- **Acceptance.** Positive: `tests/manifest_load_parse.rs`
  (string-shorthand `"eager"` and table-form `{ event = [...],
  command = [...], kind = [...] }` both decode).

### c09 — feat(rafaello-core): manifest `[[renderers]]` array raw decode

- **What.** Extend `manifest` with `Renderer { kind: String,
  priority: u32 (default 100), method: Option<String> }`.
  Built-in / prefixed kind grammar deferred to V1.
- **Why.** scope §M7 (raw decode half).
- **Depends on.** c08.
- **Acceptance.** Positive: `tests/manifest_renderers_parse.rs`
  (array-of-tables decodes; default priority).

### c10 — feat(rafaello-core): validate::manifest_standalone (V1) — grammar + cross-refs + namespace ACL + bundle keys + allow_hosts

- **What.** Land the public `validate::manifest_standalone(manifest:
  &Manifest) -> Result<(), ValidationError>` API per scope §V1.
  Performs every check the parse commits deferred:
  - tool-name grammar (`[a-z0-9_][a-z0-9_-]*`),
  - sink-class grammar (known classes + `[a-z0-9_]+` custom),
  - `manifest.name` topic-segment grammar (M1; lifted to V1
    so the test surface is uniform),
  - topic / pattern segment grammar + minimum two segments
    (security RFC §5.1, pi-5 medium 10),
  - pattern-vs-topic discipline (publishes are topics; no
    `*` / `**` in publish position; subscribes are patterns,
    `**` final-only),
  - canonical-id-independent publish ACL (`core.*` /
    `frontend.*` rejected),
  - bundle-key consistency (`Default | Named(<n>)` where
    `<n>` ∈ `provides.tools`),
  - `network.allow_hosts` requires `mode = "proxy"`,
  - tool-table presence: missing `[provides.tool.<n>]` for
    declared tools gets §15.1 defaults; orphan tables
    rejected as `UnknownToolTable`,
  - load-trigger cross-refs (`command` ∈ `provides.tools`,
    `event` matched by some `bus.subscribes` pattern,
    `kind` ∈ declared renderer kinds),
  - renderer kind grammar (built-ins reserved; plugin kinds
    require `<vendor>:<kind>` prefix per Stream E §8).
- **Why.** scope §V1 + §M1 name grammar + tool-table-defaults
  alignment with overview §15.1.
- **Depends on.** c05, c06, c07, c08, c09.
- **Acceptance.** Positives:
  `tests/manifest_validate_load_trigger_cross_refs.rs`,
  `tests/manifest_load_event_pattern_match.rs` (table-driven:
  positive subscribe-pattern match AND negative unrelated-event
  rejection per pi-1 commits-finding 8).
  Negatives:
  `tests/manifest_invalid_name.rs`,
  `tests/manifest_dotted_tool_name.rs`,
  `tests/manifest_malformed_sinks.rs`,
  `tests/manifest_publishes_core_topic.rs`,
  `tests/manifest_publishes_frontend_topic.rs`,
  `tests/manifest_publish_with_wildcard.rs`,
  `tests/manifest_subscribe_invalid_pattern.rs`,
  `tests/manifest_topic_segment_grammar.rs`,
  `tests/manifest_topic_too_few_segments.rs`,
  `tests/manifest_unknown_bundle_key.rs`,
  `tests/manifest_allow_hosts_outside_proxy.rs`,
  `tests/manifest_load_trigger_unknown_command.rs`,
  `tests/manifest_reserved_renderer_kind.rs`,
  `tests/manifest_unprefixed_renderer_kind.rs`.

### c12 — feat(rafaello-core): manifest validate-with-package + canonical_bytes + worked examples + entry/grant_match resolution + openrpc + exec_paths inside-project refusal

- **What.** Final manifest layer. Land:
  - `Manifest::canonical_bytes()` per §M9 (TOML re-emit with
    sorted keys via `toml::Table`),
  - `manifest::validate_with_package(manifest_path,
    package_dir, manifest)` per §M10:
    - `entry` resolution + escape + file-vs-dir checks,
    - `grant_match` resolution + escape + presence,
    - **openrpc.json sibling required for every plugin**
      per `decisions.md` row 31 (pi-3 finding 4),
    - **`exec_paths` / `exec_dirs` resolving inside
      `${project}` refused** per §V1 + security RFC §6.9
      (pi-6 finding 4) — uses `paths::resolve_under_root`
      from c03.
- **Why.** scope §M9, §M10, §V1 exec_paths bullet.
- **Depends on.** c03 (resolver), c10 (V1 surface; this commit
  adds package-level checks layered on top).
- **Acceptance.** Positives:
  `tests/manifest_parse_minimal.rs` (lifted from c04 per
  pi-2 commits-finding 2 — needs `canonical_bytes()`),
  `tests/manifest_canonical_bytes_stable.rs`,
  `tests/manifest_parse_tool_example.rs`,
  `tests/manifest_parse_provider_example.rs`,
  `tests/manifest_parse_renderer_example.rs`,
  `tests/manifest_openrpc_sibling_present.rs`,
  `tests/manifest_grant_match_present.rs`. (The
  `tool_table_omitted_uses_defaults.rs` test lives in c18
  alongside sink-default inference — its scoped assertion
  mentions `sinks_inferred: true` per Si1.) Negatives:
  `tests/manifest_missing_openrpc_sibling.rs`,
  `tests/manifest_missing_openrpc_provider.rs`,
  `tests/manifest_entry_traversal.rs`,
  `tests/manifest_entry_not_found.rs`,
  `tests/manifest_entry_escape_via_symlink.rs`,
  `tests/manifest_grant_match_missing.rs`,
  `tests/manifest_unknown_tool_table.rs` (orphan tables — V1
  surface; lives here because the test's worked-example
  fixture is the validate-with-package one),
  `tests/manifest_exec_path_inside_project.rs`. Fixture trees
  under `tests/fixtures/`.

---

## Group 2 — Lock schema + canonical id (L1–L9 minus L6)

### c12 — feat(rafaello-core): `CanonicalId` parser/formatter + path-traversal hardening

- **What.** New `rafaello_core::lock::CanonicalId` per §L8 with
  `parse(&str) -> Result<Self, LockError>` / `Display`. Source
  grammar `/`-separated `[a-z0-9._-]+` segments (no `..`, no
  `.`, no leading `/`, no trailing `/`, no double `/`, no empty
  segments — pi-2 commits-finding 7 caught the `.` segment
  omission). Name
  matches the topic-segment grammar; `version` parsed via
  `semver::Version`. Round-trip stable.
- **Why.** scope §L8 + pi-3 finding 1.
- **Depends on.** c02.
- **Acceptance.** Positive:
  `tests/lock_canonical_id_round_trip.rs`. Negatives:
  `tests/lock_canonical_id_invalid.rs`,
  `tests/lock_canonical_id_path_traversal.rs`.

### c13 — feat(rafaello-core): lock schema types + serde round-trip (data only — V3 lands later)

- **What.** New `rafaello_core::lock::Lock` carrying
  `plugins: BTreeMap<CanonicalId, PluginEntry>` and
  `session: SessionTable`. `PluginEntry` exposes `entry:
  SafePath` (parsed via SafePath per pi-3 finding 2 — lock
  loader applies the same rule), `digest`, `manifest_digest`,
  `granted_at: chrono::DateTime<Utc>`, `grant.bundles:
  BTreeMap<BundleKey, GrantBundle>` (reuses `BundleKey` from
  c07 manifest layer; pi-1 commits-finding 3) plus
  `.grant.subscribes` / `.grant.publishes` cross-bundle
  fields, `bindings` (`provider`, `provider_id`, `tools`,
  `renderer_kinds`, `tool_meta` with `sinks_inferred`,
  `load: LoadPolicy` per pi-6 finding 2), `flags`.
  `SessionTable` has `provider_active`, `tool_owner`. Lock
  Lock capability path fields decode as raw strings at parse
  time (pi-2 commits-finding 6: scope's
  `lock_capability_path_relative.rs` asserts
  `ValidationError::LockCapabilityPathRelative` from V3, which
  is a runtime-authority phase, not parse). The V3 mirror in
  c25 reparses each lock capability path through
  `CapabilityPathTemplate` and surfaces the typed
  `ValidationError`. `Lock::to_toml` / `from_toml` deterministic.
- **Why.** scope §L1–L5, §L7, §L9.
- **Depends on.** c07, c08, c09 (capability/load/renderer
  vocabularies), c12.
- **Acceptance.** Positives:
  `tests/lock_parse_round_trip.rs`,
  `tests/lock_load_policy_round_trip.rs`,
  `tests/lock_load_policy_eager_string.rs`,
  `tests/sinks_inferred_flag_round_trips.rs`. Negatives:
  `tests/lock_unknown_field.rs`,
  `tests/lock_helper_field_rejected.rs`,
  `tests/lock_missing_entry.rs`,
  `tests/lock_entry_traversal.rs`. (`lock_capability_path_relative.rs`
  lives in c25's V3 lock-side mirror per pi-2 commits-finding 6.)

### c14 — feat(rafaello-core): tool_meta always_confirm round-trip via programmatic lock fixtures

- **What.** Pure-fixtures test commit. Construct a programmatic
  `Lock` (per scope §"Out of scope" — m1 fixtures construct
  locks programmatically; pi-1 commits-finding 1 caught the
  earlier wording's claim that this commit covered a
  "manifest → lock projection" — that projection API is m2's
  install flow's job, not m1's). Asserts `always_confirm`
  round-trips through TOML serialise/parse byte-equal. The
  `CompiledPlugin` half lands as a second-stage extension in
  c34 (m0 two-stage pattern §4.3).
- **Why.** scope §L4 + pi-4 finding 6 (load-bearing for m5;
  pi-1 commits-finding 1 reframed scope).
- **Depends on.** c13.
- **Acceptance.** `tests/tool_meta_always_confirm_round_trip.rs`
  exercises the lock-side round-trip; extended in c34 with
  the `CompiledPlugin.tool_meta` half.

---

## Group 3 — Topic-id derivation (T1–T3)

### c15 — feat(rafaello-core): topic_id::derive + collisions_with_prefixes (public)

- **What.** New `rafaello_core::topic_id` module. `derive(canonical:
  &str) -> String` returns
  `id_<base32-no-pad-lower(sha256(canonical))[0..16]>`.
  `collisions_with_prefixes(pairs: &[(CanonicalId, String)]) ->
  Result<(), CollisionError>` is the **public** stable API
  (pi-2 finding 8 + pi-3 finding 6 — no `pub(crate)` /
  `feature = "test-seam"` boundary). `collisions(plugins:
  &[CanonicalId])` computes prefixes via `derive` then
  delegates.
- **Why.** scope §T1, §T2, §T3.
- **Depends on.** c12.
- **Acceptance.** Positive: `tests/topic_id_derivation.rs`.
  Negative: `tests/topic_id_collision_detection.rs` (forces a
  collision via the public `collisions_with_prefixes`).

---

## Group 4 — Single-plugin canonical-id-bound validation (V2)

### c16 — feat(rafaello-core): validate::manifest_with_id (V2) — canonical-id-bound publish ACL

- **What.** New `validate::manifest_with_id(manifest, canonical)`
  per §V2: rejects `plugin.<topic-id>.*` publishes whose
  `<topic-id>` doesn't match `topic_id::derive(canonical)`;
  rejects `provider.<id>.*` publishes whose `<id>` doesn't
  match `provides.provider`.
- **Why.** scope §V2.
- **Depends on.** c10, c15.
- **Acceptance.** Negatives:
  `tests/manifest_publishes_other_plugin_namespace.rs`,
  `tests/manifest_provider_namespace_mismatch.rs`.

---

## Group 5 — Digest module (D1, D2)

### c17 — feat(rafaello-core): digest::manifest_digest + content_digest (deterministic, files-only, recursion-stack cycle detection)

- **What.** New `rafaello_core::digest` module:
  `manifest_digest(canonical_bytes) -> String` and
  `content_digest(package_dir) -> Result<String, DigestError>`
  per §D1, §D2. Walk strategy: files only, sorted relative
  paths, length-prefixed path-then-content sha256 fold;
  symlinks followed inside the package, refused outside;
  directory symlinks followed with **recursion-stack cycle
  detection** (pi-6 finding 6 — distinct logical paths sharing
  a canonical target contribute under both relative paths).
  `RecomputedDigests` helper struct per §D3.
- **Why.** scope §D1, §D2.
- **Depends on.** c01.
- **Acceptance.** Positives:
  `tests/digest_content_deterministic.rs`,
  `tests/digest_distinct_paths_same_target.rs`. Negatives:
  `tests/digest_symlink_escape.rs`,
  `tests/digest_symlink_cycle.rs`.

---

## Group 6 — Sink default inference (Si1)

### c18 — feat(rafaello-core): sinks::infer_defaults over effective per-tool grant

- **What.** New `rafaello_core::sinks` module.
  `infer_defaults(effective: &GrantBundle, declared:
  &Option<Vec<String>>) -> Vec<String>` per §Si1. The
  `effective` parameter is the per-tool flatten (`default ∪
  <tool-name>`) per pi-3 finding 3 + decision row 17. `None`
  declared → infer; `Some` declared → preserve verbatim. The
  scope's `tool_table_omitted_uses_defaults.rs` lands here as
  a pure-fixtures test (build a programmatic lock with
  `tool_meta` carrying `sinks_inferred: true` + the inferred
  list; compare against `infer_defaults` over the same
  effective bundle — round-trip rather than projection per
  pi-1 commits-finding 1).
- **Why.** scope §Si1.
- **Depends on.** c13.
- **Acceptance.** Positives:
  `tests/sinks_infer_defaults.rs`,
  `tests/sinks_infer_from_named_bundle.rs`,
  `tests/tool_table_omitted_uses_defaults.rs`.

---

## **m1a / m1b checkpoint after c18.** Driver re-evaluates and either continues or opens a split request. Boundary matches scope §"Internal split" (data layer vs policy/emission layer).

---

## Group 7 — Trifecta refusal (Tr1–Tr5)

### c19 — feat(rafaello-core): trifecta::evaluate (one-hop, private-state structurally excluded)

- **What.** New `rafaello_core::trifecta` module.
  `evaluate(lock: &Lock, canonical: &CanonicalId, ctx:
  &PathContext) -> TrifectaState` per §Tr1–Tr5. Booleans
  computed across the full bundle union (per row 17 / pi-4
  finding 1 — no per-call switch). `has_workspace_write`
  excludes the per-plugin private-state subtree structurally
  (the lock has no `write_dirs` entry for it; C5 will inject
  it later). One-hop direct check across other plugins'
  subscribe-pattern-matches-publish-topic graph. `refuse =
  ... && !flags.i_know_what_im_doing`.
- **Why.** scope §Tr1–Tr5.
- **Depends on.** c03 (PathContext), c13.
- **Acceptance.** Positives:
  `tests/trifecta_two_plugins_one_hop.rs`,
  `tests/trifecta_iknowwhatimdoing_bypass.rs`. The
  private-state-exclusion *integration* test lands as
  `tests/compile_private_state_excluded_from_workspace_write.rs`
  in c31 once the compiler injects the C5 dir (m0 two-stage
  pattern); a unit-style assertion using a pre-built
  programmatic lock (no compiler injection required) lands
  here as a basic Tr4 sanity check.

---

## Group 8 — Carve-out decomposition (K1–K4)

### c20 — feat(rafaello-core): carveout::compile_against — project-class decompose / credential-class refuse / write refuse / explicit override

- **What.** New `rafaello_core::carveout` module. `CARVE_OUTS`
  constant per §K1 (two classes: project, credential).
  `compile_against(grant: &GrantBundle, canonical:
  &CanonicalId, ctx: &PathContext, allow_credential_paths:
  bool) -> Result<DecomposedGrant, CompileError>` per §K2:
  - **Project-class read** decomposes (256 cap).
  - **Credential-class read** refuses unless
    `allow_credential_paths`.
  - **Writes** (either class) refuse unless
    `allow_credential_paths`.
  - **Explicit leaf hits** on either class refuse with
    `CarveOutRefused` (no `dropped_carveouts` diagnostic per
    pi-2 finding 7).
  - With `allow_credential_paths = true`: broad grants emitted
    verbatim (no decomposition, no refusal); the override flag
    is recorded in `DecomposedGrant.flags` so m2's `rfl status`
    can render the loud override.
  Hidden-directory rule (§K3) — default workspace grant
  decomposes into immediate non-hidden children. Decomposition
  snapshotted into the output, not a live filter (§K4).
- **Why.** scope §K1, §K2, §K3, §K4.
- **Depends on.** c03 (PathContext + CapabilityPathTemplate),
  c13.
- **Acceptance.** Positives:
  `tests/carveout_default_workspace_decomposition.rs`,
  `tests/carveout_workspace_excludes_rafaello_dot_dirs.rs`.
  Negatives:
  `tests/carveout_credential_path_refused_read.rs`,
  `tests/carveout_credential_path_refused_write.rs`,
  `tests/carveout_project_write_refused.rs`,
  `tests/carveout_credential_path_override.rs`,
  `tests/carveout_decomposition_blowup.rs`,
  `tests/carveout_lockfile_path_explicit.rs`.

---

## Group 9 — Env scrubber + reserved-env C7.1 helpers (Sc1–Sc3)

### c21 — feat(rafaello-core): scrubber::strip + reserved-env C7.1 rejection helper

- **What.** New `rafaello_core::scrubber` module.
  `SECRET_PATTERNS` constant per §Sc1.
  `strip(env_pass: &[String], i_know_what_im_doing: bool) ->
  Vec<String>` per §Sc2 (override returns input verbatim).
  `scrubber::reject_reserved(env_pass, env_set) ->
  Result<(), CompileError>` per §C7.1 — rejects `RFL_BUS_FD` /
  `RFL_PLUGIN` in either collection. Compiler calls into both
  in c33.
- **Why.** scope §Sc1–Sc3, §C7.1.
- **Depends on.** c01.
- **Acceptance.** Positives:
  `tests/env_scrubber_strips_known_secrets.rs`,
  `tests/env_scrubber_override.rs` (override-preserves-input —
  positive per pi-1 commits-minor; round-1 misclassified as
  negative).
  Negatives: `tests/env_scrubber_strips_secret_globs.rs`.
  C7.1's `compile_reserved_env_in_pass.rs` /
  `compile_reserved_env_in_set.rs` land in c33 once the
  compiler invokes them (m0 two-stage pattern).

---

## Group 10 — Cross-plugin lock validation (V3 — wires Tr + carveout + sink-drift + topic-id collision + tool-owner + lock-side mirrors)

### c22 — feat(rafaello-core): validate::lock multi-plugin context + topic-id collision + provider/tool_owner integrity

- **What.** New `validate::lock(lock: &Lock, ctx:
  &LockValidationContext) -> Result<()>` per §V3 with the
  multi-plugin context per pi-6 finding 1. This commit lands
  the orchestration shell + the rules that don't require
  trifecta/carveout/sinks (which wire in c23):
  - topic-id collision (delegates to `topic_id::collisions`),
  - conflicting tool name + `[session].tool_owner` resolution
    + target integrity (pi-5 finding 2 — installed, declares
    tool, no redundant entries),
  - provider activeness consistency
    (`[session].provider_active`),
  - per-plugin `PathContext` derivation from
    `LockValidationContext.plugin_dirs`,
  - `MissingPluginDir` failure for canonicals without an entry.
- **Why.** scope §V3 (orchestration + non-Tr/K bullets).
- **Depends on.** c13, c15.
- **Acceptance.** Positive:
  `tests/validate_lock_multiplugin_context.rs`. Negatives:
  `tests/lock_provider_active_unknown.rs`,
  `tests/lock_provider_active_not_provider.rs`,
  `tests/lock_conflicting_tool_names.rs`,
  `tests/lock_tool_owner_unknown_plugin.rs`,
  `tests/lock_tool_owner_plugin_does_not_declare_tool.rs`,
  `tests/lock_tool_owner_redundant.rs`,
  `tests/topic_id_collision_at_lock.rs`.

### c23 — feat(rafaello-core): V3 wires trifecta + carveout + sink-drift

- **What.** Extend V3 to delegate per-plugin trifecta
  evaluation (c19), carve-out enforcement (c20), and
  sink-default drift detection (Si2). Failures surface as
  `ValidationError::TrifectaRefused`, `CarveOutRefused`,
  `CarveOutTooLarge`, `SinkInferenceDrift`.
- **Why.** scope §V3 (trifecta + carve-out + Si2 bullets).
- **Depends on.** c18, c19, c20, c22.
- **Acceptance.** Negative: `tests/sinks_inference_drift.rs`.
  Trifecta and carve-out negative tests landed in c19/c20;
  this commit's acceptance is `tests/validate_lock_full_pass.rs`
  building a multi-plugin fixture and asserting both pass and
  refusal cases via the public V3 entry point.

### c24 — feat(rafaello-core): V3 lock-side publish ACL mirror

- **What.** Extend V3 with the lock-side namespace ACL on
  `.grant.publishes` per pi-5 finding 1: rejects `core.*` /
  `frontend.*`; `plugin.<id>.*` must match
  `topic_id::derive(canonical)` for the lock entry;
  `provider.<id>.*` requires `bindings.provider == true` and
  matching `bindings.provider_id`.
- **Why.** scope §V3 lock-side publish authority bullet.
- **Depends on.** c22.
- **Acceptance.** Negatives:
  `tests/lock_publishes_core_topic.rs`,
  `tests/lock_publishes_frontend_topic.rs`,
  `tests/lock_publishes_other_plugin_namespace.rs`,
  `tests/lock_provider_namespace_mismatch.rs`.

### c25 — feat(rafaello-core): V3 lock-side allow_hosts mode + bundle-key + capability-path mirrors

- **What.** Extend V3 with three lock-side mirrors:
  - `allow_hosts` requires proxy mode (pi-5 finding 3) on
    every grant bundle's `network`;
  - unknown grant bundle key rejection (pi-6 finding 3) —
    every key in `.grant.bundles` must be `Default` or a
    tool name from `bindings.tools`;
  - **capability-path template re-validation** (pi-2
    commits-finding 6): each `read_paths` / `read_dirs` /
    `write_paths` / `write_dirs` / `exec_paths` / `exec_dirs`
    string in every grant bundle is re-parsed through
    `CapabilityPathTemplate`. Bare relative paths (no
    placeholder, not absolute) →
    `ValidationError::LockCapabilityPathRelative`.
- **Why.** scope §V3 lock-side `allow_hosts` / bundle key /
  capability-path bullets.
- **Depends on.** c22.
- **Acceptance.** Negatives:
  `tests/lock_allow_hosts_outside_proxy.rs`,
  `tests/lock_unknown_bundle_key.rs`,
  `tests/lock_capability_path_relative.rs`.

### c26 — feat(rafaello-core): V3 lock-side bindings grammar + tool_meta consistency mirrors

- **What.** Extend V3 with the bindings-snapshot validations
  per pi-5 finding 6 + pi-6 finding 3:
  - `bindings.tool_meta.<n>.grant_match` re-parsed via
    `SafePath`,
  - `tool_meta` keys must appear in `bindings.tools`,
  - `bindings.provider_id` grammar + `iff provider == true`,
  - `bindings.renderer_kinds` re-validated against M7 rules
    (built-ins rejected; plugin kinds prefixed),
  - `bindings.tools` values + `[session].tool_owner` keys
    re-validated against the tool-name grammar,
  - `bindings.tool_meta.<n>.sinks` values re-validated against
    the sink-class grammar.
- **Why.** scope §V3 binding-snapshot validation bullets.
- **Depends on.** c22.
- **Acceptance.** Negatives:
  `tests/lock_tool_meta_grant_match_traversal.rs`,
  `tests/lock_tool_meta_orphan.rs`,
  `tests/lock_provider_id_inconsistent.rs`,
  `tests/lock_renderer_kind_unprefixed.rs`,
  `tests/lock_renderer_kind_builtin.rs`,
  `tests/lock_bindings_tools_invalid_grammar.rs`,
  `tests/lock_tool_meta_invalid_sink.rs`.

### c27 — feat(rafaello-core): V3 lock-side exec_paths under ${project} refusal

- **What.** Extend V3 with the §6.9 exec-under-project refusal
  on the lock side (pi-6 finding 4). Uses the c31 path
  resolver; this commit lands the V3 hook + the test.
- **Why.** scope §V3 exec_paths under project bullet.
- **Depends on.** c22, c31 (path resolver). **Note ordering:**
  this commit lands AFTER c31 since it consumes the resolver;
  numbering keeps grouping coherence — see "Out-of-order land"
  note below.
- **Acceptance.** Negative:
  `tests/lock_exec_path_inside_project.rs`.

> **Out-of-order land:** c27 follows c31 in commit order
> despite its low number — the agent driver lands c28 → c31 →
> c27 → c32 → ... Renumbering would shuffle the rest of the
> doc; the depends-on chain captures the real order. (This is
> the same pattern m0 used when c21 pre-empted c22; pi-1 round
> 1 acceptable per scope's per-commit greenness rule.)

---

## Group 11 — Compiler core (C1–C7) + plan emission

### c28 — feat(rafaello-core): compile module skeleton + CompiledPlugin / FilesystemPlan / NetworkPlan / EnvPlan / LimitsPlan / LoadPolicy public types

- **What.** New `rafaello_core::compile` module with the
  public `CompiledPlugin` plan struct per §C1 + sub-types
  `FilesystemPlan`, `NetworkPlan { Deny | AllowAll | Proxy {
  allow_hosts } }`, `EnvPlan`, `LimitsPlan`, `CompiledFlags`,
  `ToolMeta`, `LoadPolicy` (reused from `lock` per c13). No
  `compile_plugin` body yet.
- **Why.** scope §C1 (data types only).
- **Depends on.** c13.
- **Acceptance.** `tests/compile_types_compile.rs` is a
  build-only assertion.

### c29 — feat(rafaello-core): compile_plugin entry point + V3-must-run-first guard

- **What.** Implement `compile_plugin(lock, canonical, ctx,
  recomputed_digests) -> Result<CompiledPlugin, CompileError>`
  per §C2 with the precondition contract per §C1.1: when the
  function detects a state V3 should have rejected (e.g.
  duplicate topic-id, conflicting tool name without
  resolution, foreign-namespace publish), it returns
  `CompileError::ValidationNotRun` (pi-1 commits-finding 9 —
  the rephrase is "a lock violating a V3 invariant returns
  `ValidationNotRun`"; the function does not own a
  validation-token mechanism). Body is a scaffold; per-section
  emitters land in c30–c34.
- **Why.** scope §C2, §C1.1.
- **Depends on.** c22, c24, c28 (pi-2 commits-finding 8 —
  the V3 invariants `compile_plugin` claims to detect include
  publish ACL violations from c24's lock-side mirror; depend
  on c24 explicitly so the contract is honest).
- **Acceptance.** Negative:
  `tests/compile_without_validate_lock_errors.rs` — a lock
  with two installed plugins resolving to the same topic-id,
  fed straight to `compile_plugin` without V3, returns
  `CompileError::ValidationNotRun`.

### c30 — feat(rafaello-core): compile bundle flatten (full union) + dedup + ordering (C4)

- **What.** Per `decisions.md` row 17 + pi-4 finding 1: union
  `default` ∪ every named bundle in `grant.bundles` into one
  spawn-time policy. Apply the C4 post-flatten deterministic
  ordering: sort scalar arrays by string value, dedup. No
  `active_bundles` selection knob.
- **Why.** scope §C2 (union flatten), §C4 (ordering).
- **Depends on.** c29.
- **Acceptance.** Positives:
  `tests/compile_default_bundle.rs`,
  `tests/compile_scoped_bundle_union.rs`.

### c31 — feat(rafaello-core): compile path resolver (existing-ancestor canonical + lexical suffix + containment) + placeholder application

- **What.** Implement C3's placeholder application + the
  containment resolver per pi-5 finding 7: walk the
  post-expansion path component-by-component; canonicalise
  the longest existing ancestor (with symlink + escape
  checks); lexically join the non-existent suffix; final
  containment check on the absolute path against
  `project_root` / `plugin_dir` for `${project}` /
  `${plugin}` placeholders. Failures:
  `CompileError::UnknownPlaceholder`,
  `CompileError::PathEscape`, `CompileError::SymlinkEscape`.
  c27 (V3 lock-side exec refusal) calls into this resolver.
- **Why.** scope §C3.
- **Depends on.** c03, c29.
- **Acceptance.** Positives:
  `tests/compile_placeholder_resolves_to_absolute.rs`,
  `tests/compile_capability_path_nonexistent_write_leaf.rs`.
  Negatives: `tests/compile_unknown_placeholder.rs`,
  `tests/compile_path_escape_after_expansion.rs`,
  `tests/compile_capability_path_symlink_ancestor_escape.rs`.

### c32 — feat(rafaello-core): compile filesystem plan via carve-out + private-state grant (C5)

- **What.** Wire `carveout::compile_against` (c20) into the
  compiler so post-flatten reads/writes pass through
  decomposition. Inject the per-plugin private-state grant
  per §C5 using the **topic-id form**
  (`${project}/.rafaello-plugin-data/<topic-id>/`) — pi-3
  finding 5 + pi-4 finding 4. Private-state dir is added
  after trifecta evaluation (Tr4's structural exclusion
  remains intact).
- **Why.** scope §C5 + §K integration.
- **Depends on.** c20, c30, c31.
- **Acceptance.** Positives:
  `tests/compile_private_state_grant.rs`,
  `tests/compile_private_state_excluded_from_workspace_write.rs`
  (second-stage of c19's trifecta unit assertion — m0
  two-stage pattern).

### c33 — feat(rafaello-core): compile network plan + outpost dry-run + env plan + reserved-env C7.1 wiring + scrubber call

- **What.** Build `NetworkPlan` per §C1: `Deny | AllowAll |
  Proxy { allow_hosts }`. For proxy mode, run
  `outpost::NetworkPolicy::from_allowed_hosts(...)` as a
  parse-time dry-run; on failure return
  `CompileError::InvalidAllowHosts` (parsed value discarded;
  m2 reconstructs at spawn). Build `EnvPlan` per §C1: call
  `scrubber::reject_reserved(env_pass, env_set)` first per
  §C7.1; then `scrubber::strip(env_pass,
  flags.i_know_what_im_doing)`. Network and env emission
  consume the post-flatten effective grant from c30.
- **Why.** scope §C1 NetworkPlan + EnvPlan + §C7 + Risks §2.
- **Depends on.** c21, c30, c32.
- **Acceptance.** Positives:
  `tests/compile_network_proxy_plan.rs` (records
  `allow_hosts` verbatim through the plan — scope's name;
  pi-1 commits-finding 6 — single canonical name),
  `tests/compile_network_proxy_allow_hosts_validates.rs`
  (the dry-run accepts a worked-example list — already in
  scope's matrix; both names refer to distinct assertions
  on the same NetworkPlan code path),
  `tests/compile_env_set_passes_through.rs`.
  Negatives:
  `tests/compile_invalid_allow_hosts.rs`,
  `tests/compile_reserved_env_in_pass.rs`,
  `tests/compile_reserved_env_in_set.rs`.

### c34 — feat(rafaello-core): compile entry resolution + limits defaults + digest gating + tool_meta projection (closes C1)

- **What.** Final compile pieces:
  - **Entry resolution** (per §L2 + pi-3 finding 2): take
    `lock.entry: SafePath`; canonicalise against
    `plugin_dir`; require existing regular file inside.
    Failures: `CompileError::EntryEscape`,
    `CompileError::EntryNotFound`, `CompileError::EntryNotFile`.
  - **Resource-limit defaults** per §C6: 300s cpu, 1024 fds
    when omitted; explicit `0` preserved.
  - **Digest gating** per §D3: take `RecomputedDigests`,
    compare against lock fields, fail with
    `CompileError::ContentDigestMismatch` /
    `CompileError::ManifestDigestMismatch` on mismatch.
  - **`tool_meta` projection**: include only entries whose
    `<name>` is owned by this plugin per
    `[session].tool_owner` resolution (pi-2 finding 4).
    Carry `always_confirm` through.
- **Why.** scope §C1 (entry_absolute, tool_meta filter), §C6,
  §D3, §L2 compile-time check.
- **Depends on.** c14, c17, c31, c32, c33.
- **Acceptance.** Positives:
  `tests/compile_resource_limit_defaults.rs`,
  `tests/compile_digest_match.rs` (the canonical scope name —
  pi-1 commits-finding 6: replaces the round-1 duplicated
  `digest_match_compiles.rs` + `compile_digest_match.rs`
  pair; this commit lands the single canonical file
  exercising both digests through `compile_plugin`).
  Extension of `tests/tool_meta_always_confirm_round_trip.rs`
  with the `CompiledPlugin.tool_meta` half (closing the c14
  two-stage test).
  Negatives:
  `tests/lock_entry_not_found.rs`,
  `tests/lock_entry_escape_via_symlink.rs`,
  `tests/lock_entry_is_directory.rs`,
  `tests/digest_content_mismatch.rs`,
  `tests/digest_manifest_mismatch.rs`.

---

## Group 12 — Broker ACL extraction (G1–G3)

### c35 — feat(rafaello-core): broker_acl::compile with PluginAcl + auto-subscribes + tool_routes + grammar revalidation

- **What.** New `rafaello_core::broker_acl` module.
  `compile(lock: &Lock) -> Result<BrokerAcl, CompileError>`
  per §G1: per-plugin `PluginAcl { topic_id, publish_topics,
  subscribe_patterns, auto_subscribes (the
  `plugin.<topic-id>.tool_request` self-subscribe),
  provider_id }`, plus the **resolved** `tool_routes:
  BTreeMap<String, CanonicalId>` (pi-2 finding 4). G2 grammar
  revalidation runs before emit. Same V3-must-run-first
  contract as `compile_plugin` (returns `ValidationNotRun`
  if invariants not enforced).
- **Why.** scope §G1, §G2, §G3.
- **Depends on.** c15, c22, c24, c26 (pi-2 commits-finding 8 —
  G2 grammar revalidation depends on tool-name +
  renderer-kind grammar checks introduced through c26).
- **Acceptance.** Positives:
  `tests/broker_acl_extraction.rs`,
  `tests/broker_acl_tool_owner_resolves_routing.rs`.

---

## Group 13 — Fittings `MethodNotFound` typed-method cutover (W)

### c36 — feat(fittings): MethodNotFound typed `method` field cutover (W1–W5)

- **What.** Single workspace-wide cutover commit on the
  `fittings` workspace, mirroring m0's c08 pattern for
  source-breaking enum changes:
  - `fittings_core::error::FittingsError::MethodNotFound`
    gains `method: Option<String>` (W1). Existing one-arg
    constructor `method_not_found(message)` keeps working
    with `method: None` / `data: None` (W3).
  - `method_not_found_with_method(method, message)`
    constructor added (W4).
  - `fittings_wire::error_map` extracts/synthesises
    `data.method` per W2's encode rules
    (typed-field-precedence; existing `data` keys preserved
    except a conflicting `method` key which is overwritten;
    when `method = None`, opaque `data.method` is preserved).
  - All in-tree consumers updated (`fittings/examples/*`,
    `mcp-server`, in-crate tests) to compile against the new
    shape.
- **Why.** scope §W1–W5; `decisions.md` row 36; m0
  retrospective §2.4.
- **Depends on.** baseline (independent of `rafaello-core`;
  may land at any point in the m1 sequence — driver may
  schedule earlier or later).
- **Acceptance.**
  `fittings/tests/method_not_found_typed_method_round_trip.rs`
  per W5 (table-driven). `cargo test --manifest-path
  fittings/Cargo.toml --workspace` green per pi-5 finding 5.

---

## Group 14 — Manual validation

### c37 — docs(rafaello-m1): write manual-validation.md

- **What.** Write
  `rafaello/plans/milestones/m1-manifest/manual-validation.md`
  capturing each item from scope §"Manual validation":
  `cargo test --manifest-path rafaello/Cargo.toml -p
  rafaello-core` green; full fittings workspace green;
  `cargo doc --manifest-path rafaello/Cargo.toml -p
  rafaello-core --no-deps` clean; `--release` green; `nix
  develop --impure -L --command ...` green; `tree
  rafaello/crates/rafaello-core/tests/fixtures` dump.
- **Why.** scope §"Manual validation"; m0 patterns §4.5/§4.6.
- **Depends on.** c35, c36.
- **Acceptance.** `manual-validation.md` exists and captures
  the required evidence; any tooling/CI/Nix follow-ups
  discovered while exercising it land alongside.

---

## Acceptance for the milestone as a whole

Beyond per-commit acceptance, m1 lands when:

- Every named test in `scope.md`'s positive + negative test
  matrices is implemented and passes.
- `cargo test --manifest-path rafaello/Cargo.toml -p
  rafaello-core` is green on Linux.
- `cargo test --manifest-path fittings/Cargo.toml --workspace`
  is green (per scope §"Acceptance summary" + pi-5 finding 5).
- `cargo doc --manifest-path rafaello/Cargo.toml -p
  rafaello-core --no-deps` is warning-free.
- `manual-validation.md` records the items in scope §"Manual
  validation".
- `retrospective.md` is written after the last commit; any
  drift surfaced during implementation lands in `overview.md`
  / `decisions.md` / stream RFCs as deltas. m1 retrospective
  specifically owns:
  - the §15.1 normative-delta items 1–4 patches into the
    Stream F RFC body;
  - the security RFC `requires_confirmation` →
    `always_confirm` rename + helper / external-attach drift;
  - the **private-state path-key clarification** (`<plugin-id>`
    → topic-id) in `overview.md` §5.5 / `decisions.md` row 16
    / `glossary.md`.

## Notes on commit sizing + per-commit greenness

- **No workspace-wide cutover required for `rafaello-core`** —
  it's a brand-new crate with no existing consumers; every
  commit can incrementally add modules.
- **§W (c36) IS a workspace-wide cutover** for fittings — the
  `MethodNotFound` enum gains a struct field, source-breaking
  for direct struct literals + named-field pattern matches.
  This single commit consolidates the change and updates every
  in-tree consumer, mirroring m0 c08.
- **Two-stage tests** per m0 pattern §4.3:
  - `compile_private_state_excluded_from_workspace_write.rs`
    (c19 trifecta unit + c32 compiler injection).
  - `tool_meta_always_confirm_round_trip.rs` (c14 lock-side
    round-trip + c34 CompiledPlugin half).

## Scope test → commit traceability table

(pi-1 commits-finding 4 — minor cleanup: a compact trace table
to make drift checks mechanical in later review rounds.)

| scope test file | commit |
|----------------|--------|
| `manifest_parse_minimal.rs` | c11 (lifted from c04 per pi-2 commits-finding 2 — needs `canonical_bytes`) |
| `manifest_parse_tool_example.rs` | c11 |
| `manifest_parse_provider_example.rs` | c11 |
| `manifest_parse_renderer_example.rs` | c11 |
| `manifest_canonical_bytes_stable.rs` | c11 |
| `manifest_placeholder_expansion.rs` | c03 |
| `manifest_validate_load_trigger_cross_refs.rs` | c10 (V1) |
| `manifest_load_event_pattern_match.rs` | c10 (V1) |
| `manifest_openrpc_sibling_present.rs` | c11 |
| `manifest_grant_match_present.rs` | c11 |
| `lock_parse_round_trip.rs` | c13 |
| `lock_load_policy_round_trip.rs` | c13 |
| `lock_load_policy_eager_string.rs` | c13 |
| `sinks_inferred_flag_round_trips.rs` | c13 |
| `lock_canonical_id_round_trip.rs` | c12 |
| `topic_id_derivation.rs` | c15 |
| `compile_default_bundle.rs` | c30 |
| `compile_scoped_bundle_union.rs` | c30 |
| `compile_placeholder_resolves_to_absolute.rs` | c31 |
| `compile_private_state_grant.rs` | c32 |
| `compile_private_state_excluded_from_workspace_write.rs` | c32 (extends c19 unit) |
| `compile_resource_limit_defaults.rs` | c34 |
| `compile_network_proxy_plan.rs` | c33 |
| `compile_env_set_passes_through.rs` | c33 |
| `compile_digest_match.rs` | c34 |
| `broker_acl_extraction.rs` | c35 |
| `carveout_default_workspace_decomposition.rs` | c20 |
| `carveout_workspace_excludes_rafaello_dot_dirs.rs` | c20 |
| `digest_match_compiles.rs` | c34 (alias for `compile_digest_match.rs` per scope round-3 wording — single file under canonical scope name) |
| `digest_content_deterministic.rs` | c17 |
| `digest_distinct_paths_same_target.rs` | c17 |
| `trifecta_two_plugins_one_hop.rs` | c19 |
| `trifecta_iknowwhatimdoing_bypass.rs` | c19 |
| `sinks_infer_defaults.rs` | c18 |
| `sinks_infer_from_named_bundle.rs` | c18 |
| `tool_table_omitted_uses_defaults.rs` | c18 |
| `tool_meta_always_confirm_round_trip.rs` | c14 base; c34 extension |
| `manifest_grant_match_present.rs` | c10 |
| `compile_network_proxy_allow_hosts_validates.rs` | c33 |
| `validate_lock_multiplugin_context.rs` | c22 |
| `env_scrubber_strips_known_secrets.rs` | c21 |

### Negative scope tests → commit (pi-2 commits-finding 9)

| scope negative test | commit |
|---------------------|--------|
| `manifest_unknown_field.rs` | c04 |
| `manifest_legacy_runtime_field.rs` | c04 |
| `manifest_legacy_rpc_block.rs` | c04 |
| `manifest_helper_for_field.rs` | c04 |
| `manifest_invalid_name.rs` | c10 (V1 grammar) |
| `manifest_publishes_core_topic.rs` | c10 (V1) |
| `manifest_publishes_other_plugin_namespace.rs` | c16 (V2) |
| `manifest_publishes_frontend_topic.rs` | c10 (V1) |
| `manifest_provider_namespace_mismatch.rs` | c16 (V2) |
| `manifest_publish_with_wildcard.rs` | c10 (V1) |
| `manifest_subscribe_invalid_pattern.rs` | c10 (V1) |
| `manifest_topic_segment_grammar.rs` | c10 (V1) |
| `manifest_dotted_tool_name.rs` | c10 (V1) |
| `manifest_unknown_tool_table.rs` | c11 (validate-with-package) |
| `manifest_unknown_bundle_key.rs` | c10 (V1) |
| `manifest_malformed_sinks.rs` | c10 (V1) |
| `manifest_reserved_renderer_kind.rs` | c10 (V1) |
| `manifest_load_trigger_unknown_command.rs` | c10 (V1) |
| `manifest_missing_openrpc_sibling.rs` | c11 |
| `lock_unknown_field.rs` | c13 |
| `lock_canonical_id_invalid.rs` | c12 |
| `lock_helper_field_rejected.rs` | c13 |
| `lock_provider_active_unknown.rs` | c22 |
| `lock_provider_active_not_provider.rs` | c22 |
| `lock_conflicting_tool_names.rs` | c22 |
| `lock_missing_entry.rs` | c13 |
| `topic_id_collision_at_lock.rs` | c22 |
| `digest_content_mismatch.rs` | c34 |
| `digest_manifest_mismatch.rs` | c34 |
| `digest_symlink_escape.rs` | c17 |
| `carveout_credential_path_refused_read.rs` | c20 |
| `carveout_credential_path_refused_write.rs` | c20 |
| `carveout_project_write_refused.rs` | c20 |
| `carveout_credential_path_override.rs` | c20 |
| `carveout_decomposition_blowup.rs` | c20 |
| `carveout_lockfile_path_explicit.rs` | c20 |
| `compile_unknown_placeholder.rs` | c31 |
| `compile_reserved_env_in_pass.rs` | c33 |
| `compile_reserved_env_in_set.rs` | c33 |
| `env_scrubber_strips_secret_globs.rs` | c21 |
| `sinks_inference_drift.rs` | c23 |
| `manifest_topic_too_few_segments.rs` | c10 (V1) |
| `lock_publishes_core_topic.rs` | c24 |
| `lock_publishes_frontend_topic.rs` | c24 |
| `lock_publishes_other_plugin_namespace.rs` | c24 |
| `lock_provider_namespace_mismatch.rs` | c24 |
| `lock_allow_hosts_outside_proxy.rs` | c25 |
| `lock_tool_owner_unknown_plugin.rs` | c22 |
| `lock_tool_owner_plugin_does_not_declare_tool.rs` | c22 |
| `lock_tool_owner_redundant.rs` | c22 |
| `lock_tool_meta_grant_match_traversal.rs` | c26 |
| `lock_tool_meta_orphan.rs` | c26 |
| `lock_provider_id_inconsistent.rs` | c26 |
| `lock_renderer_kind_unprefixed.rs` | c26 |
| `lock_renderer_kind_builtin.rs` | c26 |
| `compile_capability_path_symlink_ancestor_escape.rs` | c31 |
| `compile_invalid_allow_hosts.rs` | c33 |
| `manifest_entry_traversal.rs` | c11 |
| `manifest_entry_not_found.rs` | c11 |
| `manifest_entry_escape_via_symlink.rs` | c11 |
| `manifest_grant_match_traversal.rs` | c05 (parse-time SafePath) |
| `manifest_grant_match_missing.rs` | c11 |
| `lock_canonical_id_path_traversal.rs` | c12 |
| `compile_path_escape_after_expansion.rs` | c31 |
| `lock_missing_entry.rs` | c13 |
| `lock_unknown_bundle_key.rs` | c25 |
| `lock_capability_path_relative.rs` | c25 (V3 mirror — pi-2 commits-finding 6) |
| `lock_bindings_tools_invalid_grammar.rs` | c26 |
| `lock_tool_meta_invalid_sink.rs` | c26 |
| `manifest_exec_path_inside_project.rs` | c11 |
| `lock_exec_path_inside_project.rs` | c27 |
| `compile_without_validate_lock_errors.rs` | c29 |
| `digest_symlink_cycle.rs` | c17 |
| `manifest_unprefixed_renderer_kind.rs` | c10 (V1) |
| `manifest_allow_hosts_outside_proxy.rs` | c10 (V1) |

Driver should reconcile this table against scope §"Demo bar"
before c01 lands and flag any unplaced test.

## What changed from prior drafts

Round-2 pi review (`commits-pi-review-2.md`) prompted these
revisions:

- **Path resolver moved to c03** (pi-2 finding 1 + 4). The
  `paths::resolve_under_root` helper lives alongside SafePath
  and CapabilityPathTemplate so c11 (validate-with-package)
  and c27 (V3 exec-path) can call it without back-edges. The
  round-2 c26-after-c30 numeric out-of-order note is gone.
- **Phase-boundary restructure** (pi-2 findings 5 + 6). Parse
  commits c05–c09 decode raw — string topics, string tool
  names, string sink classes, string renderer kinds. Grammar
  + cross-ref + namespace ACL checks land in **new commit
  c10** (`validate::manifest_standalone`, the V1 entry
  point). Negative ValidationError tests for grammar / ACL
  / cross-ref move to c10. ManifestError stays parse-time
  for TOML / `deny_unknown_fields` / `ReservedField` /
  `SafePath` / `CapabilityPathTemplate` shape errors. Lock-
  side capability path negative test stays at c25 (V3
  mirror) returning `ValidationError`. All subsequent
  commit numbers bumped +1 (old c10 → c11; old c11 → c12;
  ...; old c36 → c37).
- **`manifest_parse_minimal.rs` lifted to c11** (pi-2
  finding 2 — needs `canonical_bytes`).
- **`manifest_validate_load_trigger_cross_refs.rs` lives at
  c10** (pi-2 finding 3 — kind cross-ref needs renderer
  declarations parsed at c09; V1 in c10 owns the full
  cross-ref check).
- **`manifest_exec_path_inside_project.rs` lives at c11**
  (pi-2 finding 4 — uses c03's resolver via
  `validate_with_package`).
- **CanonicalId rejects `.` segment** (pi-2 finding 7). c12
  (was c11) enumerates the rejected forms explicitly.
- **c29 / c35 dependencies expanded** (pi-2 finding 8). c29
  depends on c24 (lock-side publish ACL); c35 depends on
  c24 + c26.
- **Negative-test trace table added** at the bottom (pi-2
  finding 9).
- **c01 tempfile note clarified** (pi-2 minor 3). The
  workspace declaration is just the version pin; the dev-dep
  goes into `rafaello-core`'s `[dev-dependencies]` block in
  c11 when fixtures first need it.
- **c17 (digest) and c21 (scrubber) deps reflect error
  surface** (pi-2 minor 1+2). Both depend on c02 (typed
  errors) explicitly.

Round-1 pi review (`commits-pi-review-1.md`) prompted these
revisions:

- **E1 typed-error surface lands explicitly in c02** (pi-1
  finding 7). New foundational commit between crate skeleton
  and manifest types.
- **`PathContext` / `SafePath` / `CapabilityPathTemplate` /
  placeholder expander land in c03** (pi-1 finding 4). Earlier
  commits depending on them now have honest `Depends on`
  bullets; c08/c10 no longer hide the dep.
- **`validate::manifest_standalone` API surface introduced at
  first use (c06)**, not consolidated late (pi-1 finding 2).
  Earlier rounds had a confusing "later c14 introduces it"
  bullet; c14 is removed entirely (its consolidation role is
  handled by tests being written against the public API as it
  grows commit-by-commit).
- **c10 / c13 dep on capability/bundle types** (pi-1 finding
  3). c13 (lock schema) explicitly depends on c07 (manifest
  capabilities/bundles).
- **c32/c33 depend on c30** (pi-1 finding 5 — bundle flatten
  must precede network/env emission).
- **Test name canonicalisation** (pi-1 finding 6). Single
  trace table at the bottom; both `compile_network_proxy_plan.rs`
  and `compile_network_proxy_allow_hosts_validates.rs` exist
  in scope and now both have an explicit home (c33);
  `compile_digest_match.rs` is the canonical scope name (c34
  replaces the round-1 duplicate `digest_match_compiles.rs` —
  the trace table aliases for clarity).
- **`tool_table_omitted_uses_defaults.rs` moves to c18** (pi-1
  finding 1). Its scope assertion is "lock has
  `sinks_inferred: true` + the inferred list", which needs
  sink inference (c18) and a programmatic lock fixture
  (constructible from c13). The earlier c08 placement
  required a manifest→lock projection API m1 doesn't
  define.
- **c12 (now c14) reframed** (pi-1 finding 1). Lock-side
  round-trip test using a programmatic fixture, not a claim
  to a "manifest → lock projection" API.
- **c06 explicitly covers the negative event-pattern case**
  (pi-1 finding 8). `manifest_load_event_pattern_match.rs`
  spelled out as table-driven with both positive and
  negative.
- **`compile_without_validate_lock_errors.rs` rephrased**
  (pi-1 finding 9). "A lock violating a V3 invariant returns
  `ValidationNotRun` from `compile_plugin`."
- **m1a/m1b checkpoint moved to after c18** (pi-1 finding
  10). Boundary now matches the scope's "data layer" vs
  "policy/emission layer" rationale; c19 onwards is trifecta
  + carve-out + scrubber + V3 + compiler + broker ACL.
- **Minor cleanups** (pi-1 minor):
  - `env_scrubber_override.rs` correctly classified positive
    in c21.
  - `c34` no longer repeats a digest-test name; uses scope's
    canonical `compile_digest_match.rs`.
  - c20 (carve-out) now spells out the `allow_credential_paths
    = true` override behaviour explicitly.
  - Trace table added at the bottom.
