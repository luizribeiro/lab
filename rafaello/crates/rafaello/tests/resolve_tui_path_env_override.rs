use std::ffi::OsString;
use std::path::PathBuf;

use rafaello::resolve_tui_path;

#[test]
fn env_override_wins() {
    let tmp = tempfile::tempdir().unwrap();
    let stub = tmp.path().join("custom-rfl-tui");
    std::fs::write(&stub, b"#!/bin/sh\n").unwrap();

    let stub_os: OsString = stub.clone().into_os_string();
    let env = move |k: &str| -> Option<OsString> {
        if k == "RFL_TUI_PATH" {
            Some(stub_os.clone())
        } else {
            None
        }
    };

    let current_exe = PathBuf::from("/nonexistent/dir/rfl");
    let resolved = resolve_tui_path(&env, &current_exe).unwrap();
    assert_eq!(resolved, stub);
}
