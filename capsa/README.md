# capsa

Lightweight VMs with network sandboxing, powered by libkrun.

The `capsa` crate is the user-facing surface: a builder that produces
validated VM configs from typed boot specs, network policies, and
device attachments, plus a small runtime API to launch them.

## Quick start

```rust,no_run
use capsa::{Boot, Network, Vm};

let api_net = Network::builder()
    .allow_host("api.example.com")
    .allow_host("*.cdn.example.com")
    .build()?;

Vm::builder(Boot::kernel("/boot/vmlinuz").cmdline("console=hvc0"))
    .vcpus(2)
    .memory_mib(1024)
    .attach_with(&api_net, |a| a.forward_tcp(8080, 80))
    .build()?
    .run()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Design principles

- **Typed builder, one blessed path**: `Vm::builder(boot)` is the only way to construct a VM.
- **Boot is required**: the builder takes a `Boot` spec up front; you can't forget it.
- **Secure defaults**: a fresh `Network::builder()` is deny-all — only `allow_host` / `allow_hosts` / `allow_all_hosts` relaxes it.
- **Validated at build**: `.build()` parses host patterns and runs config validation, surfacing errors via `BuildError`.
- **Kill-on-drop runtime**: `VmHandle` SIGKILLs its supervisor children when dropped — no leaked processes on panic or abandon.

## Boot modes

Kernel + optional initramfs + optional cmdline:

```rust
use capsa::{Boot, Vm};

let vm = Vm::builder(
    Boot::kernel("/boot/vmlinuz")
        .initramfs("/boot/initramfs.cpio")
        .cmdline("console=hvc0"),
)
.build()?;
# Ok::<(), capsa::BuildError>(())
```

Or a disk-image root:

```rust
use capsa::{Boot, Vm};

let vm = Vm::builder(Boot::root("/var/lib/capsa/rootfs")).build()?;
# Ok::<(), capsa::BuildError>(())
```

## Network policy

Deny-all with an explicit allowlist:

```rust
use capsa::{Network};

let net = Network::builder()
    .allow_host("api.example.com")
    .allow_host("*.cdn.example.com")
    .build()?;
# Ok::<(), capsa::BuildError>(())
```

From an iterator:

```rust
use capsa::Network;

let net = Network::builder()
    .allow_hosts(["api.example.com", "*.cdn.example.com"])
    .build()?;
# Ok::<(), capsa::BuildError>(())
```

Allow everything (debugging only):

```rust
use capsa::Network;

let net = Network::builder().allow_all_hosts().build()?;
```

## Attaching to a VM

`.attach(&net)` attaches with defaults (auto MAC, no port forwards).
`.attach_with(&net, |a| ...)` lets you set a MAC or forward TCP ports
on this attachment:

```rust
use capsa::{Boot, Network, Vm};

let net = Network::builder().allow_all_hosts().build()?;

let vm = Vm::builder(Boot::root("/rootfs"))
    .attach_with(&net, |a| {
        a.mac([0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee])
            .forward_tcp(8080, 80)
    })
    .build()?;
# Ok::<(), capsa::BuildError>(())
```

`.attach` / `.attach_with` are generic over `Attachable`, so future
device types plug in through the same pattern:

```rust,ignore
// Future API
let vm = Vm::builder(Boot::root("/rootfs"))
    .attach(&api_net)
    .attach_with(&scratch_disk, |d| d.mount("/var/lib/data"))
    .build()?;
```

## Running a VM

Two entry points. `Vm::run()` blocks until the VM exits; `Vm::start()`
returns a `VmHandle` you can hold, kill, or drop.

Blocking (simplest — mirrors `Command::status()`):

```rust,no_run
use capsa::{Boot, Vm};

Vm::builder(Boot::root("/rootfs")).build()?.run()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Holding a handle for programmatic control:

```rust,no_run
use capsa::{Boot, Vm};

let handle = Vm::builder(Boot::root("/rootfs")).build()?.start()?;
// ... do other work while the VM is running ...
handle.wait()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Explicit kill:

```rust,no_run
use capsa::{Boot, Vm};

let mut handle = Vm::builder(Boot::root("/rootfs")).build()?.start()?;
handle.kill()?;
handle.wait()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Dropping a running `VmHandle` SIGKILLs both supervisor children. This
is the default teardown path — no explicit `kill` required for panic
safety.
