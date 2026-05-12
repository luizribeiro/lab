mod common;
use common::EnvGuard;
use rafaello_fetch::handle_web_fetch;
use serde_json::json;

#[test]
fn writes_invocation_log_when_log_path_set() {
    let dir = tempfile::tempdir().expect("tempdir");
    let body_path = dir.path().join("body.txt");
    std::fs::write(&body_path, "x").expect("write body");
    let log_path = dir.path().join("invocations.log");

    let _b = EnvGuard::set("RFL_FETCH_TEST_BODY_PATH", body_path.to_str().unwrap());
    let _l = EnvGuard::set("RFL_FETCH_TEST_LOG_PATH", log_path.to_str().unwrap());

    let r1 = json!({"args": {"url": "https://a.example/one"}});
    let r2 = json!({"args": {"url": "https://b.example/two"}});
    let _ = handle_web_fetch(&r1);
    let _ = handle_web_fetch(&r2);

    let contents = std::fs::read_to_string(&log_path).expect("read log");
    let lines: Vec<&str> = contents.lines().collect();
    assert_eq!(lines.len(), 2, "expected two lines, got: {contents:?}");
    assert_eq!(lines[0], "web-fetch: https://a.example/one");
    assert_eq!(lines[1], "web-fetch: https://b.example/two");
}
