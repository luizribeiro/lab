use std::path::PathBuf;

use anyhow::{anyhow, Result};
use capsa::{Boot, Network, Vm};
use clap::{ArgAction, ArgGroup, Parser};

#[derive(Debug, Parser)]
#[command(
    name = "capsa",
    version,
    about = "Start a microVM with libkrun",
    group(ArgGroup::new("boot-source").required(true).args(["root", "kernel"]))
)]
struct Cli {
    /// Path to VM root filesystem directory.
    #[arg(long, conflicts_with = "kernel")]
    root: Option<PathBuf>,

    /// Kernel image path.
    #[arg(long, conflicts_with = "root")]
    kernel: Option<PathBuf>,

    /// Optional initramfs path.
    #[arg(long, requires = "kernel")]
    initramfs: Option<PathBuf>,

    /// Optional kernel command line.
    #[arg(long = "kernel-cmdline")]
    kernel_cmdline: Option<String>,

    /// Number of virtual CPUs.
    #[arg(long, default_value_t = 1, value_parser = clap::value_parser!(u8).range(1..))]
    vcpus: u8,

    /// VM memory in MiB.
    #[arg(long, default_value_t = 512, value_parser = clap::value_parser!(u32).range(1..))]
    memory_mib: u32,

    /// Allow outbound connections to a host pattern (repeatable).
    /// Supported values: exact host (api.example.com), wildcard (*.example.com), or * (allow-all).
    #[arg(long = "allow-host")]
    allow_host: Vec<String>,

    /// Forward host TCP port to guest TCP port (repeatable, format: host_port:guest_port).
    #[arg(long = "forward")]
    forward: Vec<String>,

    /// Increase verbosity (-v: info + init verbosity, -vv: debug logs). Default is quiet.
    #[arg(short, long, action = ArgAction::Count)]
    verbose: u8,
}

fn parse_port_forward(value: &str) -> Result<(u16, u16)> {
    let (host, guest) = value.split_once(':').ok_or_else(|| {
        anyhow!("invalid --forward value '{value}': expected host_port:guest_port")
    })?;

    let host_port = host.parse::<u16>().map_err(|err| {
        anyhow!("invalid --forward value '{value}': invalid host port '{host}': {err}")
    })?;
    let guest_port = guest.parse::<u16>().map_err(|err| {
        anyhow!("invalid --forward value '{value}': invalid guest port '{guest}': {err}")
    })?;

    if host_port == 0 || guest_port == 0 {
        return Err(anyhow!(
            "invalid --forward value '{value}': ports must be in 1..=65535"
        ));
    }

    Ok((host_port, guest_port))
}

impl Cli {
    fn to_boot(&self) -> Boot {
        if let Some(root) = &self.root {
            return Boot::root(root.clone());
        }
        let mut kb = Boot::kernel(self.kernel.clone().expect("clap group guarantees kernel"));
        if let Some(initramfs) = &self.initramfs {
            kb = kb.initramfs(initramfs.clone());
        }
        if let Some(cmdline) = &self.kernel_cmdline {
            kb = kb.cmdline(cmdline.clone());
        }
        kb.into()
    }

    fn to_vm(&self) -> Result<Vm> {
        let port_forwards = self
            .forward
            .iter()
            .map(|value| parse_port_forward(value))
            .collect::<Result<Vec<_>>>()?;

        {
            let mut seen_host_ports = std::collections::HashSet::new();
            for &(host_port, _) in &port_forwards {
                if !seen_host_ports.insert(host_port) {
                    return Err(anyhow!("duplicate --forward host port {host_port}"));
                }
            }
        }

        if !port_forwards.is_empty() && self.allow_host.is_empty() {
            return Err(anyhow!(
                "--forward requires networking to be enabled via at least one --allow-host"
            ));
        }

        let mut builder = Vm::builder(self.to_boot())
            .vcpus(self.vcpus)
            .memory_mib(self.memory_mib)
            .verbosity(self.verbose);

        if !self.allow_host.is_empty() {
            let has_global_wildcard = self.allow_host.iter().any(|h| h.trim() == "*");
            let net_builder = if has_global_wildcard {
                Network::builder().allow_all_hosts()
            } else {
                Network::builder().allow_hosts(self.allow_host.iter())
            };
            let network = net_builder.build().map_err(|err| {
                anyhow!(
                    "invalid --allow-host value '{}': {err}",
                    self.allow_host.join("', '")
                )
            })?;

            let handle = network
                .start()
                .map_err(|err| anyhow!("failed to start network daemon: {err}"))?;

            builder = builder.attach_with(&handle, |attach| {
                port_forwards
                    .iter()
                    .fold(attach, |acc, &(host, guest)| acc.forward_tcp(host, guest))
            });
        }

        builder
            .build()
            .map_err(|err| anyhow!("invalid VM configuration: {err}"))
    }
}

fn run(args: Cli) -> Result<()> {
    args.to_vm()?.run().map_err(|err| anyhow!(err))
}

fn main() -> Result<()> {
    run(Cli::parse())
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::{error::ErrorKind, Parser};

    #[test]
    fn forward_requires_allow_host() {
        let args = Cli::parse_from(["capsa", "--root", "/tmp/root", "--forward", "9100:9100"]);

        let err = args
            .to_vm()
            .expect_err("forward without allow-host should fail");
        assert!(err
            .to_string()
            .contains("--forward requires networking to be enabled"));
    }

    #[test]
    fn malformed_forward_returns_error() {
        let args = Cli::parse_from([
            "capsa",
            "--root",
            "/tmp/root",
            "--allow-host",
            "*",
            "--forward",
            "not-a-forward",
        ]);

        let err = args.to_vm().expect_err("config should fail to build");
        assert!(err.to_string().contains("expected host_port:guest_port"));
    }

    #[test]
    fn forward_port_zero_returns_error() {
        let args = Cli::parse_from([
            "capsa",
            "--root",
            "/tmp/root",
            "--allow-host",
            "*",
            "--forward",
            "0:80",
        ]);

        let err = args.to_vm().expect_err("config should fail to build");
        assert!(err.to_string().contains("ports must be in 1..=65535"));
    }

    #[test]
    fn duplicate_host_port_returns_error() {
        let args = Cli::parse_from([
            "capsa",
            "--root",
            "/tmp/root",
            "--allow-host",
            "*",
            "--forward",
            "9100:9100",
            "--forward",
            "9100:80",
        ]);

        let err = args.to_vm().expect_err("duplicate host port should fail");
        assert!(err
            .to_string()
            .contains("duplicate --forward host port 9100"));
    }

    #[test]
    fn malformed_allow_host_pattern_returns_error() {
        let args = Cli::parse_from([
            "capsa",
            "--root",
            "/tmp/root",
            "--allow-host",
            "*example.com",
        ]);

        let err = args.to_vm().expect_err("config should fail to build");
        assert!(err
            .to_string()
            .contains("wildcard host pattern must use only a leading '*.' prefix"));
    }

    #[test]
    fn net_flag_is_rejected_during_cli_parsing() {
        let err = Cli::try_parse_from(["capsa", "--root", "/tmp/root", "--net"])
            .expect_err("legacy --net should be rejected");

        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
        assert!(err.to_string().contains("--net"));
    }
}
