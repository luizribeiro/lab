# m1 — manifest / lock / grant / compiler foundation — scope

> **Status:** draft (round 1). Pi review pending. Once owner-ratified
> the per-commit work begins on the `rafaello-v0.1` branch under the
> `agents/m1/c<NN>` agent branches; see `driver-notes.md` (written
> after `commits.md` ratifies).

## Goal

Land the **paper-first data-transformation** layer that takes a
plugin manifest plus a user grant and produces a `lockin::Sandbox`
builder ready to spawn — without yet wiring up any spawn, broker,
or CLI. m1 is intentionally pure: every output is computable from
its inputs; every test is a function-level assertion. m2 is the
first consumer (it spawns subprocess plugins from m1's compile
output).

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
- `rafaello/plans/glossary.md` (canonical definitions for
  manifest / lock / grant / lockin / topic-id / carve-out / private
  state / sink / trifecta / user grant).
- Lockin's public Rust API for the v1 sandbox backend
  (`/home/luiz/lab/lockin/crates/sandbox/src/lib.rs`):
  `lockin::Sandbox::builder()` produces a `SandboxBuilder` with
  `read_path` / `read_dir` / `write_path` / `write_dir` /
  `exec_path` / `exec_dir` / `network_*` / `inherit_fd` /
  `max_*` / `command(...)`. **Decision row 32** binds m1 to
  emitting calls against this builder, not a `lockin.toml`
  artifact.

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
- **S2.** Dependencies pinned via the workspace `Cargo.toml`
  (no new top-level deps the workspace doesn't already use).
  Expected set: `serde`, `serde_derive`, `toml`, `serde_json`,
  `sha2`, `data-encoding` (for base32), `thiserror`, the local
  `lockin` crate (path dep, no version pin yet — both crates
  evolve together inside the monorepo). Not used in m1: `tokio`,
  `fittings-*`, `tracing` (m2 brings tokio + fittings;
  observability hooks are m2+).
- **S3.** No public API leaks from `rafaello-core` outside the
  modules named in M / L / T / C / D / V / E below. The
  `rafaello`-bin entry point (`crates/rafaello/src/main.rs`)
  is unchanged in m1.

### M — manifest schema and parser (`rafaello_core::manifest`)

Parse `rafaello.toml` into a typed `Manifest` value. The schema
matches Stream F **post-simplifications**: no `runtime`, no
`[rpc]`, no `helper_for`. The `[provides]` block follows the
§15.1 normative delta in `overview.md`.

- **M1.** Top-level required fields: `schema = 1`, `name`,
  `version` (semver), `entry` (path inside the package),
  `rafaello = ">=0.1, <0.2"` (semver req). Optional metadata:
  `description`, `authors`, `license`, `homepage`. `serde`'s
  `deny_unknown_fields` is set on every struct — unknown TOML
  keys are a typed parse error, not silently ignored.
- **M2.** **`runtime` field rejected** (`decisions.md` row 30 —
  v1 omits the field; spawn defaults to subprocess+lockin).
  Manifest decoders that encounter `runtime = "..."` produce
  `ManifestError::ReservedField { field: "runtime" }`.
- **M3.** **`[rpc]` block rejected** (`decisions.md` row 31 —
  v1 references an `openrpc.json` sibling). Decoders that
  encounter `[rpc]` produce `ManifestError::ReservedField`.
- **M4.** **`helper_for` field rejected** (`decisions.md` row
  26 — helper plugins deferred to v2).
- **M5.** `[provides]` block per §15.1 item 1:
  - `tools: Vec<String>` (zero or more tool names; segment
    grammar — `[a-z0-9_-]+` per topic grammar §5.1, since tool
    names appear in `bindings.tool_meta.<name>` and in the
    `topic-id` namespace as a payload field, but the strict
    constraint is "no whitespace, no quoting hazards"; v1
    enforces the topic-segment grammar to keep things uniform).
  - `provider: Option<String>` (zero or one provider id;
    same segment grammar — provider id appears literally
    in `provider.<id>.*` topics).
  - `[provides.tool.<name>]` table per declared tool:
    - `sinks: Vec<String>` — known classes `"network"`,
      `"vcs_push"`, `"mail"`, `"workspace_write"`, `"exec"`;
      custom strings allowed but lower-snake-case-validated.
    - `grant_match: Option<PathBuf>` — relative path to a
      JSON-Schema describing the `user_grants` matcher.
      Absent → "any invocation" matcher; the schema file
      itself is **not** parsed in m1 (its consumer is m5);
      v1 only validates the path syntax + that the file
      exists at install time.
    - `always_confirm: bool` — UX gate per §15.1 item 1;
      defaults `false`.
- **M6.** `[bus]` block:
  - `subscribes: Vec<String>` — list of subscribe **patterns**
    drawn from the §5.1 pattern grammar (`*` / `**` allowed;
    `**` only as final segment).
  - `publishes: Vec<String>` — list of **topics** (no
    wildcards; pattern in publish position is rejected).
- **M7.** `[capabilities]` block, with the bundle-and-section
  grammar from manifest RFC §5: `[capabilities.<bundle>.{filesystem,network,env,limits}]`.
  Bundle name `default` plus zero or more named bundles. Each
  section is a typed inner struct with the fields the manifest
  RFC enumerates (`read_paths`, `read_dirs`, `write_paths`,
  `write_dirs`, `exec_paths`, `exec_dirs`, `network.mode`,
  `network.allow_hosts`, `env.pass`, `env.set`,
  `limits.max_cpu_time`, `limits.max_open_files`,
  `limits.max_address_space`, `limits.max_processes`).
- **M8.** `[load]` block per manifest RFC §7.2. Either a string
  (`"eager"` / `"boot"` / `"manual"` / `"lazy"` sugar) or a
  table with one or more of `event` / `command` / `kind`.
- **M9.** `[[renderers]]` array per manifest RFC §6: `kind`,
  optional `priority` (default 100), optional `method`. The
  built-in v1 renderers (`text`, `code_block`, `tool_call`,
  `tool_result`, `error`, `heading`, `thinking`, `image`) are
  reserved and **rejected** if a plugin tries to register them
  — built-in registration is hard-coded in core (m3 territory)
  and `decisions.md` row 29 defers subprocess plugin renderers
  to v2 anyway.
- **M10.** Placeholder substitution table per manifest RFC §5.1,
  exposed as `manifest::placeholders::expand(input: &str,
  ctx: &PathContext) -> Result<String, ManifestError>`.
  Closed set: `${project}`, `${home}`, `${plugin}`, `${cache}`,
  `${state}`. No env-var interpolation, no `${secret:...}` (the
  secrets sigil is open question #4 in the manifest RFC and is
  not landing in m1).
- **M11.** Canonical-form normalisation: a `Manifest::canonical_bytes()`
  method emits a deterministic byte representation used for
  hashing the manifest snapshot at install time (§D2). The
  canonicalisation is "TOML re-emit with sorted keys at every
  table level" — no semantic transforms.
- **M12.** **`openrpc.json` sibling presence**, per
  `decisions.md` row 31 — checked by
  `manifest::validate_with_package(manifest_path, package_dir)`.
  If `provides.tools` is non-empty, the validator requires
  `<package_dir>/openrpc.json` to exist as a regular file.
  Absent (or unreadable) → `ManifestError::MissingOpenRpc`.
  v1 does **not** parse openrpc.json; that contract belongs to
  fittings + m2's spawn path. The validator only confirms the
  sibling file is present.

### L — lock schema (`rafaello_core::lock`)

Mirror of the manifest plus the bindings + grant view per security
RFC §3.2. Consumed by m1's compiler; mutation API stays minimal
(loaders + serialisers, no install/grant/revoke flows yet).

- **L1.** Top-level shape:
  - Per-plugin tables keyed by canonical id `<source>:<name>@<version>`,
    e.g. `[plugin."github:acme/grep@1.4.2"]`.
  - Per-plugin sub-tables `.grant`, `.bindings`, `.flags`.
  - `[session]` table holding `provider_active = "<plugin-id>"`
    (string or absent — absent means no active provider) per
    security RFC §3.2.
- **L2.** `.grant` fields:
  - `read_paths`, `read_dirs`, `write_paths`, `write_dirs`,
    `exec_paths`, `exec_dirs` — strings, placeholder-allowed.
  - `network: { mode: "deny"|"proxy"|"allow_all", allow_hosts:
    Vec<String> }`.
  - `env_pass: Vec<String>`, `env_set: BTreeMap<String,String>`.
  - `limits` — same shape as the manifest sub-section.
  - `subscribes: Vec<String>` (patterns), `publishes: Vec<String>`
    (topics).
- **L3.** `.bindings` fields per security RFC §3.2 (manifest-derived
  authority snapshotted at install time):
  - `provider: bool`,
  - `provider_id: Option<String>` (present iff `provider == true`),
  - `tools: Vec<String>`,
  - `renderer_kinds: Vec<String>`,
  - `tool_meta: BTreeMap<String, ToolMeta>` where
    `ToolMeta = { sinks: Vec<String>, grant_match: Option<PathBuf>,
    always_confirm: bool }`.
  - **No `helpers` / `helper_for` fields** (deferred per row 26).
- **L4.** `.flags` fields — boolean lock-level overrides matching
  the security RFC's loud overrides:
  - `i_know_what_im_doing: bool` (suppresses trifecta refusal +
    env-scrubber strip per §7.1 / §7.4).
  - `allow_credential_paths: bool` (suppresses the carve-out
    refusal class per §7.3 rule 5).
  Both default `false`. Both are surfaced by the broker / runtime
  in `rfl status` (m2+ work; m1 only has to round-trip the field).
- **L5.** Identity fields per `[plugin.<canonical-id>]`: `digest =
  "sha256:<hex>"`, `manifest_digest = "sha256:<hex>"`, `granted_at`
  (RFC 3339).
- **L6.** Canonical-id parser/formatter
  (`lock::CanonicalId::parse(&str) -> Result<Self, _>` /
  `Display`) with a stable round-trip. The form is
  `<source>:<name>@<version>` where `source` matches a
  byte-restricted grammar of `[a-z0-9._/-]+` (no shell
  metacharacters; allows `github.com/acme` / `crates.io` /
  `local`), `name` matches `[a-z0-9_-]+` (the topic-segment
  grammar — *not* the more permissive name in the manifest),
  and `version` is a semver string (parsed via the workspace's
  semver crate, **not** re-implemented).
- **L7.** Round-trip `to_toml` / `from_toml` with deterministic
  ordering (sorted plugin keys, sorted scalar arrays where the
  RFC permits). The result is `serde`-driven so adding a field
  later doesn't churn the formatter.

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
- **T3.** Topic-id collision **testability**: the public derive
  function is the v1 surface, but `topic_id` exposes a
  `#[doc(hidden)]` test seam (`derive_with_hash<H: Hasher>`)
  so collision tests can inject a mock hash returning crafted
  bytes. Production code never substitutes the hasher; the
  seam is annotated as test-only and is gated behind
  `#[cfg(any(test, feature = "test-seam"))]` so it cannot be
  reached from a downstream consumer.

### V — single-plugin and lock-level validation (`rafaello_core::validate`)

Validation runs **after** parse, **before** compile. Two passes:

- **V1.** `validate::manifest(&Manifest, &PathContext) -> Result<()>`
  — the per-manifest pass. Asserts:
  - **Topic-ACL respect** in `bus.publishes`: rejects `core.*`,
    rejects `frontend.*`, rejects any `provider.<x>.*` whose
    `<x>` does not match `provides.provider`, rejects any
    `plugin.<topic-id>.*` whose `<topic-id>` does not match
    the manifest's own derived topic-id (which is computable
    only when the canonical id is known — see V2). For the
    self-namespace case, V1 exposes a hook the lock-level pass
    re-runs once `source` is bound at install time; m1 ships
    the hook + a unit-tested check that's exercised via L (a
    lock entry binds the canonical id for the manifest).
  - **Pattern-vs-topic discipline** in `bus.{publishes,subscribes}`:
    publishes must be topics (no `*`/`**`), subscribes may be
    patterns. Unknown wildcards (e.g. in-segment `*` like
    `grep.*foo`) rejected.
  - **Topic grammar** segment-by-segment: only `[a-z0-9_-]+`
    per segment (and `*`/`**` permitted only in subscribe-pattern
    positions).
  - **Reserved built-in renderer kinds** in `[[renderers]]` per
    M9.
  - **`provides.tool.<name>` table presence** for every name in
    `provides.tools`; missing tables default-fill but a typo
    in the table key (e.g. `[provides.tool.gerp]` for
    `tools = ["grep"]`) is rejected as an unknown-tool table.
  - **Sink class** values: must be either a known class
    (`network`/`vcs_push`/`mail`/`workspace_write`/`exec`) or
    match the custom-class grammar `[a-z0-9_]+`.
  - **Lazy-load triggers** (`[load]`) cross-validated against
    declared methods/topics/kinds: a `command = ["foo"]` trigger
    referencing a tool not in `provides.tools`, or an
    `event = ["x.y"]` trigger referencing a topic not in
    `bus.subscribes`, or a `kind = ["k"]` trigger referencing
    a kind not in `[[renderers]]`, is rejected.
  - **Capability bundle keys** match the `default | <segment>`
    shape; bundle names other than `default` must be either
    a tool name from `provides.tools` (per-method bundle) or a
    declared topic from `bus.subscribes` (per-trigger bundle —
    open question #1 in the manifest RFC; v1 accepts both
    forms and unions them at compile time per `decisions.md`
    row 17).
