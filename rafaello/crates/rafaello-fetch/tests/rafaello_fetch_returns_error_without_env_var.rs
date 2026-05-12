mod common;
use common::EnvGuard;
use rafaello_fetch::handle_web_fetch;
use serde_json::json;

#[test]
fn returns_error_without_env_var() {
    let _g = EnvGuard::clear("RFL_FETCH_TEST_BODY_PATH");
    let _g2 = EnvGuard::clear("RFL_FETCH_TEST_LOG_PATH");
    let req = json!({"tool": "web-fetch", "args": {"url": "https://example.com/"}});
    let resp = handle_web_fetch(&req);
    assert_eq!(
        resp,
        json!({"ok": false, "error": "fetch_test_body_unavailable"})
    );
}
