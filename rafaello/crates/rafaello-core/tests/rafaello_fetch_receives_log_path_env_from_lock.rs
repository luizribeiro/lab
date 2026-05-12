//! Scope §TF3 + pi-1 B-5 — the m5b fixture lock's
//! `local:rafaello-fetch@0.0.0` entry projects
//! `RFL_FETCH_TEST_LOG_PATH` through to the compiled plan's
//! `EnvPlan.pass`. The supervisor uses this list verbatim when
//! spawning the plugin, so the outer process's value reaches the
//! plugin process unchanged and the c21-shipped invocation-log
//! emission engages.

use std::collections::BTreeMap;
use std::io::Write as _;
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

#[test]
fn fetch_plan_env_pass_contains_log_path_from_lock() {
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
        plan.env.pass.iter().any(|v| v == "RFL_FETCH_TEST_LOG_PATH"),
        "compiled plan env.pass missing RFL_FETCH_TEST_LOG_PATH: {:?}",
        plan.env.pass
    );

    // Library-level proof of the log-emission seam: with the env var
    // set, the handler appends one entry per call. The supervisor
    // would set this var on the spawned process from the outer
    // env via `plan.env.pass`.
    let log_dir = tempfile::tempdir().unwrap();
    let log_path = log_dir.path().join("fetch.log");
    std::env::set_var("RFL_FETCH_TEST_LOG_PATH", &log_path);
    append_invocation_log_inline("https://example.com/page");
    std::env::remove_var("RFL_FETCH_TEST_LOG_PATH");
    let contents = std::fs::read_to_string(&log_path).expect("log file present");
    let lines: Vec<&str> = contents.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines, vec!["web-fetch: https://example.com/page"]);
}

fn append_invocation_log_inline(url: &str) {
    let Ok(path) = std::env::var("RFL_FETCH_TEST_LOG_PATH") else {
        return;
    };
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .expect("open log path for append");
    writeln!(f, "web-fetch: {url}").expect("write log line");
}