- **V2.** `validate::lock(&Lock, &Workspace) -> Result<()>` — the
  multi-plugin pass:
  - **Topic-id collision detection** across the lock entries
    (T2).
  - **Conflicting tool name detection**: any tool name appearing
    in `bindings.tools` of two distinct plugins is a hard error
    unless `[session]` records a `tool_owner.<name> = "<plugin-id>"`
    decision (the v1 surface for `rfl provider tool grep
    <plugin-id>` per security RFC §5.4). For m1, the surface is
    the typed error + the `[session]` field; the slash-command
    flow lives in m5.
  - **Provider activeness consistency**: `[session].provider_active`,
    if present, must reference an installed plugin whose
    `bindings.provider == true` and whose `provider_id` is
    set.
  - **Trifecta refusal** per security RFC §7.1.1 — V2 calls into
    the trifecta module (T7? actually section C below). V2
    delegates; the test surface in V is the typed error + the
    bypass flag.
  - **Carve-out enforcement** per §7.3 — same delegation
    pattern.

### C — grant compiler (`rafaello_core::compile`)

Take a validated lock entry plus the resolved path context and
emit a `lockin::SandboxBuilder` ready for `command(entry)`. The
output is **the builder value**, not a serialised TOML — per
`decisions.md` row 32.

- **C1.** `compile::compile_plugin(lock: &Lock, plugin_id:
  &CanonicalId, ctx: &PathContext) -> Result<CompiledPlugin,
  CompileError>` returning a struct that carries:
  - `sandbox_builder: lockin::SandboxBuilder` — pre-populated;
    the m2 supervisor takes ownership and calls `command(entry).spawn()`.
  - `entry_absolute: PathBuf` — resolved entry binary path
    (`${plugin}/<entry>`).
  - `topic_id: String`,
  - `subscribe_patterns: Vec<String>`,
  - `publish_topics: Vec<String>`,
  - `tool_meta: BTreeMap<String, ToolMeta>` (snapshotted from
    the lock for the broker's confirmation gate).
  - `flags: CompiledFlags { i_know_what_im_doing: bool,
    allow_credential_paths: bool }`.
- **C2.** **Bundle flatten** per `decisions.md` row 17. The
  active-bundle set for spawn is `{default}` ∪ all named
  bundles that the install/update flow has marked "currently
  active" — but in m1, with no spawn flow, the test surface
  takes the bundle set as input. Default test fixture: `{default}`.
  Tests for scoped bundles construct fixtures that explicitly
  list `{default, "rust.format"}` etc.
- **C3.** **Placeholder substitution** is invoked once per
  string field, against a `PathContext` carrying:
  `project_root`, `home`, `plugin_dir`, `cache_dir`, `state_dir`.
  Unknown placeholders rejected via `CompileError::UnknownPlaceholder`.
  The `${plugin}` expansion uses the plugin's installed-package
  dir (e.g. `${plugin_root}/<source>:<name>@<version>/`); m1
  consumes this from `PathContext` rather than computing it.
- **C4.** **Lockin call sequencing**: the compiler is the only
  caller of `lockin::Sandbox::builder()` in v1. The order of
  builder calls is mechanical and matches manifest RFC §8.2:
  network mode → allow_hosts → read_path / read_dir / write_path
  / write_dir / exec_path / exec_dir → env (handled by
  `lockin::config::apply_env` once env scrubbing is layered) →
  limits → command(entry). The m1 acceptance tests assert the
  resulting builder via a small test-surface that records every
  builder call (see "Risks" §1).
- **C5.** **Per-plugin private state grant** per security RFC §7.5:
  the compiler unconditionally adds `read_dir(${PROJECT_ROOT}/.rafaello-plugin-data/<plugin-id>/)`
  and `write_dir(${PROJECT_ROOT}/.rafaello-plugin-data/<plugin-id>/)`
  to the resolved policy, regardless of grant. This grant is
  **not** counted toward `has_workspace_write` for trifecta
  purposes (V2 / trifecta delegate).
- **C6.** **Resource limit defaults** per manifest RFC §8.2 #5:
  `max_cpu_time = 300`, `max_open_files = 1024` if the lock
  omits them. Never unbounded.
- **C7.** **Reserved env-var injection**: the compiler ensures
  `RFL_BUS_FD` and `RFL_PLUGIN` are stripped from the parent
  environment before any `env.pass` matching, and `lockin`'s
  built-in preload-style blocklist (`LD_PRELOAD`, `LD_AUDIT`,
  `LD_LIBRARY_PATH`, `DYLD_*`) is honoured by the builder
  itself (no m1 work — assertion-only).

### D — digest computation (`rafaello_core::digest`)

- **D1.** `digest::content_digest(package_dir: &Path) ->
  Result<String, DigestError>` returns `sha256:<lowercase-hex>`.
  The algorithm: walk `package_dir` deterministically (files
  only, sorted by relative path with `/` separators normalised
  on Windows-style hosts though m1 only targets Linux+macOS),
  hash each file's path bytes followed by length-prefixed
  contents, fold into a single sha256. **Symlinks** are followed
  for files that point inside the package and refused for files
  pointing outside (`DigestError::SymlinkEscape`); empty
  directories contribute their relative path only. The exact
  algorithm is pinned in m1's commit body and a doc comment on
  `content_digest` so future readers can reproduce it.
- **D2.** `digest::manifest_digest(canonical_bytes: &[u8]) ->
  String` — sha256 of `Manifest::canonical_bytes()` from M11.
- **D3.** `compile::compile_plugin` refuses to produce its
  output when the lock's `digest` does not match the freshly
  recomputed content digest (callers pass a recomputed value
  via `PathContext`), and similarly for `manifest_digest`. The
  refusals are typed: `CompileError::ContentDigestMismatch` /
  `CompileError::ManifestDigestMismatch`. m1 does **not**
  prompt for re-confirmation (security RFC §4) — that flow
  belongs in m2's install/update CLI; m1's compile path is a
  flat refusal.

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
    security RFC §5.4 ("the plugin subscribes to its own
    request topic by default — an automatic grant the
    compiler inserts").
  - For provider plugins, the bound `provider_id`.
  - For frontends in m1: nothing (TUI is m3).
- **G2.** Topic-grammar revalidation at compile time: every
  publish topic and every subscribe pattern is re-checked
  against the topic / pattern grammars before being emitted
  into the ACL — the validator at parse time is per-manifest;
  this catches lock corruption / hand-edits.
- **G3.** No live broker code lands in m1; the ACL is a typed
  value the m2 broker consumes.

### Tr — trifecta refusal (`rafaello_core::trifecta`)

Per security RFC §7.1.

- **Tr1.** `trifecta::evaluate(lock: &Lock, plugin_id:
  &CanonicalId) -> TrifectaState` returning the booleans
  `(reads_untrusted, has_outbound, has_workspace_write)` plus
  `refuse: bool`.
- **Tr2.** `reads_untrusted` per §7.1: any of (a)
  `network.mode != "deny"`, (b) `read_dirs` / `read_paths`
  contains a path outside `${PROJECT_ROOT}` (after placeholder
  expansion), (c) `subscribes` matches `core.session.tool_result`
  or `core.session.assistant_message`.
- **Tr3.** `has_outbound`: `network.mode != "deny"` OR a
  one-hop direct check — for any *other* plugin in the lock
  whose subscribe patterns match this plugin's published
  topics, where that other plugin has `network.mode != "deny"`.
- **Tr4.** `has_workspace_write`: `write_dirs` non-empty,
  excluding the per-plugin private-state subtree (which is
  C5's automatic grant — **not** part of `write_dirs` in the
  lock).
- **Tr5.** `refuse = reads_untrusted && has_outbound &&
  has_workspace_write && !flags.i_know_what_im_doing`. The
  refusal surfaces as `CompileError::TrifectaRefused` from
  V2's pre-compile pass.

### K — carve-out decomposition (`rafaello_core::carveout`)

Per security RFC §7.3 / §7.3.1.

- **K1.** `carveout::CARVE_OUTS: &[CarveOut]` is the v1 set:
  - `${PROJECT_ROOT}/rafaello.lock`
  - `${PROJECT_ROOT}/.rafaello/**`
  - `${HOME}/.config/rafaello/**`
  - `${HOME}/.ssh/**`
  - `${HOME}/.gnupg/**`
  - `${HOME}/.aws/**`
  - `${HOME}/.config/gh/**`
  - `${HOME}/.netrc`
- **K2.** `carveout::compile_against(grant: &Grant, ctx:
  &PathContext, allow_credential_paths: bool) ->
  Result<DecomposedGrant, CompileError>`. For each broad
  ancestor in `read_dirs` / `write_dirs` that covers a
  carve-out path, either:
  - decompose into immediate children minus the carve-out
    name (per §7.3 rule 3), bounded at 256 synthesised entries
    (rule 4) — over-cap → `CompileError::CarveOutTooLarge`;
  - or, for `read_dirs` covering an `${HOME}/.ssh`-class
    carve-out (rule 2) and `write_dirs` covering any carve-out
    (rule 1), refuse outright with
    `CompileError::CarveOutRefused` unless
    `allow_credential_paths` is set.
- **K3.** **Hidden-directory rule** (§7.3.1): the default
  workspace grant `${PROJECT_ROOT}` is decomposed at compile
  time into the immediate non-hidden children of the project
  root **plus** `${PROJECT_ROOT}/.rafaello-plugin-data/<plugin-id>/`
  (the per-plugin private state dir from C5). m1 implements
  this as a helper used by C / K2.
- **K4.** Decomposition is **snapshotted into the compiled
  policy**, not a live filter. Re-decomposition on lock change
  is the m2 install/update flow's job.

### Sc — env scrubber (`rafaello_core::scrubber`)

Per security RFC §7.4.

- **Sc1.** `scrubber::SECRET_PATTERNS: &[&str]` is the v1
  glob set: `*_TOKEN`, `*_SECRET`, `*_KEY`, `*_PASSWORD`,
  `AWS_*`, `GITHUB_TOKEN`, `OPENAI_*`, `ANTHROPIC_*`.
- **Sc2.** `scrubber::strip(env_pass: &[String], i_know_what_im_doing:
  bool) -> Vec<String>` returns the scrubbed list. The
  override flag suppresses the strip entirely (loud, surfaced
  in `rfl status` by m2+).
- **Sc3.** Reserved env vars `RFL_BUS_FD` and `RFL_PLUGIN`
  are **not** matched by any scrubber pattern (per security
  RFC §5.5.1) — verified by a test fixture that lists both
  in `env_pass` and asserts they survive the strip; m2's
  injection of these vars happens *after* the scrubber runs.

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
  plugin spawn. m2 is the first consumer.
- **`rfl install` / `rfl grant` / `rfl revoke` / `rfl update`
  CLI subcommands.** The lock has loaders + serialisers; the
  install/update flow that prompts for diffs and writes the
  lock is m2+. m1 fixtures construct locks programmatically.
- **Re-confirmation UX on digest mismatch.** The compiler
  flatly refuses; the diff-and-prompt flow is m2+.
- **Subprocess-renderer manifest dispatch.** Per
  `decisions.md` row 29, v1 ships only built-in renderers; m1
  rejects `[[renderers]]` entries that name a built-in kind
  (M9) and accepts plugin-prefixed kinds (`mermaid:diagram`)
  for forward-compat, but emits no dispatch wiring.
- **Helper plugins (`helper_for`, `RFL_HELPER_FD`).** Per
  `decisions.md` row 26, the field is rejected at parse time
  (M4); the v2 spec stays in security RFC §7.4.1 untouched.
- **External UDS-attached frontends.** Per `decisions.md`
  row 27, m1's lock has no `frontend.*` ACL surface; the
  TUI itself comes in m3 as a local-spawned principal.
- **Streaming entry patch ops.** Per `decisions.md` row 28;
  not relevant to m1 anyway.
- **Full openrpc.json validation.** m1 only checks the
  sibling file's presence (M12); content validation is m2's
  job (it lives on the fittings/JSON-Schema side).
- **Stream F RFC body edits.** The §15.1 normative-delta
  items 1–4 + the security RFC's `requires_confirmation` →
  `always_confirm` rename + helper / external-attach drift
  are documented in `overview.md` and `decisions.md`; m1
  retrospective patches the RFCs per `plans/milestones/README.md`
  §"Stream RFC drift" (m1 retrospective owns Stream F + Stream
  A's manifest field names).
- **`FittingsError::MethodNotFound { method: Option<String> }`
  cutover.** `decisions.md` row 36 lists this as an m1
  follow-through. It's a fittings change, not a `rafaello-core`
  change; if the m1 driver finds the cutover lands cleanly
  alongside the manifest work without bloat, fold it in;
  otherwise spawn a one-commit fittings cleanup at the end of
  m1's commits.md and document the call in the retrospective.
- **Capsa backend.** Manifest RFC §10 stays paper-only.

## Demo bar

Per the milestones README m1 row, the demo bar is `cargo test
-p rafaello-core` integration coverage. The matrices below name
every required test file. Tests may be split or merged during
`commits.md` drafting as long as every named behaviour is
exercised at least once.

### Positive integration tests in `rafaello/crates/rafaello-core/tests/`

| Test file | Exercises |
|-----------|-----------|
| `manifest_parse_minimal.rs` | Smallest valid manifest (`schema`, `name`, `version`, `entry`, `rafaello`, empty `[provides]`, no capabilities) round-trips through `Manifest::parse` and `Manifest::canonical_bytes`. |
| `manifest_parse_worked_example.rs` | Manifest RFC §9.1 (`rust-tools`) decodes; `provides.tools`, scoped `[capabilities."rust.format".filesystem]`, `[load]`-table, `[bus]` populate the typed structs. |
| `manifest_parse_provider_example.rs` | Manifest RFC §9.3 (`anthropic`)-shaped manifest decodes, including `provides.provider`, `eager` load string, `[capabilities.default.network]` with `proxy` mode + `allow_hosts`. |
| `manifest_parse_renderer_example.rs` | Manifest RFC §9.2 decodes; `[[renderers]]` list of two non-built-in kinds (e.g. `mermaid:diagram`, `code.diff`); registration accepted. |
| `manifest_canonical_bytes_stable.rs` | `Manifest::canonical_bytes` is byte-stable across two parses of the same TOML re-emitted with key reordering and trivial whitespace differences. |
| `manifest_placeholder_expansion.rs` | All five placeholders expand against a hand-built `PathContext`; deeply-nested mixes (e.g. `${project}/sub/${plugin}/foo`) resolve correctly. |
| `manifest_validate_load_trigger_cross_refs.rs` | `[load]` table referencing only declared methods/topics/kinds passes `validate::manifest`. |
| `manifest_openrpc_sibling_present.rs` | `validate_with_package` succeeds against a fixture directory containing a non-empty `openrpc.json` next to the manifest. |
| `lock_parse_round_trip.rs` | Worked-example lock (mirroring security RFC §3.2) parses, serialises, parses again byte-equal. Includes `[session].provider_active`, `.flags`, `bindings.tool_meta`. |
| `lock_canonical_id_round_trip.rs` | `CanonicalId::parse("github:acme/grep@1.4.2").to_string() == input` over a small grammar matrix (different `source` shapes, semver pre-release / build metadata). |
| `topic_id_derivation.rs` | Deterministic for a fixed input: e.g. `derive("github:acme/grep@1.4.2") == "id_<known-prefix>"`; the test pins the expected first 16 chars of base32-no-pad-lowercase against a hand-computed fixture. |
| `topic_id_collision_detection.rs` | Multi-plugin lock with two distinct canonical ids; via the test seam (T3), forces both to hash to the same prefix; `validate::lock` rejects with `CollisionError`. |
| `compile_default_bundle.rs` | Worked lock entry with `default` bundle compiles; resulting `CompiledPlugin.sandbox_builder` records the expected sequence of builder calls (read_dir / write_dir / network / env / limits / command(entry)). Asserts via the builder-introspection seam (Risks §1). |
| `compile_scoped_bundle_union.rs` | Lock specifies `{default, "rust.format"}` active; resulting builder reflects the union (default's reads + rust.format's writes); duplicate entries dedup. |
| `compile_placeholder_resolves_to_absolute.rs` | Compiled paths are absolute (no leftover `${...}` in the recorded builder calls); round-tripping through `lockin` would not need further substitution. |
| `compile_private_state_grant.rs` | Compiled output contains `read_dir` + `write_dir` for `${PROJECT_ROOT}/.rafaello-plugin-data/<plugin-id>/` regardless of whether the lock requested it. |
| `compile_private_state_excluded_from_workspace_write.rs` | Trifecta evaluation against a lock whose only writes are the private-state dir reports `has_workspace_write == false`. |
| `compile_resource_limit_defaults.rs` | Lock omits `limits`; compiled builder records `max_cpu_time=300`, `max_open_files=1024`. |
| `broker_acl_extraction.rs` | Two-plugin lock; `broker_acl::compile` emits per-plugin `publish_topics`, `subscribe_patterns`, the auto-inserted `plugin.<topic-id>.tool_request` self-subscribe, and the bound provider id. |
| `carveout_default_workspace_decomposition.rs` | `read_dirs = ["${PROJECT_ROOT}"]` decomposes to immediate non-hidden children of a fixture project root + the per-plugin private state dir. |
| `carveout_workspace_excludes_rafaello_dot_dirs.rs` | `read_dirs = ["${PROJECT_ROOT}"]` against a project that contains `.rafaello/` decomposes around the carve-out (no entry covering `.rafaello/`). |
| `digest_match_compiles.rs` | Lock's `digest` matches recomputed `content_digest`; lock's `manifest_digest` matches recomputed `manifest_digest`; compile succeeds. |
| `digest_content_deterministic.rs` | Two invocations of `content_digest` against the same fixture tree return the same value; reordering files in the parent FS doesn't change the digest. |
| `trifecta_two_plugins_one_hop.rs` | Plugin A has workspace_write + reads_untrusted but `network.mode = "deny"`; plugin B subscribes to A's published topic and has `network.mode = "proxy"`. A's `has_outbound` evaluates true via the one-hop check; combined with A's other booleans, the trifecta refusal fires. |
| `trifecta_iknowwhatimdoing_bypass.rs` | Same fixture; `flags.i_know_what_im_doing = true`; trifecta refusal suppressed; compile succeeds (the broker still gates the runtime, but that's m2). |
| `env_scrubber_strips_known_secrets.rs` | `env_pass = ["GITHUB_TOKEN", "MY_API_KEY", "AWS_REGION", "PATH"]`; scrubbed list contains only `AWS_REGION` and `PATH` (note: `AWS_REGION` matches `AWS_*` — kept after the scrubber runs against the override-off path; updated below in the negative matrix to exercise both behaviours). Tracking `AWS_*` is *also* in the strip set per security RFC §7.4 — the test asserts the documented behaviour: `AWS_REGION` is stripped because `AWS_*` matches; `PATH` survives. |
| `env_scrubber_reserved_vars_survive.rs` | `env_pass = ["RFL_BUS_FD", "RFL_PLUGIN"]`; both survive the strip (per §5.5.1 — they are core-injected and exempted). |

### Negative integration tests in `rafaello/crates/rafaello-core/tests/`

| Test file | Asserts |
|-----------|---------|
| `manifest_unknown_field.rs` | A manifest with a top-level `purpose = "foo"` is rejected as `ManifestError::UnknownField`; same for an unknown nested key under `[provides]`. |
| `manifest_legacy_runtime_field.rs` | `runtime = "subprocess"` rejected as `ManifestError::ReservedField { field: "runtime" }` per `decisions.md` row 30. |
| `manifest_legacy_rpc_block.rs` | `[rpc] openrpc = "x"` rejected as `ManifestError::ReservedField { field: "rpc" }` per `decisions.md` row 31. |
| `manifest_helper_for_field.rs` | Top-level `helper_for = "..."` rejected as `ManifestError::ReservedField` per `decisions.md` row 26. |
| `manifest_publishes_core_topic.rs` | `bus.publishes = ["core.session.tool_result"]` rejected as `ValidationError::PublishOnReservedNamespace`. |
| `manifest_publishes_other_plugin_namespace.rs` | `bus.publishes = ["plugin.id_aaaaaaaaaaaaaaaa.foo"]` from a plugin whose own topic-id resolves to a different prefix is rejected (the lock-binding pass that knows the canonical id rejects with `ValidationError::PublishOnForeignTopicId`). |
| `manifest_publishes_frontend_topic.rs` | `bus.publishes = ["frontend.tui.user_message"]` from a non-frontend plugin rejected as `ValidationError::PublishOnFrontendNamespace`. |
| `manifest_provider_namespace_mismatch.rs` | A plugin declaring `provides.provider = "anthropic"` but listing `bus.publishes = ["provider.openai.tool_request"]` rejected as `ValidationError::ProviderNamespaceMismatch`. |
| `manifest_publish_with_wildcard.rs` | `bus.publishes = ["plugin.id_xxxx.*"]` rejected as `ValidationError::PatternInPublishPosition`. |
| `manifest_subscribe_invalid_pattern.rs` | `bus.subscribes = ["core.**.tool_*"]` (in-segment `*`) rejected as `ValidationError::InvalidPatternSegment`. |
| `manifest_topic_segment_grammar.rs` | A topic / pattern segment violating `[a-z0-9_-]+` (uppercase, dot inside segment, slash) rejected as `ValidationError::IllegalTopicSegment`. |
| `manifest_conflicting_tool_table.rs` | `provides.tools = ["grep"]` but a `[provides.tool.gerp]` table (typo) — V1 reports an unknown-tool table error. |
| `manifest_malformed_sinks.rs` | `[provides.tool.foo] sinks = [42]` (non-string), and `sinks = ["Network"]` (uppercase, fails the `[a-z0-9_]+` custom-class grammar): both rejected. |
| `manifest_reserved_renderer_kind.rs` | `[[renderers]] kind = "text"` rejected per M9 (built-ins reserved). |
| `manifest_load_trigger_unknown_command.rs` | `[load] command = ["does_not_exist"]` rejected as `ValidationError::LoadTriggerUnknownCommand`. |
| `manifest_missing_openrpc_sibling.rs` | `provides.tools = ["grep"]` but no `openrpc.json` next to the manifest → `ManifestError::MissingOpenRpc`. |
| `lock_unknown_field.rs` | A lock with an unknown field under `.bindings` is rejected (`deny_unknown_fields`). |
| `lock_canonical_id_invalid.rs` | `[plugin."github:acme/Grep@1.4"]` (uppercase in `name`, missing patch) rejected by `CanonicalId::parse`. |
| `lock_helper_field_rejected.rs` | A lock containing `bindings.helpers = [...]` or `bindings.helper_for = "..."` is rejected (deferred per row 26 — both directions of the helper relationship). |
| `lock_provider_active_unknown.rs` | `[session].provider_active = "github:foo/bar@1.0"` referencing a plugin not present in the lock → `ValidationError::ProviderActiveUnknown`. |
| `lock_provider_active_not_provider.rs` | `[session].provider_active` references an installed plugin whose `bindings.provider == false` → `ValidationError::ProviderActiveNotProvider`. |
| `lock_conflicting_tool_names.rs` | Two installed plugins both listing `"grep"` in `bindings.tools` without a `[session].tool_owner.grep` decision → `ValidationError::ConflictingToolName`. |
| `topic_id_collision_at_lock.rs` | Via the test seam, two distinct canonical ids forced to the same topic-id; `validate::lock` rejects with `CollisionError`. |
| `digest_content_mismatch.rs` | Lock's `digest` field doesn't match the recomputed `content_digest`; `compile::compile_plugin` returns `CompileError::ContentDigestMismatch`. |
| `digest_manifest_mismatch.rs` | Lock's `manifest_digest` doesn't match recomputed value; `CompileError::ManifestDigestMismatch`. |
| `digest_symlink_escape.rs` | A symlink in the package directory pointing outside the package root → `DigestError::SymlinkEscape`. |
| `carveout_credential_path_refused.rs` | `write_dirs = ["${HOME}"]` covering `~/.ssh` and `~/.gnupg` with `allow_credential_paths = false` → `CompileError::CarveOutRefused`. |
| `carveout_credential_path_override.rs` | Same fixture with `allow_credential_paths = true` compiles; resulting builder writes records the broad `${HOME}` write dir verbatim (no decomposition). |
| `carveout_decomposition_blowup.rs` | A project root containing 300 immediate children plus `.rafaello/`; `read_dirs = ["${PROJECT_ROOT}"]` decomposition exceeds the 256-entry cap → `CompileError::CarveOutTooLarge`. |
| `carveout_lockfile_path.rs` | `write_dirs = ["${PROJECT_ROOT}"]` covering `${PROJECT_ROOT}/rafaello.lock` is decomposed (immediate children minus `rafaello.lock`); a request that would directly cover `rafaello.lock` (e.g. `write_paths = ["${PROJECT_ROOT}/rafaello.lock"]`) is refused unless `allow_credential_paths`. |
| `compile_unknown_placeholder.rs` | A grant containing `${nope}` rejected as `CompileError::UnknownPlaceholder`. |
| `env_scrubber_strips_secret_globs.rs` | `env_pass = ["GITHUB_TOKEN", "OPENAI_API_KEY", "MY_PASSWORD", "AWS_PROFILE"]` are all stripped in the no-override path. |
| `env_scrubber_override.rs` | With `flags.i_know_what_im_doing = true`, the same `env_pass` survives the strip verbatim. |

### Manual validation in `manual-validation.md`

The driver runs and captures:

- `cargo test -p rafaello-core` green on Linux (and CI rerun for
  macOS — m1 has no platform-specific code, so the macOS leg is
  delegated to CI per the m0 precedent).
- `cargo doc -p rafaello-core --no-deps` clean — the m1 surface
  is the future m2 consumer's API; doc warnings would surface
  drift between scope §C / §G / §V and the landed types.
- A `cargo test -p rafaello-core --release` run, since the
  digest module's hash performance and the carve-out
  decomposition's worst-case bound are easier to spot in
  release-mode timings.
- `nix develop --impure -L --command cargo test -p rafaello-core`
  green (per the m0 retrospective gotcha §4.6 — `--impure` is
  load-bearing for `nix develop` invocations).
- A small fixture-tree dump under
  `rafaello/crates/rafaello-core/tests/fixtures/` listing the
  packaged manifest+lock+openrpc fixtures the integration tests
  consume — the manual-validation captures `tree
  rafaello-core/tests/fixtures` so the fixture surface is
  visible at retrospective time.

## Risks

1. **Asserting against `lockin::SandboxBuilder`.** The builder
   doesn't expose its internal config publicly today. m1's
   compiler tests need to assert "you called `read_dir(X)`" /
   "you called `network_proxy(...)`" sequences. Two viable
   strategies, picked during `commits.md` drafting:
   - Wrap the builder in a thin `compile::SandboxOps` trait
     whose only production impl forwards to `lockin::SandboxBuilder`
     and whose test impl records calls into a `Vec<Op>`. The
     compiler is generic over `S: SandboxOps`; production
     callers pass `lockin::SandboxBuilder`, tests pass
     `RecordingOps`. Cost: one extra trait, one delegate impl;
     no upstream lockin change.
   - Land a tiny upstream `lockin::SandboxBuilder` accessor
     (`pub fn config(&self) -> &SandboxConfig`) so tests
     introspect the populated value directly. Cost: a
     cross-crate change, but conceptually cleaner and useful
     to capsa later.

   The driver picks one strategy in `commits.md` and surfaces
   the choice for owner approval at the same time as the
   `commits.md` ratification gate. The default recommendation
   is the trait-shim approach (no upstream churn).

2. **Deterministic content digest across hosts.** Symlink
   handling, file-mode bits, and trailing newline differences
   are the usual culprits. The algorithm in §D1 commits to a
   specific normalisation — the test
   `digest_content_deterministic.rs` exercises a fixture
   constructed twice from different starting orders to confirm
   the hash is order-independent. If a CI host (e.g. macOS)
   produces a different hash, that's a real bug to fix in the
   walker, not a test allowance.

3. **Topic-id collision testing requires a hash seam.** §T3
   commits to a `#[cfg(any(test, feature = "test-seam"))]`
   guarded `derive_with_hash` so the production surface is
   the natural `derive(canonical_id)`. If pi pushes back on
   feature-gated test seams, the alternative is a forced
   collision in the canonical-id space (two strings whose
   sha256 prefixes happen to collide) — no such pair is known
   in the strings the v1 grammar admits, so the seam is the
   only practical path.

4. **Validation rule completeness vs. surface size.** The
   manifest validation surface is large (V1, V2). The risk is
   that a real-world manifest the m2 driver brings in trips a
   rule m1 didn't think to write. Mitigation: every test in
   the negative matrix above is named after a real-world
   shape from security RFC §6 / manifest RFC §9; if the m2
   driver brings in a manifest that trips an *unstated* rule,
   that's m1 retrospective territory (drift to add to the
   validator), not an m1 acceptance gap.

5. **`--allow-credential-paths` lock flag plumbing.** The
   security RFC's "loud override" surfaces in `rfl status`,
   which is m2+. m1 only rounds-trips the flag through the
   lock and respects it in the carve-out compiler. Pi may
   flag this as a "the override is silent in m1" gap; the
   m1-correct answer is that the override is set only by the
   m2+ install/grant flow, never by the runtime, so its
   silence in m1 has no security consequence. Document this
   stance in the scope — already done above (V / K).

6. **`MethodNotFound` typed-method-field cutover (row 36).**
   Decision row 36 lists this as deferred to m1. It is a
   fittings change, not a `rafaello-core` change. The risk is
   that bundling it into m1 inflates the milestone scope; the
   risk on the other side is that punting again leaves an
   open follow-through tagged at this milestone. Driver
   guidance: surface the call at `commits.md` ratification
   time. Default: include the cutover as a single trailing
   commit in m1's commits.md after the `rafaello-core` work
   lands; bail to m2 only if the size analysis shows it's
   non-trivial (it shouldn't be).

7. **No external consumers yet.** `rafaello-core` is brand
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

1. **Crate skeleton + manifest types and parser**
   (S1–S3, M1–M11): `rafaello-core` lib lands; manifest types
   compile; basic decoder tests. Self-contained; ~5–7 commits.
2. **Lock types + canonical id + round-trip**
   (L1–L7): no compiler yet; ~3–4 commits.
3. **Topic-id derivation** (T1–T3): standalone; ~2 commits.
4. **Validation: per-manifest + cross-plugin lock**
   (V1, V2): builds on 1+2+3; ~5–7 commits.
5. **Digest** (D1–D2 + the manifest-canonical wiring left
   from M11): ~2–3 commits.
6. **Carve-out decomposition** (K1–K4): builds on
   placeholder substitution; ~3–4 commits.
7. **Trifecta refusal** (Tr1–Tr5): builds on lock parser
   + topic-grammar; ~2–3 commits.
8. **Env scrubber** (Sc1–Sc3): ~2 commits.
9. **Compiler core + private state grant + bundle flatten +
   resource limit defaults** (C1–C7) plus the recording-ops
   test seam (Risks §1 strategy choice baked in): ~5–7
   commits.
10. **Broker ACL extraction** (G1–G3): ~2 commits.
11. **`MethodNotFound` typed-method-field fittings cutover**
    (row 36): one commit if it stays this milestone; spawned
    only if Risks §6 resolves "include".
12. **`manual-validation.md`** (one commit at the end).

Realistic total: **~30–45 commits, sequential.** The natural
split point for an m1a/m1b cut is **after group 4** (V) — m1a
ships parsers + validation, m1b ships the compiler. m1a has
no v1 consumer; m1b is the consumer-facing piece. Default:
ship m1 as one milestone, no split. Surface a split for owner
approval if group 9 (compiler core) cannot land green on top
of groups 1–8.

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
  / external-attach drift) — the m1 retrospective patches the
  RFC bodies per the README's drift policy.
