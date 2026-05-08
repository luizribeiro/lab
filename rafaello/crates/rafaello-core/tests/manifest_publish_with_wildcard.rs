//! Negative: a `*` or `**` segment in `bus.publishes` is rejected as
//! `PatternInPublishPosition` (publishes must be topics, not
//! patterns).

use rafaello_core::error::ValidationError;
use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

#[test]
fn wildcard_in_publish_rejected() {
    let src = r#"
schema = 1
name = "naughty"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[bus]
publishes = ["plugin.id_xx.*"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    match validate::manifest_standalone(&m) {
        Err(ValidationError::PatternInPublishPosition { topic }) => {
            assert_eq!(topic, "plugin.id_xx.*");
        }
        other => panic!("expected PatternInPublishPosition, got {other:?}"),
    }
}

#[test]
fn double_star_in_publish_rejected() {
    let src = r#"
schema = 1
name = "naughty"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[bus]
publishes = ["plugin.id_xx.**"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    assert!(matches!(
        validate::manifest_standalone(&m),
        Err(ValidationError::PatternInPublishPosition { .. })
    ));
}
