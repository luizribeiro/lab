mod common;
use common::EnvGuard;
use rafaello_fetch::handle_web_fetch;
use serde_json::json;

#[test]
fn returns_body_from_env_var_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let body_path = dir.path().join("body.txt");
    std::fs::write(&body_path, "hello from fixture body").expect("write body");

    let _guard = EnvGuard::set("RFL_FETCH_TEST_BODY_PATH", body_path.to_str().unwrap());
    let _log_guard = EnvGuard::clear("RFL_FETCH_TEST_LOG_PATH");

    let req = json!({"tool": "web-fetch", "args": {"url": "https://example.com/x"}});
    let resp = handle_web_fetch(&req);
    assert_eq!(
        resp,
        json!({"ok": true, "content": "hello from fixture body"})
    );
}
