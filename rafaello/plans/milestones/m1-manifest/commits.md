# m1-manifest — commits

> **Status:** draft (round 1). Pi review pending. Once owner-ratified
> Phase 3 per-commit agent work begins on the `rafaello-v0.1` branch
> under the milestone driver — see `driver-notes.md` (written after
> ratification).

Ordered commit list for m1, derived from `scope.md`. Each commit is
one logical idea **and leaves the workspace green** — pre-commit
hooks (rustfmt + clippy + test suites) gate every commit; intermediate
non-green states are not allowed. Commits land sequentially on
per-commit branches `agents/m1/c<NN>` rebased onto `rafaello-v0.1`,
no merge commits, no force pushes. Tests land with the code that
exercises them per `~/.claude/CLAUDE.md`.

## Conventions

- Subject style `<type>(<scope>): <imperative>`. Scopes:
  `rafaello-core` (the new m1 crate), `fittings-core` /
  `fittings-wire` (for §W), `rafaello` (workspace `Cargo.toml`
  changes), `rafaello-m1` (docs).
- "Acceptance" lists new tests + the pre-commit invariants the commit
  must keep green.
- "Depends on" cites the *lowest* commit number whose code or types
  this commit references. A commit only lands after every declared
  dependency has landed on `rafaello-v0.1`.
- Test files live under `rafaello/crates/rafaello-core/tests/`
  (or `rafaello/crates/rafaello-core/tests/fixtures/...` for fixture
  trees) unless otherwise noted; §W tests live under
  `fittings/tests/`.
- Per-commit agents pre-fix `cargo test --manifest-path
  rafaello/Cargo.toml -p rafaello-core` (and the fittings workspace
  command for §W) until green before invoking pre-commit hooks.

## m1a / m1b checkpoint

Per scope §"Internal split" + pi review-2 finding 12: an explicit
go/no-go checkpoint sits **after c19** (sink default inference +
digests + topic-id + lock + parsers landed; no compiler / carve-out
/ trifecta / broker ACL yet). The driver stops, re-evaluates against
the actual landed sequence, and either continues with one milestone
or opens an m1a (c01–c19) / m1b (c20–c40) owner-ratification
request.

Default: ship m1 as one milestone. Surface a split for owner
approval if c20+ cannot land green on c01–c19.

---

## Group 1 — Crate skeleton + workspace deps + manifest types/parser (S1–S3, M1–M9)

### c01 — chore(rafaello): introduce `[workspace.dependencies]` and add `rafaello-core` crate skeleton

- **What.** Add a `[workspace.dependencies]` table to
  `rafaello/Cargo.toml` listing every crate scope §S2 names
  (`serde`, `toml`, `serde_json`, `sha2`, `data-encoding`,
  `thiserror`, `semver`, `chrono`, `outpost = { path =
  "../outpost/crates/outpost" }`, `tempfile` as a dev-dep). Add
  `crates/rafaello-core/` to `[workspace.members]`. Create
  `crates/rafaello-core/Cargo.toml` with `name = "rafaello-core"`,
  empty deps list (each test commit pulls what it needs from
  workspace.dependencies), and `crates/rafaello-core/src/lib.rs`
  containing only `// crate doc placeholder; modules land in
  subsequent m1 commits.` so the crate compiles.
- **Why.** scope §S1, §S2.
- **Depends on.** baseline.
- **Acceptance.** `cargo test --manifest-path rafaello/Cargo.toml
  -p rafaello-core` is green (no tests yet, `cargo build` only).
  `cargo doc --manifest-path rafaello/Cargo.toml -p rafaello-core
  --no-deps` warning-free. Outpost path resolves; `cargo metadata
  --manifest-path rafaello/Cargo.toml --format-version 1` lists
  outpost as a workspace dep.

### c02 — feat(rafaello-core): manifest top-level types + reserved-field pre-scan + name grammar

- **What.** New `rafaello_core::manifest` module. Types: `Manifest
  { schema, name, version, entry, rafaello, description?,
  authors?, license?, homepage? }` per scope §M1. `Manifest::parse(s:
  &str) -> Result<Self, ManifestError>` runs a `toml::Table`
  pre-scan first (per §M2) rejecting `runtime` / `rpc` /
  `helper_for` with `ManifestError::ReservedField { field,
  deferred_to }`, then deserialises with
  `#[serde(deny_unknown_fields)]`. Manifest `name` validated
  against the topic-segment grammar `[a-z0-9_][a-z0-9_-]*` per
  §M1 (pi-3 finding 8). `ManifestError` enum lives under
  `rafaello_core::error::ManifestError`.
- **Why.** scope §M1, §M2 (pi-1 finding 8 reserved-field strategy).
- **Depends on.** c01.
- **Acceptance.** `tests/manifest_parse_minimal.rs` (positive).
  Negative tests: `tests/manifest_unknown_field.rs`,
  `tests/manifest_legacy_runtime_field.rs`,
  `tests/manifest_legacy_rpc_block.rs`,
  `tests/manifest_helper_for_field.rs`,
  `tests/manifest_invalid_name.rs`. All other tests in the m1
  matrix still ungated.

### c03 — feat(rafaello-core): manifest `[provides]` block (tools, provider, tool tables)

