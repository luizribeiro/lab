# capsa (Rust crate API)

This README documents the **public API surface of the `capsa` Rust crate**.

> Scope note: this document intentionally focuses on Rust API exports only.
> Runtime architecture, daemon internals, CLI behavior, Nix packaging, and sandbox implementation details are out of scope here.

---

## What `capsa` exposes

`capsa` is currently a small façade crate. It re-exports selected public types from `capsa-core` (which itself re-exports policy types from `capsa-net`).

At crate root, these items are public:

```rust
pub use capsa::{
    DomainPattern,
    DomainPatternParseError,
    MatchCriteria,
    NetworkPolicy,
    PolicyAction,
    PolicyRule,
    VmConfig,
    VmNetworkInterfaceConfig,
};
```

---

## API reference (current)

## VM configuration

### `VmConfig`
User-facing VM configuration struct.

Fields:
- `root: Option<PathBuf>`
- `kernel: Option<PathBuf>`
- `initramfs: Option<PathBuf>`
- `kernel_cmdline: Option<String>`
- `vcpus: u8`
- `memory_mib: u32`
- `verbosity: u8`
- `interfaces: Vec<VmNetworkInterfaceConfig>`

Methods:
- `validate(&self) -> anyhow::Result<()>`

### `VmNetworkInterfaceConfig`
User-facing network interface configuration.

Fields:
- `mac: Option<[u8; 6]>` (auto-generated when omitted)
- `policy: Option<NetworkPolicy>` (runtime defaults to deny-all when omitted)

---

## Network policy DSL

### `NetworkPolicy`
Policy document with default action + ordered rules.

Fields:
- `default_action: PolicyAction`
- `rules: Vec<PolicyRule>`

Common constructors/helpers:
- `NetworkPolicy::deny_all()`
- `NetworkPolicy::allow_all()`
- `policy.allow_domain(DomainPattern)`
- `NetworkPolicy::from_allowed_hosts(iter)`

### `PolicyRule`
A single rule entry.

Fields:
- `action: PolicyAction`
- `criteria: MatchCriteria`

### `PolicyAction`
Rule/default action enum:
- `Allow`
- `Deny`
- `Log`

### `MatchCriteria`
Match expression enum:
- `Any`
- `Domain(DomainPattern)`
- `All(Vec<MatchCriteria>)`

### `DomainPattern`
Domain matcher enum:
- `Exact(String)`
- `Wildcard(String)`

Methods:
- `DomainPattern::parse(&str) -> Result<DomainPattern, DomainPatternParseError>`
- `matches(&self, domain: &str) -> bool`

### `DomainPatternParseError`
Error enum returned when parsing invalid domain patterns.

---

## What is intentionally *not* in `capsa` public API

The following are **not** exported by the `capsa` crate:
- daemon supervisor/adapters
- daemon launch spec internals (`VmmLaunchSpec`, `ResolvedNetworkInterface`, etc.)
- internal launcher/orchestration modules
- VMM/netd runtime entrypoints

Those remain internal to lower-level crates.

---

## Minimal usage example

```rust
use capsa::{DomainPattern, NetworkPolicy, VmConfig, VmNetworkInterfaceConfig};

let policy = NetworkPolicy::deny_all()
    .allow_domain(DomainPattern::parse("api.example.com")?);

let vm = VmConfig {
    root: None,
    kernel: Some("/path/to/vmlinuz".into()),
    initramfs: Some("/path/to/initramfs.cpio".into()),
    kernel_cmdline: Some("console=hvc0".into()),
    vcpus: 1,
    memory_mib: 512,
    verbosity: 0,
    interfaces: vec![VmNetworkInterfaceConfig {
        mac: None,
        policy: Some(policy),
    }],
};

vm.validate()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

---

## Compatibility expectations

- Items listed above are the intended user-facing surface of `capsa`.
- Everything else should be treated as internal implementation detail unless explicitly re-exported here.
