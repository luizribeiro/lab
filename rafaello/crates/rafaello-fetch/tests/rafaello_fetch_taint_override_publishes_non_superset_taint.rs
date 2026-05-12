mod common;
use common::EnvGuard;
use rafaello_core::bus::JsonRpcId;
use rafaello_fetch::{compute_publish_params, reset_taint_override_for_test, TaintEntry};
use serde_json::json;

#[test]
fn taint_override_publishes_non_superset_taint() {
    reset_taint_override_for_test();
    let dir = tempfile::tempdir().expect("tempdir");
    let body = dir.path().join("body.txt");
    std::fs::write(&body, "body").expect("body");

    let override_taint =
        json!([{"source": "fixture://override", "detail": "non-superset"}]).to_string();

    let _b = EnvGuard::set("RFL_FETCH_TEST_BODY_PATH", body.to_str().unwrap());
    let _l = EnvGuard::clear("RFL_FETCH_TEST_LOG_PATH");
    let _t = EnvGuard::set("RFL_FETCH_TEST_TAINT_OVERRIDE", &override_taint);

    let req = json!({"args": {"url": "https://example.com/"}});
    let bus_id = JsonRpcId::String("req-1".to_string());
    let params = compute_publish_params(&req, bus_id, "plugin.fetch.tool_result");

    let taint = params
        .get("taint")
        .expect("taint field present when override active");
    let parsed: Vec<TaintEntry> = serde_json::from_value(taint.clone()).expect("taint shape");
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].source, "fixture://override");
    assert_eq!(parsed[0].detail.as_deref(), Some("non-superset"));

    reset_taint_override_for_test();
}
