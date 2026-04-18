use std::path::PathBuf;

use anyhow::{anyhow, Result};
use capsa::{Boot, Network, PortForward, Vm};
use clap::{ArgAction, Parser};

#[derive(Debug, Parser)]
#[command(name = "capsa", version, about = "Start a microVM with libkrun")]
struct Cli {
    /// Kernel image path.
    #[arg(long)]
    kernel: PathBuf,

    /// Optional initramfs path.
    #[arg(long)]
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

    /// Forward host port to guest port (repeatable). Format:
    /// `[tcp:|udp:]host_port:guest_port`. The protocol prefix is
    /// optional and defaults to `tcp`.
    #[arg(long = "forward")]
    forward: Vec<String>,

    /// Increase verbosity (-v: info + init verbosity, -vv: debug logs). Default is quiet.
    #[arg(short, long, action = ArgAction::Count)]
    verbose: u8,
}

/// Assemble the kernel command line the CLI wants to boot with. This
/// is CLI-layer policy — library users pass their own cmdline through
/// `Boot::kernel(..).cmdline(..)` directly and none of this logic
/// applies to them.
fn assemble_kernel_cmdline(verbose: u8, user_cmdline: Option<&str>) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if verbose == 0 {
        parts.push("quiet loglevel=0");
    } else {
        parts.push("capsa_init_verbose=1");
    }
    let user = user_cmdline.map(str::trim).filter(|s| !s.is_empty());
    if let Some(user) = user {
        parts.push(user);
    }
    parts.join(" ")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForwardProtocol {
    Tcp,
    Udp,
}

