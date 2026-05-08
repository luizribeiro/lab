//! Acceptance test for c03: `manifest::placeholders::expand`
//! plus `paths::resolve_under_root` smoke (scope §M8, §C3).

use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;

use rafaello_core::manifest::placeholders;
use rafaello_core::paths::{resolve_under_root, PathContext, RootKind};
use rafaello_core::{ManifestError, PathError};
use tempfile::TempDir;

fn ctx() -> PathContext {
    PathContext {
        project_root: PathBuf::from("/proj"),
        home: PathBuf::from("/home/user"),
        plugin_dir: PathBuf::from("/plugins/foo"),
        cache_dir: PathBuf::from("/var/cache/rafaello"),
        state_dir: PathBuf::from("/var/lib/rafaello"),
    }
}

#[test]
fn each_placeholder_expands_to_its_root() {
    let c = ctx();
    assert_eq!(placeholders::expand("${project}", &c).unwrap(), "/proj");
    assert_eq!(placeholders::expand("${home}", &c).unwrap(), "/home/user");
    assert_eq!(
        placeholders::expand("${plugin}", &c).unwrap(),
        "/plugins/foo"
    );
    assert_eq!(
        placeholders::expand("${cache}", &c).unwrap(),
        "/var/cache/rafaello"
    );
    assert_eq!(
        placeholders::expand("${state}", &c).unwrap(),
        "/var/lib/rafaello"
    );
}

#[test]
fn placeholder_with_suffix() {
    let c = ctx();
    assert_eq!(
        placeholders::expand("${project}/src/main.rs", &c).unwrap(),
        "/proj/src/main.rs"
    );
    assert_eq!(
        placeholders::expand("${plugin}/openrpc.json", &c).unwrap(),
        "/plugins/foo/openrpc.json"
    );
}

#[test]
fn nested_mixed_placeholders() {
    let c = ctx();
    // Naive substitution: an embedded placeholder whose value is
    // itself absolute leaves a `//` boundary. POSIX collapses this
    // when path resolution runs; the expander does not pre-collapse.
    assert_eq!(
        placeholders::expand("${project}/sub/${plugin}/foo", &c).unwrap(),
        "/proj/sub//plugins/foo/foo"
    );
    assert_eq!(
        placeholders::expand("${home}/.cache${cache}", &c).unwrap(),
        "/home/user/.cache/var/cache/rafaello"
    );
}

#[test]
fn no_placeholders_passes_through() {
    let c = ctx();
    assert_eq!(
        placeholders::expand("/usr/bin/rustc", &c).unwrap(),
        "/usr/bin/rustc"
    );
    assert_eq!(placeholders::expand("", &c).unwrap(), "");
}

#[test]
fn unknown_placeholder_rejected() {
    let c = ctx();
    assert!(matches!(
        placeholders::expand("${secret}/x", &c),
        Err(ManifestError::UnknownPlaceholder)
    ));
}

#[test]
fn malformed_placeholder_rejected() {
    let c = ctx();
    assert!(matches!(
        placeholders::expand("${project", &c),
        Err(ManifestError::MalformedPlaceholder)
    ));
}

// --- resolver smoke tests (scope §C3) ---

fn build_ctx_for(project_root: PathBuf, plugin_dir: PathBuf) -> PathContext {
    PathContext {
        project_root,
        home: PathBuf::from("/tmp"),
        plugin_dir,
        cache_dir: PathBuf::from("/tmp"),
        state_dir: PathBuf::from("/tmp"),
    }
}

#[test]
fn resolver_existing_path_under_project() {
    let proj = TempDir::new().unwrap();
    fs::create_dir(proj.path().join("src")).unwrap();
    fs::write(proj.path().join("src/main.rs"), b"fn main(){}").unwrap();

    let c = build_ctx_for(proj.path().to_path_buf(), proj.path().to_path_buf());
    let resolved = resolve_under_root("${project}/src/main.rs", &c, RootKind::Project).unwrap();
    assert_eq!(
        resolved,
        std::fs::canonicalize(proj.path().join("src/main.rs")).unwrap()
    );
}

#[test]
fn resolver_nonexistent_leaf_under_project() {
    // pi-5 finding 7: write_dirs like `${project}/target` are
    // legitimate even when `target` doesn't yet exist.
    let proj = TempDir::new().unwrap();
    let c = build_ctx_for(proj.path().to_path_buf(), proj.path().to_path_buf());
    let resolved = resolve_under_root("${project}/target/release", &c, RootKind::Project).unwrap();
    let expected = std::fs::canonicalize(proj.path())
        .unwrap()
        .join("target/release");
    assert_eq!(resolved, expected);
}

#[test]
fn resolver_rejects_traversal_escape() {
    let proj = TempDir::new().unwrap();
    let c = build_ctx_for(proj.path().to_path_buf(), proj.path().to_path_buf());
    let err = resolve_under_root("${project}/../etc/passwd", &c, RootKind::Project)
        .expect_err("must reject ../ escape");
    assert!(matches!(err, PathError::PathEscape));
}

#[test]
fn resolver_rejects_symlink_escape() {
    let outside = TempDir::new().unwrap();
    fs::write(outside.path().join("secret"), b"hunter2").unwrap();

    let proj = TempDir::new().unwrap();
    symlink(outside.path(), proj.path().join("link")).unwrap();

    let c = build_ctx_for(proj.path().to_path_buf(), proj.path().to_path_buf());
    let err = resolve_under_root("${project}/link/secret", &c, RootKind::Project)
        .expect_err("must reject symlink escape");
    assert!(matches!(err, PathError::SymlinkEscape));
}

#[test]
fn resolver_propagates_unknown_placeholder() {
    let proj = TempDir::new().unwrap();
    let c = build_ctx_for(proj.path().to_path_buf(), proj.path().to_path_buf());
    let err = resolve_under_root("${nope}/x", &c, RootKind::Project).unwrap_err();
    assert!(matches!(err, PathError::UnknownPlaceholder));
}

#[test]
fn resolver_plugin_root_kind() {
    let plugin = TempDir::new().unwrap();
    fs::write(plugin.path().join("entry.py"), b"#!").unwrap();
    let c = build_ctx_for(plugin.path().to_path_buf(), plugin.path().to_path_buf());
    let resolved = resolve_under_root("${plugin}/entry.py", &c, RootKind::Plugin).unwrap();
    assert_eq!(
        resolved,
        std::fs::canonicalize(plugin.path().join("entry.py")).unwrap()
    );
}
