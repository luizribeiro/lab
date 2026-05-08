# m1 — manifest / lock / grant / compiler foundation — scope

> **Status:** draft (round 2). Round-1 pi review (`pi-review-1.md`)
> identified 7 blocking + 3 non-blocking findings; this revision
> resolves all of them. The "What changed from the first draft"
> section at the end records the deltas. Pi review-2 pending.

## Goal

Land the **paper-first data-transformation** layer that takes a
plugin manifest plus a user grant and produces a structured
`CompiledPlugin` plan that m2's plugin supervisor can apply to a
`lockin::SandboxBuilder` + `SandboxedCommand` at spawn time —
without yet wiring up any spawn, broker, network proxy, or CLI.
m1 is intentionally pure: every output is computable from its
inputs; every test is a function-level assertion. m2 is the first
consumer (it spawns subprocess plugins from m1's compile output).

The deliverable is a new in-tree library crate `rafaello-core`,
exercised exclusively by `cargo test -p rafaello-core`. Nothing
under `rafaello/crates/rafaello/` (the `rfl` binary) gains any new
behaviour in m1.

## Inputs

- `rafaello/plans/overview.md` §3, §4 (esp. §4.2–§4.5), §5, §6,
  §15.1, §16.
- `rafaello/plans/decisions.md` rows **5, 12, 17, 25, 26, 27, 30,
  31, 32** (load-bearing for m1) plus row 36's m1 follow-through
  on `FittingsError::MethodNotFound`.
- `rafaello/plans/streams/f-manifest/rfc-manifest-schema.md` end
  to end. v1 reads it through the simplifications pinned in rows
  30/31/32 and the §15.1 normative-delta items 1–4 of `overview.md`.
  These deltas are not yet folded into the RFC body; m1's
  retrospective patches the drift (per `plans/milestones/README.md`
  §"Stream RFC drift").
- `rafaello/plans/streams/a-security/rfc-security-model.md`
  §3 (artifacts), §5 (bus ACL — topic / pattern grammar, namespaces,
  publish authority), §6 (attack scenarios that map onto compiler
  refusals), §7 (the grant compiler — trifecta, taint, sinks,
  carve-outs, env scrubber, private state).
- `rafaello/plans/streams/a-security/rfc-camel-on-v1.md` is **not**
  load-bearing for m1; the v1 deferrals (rows 26–29) eliminate every
  m1 dependency on it.
- `rafaello/plans/glossary.md`.
- Lockin's public Rust API for the v1 sandbox backend
  (`/home/luiz/lab/lockin/crates/sandbox/src/lib.rs` and
  `/home/luiz/lab/lockin/crates/config/src/lib.rs`):
  - `lockin::Sandbox::builder() -> SandboxBuilder` exposes
    `read_path` / `read_dir` / `write_path` / `write_dir` /
    `exec_path` / `exec_dir` / `network_*` / `inherit_fd` /
    `max_*` / `disable_core_dumps` / `allow_kvm` /
    `allow_interactive_tty` / `allow_non_pie_exec` /
    `raw_seatbelt_rule` builder methods.
  - `SandboxBuilder::command(self, program: &Path) ->
    Result<SandboxedCommand>` **consumes the builder** and
    returns the command. Env (`apply_env`) is then applied to
    `&mut SandboxedCommand`, not the builder. `allow_hosts`
    is **not** a builder method — it lives in
    `lockin::config::resolve_network_plan` which uses
    `outpost::NetworkPolicy` and yields `NetworkPlan::Proxy
    { policy }`; the caller starts an `outpost` proxy and
    passes the resulting loopback port to
    `SandboxBuilder::network_proxy(port)`.
  - `lockin::config::apply_config_to_builder(builder, config,
    config_dir) -> Result<SandboxBuilder>` is the existing
    file→builder bridge; m1's compiler is a structural mirror
    that **does not depend on `lockin::config`** (config
    consumes a `lockin.toml`, which `decisions.md` row 32
    rules out for v1).

  **Decision row 32 binds m1 to emitting structured plans the
  m2 supervisor applies — *not* a pre-populated `SandboxBuilder`
  that m2 only has to call `command(...)` on. Builder vs. plan
  mismatch was pi review-1 finding 1.**

## In scope

The work decomposes into modules of the new `rafaello-core` crate.
Each in-scope item names a public API surface plus the negatives
the test matrix asserts; per-commit granularity is the driver's
call when drafting `commits.md`.

### S — crate skeleton

- **S1.** New library crate `rafaello-core` under
  `rafaello/crates/rafaello-core/`, added to the workspace
  `[workspace.members]`. `lib.rs` only — no binary, no
  `rafaello`-bin wiring. Public re-exports limited to the modules
  named below; everything else stays crate-private.
- **S2.** A `[workspace.dependencies]` table is **introduced** in
  the rafaello workspace `Cargo.toml` (it does not exist today —
  pi review-1 finding 6) and m1's deps are declared there:
  - `serde = { version = "1", features = ["derive"] }`
  - `toml = "0.8"`
  - `serde_json = "1"`
  - `sha2 = "0.10"`
  - `data-encoding = "2"`  (base32-no-pad-lower for topic-id)
  - `thiserror = "1"`
  - `semver = { version = "1", features = ["serde"] }`
  - `chrono = { version = "0.4", features = ["serde"] }`  (RFC 3339 parsing for `granted_at`; matches the workspace family default)
  - `lockin = { path = "../../../lockin/crates/sandbox" }` (path dep, no version pin)
  - `lockin-config = { package = "lockin-config", path =
    "../../../lockin/crates/config" }` for the
    `lockin::config::NetworkPlan` re-export reused by m1
    network plans (introducing this dep avoids re-implementing
    `outpost::NetworkPolicy` parsing inside `rafaello-core`).
  - `tempfile = "3"` as a `[dev-dependencies]` entry for the
    fixture trees the digest / carve-out tests build.
  m1 introduces no other top-level deps. The "no new top-level
  deps" wording from the round-1 draft was wrong; this revision
  drops it (pi review-1 finding 6).
- **S3.** No public API leaks from `rafaello-core` outside the
  modules named in M / L / T / C / D / V / E / G / Tr / K / Sc
  below. The `rafaello`-bin entry point
  (`crates/rafaello/src/main.rs`) is unchanged in m1.

### M — manifest schema and parser (`rafaello_core::manifest`)

Parse `rafaello.toml` into a typed `Manifest` value. The schema
matches Stream F **post-simplifications**: no `runtime`, no
`[rpc]`, no `helper_for`. The `[provides]` block follows the
§15.1 normative delta in `overview.md`.

- **M1.** Top-level required fields: `schema = 1`, `name`,
  `version` (`semver::Version`), `entry` (path inside the package),
  `rafaello = ">=0.1, <0.2"` (`semver::VersionReq`). Optional
  metadata: `description`, `authors`, `license`, `homepage`.
  All structs use `#[serde(deny_unknown_fields)]` so unknown TOML
  keys are a typed parse error.
- **M2.** Reserved-field handling (pi review-1 finding 8). The
  three keys `runtime`, `rpc`, and `helper_for` are reserved per
  `decisions.md` rows 30 / 31 / 26. `Manifest::parse` runs a
  **pre-scan** of the raw TOML document (using `toml::Table`)
  before the typed deserialise: if any of those keys is present
  at its expected location (top-level `runtime`, top-level
  `helper_for`, the `[rpc]` table), parse fails with
  `ManifestError::ReservedField { field, deferred_to:
  ReservedReason }` *before* `serde`'s `deny_unknown_fields`
  fires. The hint string explains that `runtime` is post-row-30,
  `[rpc]` is post-row-31 (use the `openrpc.json` sibling), and
  `helper_for` is deferred to v2. Other unknown fields fall
  through to serde and produce `ManifestError::UnknownField`.
- **M3.** `[provides]` block per §15.1 item 1:
  - `tools: Vec<String>` (zero or more **tool names** —
    `[a-z0-9_][a-z0-9_-]*`, single-segment, lower-snake/kebab
    only; no dots; pi review-1 finding 3 forced this commitment).
    Examples: `grep`, `read-file`, `format`. The earlier draft's
    use of `rust.format` as a tool name is wrong — that's an
    OpenRPC method name and is not the manifest-side tool
    routing key.
  - `provider: Option<String>` (zero or one provider id; same
    grammar — provider id appears literally in
    `provider.<id>.*` topics).
  - `[provides.tool.<name>]` table per declared tool:
    - `sinks: Vec<String>` — known classes `"network"`,
      `"vcs_push"`, `"mail"`, `"workspace_write"`, `"exec"`;
      custom strings allowed but lower-snake-case-validated
      (`[a-z0-9_]+`).
    - `grant_match: Option<PathBuf>` — relative path to a
      JSON-Schema describing the `user_grants` matcher.
      Absent → "any invocation" matcher; the schema file
      itself is **not** parsed in m1 (its consumer is m5);
      v1 only validates the path syntax + that the file
      exists at install time.
    - `always_confirm: bool` — UX gate per §15.1 item 1;
      defaults `false`.
- **M4.** `[bus]` block:
  - `subscribes: Vec<String>` — list of subscribe **patterns**
    drawn from the §5.1 pattern grammar (`*` / `**` allowed;
    `**` only as final segment).
  - `publishes: Vec<String>` — list of **topics** (no
    wildcards; pattern in publish position is rejected).
- **M5.** `[capabilities]` block, with the bundle-and-section
  grammar from manifest RFC §5: `[capabilities.<bundle>.{filesystem,network,env,limits}]`.
  Bundle name `default` plus zero or more named bundles; each
  named bundle's key **must** be a tool name from
  `provides.tools` (pi review-1 finding 3 — bundle keys are
  not free-form). Each section is a typed inner struct with the
  fields the manifest RFC enumerates: `read_paths`, `read_dirs`,
  `write_paths`, `write_dirs`, `exec_paths`, `exec_dirs`,
  `network.mode`, `network.allow_hosts`, `env.pass`, `env.set`,
  `limits.max_cpu_time`, `limits.max_open_files`,
  `limits.max_address_space`, `limits.max_processes`.
- **M6.** `[load]` block per manifest RFC §7.2. Either a string
  (`"eager"` / `"boot"` / `"manual"` / `"lazy"` sugar) or a
  table with one or more of `event` / `command` / `kind`.
- **M7.** `[[renderers]]` array per manifest RFC §6: `kind`,
  optional `priority` (default 100), optional `method`. The
  built-in v1 renderers (`text`, `code_block`, `tool_call`,
  `tool_result`, `error`, `heading`, `thinking`, `image`) are
  reserved and **rejected** if a plugin tries to register them
  — built-in registration is hard-coded in core (m3 territory)
  and `decisions.md` row 29 defers subprocess plugin renderers
  to v2 anyway.
- **M8.** Placeholder substitution table per manifest RFC §5.1,
  exposed as `manifest::placeholders::expand(input: &str,
  ctx: &PathContext) -> Result<String, ManifestError>`.
  Closed set: `${project}`, `${home}`, `${plugin}`, `${cache}`,
  `${state}`. No env-var interpolation, no `${secret:...}`.
- **M9.** Canonical-form normalisation: `Manifest::canonical_bytes()`
  emits a deterministic byte representation used for hashing
  the manifest snapshot at install time (§D2). Implemented as
  "TOML re-emit with sorted keys at every table level" via
  `toml::Table` — no semantic transforms.
- **M10.** **`openrpc.json` sibling presence**, per
  `decisions.md` row 31 — checked by
  `manifest::validate_with_package(manifest_path, package_dir,
  manifest)`. If `provides.tools` is non-empty, the validator
  requires `<package_dir>/openrpc.json` to exist as a regular
  file. Absent (or unreadable) → `ManifestError::MissingOpenRpc`.
  v1 does **not** parse openrpc.json; that contract belongs to
  fittings + m2's spawn path.

### L — lock schema (`rafaello_core::lock`)

Mirror of the manifest plus the bindings + grant view per security
RFC §3.2. Pi review-1 finding 2 forced expansion of this surface
to carry every input the compiler needs.

- **L1.** Top-level shape:
  - Per-plugin tables keyed by canonical id `<source>:<name>@<version>`,
    e.g. `[plugin."github:acme/grep@1.4.2"]`.
  - Per-plugin sub-tables `.entry`, `.grant`, `.bindings`,
    `.flags`, `.active_bundles` (pi-1 finding 2 — see L8).
  - `[session]` table with:
    - `provider_active: Option<String>` (string or absent)
    - `tool_owner: BTreeMap<String, String>` — per security RFC
      §5.4 ("conflicting tool bindings are a lock-time error;
      the user resolves with `rfl provider tool grep
      <plugin-id>`. The choice is persisted in the lock");
      m1 round-trips and respects this table.
- **L2.** **`.entry: PathBuf`** per plugin (pi-1 finding 2). The
  manifest's `entry` field is snapshotted into the lock at
  install time; the compiler reads this field to resolve
  `entry_absolute = ${plugin_dir}/<entry>`. The lock-side copy
  is what makes the spawn path independent of the on-disk
  manifest (security RFC §3.2 "spawn path reads the snapshot,
  not the live manifest").
- **L3.** **`.grant.bundles`** — bundle-aware grant shape. The
  grant block is a map `BTreeMap<BundleKey, GrantBundle>` where
  `BundleKey ∈ { Default, Named(String) }` and `GrantBundle`
  carries the per-bundle `read_paths` / `read_dirs` /
  `write_paths` / `write_dirs` / `exec_paths` / `exec_dirs`
  / `network` / `env_pass` / `env_set` / `limits`. The lock
  also carries the cross-bundle fields `subscribes:
  Vec<String>` (patterns) and `publishes: Vec<String>` (topics)
  at the `.grant` level (these are not bundle-scoped). Pi-1
  finding 2 made the bundle representation explicit.
- **L4.** `.bindings` fields per security RFC §3.2 (manifest-derived
  authority snapshotted at install time):
  - `provider: bool`,
  - `provider_id: Option<String>` (present iff `provider == true`),
  - `tools: Vec<String>`,
  - `renderer_kinds: Vec<String>`,
  - `tool_meta: BTreeMap<String, ToolMeta>` where
    `ToolMeta = { sinks: Vec<String>, grant_match:
    Option<PathBuf>, always_confirm: bool }`.
  - **No `helpers` / `helper_for` fields** (deferred per row 26).
- **L5.** `.flags` fields — boolean lock-level overrides matching
  the security RFC's loud overrides:
  - `i_know_what_im_doing: bool` (suppresses trifecta refusal +
    env-scrubber strip per §7.1 / §7.4).
  - `allow_credential_paths: bool` (suppresses the carve-out
    refusal class per §7.3 rule 5).
  Both default `false`. Both are surfaced by the broker / runtime
  in `rfl status` (m2+ work; m1 only has to round-trip the field).
- **L6.** **`.active_bundles: Vec<String>`** (pi-1 finding 2).
  The set of named-bundle keys that are currently active for
  spawn, in addition to `default`. m1 does not dictate how this
  set is populated — m5 is the consumer that flips a named
  bundle on per-call. m1 round-trips the field through the lock
  loader and respects it in the compiler's bundle-flatten step
  (C2). For m1 fixtures, the field is normally empty
  (default-bundle only).
- **L7.** Identity fields per `[plugin.<canonical-id>]`:
  `digest = "sha256:<hex>"`, `manifest_digest = "sha256:<hex>"`,
  `granted_at` (RFC 3339, parsed with `chrono::DateTime<Utc>`).
- **L8.** Canonical-id parser/formatter
  (`lock::CanonicalId::parse(&str) -> Result<Self, _>` /
  `Display`) with a stable round-trip. The form is
  `<source>:<name>@<version>` where `source` matches a
  byte-restricted grammar of `[a-z0-9._/-]+` (no shell
  metacharacters; allows `github.com/acme` / `crates.io` /
  `local`), `name` matches the topic-segment grammar
  `[a-z0-9_][a-z0-9_-]*`, and `version` is parsed via the
  `semver` crate (not re-implemented).
- **L9.** Round-trip `to_toml` / `from_toml` with deterministic
  ordering (sorted plugin keys, sorted scalar arrays where the
  RFC permits). `serde`-driven so adding a field later doesn't
  churn the formatter.

### T — topic-id derivation (`rafaello_core::topic_id`)

- **T1.** `topic_id::derive(canonical_id: &str) -> String` returning
  `id_<base32-no-pad-lower(sha256(canonical_id))[0..16]>` per
  `decisions.md` row 5 / security RFC §5.1. The 16 chars correspond
  to the leading 80 bits of the digest.
- **T2.** `topic_id::collisions(plugins: &[CanonicalId]) ->
  Result<(), CollisionError>` — checks an entire installed set
  for topic-id duplicates. The carrier is the lock-level
  validation step in V (below); the compiler refuses to compile
  any lock that fails this check.
- **T3.** Topic-id collision **testability**: the collision
  detection function is **production code** that operates on
  any `(canonical_id, computed_prefix)` pair; the test
  collision fixture passes a hand-crafted `Vec<(CanonicalId,
  String)>` directly to a lower-level
  `topic_id::collisions_with_prefixes(...)` helper that
  doesn't recompute the hash. The `derive(...)` function stays
  the only hash entry point and has no test seam (pi review-1
  finding 7-style concern: avoid feature-gated test seams).
  Tests that need a forced collision construct two distinct
  canonical ids and assert the helper rejects them when given
  identical pre-computed prefixes — the rejection logic is the
  same one production calls into.

### V — single-plugin and lock-level validation (`rafaello_core::validate`)

Two passes; APIs name every context dependency explicitly (pi-1
finding 4).

- **V1.** `validate::manifest_standalone(manifest: &Manifest) ->
  Result<()>` — per-manifest checks not requiring the canonical
  id:
  - **Pattern-vs-topic discipline** in `bus.{publishes,subscribes}`:
    publishes must be topics (no `*`/`**`), subscribes may be
    patterns. Unknown wildcards (e.g. in-segment `*` like
    `grep.*foo`) rejected.
  - **Topic grammar** segment-by-segment: only `[a-z0-9_-]+`
    per segment (and `*`/`**` permitted only in subscribe-pattern
    positions).
  - **Topic-ACL respect** in `bus.publishes` for the
    canonical-id-independent classes: rejects `core.*` and
    `frontend.*` outright.
  - **Reserved built-in renderer kinds** in `[[renderers]]` per
    M7.
  - **`provides.tool.<name>` table presence** for every name in
    `provides.tools`. A `[provides.tool.<name>]` table whose
    name is not in `provides.tools` is rejected as
    `UnknownToolTable`.
  - **Sink class** values: must be either a known class
    (`network`/`vcs_push`/`mail`/`workspace_write`/`exec`) or
    match the custom-class grammar `[a-z0-9_]+`.
  - **Lazy-load triggers** (`[load]`) cross-validated against
    declared methods/topics/kinds: a `command = ["foo"]` trigger
    referencing a tool not in `provides.tools`, or an
    `event = ["x.y"]` trigger referencing a topic not in
    `bus.subscribes`, or a `kind = ["k"]` trigger referencing
    a kind not in `[[renderers]]`, is rejected.
  - **Capability bundle keys** match `default` or a tool name
    from `provides.tools`.
- **V2.** `validate::manifest_with_id(manifest: &Manifest,
  canonical: &CanonicalId) -> Result<()>` — checks that depend
  on the canonical id:
  - **Self-namespace publish**: `plugin.<topic-id>.*` publishes
    are valid only when the `<topic-id>` matches
    `topic_id::derive(canonical.to_string())`. Foreign topic-ids
    rejected as `PublishOnForeignTopicId`.
  - **Provider namespace publish**: `provider.<id>.*` publishes
    are valid only when `<id>` matches `provides.provider`.
- **V3.** `validate::lock(lock: &Lock, ctx: &PathContext) ->
  Result<()>` — multi-plugin pass:
  - **Topic-id collision detection** across the lock entries
    (T2).
  - **Conflicting tool name detection**: any tool name
    appearing in `bindings.tools` of two distinct plugins is a
    hard error unless `[session].tool_owner.<name>` resolves
    the conflict to one specific plugin.
  - **Provider activeness consistency**: `[session].provider_active`,
    if present, must reference an installed plugin whose
    `bindings.provider == true` and whose `provider_id` is set.
  - **Trifecta refusal** per security RFC §7.1.1 — V3 calls into
    `trifecta::evaluate` per plugin (Tr below); the typed error
    is `ValidationError::TrifectaRefused` and it carries the
    `(reads_untrusted, has_outbound, has_workspace_write)`
    triple for diagnostics.
  - **Carve-out enforcement** per §7.3 — V3 calls into
    `carveout::compile_against` per plugin per active bundle; a
    carve-out failure surfaces as
    `ValidationError::CarveOutRefused` /
    `ValidationError::CarveOutTooLarge`.

### C — grant compiler (`rafaello_core::compile`)

The compiler emits a structured **plan**, not a pre-populated
builder (pi review-1 finding 1). m2's plugin supervisor consumes
the plan, starts an outpost proxy if needed, then applies the
plan onto a `lockin::SandboxBuilder` + `SandboxedCommand`.

- **C1.** Public output type:
  ```rust
  pub struct CompiledPlugin {
      pub canonical:        CanonicalId,
      pub topic_id:         String,
      pub entry_absolute:   PathBuf,
      pub filesystem:       FilesystemPlan, // read_paths/dirs, write_paths/dirs, exec_paths/dirs (all absolute)
      pub network:          NetworkPlan,    // Deny | AllowAll | Proxy { allow_hosts: Vec<String> }
      pub env:              EnvPlan,        // pass: Vec<String>, set: BTreeMap<String, String>
      pub limits:           LimitsPlan,
      pub subscribe_patterns: Vec<String>,
      pub publish_topics:     Vec<String>,
      pub auto_subscribes:    Vec<String>,  // self-subscribe: plugin.<topic-id>.tool_request
      pub tool_meta:        BTreeMap<String, ToolMeta>,
      pub provider_id:      Option<String>,
      pub flags:             CompiledFlags,
  }
  ```
  `NetworkPlan::Proxy` does **not** include a port; m2 starts
  the outpost proxy and pairs the resulting port with the
  plan's `allow_hosts` when materialising the
  `SandboxBuilder`. `EnvPlan` is populated post-scrubbing
  (§Sc) and consumed by m2's call into `lockin::config::apply_env`
  *after* `command(...)` produces the `SandboxedCommand`.
- **C2.** Public entry point:
  ```rust
  pub fn compile_plugin(
      lock:                &Lock,
      canonical:           &CanonicalId,
      ctx:                 &PathContext,
      recomputed_digests:  &RecomputedDigests,  // see D3
  ) -> Result<CompiledPlugin, CompileError>
  ```
  `Lock` carries the `active_bundles` field (L6); the compiler
  flattens `default` ∪ `active_bundles` per `decisions.md`
  row 17 (pi-1 finding 2 — no out-of-band parameter). Tests that
  want to exercise scoped-bundle behaviour set `active_bundles`
  on the fixture lock.
- **C3.** **Placeholder substitution** is invoked once per
  string field, against a `PathContext` carrying:
  `project_root`, `home`, `plugin_dir` (the installed-package
  dir for *this* plugin — m2's install layout produces
  `${plugin_root}/<source>:<name>@<version>/`), `cache_dir`,
  `state_dir`. Unknown placeholders rejected via
  `CompileError::UnknownPlaceholder`.
- **C4.** **Plan emission order is not a sequence of builder
  calls** (pi review-1 finding 1) — the plan is a value, m2
  picks the application order. The compiler does, however,
  apply a deterministic **post-flatten ordering** to scalar
  arrays inside the plan (sorted by string value, dedup) so
  test fixtures can compare plans byte-equal without relying on
  insertion order.
- **C5.** **Per-plugin private state grant** per security RFC §7.5:
  the compiler unconditionally adds
  `${PROJECT_ROOT}/.rafaello-plugin-data/<plugin-id>/` to
  `filesystem.read_dirs` and `filesystem.write_dirs` regardless
  of grant. This grant is **not** counted toward
  `has_workspace_write` for trifecta purposes (Tr4 below).
- **C6.** **Resource limit defaults** per manifest RFC §8.2 #5:
  `max_cpu_time = 300`, `max_open_files = 1024` if the lock
  omits them. Never unbounded. Manifest field `max_cpu_time =
  0` (provider plugins per RFC §9.3) is honoured verbatim and
  *not* overridden by the default — `0` means no limit, and is
  recognised at the plan stage.
- **C7.** **Reserved env-var protection** per security RFC §5.5.1
  (pi review-1 finding 10 split into the two distinct stages):
  - **Compiler stage (C7.1):** `compile_plugin` *strips*
    `RFL_BUS_FD` and `RFL_PLUGIN` from the `env.set` table and
    refuses any `env.pass` entry that literally names them
    (`CompileError::ReservedEnvVarRequested`). User-supplied
    parent values for these vars never reach the plan.
  - **Spawn stage (m2 work, not in m1 scope):** m2 injects the
    core-owned `RFL_BUS_FD` / `RFL_PLUGIN` values onto the
    `SandboxedCommand` *after* `lockin::config::apply_env`
    has applied the user `env` policy.
  m1's scrubber (Sc below) is a separate concern — it strips
  secret-pattern matches from `env.pass`. The env scrubber does
  **not** classify `RFL_BUS_FD` / `RFL_PLUGIN` as secrets;
  they are removed by C7.1 before the scrubber runs, so the
  scrubber never sees them in normal flow.

### D — digest computation (`rafaello_core::digest`)

- **D1.** `digest::content_digest(package_dir: &Path) ->
  Result<String, DigestError>` returns `sha256:<lowercase-hex>`.
  Algorithm: walk `package_dir` deterministically (files only,
  sorted by relative path normalised to `/`-separators), hash
  each file's relative path bytes (length-prefixed) followed by
  file-content bytes (length-prefixed), fold into a single
  sha256. **Symlinks** are followed for files pointing inside
  the package and refused for files pointing outside
  (`DigestError::SymlinkEscape`). Empty directories contribute
  their relative path only. File permission bits, mtimes, and
  ownership are intentionally **excluded** from the digest so
  it is reproducible across hosts. The exact algorithm is
  pinned in m1's commit body and a doc comment on
  `content_digest`.
- **D2.** `digest::manifest_digest(canonical_bytes: &[u8]) ->
  String` — sha256 of `Manifest::canonical_bytes()` from M9.
- **D3.** Compiler input:
  ```rust
  pub struct RecomputedDigests {
      pub content:  String,   // "sha256:..." from content_digest
      pub manifest: String,   // "sha256:..." from manifest_digest
  }
  ```
  `compile::compile_plugin` (C2) takes a `&RecomputedDigests`
  and refuses with `CompileError::ContentDigestMismatch` /
  `CompileError::ManifestDigestMismatch` if either does not
  match the lock's stored values. The compiler does **not**
  recompute digests itself — that's the m2 install/update
  flow's responsibility; m1 keeps the policy and the
  computation cleanly separable so tests can construct
  matched/mismatched fixtures trivially.

### G — broker ACL extraction (`rafaello_core::broker_acl`)

The compiler's secondary output: a per-session table the m2
broker consumes to authorise publishes / subscribes.

- **G1.** `broker_acl::compile(lock: &Lock) -> Result<BrokerAcl,
  CompileError>` returning, for every installed plugin:
  - `topic_id`,
  - `publish_topics: Vec<String>` (verbatim from lock),
  - `subscribe_patterns: Vec<String>` (verbatim from lock),
  - `auto_subscribes: Vec<String>` — the compiler-inserted
    `plugin.<topic-id>.tool_request` self-subscribe per
    security RFC §5.4.
  - For provider plugins, the bound `provider_id` and the
    plugin's outbound provider topics under `provider.<id>.*`.
  - For frontends in m1: nothing (TUI is m3).
- **G2.** Topic-grammar revalidation at compile time: every
  publish topic and every subscribe pattern is re-checked
  against the topic / pattern grammars before being emitted
  into the ACL — catches lock corruption / hand-edits.
- **G3.** No live broker code lands in m1; the ACL is a typed
  value the m2 broker consumes.

### Tr — trifecta refusal (`rafaello_core::trifecta`)

Per security RFC §7.1. APIs name every context dependency (pi-1
finding 4).

- **Tr1.** `trifecta::evaluate(lock: &Lock, canonical:
  &CanonicalId, ctx: &PathContext) -> TrifectaState` returning
  the booleans `(reads_untrusted, has_outbound,
  has_workspace_write)` plus `refuse: bool`. The `ctx` is
  required because Tr2 must expand placeholders before deciding
  "outside `${PROJECT_ROOT}`".
- **Tr2.** `reads_untrusted` per §7.1: any of (a)
  `network.mode != "deny"` (across the active bundle union),
  (b) `read_dirs` / `read_paths` (post-expansion) contains a
  path outside `${PROJECT_ROOT}`, (c) `subscribes` matches
  `core.session.tool_result` or `core.session.assistant_message`.
- **Tr3.** `has_outbound`: `network.mode != "deny"` OR a
  one-hop direct check — for any *other* plugin in the lock
  whose subscribe patterns match this plugin's published
  topics, where that other plugin has `network.mode != "deny"`.
- **Tr4.** `has_workspace_write`: `write_dirs` non-empty
  (across the active bundle union) **excluding** the
  per-plugin private-state subtree — this is C5's automatic
  grant added by the compiler, not a `write_dirs` entry in the
  lock; trifecta evaluation runs against the lock entries
  (pre-C5 augmentation), so the exclusion is structural rather
  than a filter.
- **Tr5.** `refuse = reads_untrusted && has_outbound &&
  has_workspace_write && !flags.i_know_what_im_doing`. The
  refusal surfaces as `ValidationError::TrifectaRefused` from
  V3.

### K — carve-out decomposition (`rafaello_core::carveout`)

Per security RFC §7.3 / §7.3.1. Pi review-1 finding 5 forced a
single, consistent rule (read = decompose for project-class /
refuse for credential-class; write = always refuse; both classes
overridable by `allow_credential_paths`).

- **K1.** `carveout::CARVE_OUTS: &[CarveOut]` is the v1 set,
  classified into two classes:
  - **Project-class (decomposable on read, refused on write):**
    - `${PROJECT_ROOT}/rafaello.lock`
    - `${PROJECT_ROOT}/.rafaello/**`
  - **Credential-class (refused on both read and write):**
    - `${HOME}/.config/rafaello/**`
    - `${HOME}/.ssh/**`
    - `${HOME}/.gnupg/**`
    - `${HOME}/.aws/**`
    - `${HOME}/.config/gh/**`
    - `${HOME}/.netrc`
- **K2.** `carveout::compile_against(grant: &GrantBundle,
  canonical: &CanonicalId, ctx: &PathContext,
  allow_credential_paths: bool) -> Result<DecomposedGrant,
  CompileError>`. The rules:
  - **Reads of project-class carve-outs**: ancestor
    `read_dirs` entries are **decomposed** into immediate
    children of the ancestor minus the carve-out name, bounded
    at 256 synthesised entries (security RFC §7.3 rule 4). Over
    the cap → `CompileError::CarveOutTooLarge`.
  - **Reads of credential-class carve-outs**: ancestor
    `read_dirs` / `read_paths` entries are **refused** with
    `CompileError::CarveOutRefused` unless
    `allow_credential_paths` is set.
  - **Writes covering any carve-out (either class)**: ancestor
    `write_dirs` / `write_paths` entries are **refused** with
    `CompileError::CarveOutRefused` unless
    `allow_credential_paths` is set. Writes are never
    decomposed in v1 — pi-1 finding 5: the security RFC's
    intent is "broad workspace writes are user error; the user
    asks for a specific subdir". The v1 rule is consistent
    across both carve-out classes for writes.
  - With `allow_credential_paths = true`, the broad grant is
    emitted verbatim (no decomposition, no refusal) and the
    flag is recorded in the compiled plan's `flags` so m2's
    `rfl status` can render the loud override (m2 work).
- **K3.** **Hidden-directory rule** (§7.3.1): the default
  workspace grant `${PROJECT_ROOT}` (when present in the
  lock's `read_dirs`) is decomposed at compile time into the
  immediate non-hidden children of the project root. The
  per-plugin private state dir (C5) is added to `read_dirs` /
  `write_dirs` separately by the compiler — *not* by the
  carve-out module.
- **K4.** Decomposition is **snapshotted into the compiled
  plan**, not a live filter. Re-decomposition on lock change
  is the m2 install/update flow's job.

### Sc — env scrubber (`rafaello_core::scrubber`)

Per security RFC §7.4.

- **Sc1.** `scrubber::SECRET_PATTERNS: &[&str]` is the v1
  glob set: `*_TOKEN`, `*_SECRET`, `*_KEY`, `*_PASSWORD`,
  `AWS_*`, `GITHUB_TOKEN`, `OPENAI_*`, `ANTHROPIC_*`.
- **Sc2.** `scrubber::strip(env_pass: &[String], i_know_what_im_doing:
  bool) -> Vec<String>` returns the scrubbed list. With the
  override flag, returns `env_pass` unchanged.
- **Sc3.** Reserved env vars `RFL_BUS_FD` and `RFL_PLUGIN`
  are **removed by the compiler in C7.1 before the scrubber
  runs** (pi review-1 finding 10). Sc tests therefore cover
  the scrubber in isolation; the C7 tests cover the
  reserved-var protection. There is no test that lists
  reserved vars in `env_pass` and then asserts they "survive
  the scrubber" (the round-1 wording was wrong; this revision
  removes that test row from the matrix).

### Si — sink-default inference (`rafaello_core::sinks`)

Per security RFC §7.2.5 (pi review-1 finding 9 — was implicit in
round 1; this revision makes it explicit).

- **Si1.** `sinks::infer_defaults(grant: &GrantBundle, declared:
  &Option<Vec<String>>) -> Vec<String>` returns the snapshotted
  sink list for a tool whose manifest omits `sinks`. Defaults
  per the security RFC table:
  - `network.mode != "deny"` → includes `"network"`,
  - `write_dirs` non-empty (excluding private state) →
    includes `"workspace_write"`,
  - both → both,
  - neither → `[]`.
  When `declared` is `Some(_)`, the declared list wins
  verbatim.
- **Si2.** The lock-level pass `validate::lock` (V3) calls
  `sinks::infer_defaults` for every entry in
  `bindings.tool_meta` whose `sinks` field is empty AND whose
  manifest provenance was "no `sinks` declared" — m1 carries a
  `bindings.tool_meta.<n>.sinks_inferred: bool` discriminator
  in the lock so the install flow (m2+) can re-prompt if the
  underlying capabilities change. Locks where `sinks_inferred =
  true` AND the inferred set differs from the snapshotted set
  are reported as `ValidationError::SinkInferenceDrift` so
  hand-edited locks are caught.

### E — typed errors (`rafaello_core::error`)

- **E1.** All m1 error types are `thiserror`-driven enums.
  Top-level `Error` (re-exported as `rafaello_core::Error`)
  unifies the module-local errors: `ManifestError`,
  `LockError`, `ValidationError`, `CompileError`,
  `DigestError`, `CarveOutError`, `TrifectaError`. Every
  variant carries enough structured context that m2's CLI
  layer can pretty-print without re-stringly-matching.

## Out of scope

- **Anything that runs.** No tokio reactor, no fittings client
  / server wiring, no `rfl` binary changes, no broker, no
  plugin spawn, no outpost proxy startup, no env application
  to `SandboxedCommand`.
- **`rfl install` / `rfl grant` / `rfl revoke` / `rfl update`
  CLI subcommands.** The lock has loaders + serialisers; the
  install/update flow that prompts for diffs and writes the
  lock is m2+. m1 fixtures construct locks programmatically.
- **Re-confirmation UX on digest mismatch.** The compiler
  flatly refuses; the diff-and-prompt flow is m2+.
- **Subprocess-renderer manifest dispatch.** Per
  `decisions.md` row 29, v1 ships only built-in renderers; m1
  rejects `[[renderers]]` entries that name a built-in kind
  (M7) and accepts plugin-prefixed kinds (`mermaid:diagram`)
  for forward-compat, but emits no dispatch wiring.
- **Helper plugins (`helper_for`, `RFL_HELPER_FD`).** Per
  `decisions.md` row 26, the field is rejected at parse time
  (M2); the v2 spec stays in security RFC §7.4.1 untouched.
- **External UDS-attached frontends.** Per `decisions.md`
  row 27.
- **Streaming entry patch ops.** Per `decisions.md` row 28.
- **Full openrpc.json validation.** m1 only checks the
  sibling file's presence (M10).
- **Stream F RFC body edits.** The §15.1 normative-delta
  items 1–4 + the security RFC's `requires_confirmation` →
  `always_confirm` rename + helper / external-attach drift
  are documented in `overview.md` and `decisions.md`; m1
  retrospective patches the RFCs per `plans/milestones/README.md`
  §"Stream RFC drift".
- **`FittingsError::MethodNotFound { method: Option<String> }`
  cutover.** `decisions.md` row 36 lists this as an m1
  follow-through. Driver call at `commits.md` ratification:
  default is to include as a single trailing fittings commit
  in m1's commits.md; bail to m2 only if the size analysis
  shows it's non-trivial.
- **Capsa backend.** Manifest RFC §10 stays paper-only.
- **Network proxy startup, port assignment, outpost wiring.**
  m1's `NetworkPlan::Proxy` records the `allow_hosts` list;
  m2's supervisor calls `outpost::NetworkPolicy::from_allowed_hosts`
  + starts the proxy + calls `SandboxBuilder::network_proxy(port)`.

## Demo bar

Per the milestones README m1 row, the demo bar is `cargo test
-p rafaello-core` integration coverage. The matrices below name
every required test file. Tests may be split or merged during
`commits.md` drafting as long as every named behaviour is
exercised at least once.

> **Note on fixture provenance.** Where the test description says
> "RFC §9.x-shaped manifest", the fixture is a **post-simplification
> rewrite** of the RFC example: `runtime` removed (row 30), `[rpc]`
> replaced by an `openrpc.json` sibling (row 31), `${secret:...}`
> in `env.set` removed (closed-set placeholders only — M8), the
> required `rafaello = ">=0.1, <0.2"` field added if the RFC
> example omitted it. The RFC body itself stays as the historical
> artefact (per `plans/README.md` §"Authoring conventions");
> patching the RFC to the post-row-30/31/32 schema is m1
> retrospective work. Pi review-1 finding 7 forced this clarification.

### Positive integration tests in `rafaello/crates/rafaello-core/tests/`

| Test file | Exercises |
|-----------|-----------|
| `manifest_parse_minimal.rs` | Smallest valid manifest (`schema`, `name`, `version`, `entry`, `rafaello`, empty `[provides]`, no capabilities) round-trips through `Manifest::parse` and `Manifest::canonical_bytes`. |
| `manifest_parse_tool_example.rs` | Post-simplification rewrite of RFC §9.1 (`rust-tools`-shaped) decodes; `provides.tools = ["format", "check"]` (single-segment names per pi-1 finding 3); scoped `[capabilities.format.filesystem]` (bundle key matches a tool name); `[load]` table; `[bus]` populate the typed structs. |
| `manifest_parse_provider_example.rs` | Post-simplification rewrite of RFC §9.3 (`anthropic`-shaped) decodes; `provides.provider = "anthropic"`; `eager` load string; `[capabilities.default.network]` with `proxy` mode + `allow_hosts`; `env.set` keys are literal strings only (no `${secret:...}`). |
| `manifest_parse_renderer_example.rs` | Post-simplification rewrite of RFC §9.2 decodes; `[[renderers]]` list of two non-built-in kinds (`mermaid:diagram`, `code.diff`); registration accepted. |
| `manifest_canonical_bytes_stable.rs` | `Manifest::canonical_bytes` is byte-stable across two parses of the same TOML re-emitted with key reordering and trivial whitespace differences. |
| `manifest_placeholder_expansion.rs` | All five placeholders expand against a hand-built `PathContext`; deeply-nested mixes (e.g. `${project}/sub/${plugin}/foo`) resolve correctly. |
| `manifest_validate_load_trigger_cross_refs.rs` | `[load]` table referencing only declared methods/topics/kinds passes `validate::manifest_standalone`. |
| `manifest_openrpc_sibling_present.rs` | `validate_with_package` succeeds against a fixture directory containing a non-empty `openrpc.json` next to the manifest. |
| `lock_parse_round_trip.rs` | Worked-example lock (mirroring security RFC §3.2 + L2/L3/L6's expansions for `entry`, `grant.bundles`, `active_bundles`, `[session].tool_owner`) parses, serialises, parses again byte-equal. Includes `[session].provider_active`, `.flags`, `bindings.tool_meta`. |
| `lock_canonical_id_round_trip.rs` | `CanonicalId::parse("github:acme/grep@1.4.2").to_string() == input` over a small grammar matrix (different `source` shapes, semver pre-release / build metadata). |
| `topic_id_derivation.rs` | Deterministic for a fixed input: e.g. `derive("github:acme/grep@1.4.2") == "id_<known-prefix>"`; the test pins the expected first 16 chars of base32-no-pad-lowercase against a hand-computed fixture. |
| `topic_id_collision_detection.rs` | Pre-computed-prefix helper (`collisions_with_prefixes`) rejects two distinct canonical ids paired with identical prefixes (no test seam in `derive` itself; T3). |
| `compile_default_bundle.rs` | Worked lock entry with `default` bundle compiles; resulting `CompiledPlugin` carries the expected `FilesystemPlan`, `NetworkPlan`, `EnvPlan`, `LimitsPlan` (post-default-fill), `entry_absolute`, `topic_id`, etc. Asserts on the structured plan, not on builder calls. |
| `compile_scoped_bundle_union.rs` | Lock specifies `active_bundles = ["format"]`; resulting plan reflects the union of `default` + the `format` bundle (default's reads + format's writes); duplicate entries dedup; ordering deterministic per C4. |
| `compile_placeholder_resolves_to_absolute.rs` | All paths in the compiled plan are absolute; `${...}` is fully expanded. |
| `compile_private_state_grant.rs` | Compiled plan contains `${PROJECT_ROOT}/.rafaello-plugin-data/<canonical-id>/` (after expansion to absolute) in both `read_dirs` and `write_dirs` regardless of whether the lock requested it. |
| `compile_private_state_excluded_from_workspace_write.rs` | Trifecta evaluation against a lock whose only writes would be the private-state dir reports `has_workspace_write == false` (Tr4 — the structural exclusion). |
| `compile_resource_limit_defaults.rs` | Lock omits `limits`; compiled plan carries `max_cpu_time = 300`, `max_open_files = 1024`. A lock that explicitly sets `max_cpu_time = 0` (provider plugin shape) preserves the `0` verbatim. |
| `compile_network_proxy_plan.rs` | Lock specifies `network.mode = "proxy"`, `allow_hosts = ["api.example.com", "*.example.com"]`; compiled `NetworkPlan::Proxy { allow_hosts }` records the host list verbatim (m2 starts the outpost proxy and supplies the port; m1 only emits the plan). |
| `compile_env_set_passes_through.rs` | Lock's `env.set = { CARGO_TERM_COLOR = "always" }` reaches `EnvPlan.set` verbatim; reserved keys (`RFL_BUS_FD`, `RFL_PLUGIN`) in `env.set` are stripped by C7.1 with a typed error. |
| `compile_digest_match.rs` | `RecomputedDigests` matches the lock's stored digests; compile succeeds; mismatched values are exercised in the negative matrix below. |
| `broker_acl_extraction.rs` | Two-plugin lock; `broker_acl::compile` emits per-plugin `publish_topics`, `subscribe_patterns`, the auto-inserted `plugin.<topic-id>.tool_request` self-subscribe, and the bound provider id (for the provider plugin). |
| `carveout_default_workspace_decomposition.rs` | `read_dirs = ["${PROJECT_ROOT}"]` decomposes to immediate non-hidden children of a fixture project root (K3); the per-plugin private state dir is added separately by C5 (also asserted via the `compile_private_state_grant.rs` test). |
| `carveout_workspace_excludes_rafaello_dot_dirs.rs` | `read_dirs = ["${PROJECT_ROOT}"]` against a project that contains `.rafaello/` decomposes around the carve-out (no entry covering `.rafaello/`). |
| `digest_match_compiles.rs` | Lock's `digest` matches recomputed `content_digest`; lock's `manifest_digest` matches recomputed `manifest_digest`; compile succeeds. |
| `digest_content_deterministic.rs` | Two invocations of `content_digest` against the same fixture tree return the same value; constructing the tree in a different order doesn't change the digest. |
| `trifecta_two_plugins_one_hop.rs` | Plugin A has workspace_write + reads_untrusted but `network.mode = "deny"`; plugin B subscribes to A's published topic and has `network.mode = "proxy"`. A's `has_outbound` evaluates true via the one-hop check; combined with A's other booleans, the trifecta refusal fires. |
| `trifecta_iknowwhatimdoing_bypass.rs` | Same fixture; `flags.i_know_what_im_doing = true`; trifecta refusal suppressed; compile succeeds. |
| `sinks_infer_defaults.rs` | Tools whose manifest omits `sinks`: a write-only grant infers `["workspace_write"]`; a network-only grant infers `["network"]`; both → both classes; neither → `[]` (Si1). |
| `env_scrubber_strips_known_secrets.rs` | `env_pass = ["GITHUB_TOKEN", "OPENAI_API_KEY", "AWS_REGION", "PATH"]`; scrubbed list contains only `["PATH"]` (`GITHUB_TOKEN` matches literally; `OPENAI_API_KEY` matches `OPENAI_*`; `AWS_REGION` matches `AWS_*`). |

### Negative integration tests in `rafaello/crates/rafaello-core/tests/`

| Test file | Asserts |
|-----------|---------|
| `manifest_unknown_field.rs` | A manifest with a top-level `purpose = "foo"` is rejected as `ManifestError::UnknownField`; same for an unknown nested key under `[provides]`. |
| `manifest_legacy_runtime_field.rs` | `runtime = "subprocess"` rejected as `ManifestError::ReservedField { field: "runtime" }` per `decisions.md` row 30. |
| `manifest_legacy_rpc_block.rs` | `[rpc] openrpc = "x"` rejected as `ManifestError::ReservedField { field: "rpc" }` per `decisions.md` row 31. |
| `manifest_helper_for_field.rs` | Top-level `helper_for = "..."` rejected as `ManifestError::ReservedField` per `decisions.md` row 26. |
| `manifest_publishes_core_topic.rs` | `bus.publishes = ["core.session.tool_result"]` rejected as `ValidationError::PublishOnReservedNamespace` (caught by V1 — canonical-id-independent). |
| `manifest_publishes_other_plugin_namespace.rs` | `bus.publishes = ["plugin.id_aaaaaaaaaaaaaaaa.foo"]` from a plugin whose own topic-id resolves to a different prefix is rejected by `validate::manifest_with_id` (V2 — needs the canonical id). |
| `manifest_publishes_frontend_topic.rs` | `bus.publishes = ["frontend.tui.user_message"]` from a non-frontend plugin rejected as `ValidationError::PublishOnFrontendNamespace`. |
| `manifest_provider_namespace_mismatch.rs` | A plugin declaring `provides.provider = "anthropic"` but listing `bus.publishes = ["provider.openai.tool_request"]` rejected as `ValidationError::ProviderNamespaceMismatch` (V2). |
| `manifest_publish_with_wildcard.rs` | `bus.publishes = ["plugin.id_xxxx.*"]` rejected as `ValidationError::PatternInPublishPosition`. |
| `manifest_subscribe_invalid_pattern.rs` | `bus.subscribes = ["core.**.tool_*"]` (in-segment `*`) rejected as `ValidationError::InvalidPatternSegment`. |
| `manifest_topic_segment_grammar.rs` | A topic / pattern segment violating `[a-z0-9_-]+` (uppercase, dot inside segment, slash) rejected as `ValidationError::IllegalTopicSegment`. |
| `manifest_dotted_tool_name.rs` | `provides.tools = ["rust.format"]` rejected as `ValidationError::IllegalToolName` (V1 — tool names are single segments per pi-1 finding 3). |
| `manifest_unknown_tool_table.rs` | `provides.tools = ["grep"]` but a `[provides.tool.gerp]` table (typo) → `ValidationError::UnknownToolTable`. |
| `manifest_unknown_bundle_key.rs` | `[capabilities.foo.filesystem]` for a bundle key not in `provides.tools` (and not `default`) → `ValidationError::UnknownBundleKey`. |
| `manifest_malformed_sinks.rs` | `[provides.tool.foo] sinks = [42]` (non-string), and `sinks = ["Network"]` (uppercase, fails the `[a-z0-9_]+` custom-class grammar): both rejected. |
| `manifest_reserved_renderer_kind.rs` | `[[renderers]] kind = "text"` rejected per M7 (built-ins reserved). |
| `manifest_load_trigger_unknown_command.rs` | `[load] command = ["does_not_exist"]` rejected as `ValidationError::LoadTriggerUnknownCommand`. |
| `manifest_missing_openrpc_sibling.rs` | `provides.tools = ["grep"]` but no `openrpc.json` next to the manifest → `ManifestError::MissingOpenRpc`. |
| `lock_unknown_field.rs` | A lock with an unknown field under `.bindings` is rejected (`deny_unknown_fields`). |
| `lock_canonical_id_invalid.rs` | `[plugin."github:acme/Grep@1.4"]` (uppercase in `name`, missing patch) rejected by `CanonicalId::parse`. |
| `lock_helper_field_rejected.rs` | A lock containing `bindings.helpers = [...]` or `bindings.helper_for = "..."` is rejected (deferred per row 26 — both directions of the helper relationship). |
| `lock_provider_active_unknown.rs` | `[session].provider_active` referencing a plugin not present in the lock → `ValidationError::ProviderActiveUnknown`. |
| `lock_provider_active_not_provider.rs` | `[session].provider_active` references an installed plugin whose `bindings.provider == false` → `ValidationError::ProviderActiveNotProvider`. |
| `lock_conflicting_tool_names.rs` | Two installed plugins both listing `"grep"` in `bindings.tools` without a `[session].tool_owner.grep` decision → `ValidationError::ConflictingToolName`. A second variant with `[session].tool_owner.grep = "<plugin-A>"` resolves the conflict; only the named plugin gets the routing. |
| `lock_missing_entry.rs` | Lock entry without `entry` field → `LockError::MissingEntry` (per L2 — required for the compiler's `entry_absolute`). |
| `lock_active_bundle_unknown.rs` | `active_bundles = ["nope"]` referencing a bundle absent from `grant.bundles` → `ValidationError::ActiveBundleUnknown`. |
| `topic_id_collision_at_lock.rs` | Two distinct canonical ids whose pre-computed prefixes (passed via `collisions_with_prefixes`) match → `CollisionError`. |
| `digest_content_mismatch.rs` | Lock's `digest` field doesn't match the recomputed `content_digest`; `compile::compile_plugin` returns `CompileError::ContentDigestMismatch`. |
| `digest_manifest_mismatch.rs` | Lock's `manifest_digest` doesn't match recomputed value; `CompileError::ManifestDigestMismatch`. |
| `digest_symlink_escape.rs` | A symlink in the package directory pointing outside the package root → `DigestError::SymlinkEscape`. |
| `carveout_credential_path_refused_read.rs` | `read_dirs = ["${HOME}"]` covering `~/.ssh` with `allow_credential_paths = false` → `CompileError::CarveOutRefused` (K2 — credential class refuses on read). |
| `carveout_credential_path_refused_write.rs` | `write_dirs = ["${HOME}"]` covering `~/.ssh` with `allow_credential_paths = false` → `CompileError::CarveOutRefused`. |
| `carveout_project_write_refused.rs` | `write_dirs = ["${PROJECT_ROOT}"]` (would cover `rafaello.lock` and `.rafaello/`) with `allow_credential_paths = false` → `CompileError::CarveOutRefused` (K2 — project-class also refuses on write; pi-1 finding 5 forced this consistency). |
| `carveout_credential_path_override.rs` | The two `_refused_*` variants above with `allow_credential_paths = true` compile; resulting plan records the broad grants verbatim and surfaces the loud-override flag. |
| `carveout_decomposition_blowup.rs` | A project root containing 300 immediate children plus `.rafaello/`; `read_dirs = ["${PROJECT_ROOT}"]` decomposition exceeds the 256-entry cap → `CompileError::CarveOutTooLarge`. |
| `carveout_lockfile_path_explicit.rs` | A request that directly names `read_paths = ["${PROJECT_ROOT}/rafaello.lock"]` (project-class) is decomposed-around → in this case the path *is* the carve-out, so the only safe behaviour is "remove the entry from the plan" — m1 implementation: explicit hits on a project-class carve-out leaf are silently dropped from the plan with the drop counted in the compiler's `dropped_carveouts: Vec<PathBuf>` diagnostic field (the test asserts the field is populated). For credential class, an explicit leaf hit is refused with `CarveOutRefused`. |
| `compile_unknown_placeholder.rs` | A grant containing `${nope}` rejected as `CompileError::UnknownPlaceholder`. |
| `compile_reserved_env_in_pass.rs` | `env.pass = ["RFL_BUS_FD"]` → `CompileError::ReservedEnvVarRequested` (C7.1). |
| `compile_reserved_env_in_set.rs` | `env.set = { RFL_PLUGIN = "..." }` → `CompileError::ReservedEnvVarRequested` (C7.1). |
| `env_scrubber_strips_secret_globs.rs` | `env_pass = ["GITHUB_TOKEN", "OPENAI_API_KEY", "MY_PASSWORD", "AWS_PROFILE"]` are all stripped in the no-override path. |
| `env_scrubber_override.rs` | With `flags.i_know_what_im_doing = true`, the same `env_pass` survives the strip verbatim. |
| `sinks_inference_drift.rs` | A lock entry with `tool_meta.<n>.sinks_inferred = true` but a `sinks` snapshot that no longer matches the current grant inference → `ValidationError::SinkInferenceDrift` (Si2). |

### Manual validation in `manual-validation.md`

The driver runs and captures:

- `cargo test -p rafaello-core` green on Linux (CI rerun for
  macOS — m1 has no platform-specific code, so the macOS leg
  is delegated to CI per the m0 precedent).
- `cargo doc -p rafaello-core --no-deps` clean — the m1 surface
  is the future m2 consumer's API; doc warnings would surface
  drift between scope §C / §G / §V and the landed types.
- `cargo test -p rafaello-core --release` green (digest-module
  performance and carve-out worst-case behaviour).
- `nix develop --impure -L --command cargo test -p
  rafaello-core` green (per the m0 retrospective gotcha §4.6 —
  `--impure` is load-bearing for `nix develop` invocations).
- A `tree rafaello/crates/rafaello-core/tests/fixtures` dump
  capturing the manifest+lock+openrpc fixture surface.

## Risks

1. **Lockin's `command(...)` consumes the builder.** Resolved
   in C1: m1 emits a structured plan, not a pre-populated
   builder. m2 calls `command(entry)` (and applies env after).
   Tests assert on the plan value directly — no recording-ops
   shim, no upstream lockin patch. (Pi review-1 finding 1.)

2. **Outpost proxy startup is m2's job.** m1's `NetworkPlan::Proxy
   { allow_hosts }` does not start the proxy; m2 does. The risk
   is that m1's `allow_hosts` is malformed in a way `outpost`
   later rejects. Mitigation: m1 calls
   `outpost::NetworkPolicy::from_allowed_hosts(...)` at compile
   time **as a dry-run validation** and returns
   `CompileError::InvalidAllowHosts` if the parse fails. The
   parsed `NetworkPolicy` itself is discarded (m2 will recompute
   it at spawn). This adds an `outpost` dep transitively (via
   `lockin-config`); m1 does not import it directly.

3. **Deterministic content digest across hosts.** The algorithm
   in §D1 commits to a specific normalisation. The test
   `digest_content_deterministic.rs` exercises a fixture
   constructed twice from different starting orders. If a CI
   host (e.g. macOS) produces a different hash, that's a real
   bug to fix in the walker, not a test allowance.

4. **Topic-id collision testing without a hash seam.** §T3
   commits to a `collisions_with_prefixes(...)` helper that
   takes pre-computed prefixes — production code is the same
   helper, called from `collisions(...)` after computing
   prefixes via `derive(...)`. No `#[cfg(test)]` boundary; the
   public surface is `derive` + `collisions`, the internal
   helper is `pub(crate)` and reachable from integration tests
   via a small `pub fn collisions_with_prefixes` re-export
   under the `topic_id` module. (Pi review-1 finding 7 wanted
   the seam removed; this revision lets the helper be the
   stable public surface.)

5. **Validation rule completeness vs. surface size.** The
   manifest validation surface is large (V1, V2, V3). The risk
   is that a real-world manifest the m2 driver brings in trips
   a rule m1 didn't think to write. Mitigation: every test in
   the negative matrix above is named after a real-world shape
   from security RFC §6 / manifest RFC §9; if the m2 driver
   brings in a manifest that trips an *unstated* rule, that's
   m1 retrospective territory (drift to add to the validator),
   not an m1 acceptance gap.

6. **`--allow-credential-paths` lock flag plumbing.** The
   security RFC's "loud override" surfaces in `rfl status`,
   which is m2+. m1 only rounds-trips the flag through the
   lock and respects it in the carve-out compiler. The
   override is set only by the m2+ install/grant flow, never
   by the runtime, so its silence in m1 has no security
   consequence.

7. **`MethodNotFound` typed-method-field cutover (row 36).**
   Driver call at `commits.md` ratification time. Default:
   include the cutover as a single trailing commit in m1's
   commits.md after the `rafaello-core` work lands; bail to
   m2 only if the size analysis shows non-trivial.

8. **No external consumers yet.** `rafaello-core` is brand
   new. Per `decisions.md` row 33, it lives under
   `rafaello-v0.1`, so breakage risk to existing crates is
   bounded. The `rfl` binary's `main.rs` does not import
   `rafaello-core` in m1; the workspace only has to keep
   compiling.

## Internal split (driver guidance for `commits.md`)

Per `milestones/README.md`, m1 may split internally by module
group. Suggested grouping for `commits.md`; the driver picks
final granularity and surfaces an m1a/m1b split for owner
approval as soon as it becomes clear the groups cannot be kept
independently green.

1. **Crate skeleton + workspace-deps + manifest types and parser**
   (S1–S3, M1–M9): `rafaello-core` lib lands; manifest types
   compile; basic decoder tests including the reserved-field
   pre-scan. Self-contained; ~5–7 commits.
2. **Manifest validate-with-package + openrpc sibling** (M10):
   ~2 commits.
3. **Lock types + canonical id + bundle-aware grant + entry +
   active_bundles + tool_owner + round-trip** (L1–L9):
   ~4–5 commits.
4. **Topic-id derivation + collision helper** (T1–T3):
   ~2 commits.
5. **Single-plugin validation** (V1, V2): ~3–4 commits.
6. **Digest module** (D1–D2 + the canonical-bytes wiring left
   from M9): ~2–3 commits.
7. **Sink default inference** (Si1): ~1–2 commits.
8. **Trifecta refusal** (Tr1–Tr5): ~2–3 commits.
9. **Carve-out decomposition + override + project-class /
   credential-class split** (K1–K4): ~3–4 commits.
10. **Env scrubber + reserved-env C7.1 protection**
    (Sc1–Sc3 + the env half of C7): ~2–3 commits.
11. **Cross-plugin lock validation** (V3 — wires up Tr +
    carveout + sink-drift + topic-id collision + tool-owner):
    ~2 commits.
12. **Compiler core: plan emission, placeholder substitution,
    bundle flatten, private state, limit defaults, network
    plan, env plan, digest gating** (C1–C7 minus the env half
    landed in step 10): ~5–7 commits.
13. **Broker ACL extraction** (G1–G3): ~2 commits.
14. **`MethodNotFound` typed-method-field fittings cutover**
    (row 36): one commit if it stays this milestone; spawned
    only if Risks §7 resolves "include".
15. **`manual-validation.md`** (one commit at the end).

Realistic total: **~35–45 commits, sequential.** The natural
split point for an m1a/m1b cut is **after group 7** (sink
default inference / digests landed) — m1a ships parsers + lock
+ digests + topic-id, m1b ships compiler + carve-out + trifecta
+ broker ACL. m1a has no v1 consumer; m1b is the consumer-facing
piece. Default: ship m1 as one milestone, no split. Surface a
split for owner approval if group 12 (compiler core) cannot
land green on top of groups 1–11.

## Acceptance summary

m1 is done when:

- Every named test in the *Positive integration tests* and
  *Negative integration tests* matrices above is implemented
  and passes. Tests may split or merge during `commits.md`
  drafting as long as the named behaviours are all covered.
- `cargo test -p rafaello-core` is green on Linux; CI rerun
  is the authoritative cross-platform signal for macOS.
- `cargo doc -p rafaello-core --no-deps` is warning-free.
- `manual-validation.md` records the items in the *Manual
  validation* section above.
- `retrospective.md` is written, with any drift surfaced
  during implementation landing in `overview.md` /
  `decisions.md` / stream RFCs as deltas. m1 retrospective
  specifically owns the Stream F drift items pinned in
  `milestones/README.md` §"Stream RFC drift" (the §15.1
  normative-delta items 1–4 plus the security RFC's
  `requires_confirmation` → `always_confirm` rename, helper
  / external-attach drift).

## What changed from the first draft

Round-1 pi review (`pi-review-1.md`) prompted these revisions:

- **Compile output is a structured plan, not a pre-populated
  `SandboxBuilder`** (pi finding 1). C1 redefines the public
  output as `CompiledPlugin { filesystem, network, env, limits,
  ... }`. m2 owns `command(...)`, env application, and outpost
  proxy startup. `compile_default_bundle.rs` and the other
  compile tests assert on the plan value directly.
- **Lock schema expanded** (pi finding 2):
  - L2 adds `entry: PathBuf` (compiler reads this for
    `entry_absolute`).
  - L3 reshapes `.grant` as `BTreeMap<BundleKey, GrantBundle>`
    so scoped bundles round-trip.
  - L6 adds `active_bundles: Vec<String>` to drive C2's
    bundle flatten.
  - L1 adds `[session].tool_owner: BTreeMap<String, String>`
    for V3's conflicting-tool-name resolution.
  - C2 takes `RecomputedDigests` (D3) explicitly; no
    out-of-band digest input.
- **Tool / method / bundle naming pinned** (pi finding 3).
  Tool names are single segments (`[a-z0-9_][a-z0-9_-]*`);
  bundle keys are `default` or a tool name. Worked-example
  fixtures use `format` not `rust.format`. New negative tests
  `manifest_dotted_tool_name.rs` and `manifest_unknown_bundle_key.rs`.
- **APIs name every context dependency** (pi finding 4).
  `trifecta::evaluate(lock, canonical, ctx)` takes
  `PathContext`; `validate::manifest_with_id(manifest,
  canonical)` is the explicit lock-bound validator;
  `validate::lock(lock, ctx)` documents the multi-plugin
  surface.
- **Carve-out rule pinned to a single policy** (pi finding 5).
  Project-class: read decomposes, write refuses. Credential-
  class: both refuse. Both classes overridable by
  `allow_credential_paths`. New negative tests
  `carveout_project_write_refused.rs` and the matched read /
  write split for credential-class.
- **Workspace dependency policy realistic** (pi finding 6).
  S2 introduces a `[workspace.dependencies]` table and lists
  every required crate (`semver`, `chrono`, `tempfile`,
  `lockin-config`, etc.). Earlier "no new deps" wording dropped.
- **Worked-example fixtures called out as post-simplification
  rewrites** (pi finding 7). Note added above the test matrix.
  Env scrubber positive row corrected: `AWS_REGION` is
  stripped (matches `AWS_*`); only `PATH` survives.
- **Reserved-field parsing strategy explicit** (pi finding 8).
  M2 specifies a TOML pre-scan ahead of serde so
  `runtime`/`[rpc]`/`helper_for` produce typed
  `ReservedField` errors with deferral hints rather than
  generic `UnknownField`.
- **Sink-default inference is in scope** (pi finding 9). New
  module `sinks` (Si1, Si2) inferring sink lists per security
  RFC §7.2.5 and detecting drift on hand-edited locks.
  `sinks_infer_defaults.rs` and `sinks_inference_drift.rs`
  added to the test matrix.
- **Reserved-env-var protection split into two stages** (pi
  finding 10). Compiler-stage protection (C7.1) refuses
  `RFL_BUS_FD` / `RFL_PLUGIN` in `env.pass` / `env.set`;
  scrubber (Sc) operates after that and never sees reserved
  vars. Round-1's "reserved vars survive the scrubber" test
  removed; replaced by `compile_reserved_env_in_pass.rs` and
  `compile_reserved_env_in_set.rs`.
- **Topic-id test seam removed** (pi finding 7's spirit,
  Risks §4). The `collisions_with_prefixes` helper is the
  stable internal surface; both production `collisions` and
  the integration tests call into it. No `#[cfg(test)]` /
  `feature = "test-seam"` boundary.
- **Internal split renumbered** to reflect the new module
  layout and the additional sinks / reserved-env / network-plan
  work; total estimate widened to 35–45 commits.
