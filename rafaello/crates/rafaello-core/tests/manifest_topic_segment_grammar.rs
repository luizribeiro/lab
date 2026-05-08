//! Negative: a topic / pattern segment violating `[a-z0-9_-]+`
//! (uppercase) is rejected as `IllegalTopicSegment` (per security
//! RFC §5.1, scope §V1).

use rafaello_core::error::ValidationError;
use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

#[test]
fn uppercase_publish_segment_rejected() {
    let src = r#"
schema = 1
name = "events"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[bus]
publishes = ["FOO.bar"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    match validate::manifest_standalone(&m) {
        Err(ValidationError::IllegalTopicSegment { topic, segment }) => {
            assert_eq!(topic, "FOO.bar");
            assert_eq!(segment, "FOO");
        }
        other => panic!("expected IllegalTopicSegment, got {other:?}"),
    }
}
