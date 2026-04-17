# capsa

Lightweight VMs with network sandboxing, powered by libkrun.

The `capsa` crate is the user-facing surface: typed configuration for
VMs and per-interface network policies.

## Quick start

```rust
use capsa::{DomainPattern, NetworkPolicy, VmConfig, VmNetworkInterfaceConfig};

let policy = NetworkPolicy::deny_all()
    .allow_domain(DomainPattern::parse("api.example.com")?);

let vm = VmConfig {
    root: None,
    kernel: Some("/boot/vmlinuz".into()),
    initramfs: Some("/boot/initramfs.cpio".into()),
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

## Design principles

- **Typed config, not a DSL**: Rust types are the source of truth for VM and policy shape.
- **Secure defaults**: omitting an interface policy is deny-all at runtime; `NetworkPolicy::deny_all()` is the canonical starting point.
- **Explicit relaxation**: outbound access is granted only by adding allow rules.
- **Validated upfront**: `VmConfig::validate()` rejects malformed configuration before boot.

## Network policy

Deny everything, then allow a single exact host:

```rust
use capsa::{DomainPattern, NetworkPolicy};

let policy = NetworkPolicy::deny_all()
    .allow_domain(DomainPattern::parse("api.example.com")?);
# Ok::<(), capsa::DomainPatternParseError>(())
```

Wildcards — `*.example.com` matches subdomains, not the apex:

```rust
use capsa::{DomainPattern, NetworkPolicy};

let policy = NetworkPolicy::deny_all()
    .allow_domain(DomainPattern::parse("*.example.com")?);
# Ok::<(), capsa::DomainPatternParseError>(())
```

Build a policy from a list of hosts (invalid patterns return an error):

```rust
use capsa::NetworkPolicy;

let policy = NetworkPolicy::from_allowed_hosts([
    "api.example.com",
    "*.cdn.example.com",
])?;
# Ok::<(), capsa::DomainPatternParseError>(())
```

Allow everything (debugging only):

```rust
use capsa::NetworkPolicy;

let policy = NetworkPolicy::allow_all();
```

## Multiple interfaces

Each interface carries its own policy, so a VM can route some traffic
unrestricted and funnel the rest through a deny-by-default filter:

```rust
use capsa::{DomainPattern, NetworkPolicy, VmConfig, VmNetworkInterfaceConfig};

let vm = VmConfig {
    root: Some("/var/lib/capsa/rootfs".into()),
    kernel: None,
    initramfs: None,
    kernel_cmdline: None,
    vcpus: 2,
    memory_mib: 1024,
    verbosity: 0,
    interfaces: vec![
        VmNetworkInterfaceConfig {
            mac: None,
            policy: Some(NetworkPolicy::allow_all()),
        },
        VmNetworkInterfaceConfig {
            mac: None,
            policy: Some(
                NetworkPolicy::deny_all()
                    .allow_domain(DomainPattern::parse("api.example.com")?),
            ),
        },
    ],
};
# Ok::<(), Box<dyn std::error::Error>>(())
```
