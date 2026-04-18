# capsa

Lightweight VMs with network sandboxing, powered by libkrun.

The `capsa` crate is the user-facing surface: a builder that produces
validated VM configs from typed boot specs, network policies, and
device attachments, plus a small runtime API to launch them.

## Quick start

```rust,no_run
use capsa::{Boot, Network, PortForward, Vm};

let api_net = Network::builder()
    .allow_host("api.example.com")
    .allow_host("*.cdn.example.com")
    .build()?
    .start()?;

Vm::builder(Boot::kernel("/boot/vmlinuz").cmdline("console=hvc0"))
    .vcpus(2)
    .memory_mib(1024)
    .attach_with(&api_net, |a| a.forward(PortForward { host: 8080, guest: 80 }))
    .build()?
    .run()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Design principles

- **Typed builder, one blessed path**: `Vm::builder(boot)` is the only way to construct a VM.
- **Boot is required**: the builder takes a `Boot` spec up front; you can't forget it.
- **Secure defaults**: a fresh `Network::builder()` is deny-all — only `allow_host` / `allow_hosts` / `allow_all_hosts` relaxes it.
- **Networks are first-class**: a `Network` is built once, `start()`ed to spawn its daemon, and attached to any number of VMs.
- **Validated at build**: `.build()` parses host patterns and runs config validation, surfacing errors via `BuildError`.
- **Kill-on-drop runtime**: `VmHandle` SIGKILLs the vmm when dropped; the last `NetworkHandle` clone SIGKILLs the network daemon. No leaked processes on panic or abandon.

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

## Network policy

`Network::builder()` starts deny-all; the DSL is allowlist-first:

```rust
use capsa::Network;

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
# Ok::<(), capsa::BuildError>(())
```

## Starting a network

`Network::start()` spawns the network daemon and returns a
[`NetworkHandle`]. The handle is cheaply cloneable — every clone
shares the same daemon — and dropping the last clone SIGKILLs it.

```rust,no_run
use capsa::Network;

let handle = Network::builder()
    .allow_host("api.example.com")
    .build()?
    .start()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Multiple VMs can share a single running network:

```rust,no_run
use capsa::{Boot, Network, Vm};

let api = Network::builder().allow_host("api.example.com").build()?.start()?;

let vm1 = Vm::builder(Boot::kernel("/boot/vmlinuz")).attach(&api).build()?;
let vm2 = Vm::builder(Boot::kernel("/boot/vmlinuz")).attach(&api).build()?;
// vm1 and vm2 share the same netd; they're on the same virtual subnet.
# Ok::<(), Box<dyn std::error::Error>>(())
```

## Attaching a VM

`.attach(&handle)` attaches with defaults (auto MAC, no port
forwards). `.attach_with(&handle, |a| ...)` lets you set a MAC or
forward TCP ports on this attachment:

```rust,no_run
use capsa::{Boot, Network, PortForward, Vm};

let net = Network::builder().allow_all_hosts().build()?.start()?;

let vm = Vm::builder(Boot::kernel("/boot/vmlinuz"))
    .attach_with(&net, |a| {
        a.mac([0x02, 0xaa, 0xbb, 0xcc, 0xdd, 0xee])
            .forward(PortForward { host: 8080, guest: 80 })
    })
    .build()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

`.attach` / `.attach_with` are generic over `Attachable`, so future
device types shipped in-tree (disks, GPU passthrough, …) will plug
in through the same pattern. The trait is **sealed**: third-party
crates can't add device types — attachments touch the vmm's fd
table and sandbox policy, which we don't expose as an unstable
extension point.

```rust,ignore
// Future in-tree API
let vm = Vm::builder(Boot::kernel("/boot/vmlinuz"))
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

Vm::builder(Boot::kernel("/boot/vmlinuz")).build()?.run()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Holding a handle for programmatic control:

```rust,no_run
use capsa::{Boot, Vm};

let handle = Vm::builder(Boot::kernel("/boot/vmlinuz")).build()?.start()?;
// ... do other work while the VM is running ...
handle.wait()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Explicit kill:

```rust,no_run
use capsa::{Boot, Vm};

let mut handle = Vm::builder(Boot::kernel("/boot/vmlinuz"))
    .build()?
    .start()?;
handle.kill()?;
handle.wait()?;
# Ok::<(), Box<dyn std::error::Error>>(())
```

Dropping a running `VmHandle` SIGKILLs the vmm. The
[`NetworkHandle`]s it attached to stay alive as long as you (or
another VM) still hold clones.