- **What.** Extend `manifest` module with `Provides` typed struct:
  `tools: Vec<String>` (validated against tool-name grammar per
  §M3), `provider: Option<String>` (same grammar), and
  `tools_meta: BTreeMap<String, ToolMetaManifest>` where
  `ToolMetaManifest = { sinks: Option<Vec<String>>, grant_match:
  Option<SafePath>, always_confirm: bool (default false) }`. M11
  `SafePath` lands here (relative-package paths only — no
  placeholders, no `..`, no leading `/`, no empty segments, no
  `\`, no control chars). Tool-table presence rule per §V1
  finding-5 alignment: missing tables in `tools` get the overview
  §15.1 defaults at validate-time; orphan tables (not in `tools`)
  rejected.
- **Why.** scope §M3, §M11 (SafePath half), §V1 (tool-table
  presence per pi-6 finding 5).
- **Depends on.** c02.
- **Acceptance.** Positive: `tests/manifest_parse_tool_example.rs`.
  Negatives:
  `tests/manifest_dotted_tool_name.rs`,
  `tests/manifest_unknown_tool_table.rs`,
  `tests/manifest_malformed_sinks.rs`,
  `tests/manifest_grant_match_traversal.rs`. Tool-table omission
  + defaults exercised in c08's
  `tests/tool_table_omitted_uses_defaults.rs` once the
  validate-with-package call exists.

### c04 — feat(rafaello-core): manifest `[bus]` block + topic / pattern grammar + namespace ACL (canonical-id-independent)

- **What.** Extend `manifest` with `Bus { subscribes:
  Vec<SubscribePattern>, publishes: Vec<Topic> }`. `Topic` and
  `SubscribePattern` are typed at parse time and enforce: at
  least two segments per security RFC §5.1 (pi-5 medium 10),
  segment grammar `[a-z0-9_-]+`, `*` / `**` only in subscribe
  positions (`**` final-only). `validate::manifest_standalone`
  rejects publishes on `core.*` and `frontend.*` outright (V1's
  canonical-id-independent class). Provider/foreign-topic-id
  checks defer to V2 (c10).
- **Why.** scope §M4, §V1 (topic ACL / pattern discipline).
- **Depends on.** c03.
- **Acceptance.** Positive coverage from the
  `manifest_parse_tool_example.rs` fixture. Negatives:
  `tests/manifest_publishes_core_topic.rs`,
  `tests/manifest_publishes_frontend_topic.rs`,
  `tests/manifest_publish_with_wildcard.rs`,
  `tests/manifest_subscribe_invalid_pattern.rs`,
  `tests/manifest_topic_segment_grammar.rs`,
  `tests/manifest_topic_too_few_segments.rs`.

### c05 — feat(rafaello-core): manifest `[capabilities]` block + `CapabilityPathTemplate`

- **What.** Extend `manifest` with `Capabilities` map:
  `BTreeMap<BundleKey, CapabilityBundle>` where `BundleKey` is
  `Default | Named(String)` (the named form must match
  `provides.tools` per §M5; deferred check until c08's
  validate-with-package). `CapabilityBundle` has
  `filesystem`, `network`, `env`, `limits` sub-tables per the
  manifest RFC §5. `manifest::CapabilityPathTemplate::parse`
  lands here (M11's second vocabulary): accepts the closed
  placeholder set from M8 as a prefix OR an absolute host path;
  rejects bare relative paths, control chars, non-UTF-8, `\`.
  `network.allow_hosts` requires `mode = "proxy"` per V1 (pi-4
  finding 8); rejection lands here.
- **Why.** scope §M5, §M11 CapabilityPathTemplate half, §V1
  allow_hosts mode rule.
- **Depends on.** c03.
- **Acceptance.** Positive: extended
  `manifest_parse_tool_example.rs` covers scoped
  `[capabilities.format.filesystem]`. Negatives:
  `tests/manifest_unknown_bundle_key.rs` (deferred to c08 if
  the bundle-key check needs `provides.tools` context — file
  written here with `#[ignore]` removed in c08, OR landed
  here as a parse-only check; commits.md picks the parse-only
  variant: bundle-key validation against `provides.tools`
  runs in `validate::manifest_standalone` once both blocks
  parse, so the test does land here),
  `tests/manifest_allow_hosts_outside_proxy.rs`.

### c06 — feat(rafaello-core): manifest `[load]` block + load-trigger pattern matching

- **What.** Extend `manifest` with `Load` enum: `Eager | Boot |
  Manual | Lazy { event: Vec<String>, command: Vec<String>,
  kind: Vec<String> }`. Parser handles both string-shorthand and
  table forms per scope §M6. `validate::manifest_standalone`
  cross-validates triggers against declared `provides.tools`,
  `bus.subscribes` (pattern-match per pi-3 finding 9; not
  literal-equality), and renderer kinds.
- **Why.** scope §M6, §V1 load-trigger cross-refs.
- **Depends on.** c03, c04.
- **Acceptance.** Positives:
  `tests/manifest_validate_load_trigger_cross_refs.rs`,
  `tests/manifest_load_event_pattern_match.rs`. Negative:
  `tests/manifest_load_trigger_unknown_command.rs`. Renderer
  kind cross-ref deferred to c07.

### c07 — feat(rafaello-core): manifest `[[renderers]]` array + Stream E prefix grammar + built-in reservation

