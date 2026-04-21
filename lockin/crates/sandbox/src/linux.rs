use std::path::{Path, PathBuf};
use std::process::Command;

use crate::paths::{path_candidates, push_unique, stdio_tty_paths};
use crate::{NetworkMode, SandboxSpec};

/// Builds a `Command` that runs `program` under `syd` with rules derived from
/// `spec`. `syd` must be an absolute path supplied by the caller.
pub(crate) fn build_sandbox_command(
    spec: &SandboxSpec,
    private_tmp: &Path,
    syd: &Path,
    program: &Path,
) -> Command {
    let mut command = Command::new(syd);
    for rule in syd_rules(program, spec, private_tmp) {
        command.arg("-m").arg(rule);
    }
    command
        .env("TMPDIR", private_tmp)
        .env("TMP", private_tmp)
        .env("TEMP", private_tmp)
        .arg("--")
        .arg(program);
    command
}

fn syd_rules(program: &Path, spec: &SandboxSpec, private_tmp: &Path) -> Vec<String> {
    // Keep policy focused on path-based controls plus fs/ioctl allowlists.
    let mut rules = vec![
        "sandbox/read,stat:on".to_string(),
        "sandbox/exec:on".to_string(),
        "sandbox/fs:on".to_string(),
        "sandbox/ioctl:on".to_string(),
        "sandbox/write,create,truncate,delete:off".to_string(),
        "default/read:deny".to_string(),
        "default/stat:deny".to_string(),
        "default/exec:deny".to_string(),
        "default/write:deny".to_string(),
        "default/create:deny".to_string(),
        "default/truncate:deny".to_string(),
        "default/delete:deny".to_string(),
        "default/ioctl:deny".to_string(),
        "trace/deny_dotdot:on".to_string(),
        "trace/force_cloexec:on".to_string(),
    ];

    if spec.allow_non_pie_exec {
        rules.push("trace/allow_unsafe_exec_nopie:1".to_string());
    }

    match spec.network {
        NetworkMode::AllowAll => {
            // Network-enabled daemons (e.g. capsa-netd) disable syd's
            // seccomp-level network mediation so the daemon can own
            // outbound traffic policy. Landlock is still enabled (below)
            // with an all-ports allowlist so filesystem enforcement
            // remains two-layer.
            rules.push("sandbox/net:off".to_string());
        }
        NetworkMode::Deny => {
            rules.push("sandbox/net:on".to_string());
            rules.push("default/net:deny".to_string());
        }
        NetworkMode::Proxy { .. } => {
            rules.push("sandbox/net:on".to_string());
            rules.push("default/net:deny".to_string());
        }
    }

    // Landlock is always enabled as a second enforcement layer for
    // filesystem access. On kernels with Landlock ABI v4+ (6.7+),
    // network rules are also enforced; on older kernels syd
    // degrades gracefully to filesystem-only Landlock.
    rules.push("sandbox/lock:on".to_string());

    match spec.network {
        NetworkMode::AllowAll => {
            // Permit all TCP connect/bind via Landlock so network-enabled
            // daemons are not blocked by the Landlock network ruleset.
            rules.push("allow/lock/connect+0-65535".to_string());
            rules.push("allow/lock/bind+0-65535".to_string());
        }
        NetworkMode::Proxy { loopback_port } => {
            // Seccomp-level allowlist: connect only to the loopback
            // proxy endpoint.
            rules.push(format!("allow/net/connect+127.0.0.1/32!{loopback_port}"));
            // Landlock is port-only (Linux 6.7+); narrow the allowed
            // port to the proxy's ephemeral port.
            rules.push(format!(
                "allow/lock/connect+{loopback_port}-{loopback_port}"
            ));
        }
        NetworkMode::Deny => {}
    }

    // Allow common filesystem types touched by capsa-vmm and host runtime.
    for fs in ["ext", "tmpfs", "proc", "sysfs", "cgroup"] {
        rules.push(format!("allow/fs+{fs}"));
    }

    if spec.allow_interactive_tty {
        // Terminal ioctls needed by libkrun's VMM console path and any
        // other caller that exposes an interactive TTY to the guest.
        for ioctl in [
            "TCGETS",
            "TCGETS2",
            "TCSETS",
            "TCSETS2",
            "TCSETSW",
            "TCSETSF",
            "TIOCGWINSZ",
            "TIOCSWINSZ",
            "FIONREAD",
        ] {
            rules.push(format!("allow/ioctl+{ioctl}"));
        }
    }

    if spec.allow_kvm {
        // KVM ioctls issued by libkrun when running a VM. Gated behind
        // `allow_kvm` so non-VMM daemons (e.g. capsa-netd) don't inherit
        // the hypervisor attack surface.
        for ioctl in [
            "KVM_GET_API_VERSION",
            "KVM_CHECK_EXTENSION",
            "KVM_GET_VCPU_MMAP_SIZE",
            "KVM_CREATE_VM",
            "KVM_CREATE_VCPU",
            "KVM_SET_TSS_ADDR",
            "KVM_CREATE_IRQCHIP",
            "KVM_CREATE_PIT2",
            "KVM_IOEVENTFD",
            "KVM_IRQFD",
            "KVM_SET_USER_MEMORY_REGION",
            "KVM_SET_CPUID2",
            "KVM_GET_SUPPORTED_CPUID",
            "KVM_GET_MSR_INDEX_LIST",
            "KVM_SET_MSRS",
            "KVM_GET_MSRS",
            "KVM_SET_REGS",
            "KVM_GET_REGS",
            "KVM_SET_SREGS",
            "KVM_GET_SREGS",
            "KVM_SET_FPU",
            "KVM_GET_FPU",
            "KVM_GET_LAPIC",
            "KVM_SET_LAPIC",
            "KVM_SET_SIGNAL_MASK",
            "KVM_SET_GSI_ROUTING",
            "KVM_SET_CLOCK",
            "KVM_RUN",
        ] {
            rules.push(format!("allow/ioctl+{ioctl}"));
        }
    }

    let mut read_paths = Vec::new();
    let mut read_recursive_paths = Vec::new();
    let mut exec_paths = Vec::new();
    let mut exec_recursive_paths = Vec::new();

    for candidate in path_candidates(program) {
        push_with_ancestors(&mut read_paths, &candidate);
        push_unique(&mut exec_paths, candidate);
    }

    for path in &spec.read_only_paths {
        for candidate in path_candidates(path) {
            push_with_ancestors(&mut read_paths, &candidate);
        }
    }

    for dir in &spec.read_only_dirs {
        for candidate in path_candidates(dir) {
            push_with_ancestors(&mut read_paths, &candidate);
            push_unique(&mut read_recursive_paths, candidate);
        }
    }

    for path in &spec.read_write_paths {
        for candidate in path_candidates(path) {
            push_with_ancestors(&mut read_paths, &candidate);
        }
    }

    for dir in &spec.read_write_dirs {
        for candidate in path_candidates(dir) {
            push_with_ancestors(&mut read_paths, &candidate);
            push_unique(&mut read_recursive_paths, candidate);
        }
    }

    for path in &spec.ioctl_paths {
        for candidate in path_candidates(path) {
            push_with_ancestors(&mut read_paths, &candidate);
        }
    }

    for dir in &spec.ioctl_dirs {
        for candidate in path_candidates(dir) {
            push_with_ancestors(&mut read_paths, &candidate);
            push_unique(&mut read_recursive_paths, candidate);
        }
    }

    for dir in &spec.library_paths {
        for candidate in path_candidates(dir) {
            push_with_ancestors(&mut read_paths, &candidate);
            push_unique(&mut read_recursive_paths, candidate.clone());
            push_unique(&mut exec_recursive_paths, candidate);
        }
    }

    for path in [
        PathBuf::from("/proc/self/maps"),
        PathBuf::from("/etc/ld.so.cache"),
        PathBuf::from("/etc/ld.so.preload"),
    ] {
        for candidate in path_candidates(&path) {
            push_with_ancestors(&mut read_paths, &candidate);
        }
    }

    for path in stdio_tty_paths("/proc/self/fd") {
        for candidate in path_candidates(&path) {
            push_with_ancestors(&mut read_paths, &candidate);
        }
    }

    if spec.allow_kvm {
        for candidate in path_candidates(Path::new("/dev/kvm")) {
            push_with_ancestors(&mut read_paths, &candidate);
        }
    }

    for candidate in path_candidates(private_tmp) {
        push_with_ancestors(&mut read_paths, &candidate);
        push_unique(&mut read_recursive_paths, candidate);
    }

    for path in read_paths {
        add_allow_rule(&mut rules, "allow/read,stat", &path);
        add_lock_allow_rule(&mut rules, "allow/lock/read", &path);
    }

    for path in read_recursive_paths {
        add_allow_recursive_rule(&mut rules, "allow/read,stat", &path);
        add_allow_recursive_rule(&mut rules, "allow/readdir", &path);
        add_lock_allow_rule(&mut rules, "allow/lock/readdir", &path);
    }

    for path in exec_paths {
        add_allow_rule(&mut rules, "allow/exec", &path);
        add_lock_allow_rule(&mut rules, "allow/lock/exec", &path);
    }

    for path in exec_recursive_paths {
        add_allow_recursive_rule(&mut rules, "allow/exec", &path);
        add_lock_allow_rule(&mut rules, "allow/lock/exec", &path);
    }

    let mut lock_ioctl_paths = Vec::new();
    if spec.allow_kvm {
        lock_ioctl_paths.push(PathBuf::from("/dev/kvm"));
    }
    lock_ioctl_paths.extend(spec.ioctl_paths.iter().cloned());

    for path in lock_ioctl_paths {
        for candidate in path_candidates(&path) {
            add_lock_allow_rule(&mut rules, "allow/lock/ioctl", &candidate);
        }
    }

    for dir in &spec.ioctl_dirs {
        for candidate in path_candidates(dir) {
            add_lock_allow_rule(&mut rules, "allow/lock/ioctl", &candidate);
            let escaped = escape_syd_path(&candidate);
            rules.push(format!("allow/lock/ioctl+{escaped}/***"));
        }
    }

    rules.push("sandbox/write,create,truncate,delete:on".to_string());

    let mut write_dirs = vec![private_tmp.to_path_buf()];
    write_dirs.extend(spec.read_write_dirs.iter().cloned());

    for dir in write_dirs {
        for candidate in path_candidates(&dir) {
            add_allow_recursive_rule(&mut rules, "allow/write,create,truncate,delete", &candidate);
            add_allow_recursive_rule(&mut rules, "allow/mkdir", &candidate);
            add_allow_recursive_rule(&mut rules, "allow/rmdir", &candidate);
            add_allow_recursive_rule(&mut rules, "allow/chmod", &candidate);
            add_allow_recursive_rule(&mut rules, "allow/rename", &candidate);
            add_allow_recursive_rule(&mut rules, "allow/utime", &candidate);
            add_lock_allow_rule(&mut rules, "allow/lock/write,create", &candidate);
            add_lock_allow_rule(&mut rules, "allow/lock/mkdir", &candidate);
            add_lock_allow_rule(&mut rules, "allow/lock/rmdir", &candidate);
            add_lock_allow_rule(&mut rules, "allow/lock/delete", &candidate);
            add_lock_allow_rule(&mut rules, "allow/lock/truncate", &candidate);
        }
    }

    let mut write_paths: Vec<PathBuf> = Vec::new();
    if spec.allow_kvm {
        write_paths.push(PathBuf::from("/dev/kvm"));
    }
    write_paths.extend(spec.read_write_paths.iter().cloned());

    for path in write_paths {
        for candidate in path_candidates(&path) {
            add_allow_rule(&mut rules, "allow/write,create,truncate,delete", &candidate);
            add_lock_allow_rule(&mut rules, "allow/lock/write", &candidate);
        }
    }

    for &(resource, value) in &spec.rlimits {
        if let Some(name) = rlimit_syd_name(resource) {
            rules.push(format!("rlimit/{name}:{value}"));
        }
    }

    rules
}

