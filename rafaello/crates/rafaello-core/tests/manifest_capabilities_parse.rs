//! `[capabilities]` raw decode (scope §M5, c07 acceptance).
//!
//! Bundle-key resolution (`default | <tool-name>`),
//! `network.allow_hosts`-vs-`mode`, and `exec_paths`-inside-project
//! checks are deferred to V1 (c10); this exercises only typed decode.

use rafaello_core::manifest::{Manifest, NetworkMode};

#[test]
fn capabilities_default_filesystem_decodes() {
    let src = r#"
schema = 1
name = "fs"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[capabilities.default.filesystem]
read_paths = ["/etc/hosts", "${project}/README.md"]
read_dirs = ["${home}/docs"]
write_paths = ["${state}/out.json"]
write_dirs = ["${cache}/scratch"]
exec_paths = ["/usr/bin/rustc"]
exec_dirs = ["${plugin}/bin"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    let caps = m.capabilities.expect("capabilities present");
    let bundle = caps.get("default").expect("default bundle present");
    let fs = bundle.filesystem.as_ref().expect("filesystem present");
    assert_eq!(fs.read_paths.len(), 2);
    assert_eq!(fs.read_paths[0].as_str(), "/etc/hosts");
    assert_eq!(fs.read_paths[1].as_str(), "${project}/README.md");
    assert_eq!(fs.read_dirs[0].as_str(), "${home}/docs");
    assert_eq!(fs.write_paths[0].as_str(), "${state}/out.json");
    assert_eq!(fs.write_dirs[0].as_str(), "${cache}/scratch");
    assert_eq!(fs.exec_paths[0].as_str(), "/usr/bin/rustc");
    assert_eq!(fs.exec_dirs[0].as_str(), "${plugin}/bin");
}

#[test]
fn capabilities_network_env_limits_decode() {
    let src = r#"
schema = 1
name = "net"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[capabilities.default.network]
mode = "proxy"
allow_hosts = ["api.example.com"]

[capabilities.default.env]
pass = ["HOME", "USER"]
set = { GREETING = "hi" }

[capabilities.default.limits]
max_cpu_time = 30
max_open_files = 256
max_address_space = 1073741824
max_processes = 8
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    let bundle = m
        .capabilities
        .as_ref()
        .and_then(|c| c.get("default"))
        .expect("default present");
    let net = bundle.network.as_ref().expect("network present");
    assert_eq!(net.mode, NetworkMode::Proxy);
    assert_eq!(net.allow_hosts, vec!["api.example.com".to_string()]);
    let env = bundle.env.as_ref().expect("env present");
    assert_eq!(env.pass, vec!["HOME".to_string(), "USER".to_string()]);
    assert_eq!(env.set.get("GREETING"), Some(&"hi".to_string()));
    let limits = bundle.limits.as_ref().expect("limits present");
    assert_eq!(limits.max_cpu_time, Some(30));
    assert_eq!(limits.max_open_files, Some(256));
    assert_eq!(limits.max_address_space, Some(1073741824));
    assert_eq!(limits.max_processes, Some(8));
}

#[test]
fn capabilities_named_bundle_decodes() {
    let src = r#"
schema = 1
name = "named"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[capabilities.default.filesystem]
read_paths = ["/etc/hosts"]

[capabilities.run_tool.filesystem]
exec_paths = ["/usr/bin/rustc"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    let caps = m.capabilities.expect("capabilities present");
    assert!(caps.contains_key("default"));
    assert!(caps.contains_key("run_tool"));
}

#[test]
fn capabilities_absent_is_ok() {
    let src = r#"
schema = 1
name = "minimal"
version = "1.0.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    assert!(m.capabilities.is_none());
}

#[test]
fn capabilities_unknown_section_rejected() {
    let src = r#"
schema = 1
name = "bad"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[capabilities.default.bogus]
x = 1
"#;
    assert!(Manifest::parse(src).is_err());
}

#[test]
fn capabilities_bare_relative_path_rejected() {
    let src = r#"
schema = 1
name = "bad"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[capabilities.default.filesystem]
read_paths = ["relative/path"]
"#;
    assert!(Manifest::parse(src).is_err());
}

#[test]
fn capabilities_unknown_network_mode_rejected() {
    let src = r#"
schema = 1
name = "bad"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[capabilities.default.network]
mode = "yolo"
"#;
    assert!(Manifest::parse(src).is_err());
}
