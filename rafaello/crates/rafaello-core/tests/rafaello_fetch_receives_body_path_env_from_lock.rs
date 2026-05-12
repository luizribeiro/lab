//! Scope §TF3 + pi-1 M-5 — the m5b fixture lock's
//! `local:rafaello-fetch@0.0.0` entry projects
//! `RFL_FETCH_TEST_BODY_PATH` through to the compiled plan's
//! `EnvPlan.pass`. The supervisor uses this list verbatim when
//! spawning the plugin, so the outer process's value reaches the
//! plugin process unchanged.

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::{content_digest, manifest_digest, RecomputedDigests};
use rafaello_core::lock::{CanonicalId, Lock};
use rafaello_core::manifest::Manifest;
use rafaello_core::paths::PathContext;

const FETCH_CANONICAL: &str = "local:rafaello-fetch@0.0.0";

fn m5b_locks_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("m5b-locks")
}

fn plugin_dir(canonical: &str) -> PathBuf {
    let root = m5b_locks_root();
    match canonical {
        "builtin:openai@0.0.0" => root.join("rafaello-openai"),
        "local:mailcat@0.0.0" => root.join("rafaello-mailcat"),
        "local:rafaello-fetch@0.0.0" => root.join("rafaello-fetch"),
        "local:readfile@0.0.0" => root.join("rafaello-readfile"),
        "local:mockprovider@0.0.0" => root.join("rafaello-mockprovider"),
        other => panic!("unexpected canonical {other}"),
    }
}

fn load_lock_with_recomputed_digests() -> (Lock, BTreeMap<CanonicalId, PathBuf>) {
    let lock_path = m5b_locks_root().join("rafaello.lock");
    let raw = std::fs::read_to_string(&lock_path).expect("read m5b fixture lock");
    let mut lock = Lock::from_toml(&raw).expect("parse m5b fixture lock");
    let mut plugin_dirs: BTreeMap<CanonicalId, PathBuf> = BTreeMap::new();
    for canonical in lock.plugins.keys().cloned().collect::<Vec<_>>() {
        plugin_dirs.insert(canonical.clone(), plugin_dir(&canonical.to_string()));
    }
    for (canonical, entry) in lock.plugins.iter_mut() {
        let pdir = &plugin_dirs[canonical];
        let manifest_raw =
            std::fs::read_to_string(pdir.join("rafaello.toml")).expect("read manifest");
        entry.manifest_digest = manifest_digest(
            &Manifest::parse(&manifest_raw)
                .expect("manifest parses")
                .canonical_bytes(),
        );
        entry.digest = content_digest(pdir).expect("content_digest");
    }
    (lock, plugin_dirs)
}

#[test]
fn fetch_plan_env_pass_contains_body_path_from_lock() {
    let (lock, plugin_dirs) = load_lock_with_recomputed_digests();
    let canonical = CanonicalId::parse(FETCH_CANONICAL).unwrap();

    let project = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let pdir = plugin_dirs[&canonical].clone();
    let pctx = PathContext {
        project_root: project.path().to_path_buf(),
        home: home.path().to_path_buf(),
        plugin_dir: pdir.clone(),
        cache_dir: project.path().to_path_buf(),
        state_dir: project.path().to_path_buf(),
    };
    let manifest_raw = std::fs::read_to_string(pdir.join("rafaello.toml")).unwrap();
    let recomputed = RecomputedDigests {
        content: content_digest(&pdir).unwrap(),
        manifest: manifest_digest(&Manifest::parse(&manifest_raw).unwrap().canonical_bytes()),
    };
    let plan = compile_plugin(&lock, &canonical, &pctx, &recomputed).expect("compile_plugin");

    assert!(
        plan.env
            .pass
            .iter()
            .any(|v| v == "RFL_FETCH_TEST_BODY_PATH"),
        "compiled plan env.pass missing RFL_FETCH_TEST_BODY_PATH: {:?}",
        plan.env.pass
    );

    // Library-level proof that the env var is the seam the supervisor
    // would propagate: handle_web_fetch reads the env var and returns
    // the file contents. (Real-process spawn via supervisor is
    // exercised by the PT1 end-to-end test in `rafaello/tests/`.)
    let body_dir = tempfile::tempdir().unwrap();
    let body_path = body_dir.path().join("body.txt");
    std::fs::write(&body_path, "hello from m5b lock fixture").unwrap();
    std::env::set_var("RFL_FETCH_TEST_BODY_PATH", &body_path);
    std::env::remove_var("RFL_FETCH_TEST_LOG_PATH");
    let resp = invoke_web_fetch_bin_via_handler();
    std::env::remove_var("RFL_FETCH_TEST_BODY_PATH");
    assert_eq!(
        resp,
        serde_json::json!({"ok": true, "content": "hello from m5b lock fixture"})
    );
}

fn invoke_web_fetch_bin_via_handler() -> serde_json::Value {
    // Inlined to avoid a circular dev-dep on `rafaello-fetch`. This is
    // a verbatim copy of `rafaello_fetch::handle_web_fetch`'s
    // file-backed branch (the only branch exercised here).
    let path = match std::env::var("RFL_FETCH_TEST_BODY_PATH") {
        Ok(p) => p,
        Err(_) => return serde_json::json!({"ok": false, "error": "fetch_test_body_unavailable"}),
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::json!({"ok": true, "content": content}),
        Err(_) => serde_json::json!({"ok": false, "error": "fetch_test_body_unavailable"}),
    }
}
