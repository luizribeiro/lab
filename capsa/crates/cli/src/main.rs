use std::path::PathBuf;

use anyhow::{anyhow, Result};
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
    fn to_vm_config(&self) -> Result<capsa::VmConfig> {
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

        let interfaces = if self.allow_host.is_empty() {
            vec![]
        } else {
            let policy = capsa::NetworkPolicy::from_allowed_hosts(
                self.allow_host.iter().map(String::as_str),
            )
            .map_err(|err| {
                anyhow!(
                    "invalid --allow-host value '{}': {err}",
                    self.allow_host.join("', '")
                )
            })?;

            vec![capsa::VmNetworkInterfaceConfig {
                mac: None,
                policy: Some(policy),
                port_forwards,
            }]
        };

        Ok(capsa::VmConfig {
            root: self.root.clone(),
            kernel: self.kernel.clone(),
            initramfs: self.initramfs.clone(),
            kernel_cmdline: self.kernel_cmdline.clone(),
            vcpus: self.vcpus,
            memory_mib: self.memory_mib,
            verbosity: self.verbose,
            interfaces,
        })
    }
}

fn run(args: Cli) -> Result<()> {
    args.to_vm_config()?.start()
}

fn main() -> Result<()> {
    run(Cli::parse())
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use capsa::{DomainPattern, MatchCriteria, PolicyAction};
    use clap::{error::ErrorKind, Parser};

    #[test]
    fn missing_allow_host_keeps_interfaces_empty() {
        let args = Cli::parse_from(["capsa", "--root", "/tmp/root"]);
        let config = args.to_vm_config().expect("config should build");

        assert!(config.interfaces.is_empty());
    }

    #[test]
    fn forward_requires_allow_host() {
        let args = Cli::parse_from(["capsa", "--root", "/tmp/root", "--forward", "9100:9100"]);

        let err = args
            .to_vm_config()
            .expect_err("forward without allow-host should fail");
        assert!(err
            .to_string()
            .contains("--forward requires networking to be enabled"));
    }

    #[test]
    fn forward_values_are_parsed_into_interface_config() {
        let args = Cli::parse_from([
            "capsa",
            "--root",
            "/tmp/root",
            "--allow-host",
            "*",
            "--forward",
            "9100:9100",
            "--forward",
            "2222:22",
        ]);

        let config = args.to_vm_config().expect("config should build");

        assert_eq!(config.interfaces.len(), 1);
        assert_eq!(
            config.interfaces[0].port_forwards,
            vec![(9100, 9100), (2222, 22)]
        );
    }

    #[test]
    fn one_allow_host_adds_one_default_interface_with_deny_default_policy() {
        let args = Cli::parse_from([
            "capsa",
            "--root",
            "/tmp/root",
            "--allow-host",
            " API.Example.COM. ",
        ]);
        let config = args.to_vm_config().expect("config should build");

        assert_eq!(config.interfaces.len(), 1);
        assert_eq!(config.interfaces[0].mac, None);

        let policy = config.interfaces[0]
            .policy
            .as_ref()
            .expect("policy should be present");
        assert_eq!(policy.default_action, PolicyAction::Deny);
        assert_eq!(policy.rules.len(), 1);
        assert!(matches!(
            policy.rules[0].criteria,
            MatchCriteria::Domain(DomainPattern::Exact(ref host)) if host == "api.example.com"
        ));
    }

    #[test]
    fn many_allow_host_entries_build_multiple_allow_rules() {
        let args = Cli::parse_from([
            "capsa",
            "--root",
            "/tmp/root",
            "--allow-host",
            "api.example.com",
            "--allow-host",
            "*.example.org",
        ]);
        let config = args.to_vm_config().expect("config should build");

        let policy = config.interfaces[0]
            .policy
            .as_ref()
            .expect("policy should be present");
        assert_eq!(policy.default_action, PolicyAction::Deny);
        assert_eq!(policy.rules.len(), 2);
    }

    #[test]
    fn allow_host_star_maps_to_allow_all() {
        let args = Cli::parse_from([
            "capsa",
            "--root",
            "/tmp/root",
            "--allow-host",
            "api.example.com",
            "--allow-host",
            "*",
        ]);
        let config = args.to_vm_config().expect("config should build");

        let policy = config.interfaces[0]
            .policy
            .as_ref()
            .expect("policy should be present");
        assert_eq!(policy.default_action, PolicyAction::Allow);
        assert!(policy.rules.is_empty());
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

        let err = args
            .to_vm_config()
            .expect_err("config should fail to build");
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

        let err = args
            .to_vm_config()
            .expect_err("config should fail to build");
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

        let err = args
            .to_vm_config()
            .expect_err("duplicate host port should fail");
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

        let err = args
            .to_vm_config()
            .expect_err("config should fail to build");
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