- **What.** Extend `manifest` with `Renderer { kind: RendererKind,
  priority: u32 (default 100), method: Option<String> }`.
  `RendererKind` parser rejects built-in names (`text`,
  `code_block`, `tool_call`, `tool_result`, `error`, `heading`,
  `thinking`, `image`) per §M7, AND requires plugin kinds match
  the prefix grammar `<vendor-prefix>:<kind-name>` per Stream E
  §8 (pi-4 finding 7). `validate::manifest_standalone`'s
  load-trigger `kind` cross-ref checks against the declared
  renderer set.
- **Why.** scope §M7, §V1 (renderer kind grammar).
- **Depends on.** c06.
- **Acceptance.** Positive:
  `tests/manifest_parse_renderer_example.rs` (with `mermaid:diagram`,
  `diff:code` — both Stream-E-prefixed). Negatives:
  `tests/manifest_reserved_renderer_kind.rs`,
  `tests/manifest_unprefixed_renderer_kind.rs`.

### c08 — feat(rafaello-core): manifest validate-with-package + provider example + canonical bytes

- **What.** Land `manifest::placeholders::expand`,
  `Manifest::canonical_bytes()`, and the package-level validator
  `manifest::validate_with_package(manifest_path, package_dir,
  manifest)` per §M8, §M9, §M10. The validator covers: `entry`
  resolution + escape + file-vs-dir checks, `grant_match`
  resolution + escape + presence, openrpc.json sibling presence
  for **every** plugin (pi-3 finding 4 — drop the
  `provides.tools` qualifier), `exec_paths` / `exec_dirs` under
  `${project}` refusal per §V1 (pi-6 finding 4).
- **Why.** scope §M8, §M9, §M10, §V1 (exec_paths inside project).
- **Depends on.** c05, c07.
- **Acceptance.** Positives:
  `tests/manifest_canonical_bytes_stable.rs`,
  `tests/manifest_placeholder_expansion.rs`,
  `tests/manifest_openrpc_sibling_present.rs`,
  `tests/manifest_grant_match_present.rs`,
  `tests/manifest_parse_provider_example.rs`,
  `tests/tool_table_omitted_uses_defaults.rs`. Negatives:
  `tests/manifest_missing_openrpc_sibling.rs`,
  `tests/manifest_missing_openrpc_provider.rs`,
  `tests/manifest_entry_traversal.rs`,
  `tests/manifest_entry_not_found.rs`,
  `tests/manifest_entry_escape_via_symlink.rs`,
  `tests/manifest_grant_match_missing.rs`,
  `tests/manifest_exec_path_inside_project.rs`. Fixture trees
  for each test live under `tests/fixtures/`.

---

## Group 2 — Lock types (L1–L9 minus L6) + canonical id (S → L)

### c09 — feat(rafaello-core): canonical id parser/formatter + grammar

- **What.** New `rafaello_core::lock::CanonicalId` type per §L8
  with `parse(&str) -> Result<Self, LockError>` / `Display`.
  Source grammar `/`-separated `[a-z0-9._-]+` segments, no `..`,
  no leading/trailing/double `/`, no empty segments. `name`
  matches the topic-segment grammar; `version` parsed via
  `semver::Version`. Round-trip stable.
- **Why.** scope §L8 (with pi-3 finding 1 path-traversal hardening).
- **Depends on.** c01.
- **Acceptance.** Positives: `tests/lock_canonical_id_round_trip.rs`.
  Negatives: `tests/lock_canonical_id_invalid.rs`,
  `tests/lock_canonical_id_path_traversal.rs`.

### c10 — feat(rafaello-core): lock schema types + serde round-trip (no validation yet)

- **What.** `rafaello_core::lock::Lock` carrying:
  `plugins: BTreeMap<CanonicalId, PluginEntry>` and
  `session: SessionTable` per §L1. `PluginEntry` exposes
  `entry: SafePath`, `digest`, `manifest_digest`,
  `granted_at: chrono::DateTime<Utc>` (per §L7), `grant.bundles:
  BTreeMap<BundleKey, GrantBundle>` + `.grant.subscribes` /
  `.grant.publishes` cross-bundle fields per §L3, `bindings`
  (per §L4 — `provider`, `provider_id`, `tools`,
  `renderer_kinds`, `tool_meta` with `sinks_inferred`, plus
  `load: LoadPolicy` per pi-6 finding 2), `flags` (per §L5).
  `SessionTable` has `provider_active`, `tool_owner`. Lock
  loader uses `SafePath` for `.entry` (pi-3 finding 2). All
  structs `#[serde(deny_unknown_fields)]`. `Lock::to_toml` /
  `Lock::from_toml` deterministic ordering.
- **Why.** scope §L1–L5, §L7, §L9 (data-only round-trip; cross-
  plugin validation lands in V3 / Group 6).