#[allow(clippy::unnecessary_cast)]
fn rlimit_syd_name(resource: i32) -> Option<&'static str> {
    match resource {
        x if x == libc::RLIMIT_NOFILE as i32 => Some("nofile"),
        x if x == libc::RLIMIT_AS as i32 => Some("as"),
        x if x == libc::RLIMIT_CPU as i32 => Some("cpu"),
        x if x == libc::RLIMIT_CORE as i32 => Some("core"),
        x if x == libc::RLIMIT_NPROC as i32 => Some("nproc"),
        _ => None,
    }
}

fn add_allow_rule(rules: &mut Vec<String>, prefix: &str, path: &Path) {
    let escaped = escape_syd_path(path);
    rules.push(format!("{prefix}+{escaped}"));
}

fn add_allow_recursive_rule(rules: &mut Vec<String>, prefix: &str, path: &Path) {
    add_allow_rule(rules, prefix, path);
    let escaped = escape_syd_path(path);
    rules.push(format!("{prefix}+{escaped}/***"));
}

fn add_lock_allow_rule(rules: &mut Vec<String>, prefix: &str, path: &Path) {
    if !path.exists() {
        return;
    }

    let escaped = escape_syd_path(path);
    rules.push(format!("{prefix}+{escaped}"));
}

fn push_with_ancestors(paths: &mut Vec<PathBuf>, path: &Path) {
    for ancestor in path.ancestors() {
        if ancestor == Path::new("/") {
            break;
        }
        push_unique(paths, ancestor.to_path_buf());
    }
}

