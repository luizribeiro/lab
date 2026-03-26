use std::process::ExitStatus;

use anyhow::Result;

pub struct DaemonProcess {
    name: &'static str,
    child: capsa_sandbox::SandboxedChild,
}

impl DaemonProcess {
    pub fn new(name: &'static str, child: capsa_sandbox::SandboxedChild) -> Self {
        Self { name, child }
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
            .wait_blocking()
            .map_err(anyhow::Error::from)
            .map_err(|err| err.context(format!("failed waiting for {} process", self.name)))
    }
}
