//! v1 `StreamState` only admits `final`. Streaming patches (`open`,
//! `patch`, `closed`) land in a future commit; until then a producer that
//! emits them must fail loudly at the decode boundary.

use rafaello_core::StreamState;
use serde_json::json;

#[test]
fn stream_state_open_is_rejected() {
    let err = serde_json::from_value::<StreamState>(json!("open")).unwrap_err();
    assert!(
        err.to_string().contains("unknown variant"),
        "expected unknown-variant error for `open`, got: {err}"
    );
}

#[test]
fn stream_state_final_decodes() {
    let v: StreamState = serde_json::from_value(json!("final")).unwrap();
    assert_eq!(v, StreamState::Final);
}