fn escape_syd_path(path: &Path) -> String {
    let raw = path.display().to_string();
    let mut escaped = String::with_capacity(raw.len());
    for ch in raw.chars() {
        match ch {
            '\\' | '+' | ':' | ',' | '*' | '?' | '[' | ']' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SandboxSpec;

    fn rules_for(spec: &SandboxSpec) -> Vec<String> {
        let tmp = tempfile::tempdir().expect("tempdir");
        syd_rules(Path::new("/bin/sh"), spec, tmp.path())
    }

    #[test]
    fn escape_syd_path_escapes_all_metacharacters() {
        let cases = [
            ("/simple/path", "/simple/path"),
            ("/has+plus", "/has\\+plus"),
            ("/has:colon", "/has\\:colon"),
            ("/has,comma", "/has\\,comma"),
            (r"/has\backslash", r"/has\\backslash"),
            (
                r"/tmp/evil+allow/exec+/bin/sh",
                r"/tmp/evil\+allow/exec\+/bin/sh",
            ),
            ("/has*star", "/has\\*star"),
            ("/has?question", "/has\\?question"),
            ("/has[bracket]", "/has\\[bracket\\]"),
            ("/path/with]only", "/path/with\\]only"),
            (
                "/tmp/read*[all]+/etc/shadow",
                "/tmp/read\\*\\[all\\]\\+/etc/shadow",
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(
                escape_syd_path(Path::new(input)),
                expected,
                "escaping {input:?}"
            );
        }
    }

    #[test]
    fn non_pie_exec_rule_is_gated_on_allow_non_pie_exec() {
        let default_rules = rules_for(&SandboxSpec::default());
        assert!(
            !default_rules
                .iter()
                .any(|r| r == "trace/allow_unsafe_exec_nopie:1"),
            "default spec must keep syd's PIE enforcement on"
        );

        let allowed = SandboxSpec {
            allow_non_pie_exec: true,
            ..SandboxSpec::default()
        };
        let allowed_rules = rules_for(&allowed);
        assert!(
            allowed_rules
                .iter()
                .any(|r| r == "trace/allow_unsafe_exec_nopie:1"),
            "allow_non_pie_exec=true must disable syd's PIE enforcement"
        );
    }

    #[test]
    fn kvm_grants_are_gated_on_allow_kvm() {
        let allowed = SandboxSpec {
            allow_kvm: true,
            ..SandboxSpec::default()
        };
        let with_kvm = rules_for(&allowed);

        assert!(
            with_kvm.iter().any(|r| r == "allow/ioctl+KVM_RUN"),
            "allow_kvm=true should emit KVM ioctl rules"
        );
        assert!(
            with_kvm.iter().any(|r| r.contains("/dev/kvm")),
            "allow_kvm=true should reference /dev/kvm in path rules"
        );

        let without = rules_for(&SandboxSpec::default());
        assert!(
            !without.iter().any(|r| r.contains("KVM_")),
            "default spec should not emit any KVM ioctl rules, got: {without:?}"
        );
        assert!(
            !without.iter().any(|r| r.contains("/dev/kvm")),
            "default spec should not reference /dev/kvm, got: {without:?}"
        );
    }

    #[test]
    fn rlimits_are_emitted_as_syd_directives() {
        #[allow(clippy::unnecessary_cast)]
        let spec = SandboxSpec {
            rlimits: vec![(libc::RLIMIT_NOFILE as i32, 64)],
            ..SandboxSpec::default()
        };
        let rules = rules_for(&spec);
        assert!(
            rules.iter().any(|r| r == "rlimit/nofile:64"),
            "expected rlimit/nofile:64 in rules, got: {rules:?}"
        );
    }

    #[test]
    fn tty_ioctls_are_gated_on_allow_interactive_tty() {
        let allowed = SandboxSpec {
            allow_interactive_tty: true,
            ..SandboxSpec::default()
        };
        let with_tty = rules_for(&allowed);

        for ioctl in ["TCGETS", "TIOCGWINSZ", "FIONREAD"] {
            let rule = format!("allow/ioctl+{ioctl}");
            assert!(
                with_tty.iter().any(|r| r == &rule),
                "allow_interactive_tty=true should emit {rule}"
            );
        }

        let without = rules_for(&SandboxSpec::default());
        for ioctl in ["TCGETS", "TIOCGWINSZ", "FIONREAD"] {
            let rule = format!("allow/ioctl+{ioctl}");
            assert!(
                !without.iter().any(|r| r == &rule),
                "default spec should not emit {rule}, got: {without:?}"
            );
        }
    }

    #[test]
    fn landlock_is_always_enabled() {
        let without_net = rules_for(&SandboxSpec::default());
        assert!(
            without_net.iter().any(|r| r == "sandbox/lock:on"),
            "default spec should enable Landlock, got: {without_net:?}"
        );

        let with_net = rules_for(&SandboxSpec {
            network: NetworkMode::AllowAll,
            ..SandboxSpec::default()
        });
        assert!(
            with_net.iter().any(|r| r == "sandbox/lock:on"),
            "AllowAll should still enable Landlock, got: {with_net:?}"
        );
    }

    #[test]
    fn allow_all_emits_landlock_network_allowlist() {
        let with_net = rules_for(&SandboxSpec {
            network: NetworkMode::AllowAll,
            ..SandboxSpec::default()
        });
        assert!(
            with_net.iter().any(|r| r == "allow/lock/connect+0-65535"),
            "AllowAll should emit Landlock connect allowlist"
        );
        assert!(
            with_net.iter().any(|r| r == "allow/lock/bind+0-65535"),
            "AllowAll should emit Landlock bind allowlist"
        );

        let without = rules_for(&SandboxSpec::default());
        assert!(
            !without.iter().any(|r| r.starts_with("allow/lock/connect")),
            "default spec should not emit Landlock connect rules"
        );
        assert!(
            !without.iter().any(|r| r.starts_with("allow/lock/bind")),
            "default spec should not emit Landlock bind rules"
        );
    }

    #[test]
    fn proxy_mode_allows_only_loopback_port() {
        let rules = rules_for(&SandboxSpec {
            network: NetworkMode::Proxy {
                loopback_port: 51234,
            },
            ..SandboxSpec::default()
        });

        assert!(
            rules.iter().any(|r| r == "sandbox/net:on"),
            "proxy mode must enable seccomp net sandbox"
        );
        assert!(
            rules.iter().any(|r| r == "default/net:deny"),
            "proxy mode must deny network by default at seccomp layer"
        );
        assert!(
            rules
                .iter()
                .any(|r| r == "allow/net/connect+127.0.0.1/32!51234"),
            "proxy mode must allow only the loopback proxy endpoint at seccomp layer"
        );
        assert!(
            rules.iter().any(|r| r == "allow/lock/connect+51234-51234"),
            "proxy mode must narrow Landlock allow to the proxy port"
        );
        assert!(
            !rules.iter().any(|r| r == "allow/lock/connect+0-65535"),
            "proxy mode must not emit the all-ports Landlock allow"
        );
        assert!(
            !rules.iter().any(|r| r.starts_with("allow/lock/bind")),
            "proxy mode does not need inbound bind, got: {rules:?}"
        );
    }
}
