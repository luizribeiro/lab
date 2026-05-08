//! Negative: a single-segment topic is rejected as
//! `TopicTooFewSegments` per security RFC §5.1 `topic := segment
//! ("." segment)+` (pi-5 medium 10).

use rafaello_core::error::ValidationError;
use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

#[test]
fn single_segment_publish_rejected() {
    let src = r#"
schema = 1
name = "events"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[bus]
publishes = ["solo"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    match validate::manifest_standalone(&m) {
        Err(ValidationError::TopicTooFewSegments { topic }) => assert_eq!(topic, "solo"),
        other => panic!("expected TopicTooFewSegments, got {other:?}"),
    }
}
