mod common;
use common::EnvGuard;
use rafaello_fetch::handle_web_fetch;
use serde_json::json;

#[test]
fn returns_error_on_missing_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("does-not-exist.txt");
    let _g = EnvGuard::set("RFL_FETCH_TEST_BODY_PATH", missing.to_str().unwrap());
    let _g2 = EnvGuard::clear("RFL_FETCH_TEST_LOG_PATH");
    let req = json!({"tool": "web-fetch", "args": {"url": "https://example.com/"}});
    let resp = handle_web_fetch(&req);
    assert_eq!(
        resp,
        json!({"ok": false, "error": "fetch_test_body_unavailable"})
    );
}
