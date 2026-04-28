use std::path::Path;

use crate::{NetworkMode, SandboxSpec};

use super::paths::PathSets;
use super::seatbelt::SeatbeltPolicy;

/// Builds the Seatbelt policy in this emission order: `(deny default)`,
/// then `(import "system.sb")`, then structured allows derived from
/// `spec` (filesystem, network, tty, executable paths), then the
/// unconditional baseline-hardening denies (syslog Unix socket,
/// blanket `mach-register`, XPC service-name lookup, `/cores`
/// writes), then any caller-provided raw rules.
pub(super) fn build_policy(
    program: &Path,
    spec: &SandboxSpec,
    private_tmp: &Path,
) -> SeatbeltPolicy {
    let paths = PathSets::from_inputs(program, spec, private_tmp);

    let mut policy = SeatbeltPolicy::default();
    policy.import_system();

    if spec.allow_interactive_tty {
        policy.allow(&["pseudo-tty"]);
        policy.allow_literal(
            &["file-read*", "file-write*", "file-ioctl"],
            Path::new("/dev/tty"),
        );
        policy.allow_regex(
            &["file-read*", "file-write*", "file-ioctl"],
            "^/dev/ttys[0-9]*",
        );
    }

    for path in &paths.traversal_paths {
        policy.allow_literal(&["file-read-metadata"], path);
    }

    for path in &paths.executable_paths {
        policy.allow_literal(&["process-exec"], path);
    }

    for path in &paths.read_only_paths {
        policy.allow_literal(&["file-read*"], path);
        policy.allow_literal(&["file-map-executable"], path);
    }
    for dir in &paths.read_only_dirs {
        policy.allow_subpath(&["file-read*"], dir);
        policy.allow_subpath(&["file-map-executable"], dir);
    }

    for path in &paths.read_write_paths {
        policy.allow_literal(&["file-read*"], path);
        policy.allow_literal(&["file-write*"], path);
    }
    for dir in &paths.read_write_dirs {
        policy.allow_subpath(&["file-read*"], dir);
        policy.allow_subpath(&["file-write*"], dir);
    }

    for path in &paths.ioctl_paths {
        policy.allow_literal(&["file-read*"], path);
        policy.allow_literal(&["file-ioctl"], path);
    }
    for dir in &paths.ioctl_dirs {
        policy.allow_subpath(&["file-read*"], dir);
        policy.allow_subpath(&["file-ioctl"], dir);
    }

    match spec.network {
        NetworkMode::AllowAll => {
            policy.allow(&["network*"]);
        }
        NetworkMode::Proxy { loopback_port } => {
            policy.append_raw(format!(
                r#"(allow network-outbound (remote ip "localhost:{loopback_port}"))"#
            ));
        }
        NetworkMode::Deny => {}
    }

    policy.append_raw(r#"(deny network-outbound (literal "/private/var/run/syslog"))"#);
    policy.append_raw(r#"(deny mach-register (local-name-prefix ""))"#);
    policy.append_raw(r#"(deny mach-lookup (xpc-service-name-prefix ""))"#);
    policy.append_raw(r#"(deny file-write* (subpath "/cores"))"#);

    for rule in &spec.raw_seatbelt_rules {
        policy.append_raw(rule.as_str());
    }

    policy
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use crate::{NetworkMode, SandboxSpec};

    use super::build_policy;

    #[test]
    fn ioctl_is_only_granted_for_ioctl_paths() {
        let base = tempfile::Builder::new()
            .prefix("lockin-policy-test-")
            .tempdir()
            .expect("create test base dir");

        let rw_file = base.path().join("rw.dat");
        std::fs::write(&rw_file, b"data").expect("create rw file");

        let ioctl_file = base.path().join("ioctl.dev");
        std::fs::write(&ioctl_file, b"dev").expect("create ioctl file");

        let private_tmp = base.path().join("tmp");
        std::fs::create_dir_all(&private_tmp).expect("create private tmp");

        let mut spec = SandboxSpec::default();
        spec.read_write_paths.push(rw_file.clone());
        spec.ioctl_paths.push(ioctl_file.clone());

        let policy = build_policy(PathBuf::from("/bin/ls").as_path(), &spec, &private_tmp);
        let rendered: String = policy.into();

        let rw_ioctl_rule = format!("(allow file-ioctl (literal \"{}\"))", rw_file.display());
        assert!(
            !rendered.contains(&rw_ioctl_rule),
            "rw path unexpectedly granted ioctl: {rw_ioctl_rule}"
        );

        let ioctl_rule = format!("(allow file-ioctl (literal \"{}\"))", ioctl_file.display());
        assert!(
            rendered.contains(&ioctl_rule),
            "ioctl path missing ioctl rule: {ioctl_rule}"
        );
    }

    #[test]
    fn raw_seatbelt_rules_are_appended_after_structured_allows() {
        let base = tempfile::Builder::new()
            .prefix("lockin-raw-test-")
            .tempdir()
            .expect("create test base dir");
        let private_tmp = base.path().join("tmp");
        std::fs::create_dir_all(&private_tmp).expect("create private tmp");

        let raw_rule = "(allow iokit-open (iokit-user-client-class \"AGXDeviceUserClient\"))";
        let spec = SandboxSpec {
            network: NetworkMode::AllowAll,
            raw_seatbelt_rules: vec![raw_rule.to_string()],
            ..SandboxSpec::default()
        };
        let rendered: String = build_policy(Path::new("/bin/ls"), &spec, &private_tmp).into();

        assert!(
            rendered.contains(raw_rule),
            "raw rule missing from rendered policy:\n{rendered}"
        );

        let network_idx = rendered
            .find("(allow network*)")
            .expect("network allow should be present");
        let raw_idx = rendered.find(raw_rule).unwrap();
        assert!(
            raw_idx > network_idx,
            "raw rules should come after structured allows"
        );
    }

    #[test]
    fn raw_seatbelt_rules_default_empty() {
        let base = tempfile::Builder::new()
            .prefix("lockin-raw-empty-test-")
            .tempdir()
            .expect("create test base dir");
        let private_tmp = base.path().join("tmp");
        std::fs::create_dir_all(&private_tmp).expect("create private tmp");

        let rendered: String =
            build_policy(Path::new("/bin/ls"), &SandboxSpec::default(), &private_tmp).into();
        assert!(
            !rendered.contains("iokit-open"),
            "default spec should not emit raw rules, got:\n{rendered}"
        );
    }

    #[test]
    fn network_mode_renders_expected_rules() {
        let base = tempfile::Builder::new()
            .prefix("lockin-net-policy-test-")
            .tempdir()
            .expect("create test base dir");
        let private_tmp = base.path().join("tmp");
        std::fs::create_dir_all(&private_tmp).expect("create private tmp");

        let deny: String = build_policy(
            Path::new("/bin/ls"),
            &SandboxSpec {
                network: NetworkMode::Deny,
                ..SandboxSpec::default()
            },
            &private_tmp,
        )
        .into();
        assert!(
            !deny.contains("network*"),
            "Deny mode must not grant network*, got:\n{deny}"
        );
        assert!(
            !deny.contains("(allow network-outbound"),
            "Deny mode must not emit any network-outbound allow, got:\n{deny}"
        );

        let allow_all: String = build_policy(
            Path::new("/bin/ls"),
            &SandboxSpec {
                network: NetworkMode::AllowAll,
                ..SandboxSpec::default()
            },
            &private_tmp,
        )
        .into();
        assert!(
            allow_all.contains("(allow network*)"),
            "AllowAll must grant network*, got:\n{allow_all}"
        );

        let proxy: String = build_policy(
            Path::new("/bin/ls"),
            &SandboxSpec {
                network: NetworkMode::Proxy {
                    loopback_port: 51234,
                },
                ..SandboxSpec::default()
            },
            &private_tmp,
        )
        .into();
        assert!(
            proxy.contains(r#"(allow network-outbound (remote ip "localhost:51234"))"#),
            "Proxy mode must allow the loopback proxy endpoint, got:\n{proxy}"
        );
        assert!(
            !proxy.contains("(allow network*)"),
            "Proxy mode must not grant all network, got:\n{proxy}"
        );
        let network_outbound_allows = proxy.matches("(allow network-outbound").count();
        assert_eq!(
            network_outbound_allows, 1,
            "Proxy mode must emit exactly one network-outbound allow, got {network_outbound_allows} in:\n{proxy}"
        );
    }

    #[test]
    fn baseline_hardening_denies_are_emitted_after_system_import() {
        let base = tempfile::Builder::new()
            .prefix("lockin-baseline-deny-test-")
            .tempdir()
            .expect("create test base dir");
        let private_tmp = base.path().join("tmp");
        std::fs::create_dir_all(&private_tmp).expect("create private tmp");

        let expected = [
            r#"(deny network-outbound (literal "/private/var/run/syslog"))"#,
            r#"(deny mach-register (local-name-prefix ""))"#,
            r#"(deny mach-lookup (xpc-service-name-prefix ""))"#,
            r#"(deny file-write* (subpath "/cores"))"#,
        ];

        for mode in [
            NetworkMode::Deny,
            NetworkMode::AllowAll,
            NetworkMode::Proxy {
                loopback_port: 4242,
            },
        ] {
            let rendered: String = build_policy(
                Path::new("/bin/ls"),
                &SandboxSpec {
                    network: mode,
                    ..SandboxSpec::default()
                },
                &private_tmp,
            )
            .into();

            let import_idx = rendered
                .find("(import \"system.sb\")")
                .expect("system.sb import should be present");

            for rule in expected {
                let idx = rendered
                    .find(rule)
                    .unwrap_or_else(|| panic!("missing baseline deny `{rule}` in:\n{rendered}"));
                assert!(
                    idx > import_idx,
                    "baseline deny `{rule}` must come after system.sb import, got:\n{rendered}"
                );
            }
        }
    }

    #[test]
    fn baseline_hardening_denies_precede_user_raw_rules() {
        let base = tempfile::Builder::new()
            .prefix("lockin-baseline-raw-order-test-")
            .tempdir()
            .expect("create test base dir");
        let private_tmp = base.path().join("tmp");
        std::fs::create_dir_all(&private_tmp).expect("create private tmp");

        let user_rule = r#"(allow mach-lookup (xpc-service-name "com.example.svc"))"#;
        let spec = SandboxSpec {
            raw_seatbelt_rules: vec![user_rule.to_string()],
            ..SandboxSpec::default()
        };
        let rendered: String = build_policy(Path::new("/bin/ls"), &spec, &private_tmp).into();

        let baseline_deny_idx = rendered
            .find(r#"(deny mach-lookup (xpc-service-name-prefix ""))"#)
            .expect("baseline mach-lookup deny missing");
        let user_idx = rendered.find(user_rule).expect("user raw rule missing");
        assert!(
            user_idx > baseline_deny_idx,
            "user raw rules must follow baseline denies so callers can re-allow surgically:\n{rendered}"
        );
    }

    #[test]
    fn interactive_tty_rules_are_gated_on_allow_interactive_tty() {
        let base = tempfile::Builder::new()
            .prefix("lockin-tty-test-")
            .tempdir()
            .expect("create test base dir");
        let private_tmp = base.path().join("tmp");
        std::fs::create_dir_all(&private_tmp).expect("create private tmp");

        let without: String =
            build_policy(Path::new("/bin/ls"), &SandboxSpec::default(), &private_tmp).into();
        assert!(
            !without.contains("pseudo-tty"),
            "default spec should not grant pseudo-tty, got:\n{without}"
        );
        assert!(
            !without.contains("/dev/tty"),
            "default spec should not grant /dev/tty rules, got:\n{without}"
        );
        assert!(
            !without.contains("/dev/ttys"),
            "default spec should not grant /dev/ttys* rules, got:\n{without}"
        );

        let spec = SandboxSpec {
            allow_interactive_tty: true,
            ..SandboxSpec::default()
        };
        let with_tty: String = build_policy(Path::new("/bin/ls"), &spec, &private_tmp).into();
        assert!(
            with_tty.contains("pseudo-tty"),
            "allow_interactive_tty=true should grant pseudo-tty, got:\n{with_tty}"
        );
        assert!(
            with_tty.contains("/dev/tty"),
            "allow_interactive_tty=true should reference /dev/tty, got:\n{with_tty}"
        );
    }
}
