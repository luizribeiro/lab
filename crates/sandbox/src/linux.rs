use std::path::{Path, PathBuf};
use std::process::Command;

use crate::discover::library_dirs;
use crate::SandboxSpec;

/// Builds a `Command` that runs `program` under `syd` with rules derived from
/// `spec`. `syd` must already be resolved (see [`find_in_path`]).
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

    if spec.allow_network {
        // Network-enabled daemons (e.g. capsa-netd) need two relaxations:
        //
        // 1. `sandbox/net:off` — disables syd's seccomp-level network
        //    mediation so the daemon can own outbound traffic policy.
        //
        // 2. No `sandbox/lock:on` — `sandbox/lock:on` activates Landlock
        //    as a second enforcement layer, and on recent kernels
        //    Landlock's network ruleset denies `connect()` with EACCES
        //    for any locked child regardless of the seccomp mediator
        //    state. syd 3.49 does not expose Landlock network-allow
        //    rules, so the only way to permit outbound connections is
        //    to skip Landlock for these daemons. Seccomp-based
        //    read/exec/fs/ioctl mediation still applies.
        rules.push("sandbox/net:off".to_string());
    } else {
        rules.push("sandbox/net:on".to_string());
        rules.push("default/net:deny".to_string());
        rules.push("sandbox/lock:on".to_string());
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
            if candidate.is_dir() {
                push_unique(&mut read_recursive_paths, candidate);
            }
        }
    }

    for path in &spec.read_write_paths {
        for candidate in path_candidates(path) {
            push_with_ancestors(&mut read_paths, &candidate);
            if candidate.is_dir() {
                push_unique(&mut read_recursive_paths, candidate);
            }
        }
    }

    for path in &spec.ioctl_paths {
        for candidate in path_candidates(path) {
            push_with_ancestors(&mut read_paths, &candidate);
        }
    }

    // Grant read+exec recursively on each directory the dynamic linker will
    // search for `program`. Covers the link-time closure as well as any
    // runtime `dlopen` of siblings in the same directory (NSS modules,
    // locale data, ICU plugins, ...).
    for dir in library_dirs(program) {
        for candidate in path_candidates(&dir) {
            push_with_ancestors(&mut read_paths, &candidate);
            if candidate.is_dir() {
                push_unique(&mut read_recursive_paths, candidate.clone());
                push_unique(&mut exec_recursive_paths, candidate);
            }
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

    for path in stdio_tty_paths() {
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
        if candidate.is_dir() {
            push_unique(&mut read_recursive_paths, candidate);
        }
    }

    for path in read_paths {
        add_allow_rule(&mut rules, "allow/read,stat", &path);
        add_lock_allow_rule(&mut rules, "allow/lock/read", &path);
    }

    for path in read_recursive_paths {
        add_allow_recursive_rule(&mut rules, "allow/read,stat", &path);
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

    rules.push("sandbox/write,create,truncate,delete:on".to_string());

    let mut write_paths = vec![private_tmp.to_path_buf()];
    if spec.allow_kvm {
        write_paths.push(PathBuf::from("/dev/kvm"));
    }
    write_paths.extend(spec.read_write_paths.iter().cloned());

    for path in write_paths {
        for candidate in path_candidates(&path) {
            if candidate.is_dir() {
                add_allow_recursive_rule(
                    &mut rules,
                    "allow/write,create,truncate,delete",
                    &candidate,
                );
                add_lock_allow_rule(&mut rules, "allow/lock/write,create", &candidate);
            } else {
                add_allow_rule(&mut rules, "allow/write,create,truncate,delete", &candidate);
                add_lock_allow_rule(&mut rules, "allow/lock/write", &candidate);
            }
        }
    }

    rules
}

fn add_allow_rule(rules: &mut Vec<String>, prefix: &str, path: &Path) {
    let escaped = escape_syd_path(path);
    rules.push(format!("{prefix}+{escaped}"));
}

fn add_allow_recursive_rule(rules: &mut Vec<String>, prefix: &str, path: &Path) {
    add_allow_rule(rules, prefix, path);

    if path.is_dir() {
        let escaped = escape_syd_path(path);
        rules.push(format!("{prefix}+{escaped}/***"));
    }
}

fn add_lock_allow_rule(rules: &mut Vec<String>, prefix: &str, path: &Path) {
    if !path.exists() {
        return;
    }

    let escaped = escape_syd_path(path);
    rules.push(format!("{prefix}+{escaped}"));
}

fn stdio_tty_paths() -> Vec<PathBuf> {
    let mut out = Vec::new();

    for fd in [0, 1, 2] {
        let fd_path = PathBuf::from(format!("/proc/self/fd/{fd}"));
        if let Ok(target) = std::fs::canonicalize(&fd_path) {
            if target.starts_with("/dev/") {
                push_unique(&mut out, target);
            }
        }
    }

    out
}

fn path_candidates(path: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();

    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else if let Ok(cwd) = std::env::current_dir() {
        cwd.join(path)
    } else {
        path.to_path_buf()
    };

    push_unique(&mut out, absolute.clone());

    if let Ok(canonical) = std::fs::canonicalize(&absolute) {
        push_unique(&mut out, canonical);
    }

    out
}

fn push_with_ancestors(paths: &mut Vec<PathBuf>, path: &Path) {
    for ancestor in path.ancestors() {
        if ancestor == Path::new("/") {
            break;
        }
        push_unique(paths, ancestor.to_path_buf());
    }
}

fn push_unique(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|p| p == &path) {
        paths.push(path);
    }
}

fn escape_syd_path(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace(',', "\\,")
}

pub(crate) fn find_in_path(binary_name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(binary_name);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
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
    fn kvm_grants_are_gated_on_allow_kvm() {
        let mut allowed = SandboxSpec::default();
        allowed.allow_kvm = true;
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
    fn tty_ioctls_are_gated_on_allow_interactive_tty() {
        let mut allowed = SandboxSpec::default();
        allowed.allow_interactive_tty = true;
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
}
