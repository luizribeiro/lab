use std::path::Path;

use crate::{NetworkMode, SandboxSpec};

use super::paths::PathSets;
use super::seatbelt::SeatbeltPolicy;

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
            !deny.contains("network-outbound"),
            "Deny mode must not emit network-outbound, got:\n{deny}"
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
        let network_outbound_rules = proxy.matches("network-outbound").count();
        assert_eq!(
            network_outbound_rules, 1,
            "Proxy mode must emit exactly one network-outbound rule, got {network_outbound_rules} in:\n{proxy}"
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