- **Depends on.** c09, c06 (LoadPolicy mirrors M6's enum).
- **Acceptance.** Positives:
  `tests/lock_parse_round_trip.rs`,
  `tests/lock_load_policy_round_trip.rs`,
  `tests/lock_load_policy_eager_string.rs`,
  `tests/sinks_inferred_flag_round_trips.rs`. Negatives:
  `tests/lock_unknown_field.rs`,
  `tests/lock_helper_field_rejected.rs`,
  `tests/lock_missing_entry.rs`,
  `tests/lock_entry_traversal.rs`.

### c11 — feat(rafaello-core): always_confirm tool meta round-trip via lock

- **What.** Tests-only thin commit. With c03 + c10 landed, assert
  the manifest → lock projection of `always_confirm` (the
  CompiledPlugin half of the round-trip lands in c33 once the
  compiler exists; per scope §"two-stage tests" pattern). This
  commit lands the manifest→lock half so the field is exercised
  before the compiler is built.
- **Why.** scope §L4 + pi-4 finding 6 (load-bearing for m5).
- **Depends on.** c10.
- **Acceptance.** Initial `tests/tool_meta_always_confirm_round_trip.rs`
  exercises the manifest→lock half; extended in c33 with the
  `CompiledPlugin.tool_meta` half (two-stage test per m0
  pattern §4.3).

---

## Group 3 — Topic-id derivation (T1–T3)

### c12 — feat(rafaello-core): topic_id::derive + collisions_with_prefixes (public)

- **What.** New `rafaello_core::topic_id` module. `derive(canonical:
  &str) -> String` returns `id_<base32-no-pad-lower(sha256(canonical))[0..16]>`.
  `collisions_with_prefixes(pairs: &[(CanonicalId, String)]) ->
  Result<(), CollisionError>` is the **public** stable API
  (pi-2 finding 8 + pi-3 finding 6 — no `pub(crate)` /
  `feature = "test-seam"` boundary). `collisions(plugins:
  &[CanonicalId])` computes prefixes via `derive` then
  delegates.
- **Why.** scope §T1, §T2, §T3.
- **Depends on.** c09.
- **Acceptance.** Positive: `tests/topic_id_derivation.rs`.
  Negative: `tests/topic_id_collision_detection.rs` (forces a
  collision via the public `collisions_with_prefixes`).

---

## Group 4 — Single-plugin validation (V1, V2)

### c13 — feat(rafaello-core): validate::manifest_standalone (V1) consolidated

- **What.** Wire up the V1 entry point that exercises the parse-
  time checks distributed across c03–c08 plus the cross-bundle /
  cross-trigger checks. Most rules already land in their owning
  parser commits; this commit:
  - exposes `validate::manifest_standalone(manifest: &Manifest)`
    as a single API,
  - moves any deferred checks (e.g. capability bundle key
    consistency with `provides.tools`, sink class grammar) to
    this entry point so callers don't depend on parse-time
    validation order.
- **Why.** scope §V1 single API surface.
- **Depends on.** c03, c04, c05, c06, c07, c08.
- **Acceptance.** A consolidating test
  `tests/validate_manifest_standalone.rs` runs the worked-example
  manifests (tool, provider, renderer) through the public V1
  entry point and asserts each succeeds. The negative tests
  landed in c02–c08 already exercise V1's failure modes.

### c14 — feat(rafaello-core): validate::manifest_with_id (V2) — canonical-id-bound publish ACL

- **What.** New `validate::manifest_with_id(manifest, canonical)`
  per §V2: rejects `plugin.<topic-id>.*` publishes whose
  `<topic-id>` doesn't match `topic_id::derive(canonical)`;
  rejects `provider.<id>.*` publishes whose `<id>` doesn't match
  `provides.provider`.
- **Why.** scope §V2.
- **Depends on.** c12, c13.
- **Acceptance.** Negatives:
  `tests/manifest_publishes_other_plugin_namespace.rs`,
  `tests/manifest_provider_namespace_mismatch.rs`.

---

## Group 5 — Digest module (D1, D2) + canonical-bytes wiring

### c15 — feat(rafaello-core): digest::manifest_digest + content_digest (deterministic, files-only, recursion-stack cycle detection)

- **What.** New `rafaello_core::digest` module:
  `manifest_digest(canonical_bytes) -> String` and
  `content_digest(package_dir) -> Result<String, DigestError>`
  per §D1, §D2. Walk strategy: files only, sorted relative
  paths, length-prefixed path-then-content sha256 fold; symlinks
  followed inside the package, refused outside; directory
  symlinks followed with **recursion-stack cycle detection** (pi-6
  finding 6 — distinct logical paths sharing a canonical target
  contribute under both relative paths). `RecomputedDigests`
  helper struct per §D3 ready for the compiler in c33.
- **Why.** scope §D1, §D2.
- **Depends on.** c01.
- **Acceptance.** Positives:
  `tests/digest_content_deterministic.rs`,
  `tests/digest_distinct_paths_same_target.rs`. Negatives:
  `tests/digest_symlink_escape.rs`,
  `tests/digest_symlink_cycle.rs`.

---

## Group 6 — Sink default inference (Si1, Si2)

### c16 — feat(rafaello-core): sinks::infer_defaults over effective per-tool grant (Si1)

- **What.** New `rafaello_core::sinks` module.
  `infer_defaults(effective: &GrantBundle, declared:
  &Option<Vec<String>>) -> Vec<String>` per §Si1. The
  `effective` parameter is the per-tool flatten (`default ∪
  <tool-name>`) per pi-3 finding 3 + decision row 17. `None`
  declared → infer; `Some` declared → preserve verbatim.
- **Why.** scope §Si1.
- **Depends on.** c10.
- **Acceptance.** Positives: `tests/sinks_infer_defaults.rs`,
  `tests/sinks_infer_from_named_bundle.rs`.

---

## Group 7 — Trifecta refusal (Tr1–Tr5)

### c17 — feat(rafaello-core): trifecta::evaluate (one-hop, private-state structurally excluded)

- **What.** New `rafaello_core::trifecta` module.
  `evaluate(lock: &Lock, canonical: &CanonicalId, ctx:
  &PathContext) -> TrifectaState` per §Tr1–Tr5. Booleans
  computed across the full bundle union (per row 17 / pi-4
  finding 1 — no per-call switch). `has_workspace_write`
  excludes the per-plugin private-state subtree structurally
  (the lock has no `write_dirs` entry for it; C5 will add it
  later). One-hop direct check across other plugins'
  subscribe-pattern-matches-publish-topic graph.
  `refuse = ... && !flags.i_know_what_im_doing`.
- **Why.** scope §Tr1–Tr5.
- **Depends on.** c10.
- **Acceptance.** Positives:
  `tests/trifecta_two_plugins_one_hop.rs`,
  `tests/trifecta_iknowwhatimdoing_bypass.rs`. The
  private-state exclusion test lands as
  `tests/compile_private_state_excluded_from_workspace_write.rs`
  in c33 once the compiler injects the C5 dir.

---

## Group 8 — Carve-out decomposition (K1–K4)

### c18 — feat(rafaello-core): carveout::compile_against — project-class decompose / credential-class refuse / write refuse

- **What.** New `rafaello_core::carveout` module. `CARVE_OUTS`
  constant (the v1 set per §K1, two classes). `compile_against(grant:
  &GrantBundle, canonical: &CanonicalId, ctx: &PathContext,
  allow_credential_paths: bool) -> Result<DecomposedGrant,
  CompileError>` per §K2: project-class read decomposes (256
  cap), credential-class read refuses, all writes refuse,
  explicit leaf hits on either class refuse (no
  `dropped_carveouts` diagnostic — pi-2 finding 7).
  Decomposition uses the immediate-non-hidden-children rule for
  hidden-directory case per §K3.
- **Why.** scope §K1, §K2, §K3, §K4.
- **Depends on.** c05 (CapabilityPathTemplate), c10 (Lock).
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

## Group 9 — Env scrubber + reserved-env C7.1 (Sc1–Sc3)

### c19 — feat(rafaello-core): scrubber::strip + reserved-env C7.1 rejection helper

- **What.** New `rafaello_core::scrubber` module.
  `SECRET_PATTERNS` constant per §Sc1.
  `strip(env_pass: &[String], i_know_what_im_doing: bool) ->
  Vec<String>` per §Sc2 (override returns input verbatim).
  `scrubber::reject_reserved(env_pass: &[String], env_set:
  &BTreeMap<String, String>) -> Result<(), CompileError>` per
  §C7.1 — rejects `RFL_BUS_FD` / `RFL_PLUGIN` in either
  collection. Compiler will call it in c33.
- **Why.** scope §Sc1–Sc3, §C7.1.
- **Depends on.** c01.
- **Acceptance.** Positives:
  `tests/env_scrubber_strips_known_secrets.rs`. Negatives:
  `tests/env_scrubber_strips_secret_globs.rs`,
  `tests/env_scrubber_override.rs`. (C7.1 reserved-env negative
  tests `compile_reserved_env_in_pass.rs` /
  `compile_reserved_env_in_set.rs` land in c33 once the compiler
  invokes them.)

---

## **m1a / m1b checkpoint after c19.** Driver re-evaluates and either continues or opens a split request.

---

## Group 10 — Cross-plugin lock validation (V3 — wires Tr + carveout + sink-drift + topic-id collision + tool-owner + lock-side mirrors)

### c20 — feat(rafaello-core): validate::lock multi-plugin context + topic-id collision + provider/tool_owner integrity

- **What.** New `validate::lock(lock: &Lock, ctx:
  &LockValidationContext) -> Result<()>` per §V3 (with the
  multi-plugin context per pi-6 finding 1). This commit lands
  the orchestration shell + the rules that don't require
  trifecta/carveout/sinks (those wire in c21):
  - topic-id collision (delegates to `topic_id::collisions`),
  - conflicting tool name + `[session].tool_owner` resolution
    + target integrity (pi-5 finding 2 — installed, declares
    tool, no redundant entries),
  - provider activeness consistency (`provider_active`
    references an installed provider plugin),
  - per-plugin `PathContext` derivation from
    `LockValidationContext.plugin_dirs`,
  - `MissingPluginDir` failure for canonicals without an entry.
- **Why.** scope §V3.
- **Depends on.** c10, c12.
- **Acceptance.** Positives:
  `tests/validate_lock_multiplugin_context.rs`. Negatives:
  `tests/lock_provider_active_unknown.rs`,
  `tests/lock_provider_active_not_provider.rs`,
  `tests/lock_conflicting_tool_names.rs`,
  `tests/lock_tool_owner_unknown_plugin.rs`,
  `tests/lock_tool_owner_plugin_does_not_declare_tool.rs`,
  `tests/lock_tool_owner_redundant.rs`,
  `tests/topic_id_collision_at_lock.rs`.

### c21 — feat(rafaello-core): V3 wires trifecta + carveout + sink-drift

- **What.** Extend V3 to delegate per-plugin trifecta evaluation
  (c17), carve-out enforcement (c18), and sink-default drift
  detection (Si2). Failures surface as
  `ValidationError::TrifectaRefused`, `CarveOutRefused`,
  `CarveOutTooLarge`, `SinkInferenceDrift`.
- **Why.** scope §V3 (trifecta + carve-out + Si2 bullets).
- **Depends on.** c16, c17, c18, c20.
- **Acceptance.** Negative: `tests/sinks_inference_drift.rs`.
  Trifecta and carve-out negative tests landed in their own
  commits (c17, c18); this commit's acceptance is that the V3
  caller path also exercises them via a small
  `tests/validate_lock_full_pass.rs` integration that builds a
  multi-plugin fixture and asserts both pass and refusal cases.

### c22 — feat(rafaello-core): V3 lock-side publish ACL mirror

- **What.** Extend V3 with the lock-side namespace ACL on
  `.grant.publishes` per pi-5 finding 1: rejects `core.*` /
  `frontend.*`; `plugin.<id>.*` must match
  `topic_id::derive(canonical)` for the lock entry;
  `provider.<id>.*` requires `bindings.provider == true` and
  matching `bindings.provider_id`.
- **Why.** scope §V3 lock-side publish authority bullet.
- **Depends on.** c20.
- **Acceptance.** Negatives:
  `tests/lock_publishes_core_topic.rs`,
  `tests/lock_publishes_frontend_topic.rs`,
  `tests/lock_publishes_other_plugin_namespace.rs`,
  `tests/lock_provider_namespace_mismatch.rs`.

### c23 — feat(rafaello-core): V3 lock-side allow_hosts mode + bundle-key + cap-path mirrors

- **What.** Extend V3 with three more lock-side mirrors:
  `allow_hosts` requires proxy mode (pi-5 finding 3), unknown
  grant bundle key rejection (pi-6 finding 3), capability path
  template re-validation per `CapabilityPathTemplate` rules
  (pi-6 finding 3).
- **Why.** scope §V3 lock-side `allow_hosts` / bundle key /
  capability path bullets.
- **Depends on.** c20.
- **Acceptance.** Negatives:
  `tests/lock_allow_hosts_outside_proxy.rs`,
  `tests/lock_unknown_bundle_key.rs`,
  `tests/lock_capability_path_relative.rs`.

### c24 — feat(rafaello-core): V3 lock-side bindings grammar + tool_meta consistency mirrors

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
- **Depends on.** c20.
- **Acceptance.** Negatives:
  `tests/lock_tool_meta_grant_match_traversal.rs`,
  `tests/lock_tool_meta_orphan.rs`,
  `tests/lock_provider_id_inconsistent.rs`,
  `tests/lock_renderer_kind_unprefixed.rs`,
  `tests/lock_renderer_kind_builtin.rs`,
  `tests/lock_bindings_tools_invalid_grammar.rs`,
  `tests/lock_tool_meta_invalid_sink.rs`.

### c25 — feat(rafaello-core): V3 lock-side exec_paths under ${project} refusal

- **What.** Extend V3 with the §6.9 exec-under-project refusal
  on the lock side (pi-6 finding 4). After expansion +
  existing-ancestor canonicalisation, `exec_paths` /
  `exec_dirs` resolving inside `project_root` →
  `ValidationError::ExecPathInsideProject`. Manifest-side V1
  half landed in c08; this is the lock mirror.
- **Why.** scope §V3 exec_paths under project bullet.
- **Depends on.** c20.
- **Acceptance.** Negative: `tests/lock_exec_path_inside_project.rs`.

---

## Group 11 — Compiler core (C1–C7) + plan emission

### c26 — feat(rafaello-core): compile module skeleton + CompiledPlugin / FilesystemPlan / NetworkPlan / EnvPlan / LimitsPlan / LoadPolicy public types

- **What.** New `rafaello_core::compile` module with the public
  `CompiledPlugin` plan struct per §C1 + sub-types
  `FilesystemPlan`, `NetworkPlan { Deny | AllowAll | Proxy {
  allow_hosts } }`, `EnvPlan`, `LimitsPlan`, `CompiledFlags`,
  `ToolMeta`, `LoadPolicy` (the last reused from `lock` per
  c10). No `compile_plugin` body yet — this commit only lands
  the type surface so subsequent commits can implement against
  it.
- **Why.** scope §C1 (data types only, no compile logic).
- **Depends on.** c10.
- **Acceptance.** A `tests/compile_types_compile.rs` asserts the
  public types compile and have the documented field shapes
  (build-only assertion via type-level expressions). No
  behavioural test here.

### c27 — feat(rafaello-core): compile_plugin entry point + API contract + V3-must-run-first guard

- **What.** Implement `compile_plugin(lock, canonical, ctx,
  recomputed_digests) -> Result<CompiledPlugin, CompileError>`
  per §C2 with the precondition contract per pi-6 medium 7
  (§C1.1): if invariants V3 should have rejected are detected,
  return `CompileError::ValidationNotRun`. The body is a
  scaffold — bundle flatten, placeholder substitution, and the
  per-section emitters land in c28–c32.
- **Why.** scope §C2, §C1.1.
- **Depends on.** c26, c20.
- **Acceptance.** Negative: `tests/compile_without_validate_lock_errors.rs`.
  Positive harness used by later commits lives here as a
  `#[test]` helper module.

### c28 — feat(rafaello-core): compile placeholder substitution + path resolver (existing-ancestor canonical + lexical suffix + containment)

- **What.** Implement C3's placeholder expansion + the
  containment resolver per pi-5 finding 7: walk the
  post-expansion path component-by-component; canonicalise the
  longest existing ancestor (with symlink + escape checks);
  lexically join the non-existent suffix; final containment
  check on the absolute path against `project_root` /
  `plugin_dir` for `${project}` / `${plugin}` placeholders.
  Failures: `CompileError::UnknownPlaceholder`,
  `CompileError::PathEscape`, `CompileError::SymlinkEscape`.
- **Why.** scope §C3.
- **Depends on.** c27.
- **Acceptance.** Positives:
  `tests/manifest_placeholder_expansion.rs` extends to cover
  the compile-side resolver,
  `tests/compile_placeholder_resolves_to_absolute.rs`,
  `tests/compile_capability_path_nonexistent_write_leaf.rs`.
  Negatives:
  `tests/compile_unknown_placeholder.rs`,
  `tests/compile_path_escape_after_expansion.rs`,
  `tests/compile_capability_path_symlink_ancestor_escape.rs`.

### c29 — feat(rafaello-core): compile bundle flatten (full union) + dedup + ordering (C4)

- **What.** Per `decisions.md` row 17 + pi-4 finding 1: the
  compiler unions `default` ∪ every named bundle in
  `grant.bundles` into one spawn-time policy. Apply the C4
  post-flatten deterministic ordering: sort scalar arrays by
  string value, dedup. No `active_bundles` selection knob.
- **Why.** scope §C2 (union flatten), §C4 (ordering).
- **Depends on.** c28.
- **Acceptance.** Positives:
  `tests/compile_default_bundle.rs`,
  `tests/compile_scoped_bundle_union.rs`.

### c30 — feat(rafaello-core): compile filesystem plan via carve-out + private state grant (C5) + carveout integration

- **What.** Wire `carveout::compile_against` (c18) into the
  compiler so post-flatten reads/writes pass through
  decomposition. Inject the per-plugin private-state grant per
  §C5 using the **topic-id form**
  (`${project}/.rafaello-plugin-data/<topic-id>/`) — pi-3
  finding 5 + pi-4 finding 4. Private-state dir is added after
  trifecta evaluation (Tr4 structural exclusion).
- **Why.** scope §C5 + §K integration.
- **Depends on.** c18, c29.
- **Acceptance.** Positives:
  `tests/compile_private_state_grant.rs`,
  `tests/compile_private_state_excluded_from_workspace_write.rs`
  (the second-stage of the c17 trifecta test now that the
  compiler injects C5 — m0 two-stage test pattern).

### c31 — feat(rafaello-core): compile network plan + outpost dry-run validation

- **What.** Build `NetworkPlan` per §C1: `Deny | AllowAll |
  Proxy { allow_hosts }`. For proxy mode, run
  `outpost::NetworkPolicy::from_allowed_hosts(...)` as a
  parse-time dry-run; on failure return
  `CompileError::InvalidAllowHosts`. The parsed `NetworkPolicy`
  is discarded (m2 reconstructs at spawn time alongside the
  proxy startup) per Risks §2.
- **Why.** scope §C1 NetworkPlan + Risks §2.
- **Depends on.** c27.
- **Acceptance.** Positive:
  `tests/compile_network_proxy_allow_hosts_validates.rs`.
  Negative: `tests/compile_invalid_allow_hosts.rs`.

### c32 — feat(rafaello-core): compile env plan + reserved-env C7.1 wiring + scrubber call

- **What.** Build `EnvPlan` per §C1: `pass` (post-scrubber via
  c19), `set` verbatim. Call
  `scrubber::reject_reserved(env_pass, env_set)` first per
  §C7.1; if non-conforming, return
  `CompileError::ReservedEnvVarRequested`. Then call
  `scrubber::strip(env_pass, flags.i_know_what_im_doing)`.
- **Why.** scope §C7 + §EnvPlan portion of §C1.
- **Depends on.** c19, c27.
- **Acceptance.** Positive: `tests/compile_env_set_passes_through.rs`.
  Negatives: `tests/compile_reserved_env_in_pass.rs`,
  `tests/compile_reserved_env_in_set.rs`.

### c33 — feat(rafaello-core): compile entry resolution + limits defaults + digest gating + tool_meta projection (closes C1)

- **What.** Final compile pieces:
  - **Entry resolution** (per §L2 + pi-3 finding 2): re-parse
    `lock.entry` through `SafePath`; canonicalise against
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
    `<name>` is owned by this plugin per `[session].tool_owner`
    resolution (pi-2 finding 4). Carry `always_confirm` through
    (closing the c11 two-stage test).
- **Why.** scope §C1 (entry_absolute, tool_meta filter), §C6,
  §D3, §L2 compile-time check.
- **Depends on.** c11, c15, c30, c31, c32.
- **Acceptance.** Positives:
  `tests/compile_resource_limit_defaults.rs`,
  `tests/digest_match_compiles.rs`,
  `tests/digest_match_compiles.rs` extended for both digest
  fields, `tests/tool_meta_always_confirm_round_trip.rs`
  extended with the CompiledPlugin half (c11 two-stage closure).
  Negatives:
  `tests/lock_entry_not_found.rs`,
  `tests/lock_entry_escape_via_symlink.rs`,
  `tests/lock_entry_is_directory.rs`,
  `tests/digest_content_mismatch.rs`,
  `tests/digest_manifest_mismatch.rs`.

---

## Group 12 — Broker ACL extraction (G1–G3)

### c34 — feat(rafaello-core): broker_acl::compile with PluginAcl + auto-subscribes + tool_routes + grammar revalidation

- **What.** New `rafaello_core::broker_acl` module.
  `compile(lock: &Lock) -> Result<BrokerAcl, CompileError>`
  per §G1: per-plugin `PluginAcl { topic_id, publish_topics,
  subscribe_patterns, auto_subscribes (the
  `plugin.<topic-id>.tool_request` self-subscribe),
  provider_id }`, plus the **resolved** `tool_routes:
  BTreeMap<String, CanonicalId>` table (pi-2 finding 4).
  G2 grammar revalidation runs before emit. Same V3-must-run-
  first contract as `compile_plugin` (returns
  `ValidationNotRun` if invariants not enforced).
- **Why.** scope §G1, §G2, §G3.
- **Depends on.** c20 (V3 contract), c12 (topic-id), c10.
- **Acceptance.** Positives:
  `tests/broker_acl_extraction.rs`,
  `tests/broker_acl_tool_owner_resolves_routing.rs`.

---

## Group 13 — Fittings `MethodNotFound` typed-method cutover (W)

### c35 — feat(fittings): MethodNotFound typed `method` field cutover (W1–W5)

- **What.** Single workspace-wide cutover commit on the
  `fittings` workspace, mirroring m0's c08 pattern for
  source-breaking enum changes:
  - `fittings_core::error::FittingsError::MethodNotFound`
    gains `method: Option<String>` field (W1). Existing
    one-arg constructor `method_not_found(message)` keeps
    working with `method: None` / `data: None` (W3).
  - `method_not_found_with_method(method, message)`
    constructor added (W4).
  - `fittings_wire::error_map` extracts/synthesises
    `data.method` per W2's encode rules
    (typed-field-precedence; existing `data` keys preserved
    except a conflicting `method` key which is overwritten;
    when `method = None`, opaque `data.method` is preserved).
  - All in-tree consumers (`fittings/examples/*`,
    `mcp-server`, in-crate tests) updated to compile against
    the new shape.
- **Why.** scope §W1–W5; `decisions.md` row 36; m0
  retrospective §2.4.
- **Depends on.** baseline (independent of `rafaello-core`;
  may land at any point in m1's sequence).
- **Acceptance.** New
  `fittings/tests/method_not_found_typed_method_round_trip.rs`
  per W5 (table-driven: None round-trip; `Some(name)` synth +
  recovery; conflicting `data.method` overwrite; opaque
  `data.method` preserved when `method == None`; one-arg
  constructor builds `None`). `cargo test --manifest-path
  fittings/Cargo.toml --workspace` green per pi-5 finding 5.

---

## Group 14 — Manual validation

### c36 — docs(rafaello-m1): write manual-validation.md

- **What.** Write
  `rafaello/plans/milestones/m1-manifest/manual-validation.md`
  capturing each item from scope §"Manual validation":
  `cargo test --manifest-path rafaello/Cargo.toml -p
  rafaello-core` green on Linux; full fittings workspace
  green per pi-5 finding 5; `cargo doc --manifest-path
  rafaello/Cargo.toml -p rafaello-core --no-deps` clean;
  `cargo test --release` green; `nix develop --impure -L
  --command cargo test --manifest-path rafaello/Cargo.toml
  -p rafaello-core` green; `tree
  rafaello/crates/rafaello-core/tests/fixtures` dump.
- **Why.** scope §"Manual validation"; m0 pattern §4.5/§4.6.
- **Depends on.** c34, c35.
- **Acceptance.** `manual-validation.md` exists and captures
  the required evidence. Any tooling/CI/Nix follow-ups
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
    → topic-id) in `overview.md` §5.5 / `decisions.md` row
    16 / `glossary.md`.

## Notes on commit sizing + per-commit greenness

- **No workspace-wide cutover required for `rafaello-core`** —
  it's a brand-new crate with no existing consumers; every
  commit can incrementally add modules without forcing a
  consolidated breaking change. m0's c08 pattern is therefore
  not needed inside Group 1–12.
- **§W (c35) IS a workspace-wide cutover** for fittings — the
  `MethodNotFound` enum gains a struct field, which is
  source-breaking for direct struct literals and named-field
  pattern matches. This single commit consolidates the change
  and updates every in-tree consumer, mirroring m0 c08.
- **Two-stage tests** per m0 pattern §4.3:
  - `compile_private_state_excluded_from_workspace_write.rs`
    (c17 trifecta module + c30 compiler injection).
  - `tool_meta_always_confirm_round_trip.rs` (c11 manifest→lock
    + c33 CompiledPlugin half).
  - `digest_match_compiles.rs` (c15 digest module + c33
    compile gating).

## What changed from the first draft

This is the first draft. Pi review pending.
