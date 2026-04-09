use std::process::{Child, ExitStatus};

use anyhow::Result;

use capsa_sandbox::Sandbox;

pub struct DaemonProcess {
    name: &'static str,
    // Field order matters: `child` must be declared before `_sandbox` so that
    // if a `DaemonProcess` is dropped without an explicit teardown, the child
    // handle is released before the sandbox's private tmp directory is
    // removed. Normal shutdown paths wait on the child first anyway; this
    // ordering is a defense-in-depth for panic/unwinding paths.
    child: Child,
    // Held to keep the sandbox's private tmp directory alive until the child
    // exits. `None` when sandboxing was bypassed (CAPSA_DISABLE_SANDBOX).
    _sandbox: Option<Sandbox>,
}

impl DaemonProcess {
    pub fn new(name: &'static str, sandbox: Option<Sandbox>, child: Child) -> Self {
        Self {
            name,
            child,
            _sandbox: sandbox,
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn try_wait(&mut self) -> Result<Option<ExitStatus>> {
        self.child
            .try_wait()
            .map_err(anyhow::Error::from)
            .map_err(|err| err.context(format!("failed to poll {} process", self.name)))
    }

    pub fn kill(&mut self) -> Result<()> {
        self.child
            .kill()
            .map_err(anyhow::Error::from)
            .map_err(|err| err.context(format!("failed to kill {} process", self.name)))
    }

    pub fn wait_blocking(&mut self) -> Result<ExitStatus> {
        self.child
            .wait()
            .map_err(anyhow::Error::from)
            .map_err(|err| err.context(format!("failed waiting for {} process", self.name)))
    }
}
