mod common;

use std::os::unix::fs::PermissionsExt;

use common::workspace_bin_path::workspace_bin;

#[test]
fn resolves_rfl_bus_fixture() {
    let path = workspace_bin("rfl-bus-fixture");
    assert!(
        path.is_absolute(),
        "path is not absolute: {}",
        path.display()
    );
    assert!(path.is_file(), "not a file: {}", path.display());
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert!(
        mode & 0o111 != 0,
        "not executable: {} mode={:o}",
        path.display(),
        mode
    );
}
