//! c05 §B3 regression anchor — m5a-ratified `rfl install --fixture
//! <path>` keeps working; the new PP1 copy materialises a
//! `.rafaello/plugins/<topic-id>/` directory containing the package
//! tree.

mod common;

use common::install_test_kit::{run_install, write_fixture, BENIGN_MANIFEST};
use rafaello_core::topic_id;

#[test]
fn rfl_install_fixture_flag_still_works() {
    let project = tempfile::tempdir().unwrap();
    let fixture = tempfile::tempdir().unwrap();
    write_fixture(fixture.path(), BENIGN_MANIFEST);

    let out = run_install(project.path(), fixture.path(), &[]);
    assert!(
        out.status.success(),
        "install failed: stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );

    let topic = topic_id::derive("local:readfile@0.0.0");
    let materialised = project
        .path()
        .join(".rafaello")
        .join("plugins")
        .join(&topic);
    assert!(
        materialised.is_dir(),
        "PP1 dir missing at {}",
        materialised.display()
    );
    assert!(
        materialised.join("rafaello.toml").is_file(),
        "PP1 manifest not copied"
    );
    assert!(
        materialised.join("bin").join("x").is_file(),
        "PP1 entry not copied"
    );
}
