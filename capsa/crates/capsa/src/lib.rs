//! Lightweight VMs with network sandboxing, backed by
//! [libkrun](https://github.com/containers/libkrun).
//!
//! `capsa` is a typed builder over two orthogonal primitives:
//!
//! - [`Network`] — a deny-by-default outbound policy compiled into a
//!   `capsa-netd` daemon. [`Network::start`] returns a cheaply-cloneable
//!   [`NetworkHandle`]; the daemon is SIGKILLed when the last clone drops.
//! - [`Vm`] — a VM spec built from a [`Boot`] plus resource knobs and
//!   any number of attached [`NetworkHandle`]s. [`Vm::start`] returns a
//!   [`VmHandle`] that tears down the vmm child on drop.
//!
//! Networks are first-class: one `Network` can back many VMs, and one
//! VM can attach to many networks.
//!
//! ```no_run
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! use capsa::{Boot, Network, PortForward, Vm};
//!
//! let api = Network::builder().allow_host("api.example.com").build()?.start().await?;
//! let exit = Vm::builder(Boot::kernel("/boot/vmlinuz"))
//!     .attach_with(&api, |a| a.forward(PortForward { host: 8080, guest: 80 }))
//!     .build()?
//!     .run()
//!     .await?;
//! assert!(exit.success());
//! # Ok(())
//! # }
//! ```
//!
//! Errors: [`BuildError`] at `.build()`, [`StartError`] at `.start()`,
//! [`RuntimeError`] at `.wait()` / `.kill()`. All are
//! `#[non_exhaustive]` enums preserving their source chain.

mod attach;
mod boot;
mod error;
mod network;
mod vm;

pub use self::attach::{Attachable, NetworkAttach, PortForward};
pub use self::boot::{Boot, KernelBoot};
pub use self::error::{BuildError, RuntimeError, StartError};
pub use self::network::{Network, NetworkBuilder, NetworkHandle};
pub use self::vm::{Vm, VmBuilder, VmExit, VmHandle};
