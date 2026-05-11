//! c27 — `rfl install --i-know-what-im-doing` against the trifecta
//! manifest succeeds; lock entry's `flags.i_know_what_im_doing == true`.

mod common;

use common::install_test_kit::{run_install, write_fixture, TRIFECTA_MANIFEST};

#[test]
fn rfl_install_accepts_trifecta_plugin_with_override() {
    let project = tempfile::tempdir().unwrap();
    let fixture = tempfile::tempdir().unwrap();
    write_fixture(fixture.path(), TRIFECTA_MANIFEST);

    let out = run_install(project.path(), fixture.path(), &["--i-know-what-im-doing"]);
    assert!(
        out.status.success(),
        "install failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let lock = common::install_test_kit::read_lock(project.path());
    let canonical = rafaello_core::lock::CanonicalId::parse("local:trifecta@0.0.0").unwrap();
    let entry = lock.plugins.get(&canonical).expect("entry present");
    assert!(entry.flags.i_know_what_im_doing);
}
