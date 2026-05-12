//! c07 §B3 — table-driven acceptance: for each bundled plugin
//! promoted in c06 (`rfl-mailcat`, `rfl-readfile`, `rafaello-fetch`,
//! `rfl-mockprovider`), `rfl install <name>` against a constructed
//! release tree (`<tmpdir>/share/rafaello/plugins/<name>/`) writes
//! the expected `[plugin."<canonical-id>"]` entry with non-empty
//! `digest` and `manifest_digest`.

mod common;

use std::process::Command;

use common::install_test_kit::copy_in_tree_to_bundled_dir;
use common::workspace_bin_path::workspace_bin;
use rafaello_core::lock::{CanonicalId, Lock};

struct Case {
    bundled_name: &'static str,
    crate_dir: &'static str,
    canonical: &'static str,
}

const CASES: &[Case] = &[
    Case {
        bundled_name: "rfl-mailcat",
        crate_dir: "rafaello-mailcat",
        canonical: "local:mailcat@0.0.0",
    },
    Case {
        bundled_name: "rfl-readfile",
        crate_dir: "rafaello-readfile",
        canonical: "local:readfile@0.0.0",
    },
    Case {
        bundled_name: "rafaello-fetch",
        crate_dir: "rafaello-fetch",
        canonical: "local:rafaello-fetch@0.0.0",
    },
    Case {
        bundled_name: "rfl-mockprovider",
        crate_dir: "rafaello-mockprovider",
        canonical: "local:mockprovider@0.0.0",
    },
];

#[test]
fn rfl_install_writes_lock_entry_for_each_bundled_plugin() {
    for case in CASES {
        let project = tempfile::tempdir().unwrap();
        let release = tempfile::tempdir().unwrap();
        let plugins_root = release
            .path()
            .join("share")
            .join("rafaello")
            .join("plugins");
        std::fs::create_dir_all(&plugins_root).unwrap();
        copy_in_tree_to_bundled_dir(&plugins_root, case.bundled_name, case.crate_dir);

        let rfl = workspace_bin("rfl");
        let out = Command::new(rfl)
            .current_dir(project.path())
            .args(["install", case.bundled_name])
            .env("RFL_BUNDLED_PLUGINS_DIR", &plugins_root)
            .output()
            .expect("spawn rfl install");
        assert!(
            out.status.success(),
            "rfl install {} failed: stderr={}",
            case.bundled_name,
            String::from_utf8_lossy(&out.stderr)
        );

        let lock_raw =
            std::fs::read_to_string(project.path().join("rafaello.lock")).expect("read lock");
        let lock = Lock::from_toml(&lock_raw).expect("parse lock");
        let canonical = CanonicalId::parse(case.canonical).unwrap();
        let entry = lock
            .plugins
            .get(&canonical)
            .unwrap_or_else(|| panic!("missing lock entry for {}", case.canonical));
        assert!(
            !entry.digest.is_empty(),
            "{}: empty content digest",
            case.canonical
        );
        assert!(
            !entry.manifest_digest.is_empty(),
            "{}: empty manifest digest",
            case.canonical
        );
    }
}
