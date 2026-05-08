//! Negative: a non-empty `network.allow_hosts` outside `mode =
//! "proxy"` is rejected as `AllowHostsOutsideProxy` (pi review-4
//! finding 8 / scope §V1).

use rafaello_core::error::ValidationError;
use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

#[test]
fn allow_hosts_in_deny_mode_rejected() {
    let src = r#"
schema = 1
name = "net"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[capabilities.default.network]
mode = "deny"
allow_hosts = ["api.example.com"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    match validate::manifest_standalone(&m) {
        Err(ValidationError::AllowHostsOutsideProxy { bundle }) => assert_eq!(bundle, "default"),
        other => panic!("expected AllowHostsOutsideProxy, got {other:?}"),
    }
}

#[test]
fn allow_hosts_in_allow_all_mode_rejected() {
    let src = r#"
schema = 1
name = "net"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[capabilities.default.network]
mode = "allow_all"
allow_hosts = ["api.example.com"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    assert!(matches!(
        validate::manifest_standalone(&m),
        Err(ValidationError::AllowHostsOutsideProxy { .. })
    ));
}

#[test]
fn proxy_mode_with_allow_hosts_accepted() {
    let src = r#"
schema = 1
name = "net"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[capabilities.default.network]
mode = "proxy"
allow_hosts = ["api.example.com"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    validate::manifest_standalone(&m).expect("proxy + allow_hosts is valid");
}