fn parse_port_forward(value: &str) -> Result<(ForwardProtocol, u16, u16)> {
    let (protocol, remainder) = match value.split_once(':') {
        Some(("tcp", rest)) => (ForwardProtocol::Tcp, rest),
        Some(("udp", rest)) => (ForwardProtocol::Udp, rest),
        _ => (ForwardProtocol::Tcp, value),
    };

    let (host, guest) = remainder.split_once(':').ok_or_else(|| {
        anyhow!("invalid --forward value '{value}': expected [tcp:|udp:]host_port:guest_port")
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

    Ok((protocol, host_port, guest_port))
}

impl Cli {
    fn to_boot(&self) -> Boot {
        let mut kb = Boot::kernel(self.kernel.clone());
        if let Some(initramfs) = &self.initramfs {
            kb = kb.initramfs(initramfs.clone());
        }
        let cmdline = assemble_kernel_cmdline(self.verbose, self.kernel_cmdline.as_deref());
        if !cmdline.is_empty() {
            kb = kb.cmdline(cmdline);
        }
        kb.into()
    }

    async fn to_vm(&self) -> Result<Vm> {
        let parsed_forwards = self
            .forward
            .iter()
            .map(|value| parse_port_forward(value))
            .collect::<Result<Vec<_>>>()?;

        // Duplicate host ports are only a problem within a single
        // protocol — the host kernel will happily bind the same
        // number on TCP and UDP.
        {
            let mut seen_tcp = std::collections::HashSet::new();
            let mut seen_udp = std::collections::HashSet::new();
            for &(protocol, host_port, _) in &parsed_forwards {
                let seen = match protocol {
                    ForwardProtocol::Tcp => &mut seen_tcp,
                    ForwardProtocol::Udp => &mut seen_udp,
                };
                if !seen.insert(host_port) {
                    let label = match protocol {
                        ForwardProtocol::Tcp => "tcp",
                        ForwardProtocol::Udp => "udp",
                    };
                    return Err(anyhow!(
                        "duplicate --forward host port {host_port} ({label})"
                    ));
                }
            }
        }

        if !parsed_forwards.is_empty() && self.allow_host.is_empty() {
            return Err(anyhow!(
                "--forward requires networking to be enabled via at least one --allow-host"
            ));
        }

        let tcp_forwards: Vec<(u16, u16)> = parsed_forwards
            .iter()
            .filter(|&&(p, _, _)| p == ForwardProtocol::Tcp)
            .map(|&(_, h, g)| (h, g))
            .collect();
        let udp_forwards: Vec<(u16, u16)> = parsed_forwards
            .iter()
            .filter(|&&(p, _, _)| p == ForwardProtocol::Udp)
            .map(|&(_, h, g)| (h, g))
            .collect();

        let mut builder = Vm::builder(self.to_boot())
            .vcpus(self.vcpus)
            .memory_mib(self.memory_mib);

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
                .await
                .map_err(|err| anyhow!("failed to start network daemon: {err}"))?;

            builder = builder.attach_with(&handle, |mut attach| {
                for &(host, guest) in &tcp_forwards {
                    attach = attach.forward(PortForward { host, guest });
                }
                for &(host, guest) in &udp_forwards {
                    attach = attach.forward_udp(PortForward { host, guest });
                }
                attach
            });
        }

        builder
            .build()
            .map_err(|err| anyhow!("invalid VM configuration: {err}"))
    }
}

async fn run(args: Cli) -> Result<()> {
    apply_vmm_log_env(args.verbose);
    let exit = args
        .to_vm()
        .await?
        .run()
        .await
        .map_err(|err| anyhow!(err))?;
    if exit.success() {
        Ok(())
    } else {
        Err(anyhow!("VM exited with {exit}"))
    }
}

/// Set `CAPSA_VMM_LOG` in the current process's env based on -v count
/// so the spawned vmm child inherits it. Only sets the var when the
/// CLI actually wants to raise the level, so a user-provided
/// `CAPSA_VMM_LOG=...` still survives the no-flag case.
fn apply_vmm_log_env(verbose: u8) {
    let level = match verbose {
        0 => return,
        1 => "info",
        _ => "debug",
    };
    // SAFETY: single-threaded context at CLI startup, before any
    // child process is spawned.
    unsafe {
        std::env::set_var("CAPSA_VMM_LOG", level);
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    run(Cli::parse()).await
}

#[cfg(test)]
mod tests {
    use super::{assemble_kernel_cmdline, parse_port_forward, Cli, ForwardProtocol};
    use clap::{error::ErrorKind, Parser};

    #[test]
    fn assemble_kernel_cmdline_is_quiet_at_verbose_zero() {
        let cmdline = assemble_kernel_cmdline(0, None);
        assert_eq!(cmdline, "quiet loglevel=0");
    }

    #[test]
    fn assemble_kernel_cmdline_switches_to_verbose_init_at_verbose_one() {
        let cmdline = assemble_kernel_cmdline(1, None);
        assert_eq!(cmdline, "capsa_init_verbose=1");

        let cmdline = assemble_kernel_cmdline(3, None);
        assert_eq!(cmdline, "capsa_init_verbose=1");
    }

    #[test]
    fn assemble_kernel_cmdline_appends_user_segment() {
        let cmdline = assemble_kernel_cmdline(0, Some("console=hvc0 rdinit=/init"));
        assert_eq!(cmdline, "quiet loglevel=0 console=hvc0 rdinit=/init");
    }

    #[test]
    fn assemble_kernel_cmdline_ignores_empty_user_segment() {
        assert_eq!(assemble_kernel_cmdline(0, Some("")), "quiet loglevel=0");
        assert_eq!(assemble_kernel_cmdline(0, Some("   ")), "quiet loglevel=0");
    }

    #[tokio::test]
    async fn forward_requires_allow_host() {
        let args = Cli::parse_from(["capsa", "--kernel", "/tmp/kernel", "--forward", "9100:9100"]);

        let err = args
            .to_vm()
            .await
            .expect_err("forward without allow-host should fail");
        assert!(err
            .to_string()
            .contains("--forward requires networking to be enabled"));
    }

    #[tokio::test]
    async fn malformed_forward_returns_error() {
        let args = Cli::parse_from([
            "capsa",
            "--kernel",
            "/tmp/kernel",
            "--allow-host",
            "*",
            "--forward",
            "not-a-forward",
        ]);

        let err = args.to_vm().await.expect_err("config should fail to build");
        assert!(err
            .to_string()
            .contains("expected [tcp:|udp:]host_port:guest_port"));
    }

    #[tokio::test]
    async fn forward_port_zero_returns_error() {
        let args = Cli::parse_from([
            "capsa",
            "--kernel",
            "/tmp/kernel",
            "--allow-host",
            "*",
            "--forward",
            "0:80",
        ]);

        let err = args.to_vm().await.expect_err("config should fail to build");
        assert!(err.to_string().contains("ports must be in 1..=65535"));
    }

    #[tokio::test]
    async fn duplicate_host_port_returns_error() {
        let args = Cli::parse_from([
            "capsa",
            "--kernel",
            "/tmp/kernel",
            "--allow-host",
            "*",
            "--forward",
            "9100:9100",
            "--forward",
            "9100:80",
        ]);

        let err = args
            .to_vm()
            .await
            .expect_err("duplicate host port should fail");
        assert!(err
            .to_string()
            .contains("duplicate --forward host port 9100"));
    }

    #[tokio::test]
    async fn malformed_allow_host_pattern_returns_error() {
        let args = Cli::parse_from([
            "capsa",
            "--kernel",
            "/tmp/kernel",
            "--allow-host",
            "*example.com",
        ]);

        let err = args.to_vm().await.expect_err("config should fail to build");
        assert!(err
            .to_string()
            .contains("wildcard host pattern must use only a leading '*.' prefix"));
    }

    #[test]
    fn net_flag_is_rejected_during_cli_parsing() {
        let err = Cli::try_parse_from(["capsa", "--kernel", "/tmp/kernel", "--net"])
            .expect_err("legacy --net should be rejected");

        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
        assert!(err.to_string().contains("--net"));
    }

    #[test]
    fn parse_port_forward_defaults_to_tcp() {
        let (protocol, host, guest) = parse_port_forward("8080:80").unwrap();
        assert_eq!(protocol, ForwardProtocol::Tcp);
        assert_eq!(host, 8080);
        assert_eq!(guest, 80);
    }

    #[test]
    fn parse_port_forward_accepts_tcp_prefix() {
        let (protocol, _, _) = parse_port_forward("tcp:8080:80").unwrap();
        assert_eq!(protocol, ForwardProtocol::Tcp);
    }

    #[test]
    fn parse_port_forward_accepts_udp_prefix() {
        let (protocol, host, guest) = parse_port_forward("udp:5353:53").unwrap();
        assert_eq!(protocol, ForwardProtocol::Udp);
        assert_eq!(host, 5353);
        assert_eq!(guest, 53);
    }

    #[tokio::test]
    async fn duplicate_host_port_across_protocols_is_allowed() {
        // Host kernel binds TCP and UDP independently on the same
        // port number, so the CLI must not treat them as duplicates.
        let args = Cli::parse_from([
            "capsa",
            "--kernel",
            "/tmp/kernel",
            "--allow-host",
            "*",
            "--forward",
            "tcp:53:53",
            "--forward",
            "udp:53:53",
        ]);

        // to_vm() goes on to spawn netd which will fail in this
        // sandboxed test env — but crucially, not with a "duplicate"
        // validation error before that.
        let err = args
            .to_vm()
            .await
            .expect_err("netd spawn will fail in this env");
        assert!(
            !err.to_string().contains("duplicate"),
            "tcp+udp on same host port should not be flagged as duplicate: {err}"
        );
    }

    #[tokio::test]
    async fn duplicate_udp_host_port_is_rejected() {
        let args = Cli::parse_from([
            "capsa",
            "--kernel",
            "/tmp/kernel",
            "--allow-host",
            "*",
            "--forward",
            "udp:9100:53",
            "--forward",
            "udp:9100:54",
        ]);

        let err = args
            .to_vm()
            .await
            .expect_err("duplicate udp host port fails");
        assert!(err.to_string().contains("duplicate --forward host port"));
        assert!(err.to_string().contains("udp"));
    }
}
