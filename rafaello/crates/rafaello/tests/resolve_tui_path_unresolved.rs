use std::ffi::OsString;

use rafaello::{resolve_tui_path, RflChatError};

#[test]
fn unresolved_when_neither_env_nor_sibling_present() {
    let tmp = tempfile::tempdir().unwrap();
    let current_exe = tmp.path().join("rfl");
    std::fs::write(&current_exe, b"#!/bin/sh\n").unwrap();

    let env = |_k: &str| -> Option<OsString> { None };

    let err = resolve_tui_path(&env, &current_exe).unwrap_err();
    assert!(matches!(err, RflChatError::TuiPathUnresolved));
}
