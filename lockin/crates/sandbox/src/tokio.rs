//! Tokio-flavored [`SandboxedCommand`] and [`SandboxedChild`].
//!
//! Mirrors the sync API in the parent module but wraps
//! [`tokio::process::Command`] and [`tokio::process::Child`] so
//! callers can `.await` on spawn / wait / status / output.
//!
//! Enable with `--features tokio`.

use std::ffi::OsStr;
use std::path::Path;
use std::process::{ExitStatus, Output, Stdio};

use anyhow::Result;

use crate::{is_dynamic_linker_blocked, Sandbox, SandboxBuilder, DYNAMIC_LINKER_ENV_BLOCKLIST};

impl SandboxBuilder {
    /// Tokio equivalent of [`SandboxBuilder::command`](crate::SandboxBuilder::command).
    pub fn tokio_command(self, program: &Path) -> Result<SandboxedCommand> {
        let (command, sandbox) = self.build(program)?;
        Ok(SandboxedCommand {
            command: tokio::process::Command::from(command),
            sandbox,
        })
    }
}

pub struct SandboxedCommand {
    command: tokio::process::Command,
    sandbox: Sandbox,
}

impl SandboxedCommand {
    pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.command.arg(arg);
        self
    }

    pub fn args(&mut self, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> &mut Self {
        self.command.args(args);
        self
    }

    /// Sets a child env var. Keys in the dynamic-linker blocklist
    /// (e.g. `LD_PRELOAD`, `DYLD_INSERT_LIBRARIES`) are silently
    /// dropped — the sandbox guarantees they do not reach the child.
    pub fn env(&mut self, key: impl AsRef<OsStr>, val: impl AsRef<OsStr>) -> &mut Self {
        let key = key.as_ref();
        if !is_dynamic_linker_blocked(key) {
            self.command.env(key, val);
        }
        self
    }

    /// Sets a batch of child env vars. Entries whose key is in the
    /// dynamic-linker blocklist are silently dropped.
    pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        for (k, v) in vars {
            self.env(k, v);
        }
        self
    }

    pub fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Self {
        self.command.env_remove(key);
        self
    }

    pub fn env_clear(&mut self) -> &mut Self {
        self.command.env_clear();
        self
    }

    pub fn current_dir(&mut self, dir: impl AsRef<Path>) -> &mut Self {
        self.command.current_dir(dir);
        self
    }

    pub fn stdin(&mut self, cfg: impl Into<Stdio>) -> &mut Self {
        self.command.stdin(cfg);
        self
    }

    pub fn stdout(&mut self, cfg: impl Into<Stdio>) -> &mut Self {
        self.command.stdout(cfg);
        self
    }

    pub fn stderr(&mut self, cfg: impl Into<Stdio>) -> &mut Self {
        self.command.stderr(cfg);
        self
    }

    pub fn kill_on_drop(&mut self, kill: bool) -> &mut Self {
        self.command.kill_on_drop(kill);
        self
    }

    pub async fn status(&mut self) -> std::io::Result<ExitStatus> {
        self.strip_dynamic_linker_env();
        self.command.status().await
    }

    pub async fn output(&mut self) -> std::io::Result<Output> {
        self.strip_dynamic_linker_env();
        self.command.output().await
    }

    fn strip_dynamic_linker_env(&mut self) {
        for key in DYNAMIC_LINKER_ENV_BLOCKLIST {
            self.command.env_remove(key);
        }
    }

    /// Spawns the sandboxed child, transferring sandbox ownership to
    /// the returned [`SandboxedChild`].
    pub fn spawn(mut self) -> std::io::Result<SandboxedChild> {
        self.strip_dynamic_linker_env();
        let child = self.command.spawn()?;
        Ok(SandboxedChild {
            child,
            sandbox: self.sandbox,
        })
    }

    pub fn as_command(&self) -> &tokio::process::Command {
        &self.command
    }

    pub fn as_command_mut(&mut self) -> &mut tokio::process::Command {
        &mut self.command
    }
}

pub struct SandboxedChild {
    child: tokio::process::Child,
    sandbox: Sandbox,
}

impl SandboxedChild {
    pub async fn wait(&mut self) -> std::io::Result<ExitStatus> {
        self.child.wait().await
    }

    pub fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        self.child.try_wait()
    }

    pub fn start_kill(&mut self) -> std::io::Result<()> {
        self.child.start_kill()
    }

    pub async fn kill(&mut self) -> std::io::Result<()> {
        self.child.kill().await
    }

    pub fn id(&self) -> Option<u32> {
        self.child.id()
    }

    pub async fn wait_with_output(self) -> std::io::Result<Output> {
        let _sandbox = self.sandbox;
        self.child.wait_with_output().await
    }

    pub fn as_child(&self) -> &tokio::process::Child {
        &self.child
    }

    pub fn as_child_mut(&mut self) -> &mut tokio::process::Child {
        &mut self.child
    }

    pub fn into_parts(self) -> (tokio::process::Child, Sandbox) {
        (self.child, self.sandbox)
    }
}
