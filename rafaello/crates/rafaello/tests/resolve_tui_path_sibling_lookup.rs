use std::ffi::OsString;

use rafaello::resolve_tui_path;

#[test]
fn sibling_of_current_exe_is_returned() {
    let tmp = tempfile::tempdir().unwrap();
    let bin_dir = tmp.path();
    let current_exe = bin_dir.join("rfl");
    std::fs::write(&current_exe, b"#!/bin/sh\n").unwrap();
    let sibling = bin_dir.join("rfl-tui");
    std::fs::write(&sibling, b"#!/bin/sh\n").unwrap();

    let env = |_k: &str| -> Option<OsString> { None };

    let resolved = resolve_tui_path(&env, &current_exe).unwrap();
    assert_eq!(resolved, sibling);
}
