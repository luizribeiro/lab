//! Scope §OP5 + pi-2 B-3 + pi-3 M-3: the combined m5a fixture lock
//! at `rafaello/fixtures/m5a-locks/rafaello.lock` validates and
//! compiles cleanly for all four installed plugins (openai +
//! mailcat + readfile + mockprovider) before c39 consumes it. The
//! resulting `ToolSchemaCatalog` contains exactly the
//! tool-providing plugins' methods (send-mail, read-file).

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::broker_acl;
use rafaello_core::compile::{compile_plugin, CompiledPlugin};
use rafaello_core::digest::{content_digest, manifest_digest, RecomputedDigests};
use rafaello_core::lock::{CanonicalId, Lock};
use rafaello_core::manifest::Manifest;
use rafaello_core::paths::PathContext;
use rafaello_core::supervisor::tool_catalog::ToolSchemaCatalog;
use rafaello_core::validate::{self, LockValidationContext};

use common::lock_kit::{fixtures_root, m5a_locks_root};

fn plugin_dir(canonical: &str) -> PathBuf {
    match canonical {
        "builtin:openai@0.0.0" => m5a_locks_root().join("rafaello-openai"),
        "local:mailcat@0.0.0" => m5a_locks_root().join("rafaello-mailcat"),
        "local:readfile@0.0.0" => fixtures_root().join("rafaello-readfile"),
        "local:mockprovider@0.0.0" => fixtures_root().join("rafaello-mockprovider"),
        other => panic!("unexpected canonical {other}"),
    }
}

fn load_lock_with_recomputed_digests(plugin_dirs: &BTreeMap<CanonicalId, PathBuf>) -> Lock {
    let lock_path = m5a_locks_root().join("rafaello.lock");
    let raw = std::fs::read_to_string(&lock_path).expect("read combined fixture lock");
    let mut lock = Lock::from_toml(&raw).expect("parse combined fixture lock");
    for (canonical, entry) in lock.plugins.iter_mut() {
        let pdir = plugin_dirs.get(canonical).expect("plugin_dir present");
        let manifest_raw =
            std::fs::read_to_string(pdir.join("rafaello.toml")).expect("read manifest");
        entry.manifest_digest = manifest_digest(
            &Manifest::parse(&manifest_raw)
                .expect("manifest parses")
                .canonical_bytes(),
        );
        entry.digest = content_digest(pdir).expect("content_digest");
    }
    lock
}

#[test]
fn m5a_fixture_lock_validates_and_compiles() {
    let canonicals = [
        "builtin:openai@0.0.0",
        "local:mailcat@0.0.0",
        "local:readfile@0.0.0",
        "local:mockprovider@0.0.0",
    ];
    let mut plugin_dirs: BTreeMap<CanonicalId, PathBuf> = BTreeMap::new();
    for c in canonicals {
        plugin_dirs.insert(CanonicalId::parse(c).unwrap(), plugin_dir(c));
    }

    let lock = load_lock_with_recomputed_digests(&plugin_dirs);

    let project = tempfile::tempdir().expect("project tempdir");
    let home = tempfile::tempdir().expect("home tempdir");
    let project_root = project.path().to_path_buf();
    let home_root = home.path().to_path_buf();

    let lvc = LockValidationContext {
        project_root: project_root.clone(),
        home: home_root.clone(),
        plugin_dirs: plugin_dirs.clone(),
        cache_root: project_root.clone(),
        state_root: project_root.clone(),
    };
    validate::lock(&lock, &lvc).expect("validate::lock on combined m5a fixture lock");

    let mut compiled: BTreeMap<CanonicalId, CompiledPlugin> = BTreeMap::new();
    for canonical in lock.plugins.keys() {
        let pdir = plugin_dirs.get(canonical).expect("plugin_dir present");
        let pctx = PathContext {
            project_root: project_root.clone(),
            home: home_root.clone(),
            plugin_dir: pdir.clone(),
            cache_dir: project_root.clone(),
            state_dir: project_root.clone(),
        };
        let manifest_raw =
            std::fs::read_to_string(pdir.join("rafaello.toml")).expect("read manifest");
        let recomputed = RecomputedDigests {
            content: content_digest(pdir).expect("content_digest"),
            manifest: manifest_digest(&Manifest::parse(&manifest_raw).unwrap().canonical_bytes()),
        };
        let plan = compile_plugin(&lock, canonical, &pctx, &recomputed)
            .unwrap_or_else(|e| panic!("compile_plugin {canonical}: {e:?}"));
        compiled.insert(canonical.clone(), plan);
    }

    let acl = broker_acl::compile(&lock).expect("broker_acl::compile");
    let catalog =
        ToolSchemaCatalog::build(&acl, &compiled, &plugin_dirs).expect("ToolSchemaCatalog::build");
    let names: Vec<&str> = catalog.list().iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["read-file", "send-mail"]);
}
