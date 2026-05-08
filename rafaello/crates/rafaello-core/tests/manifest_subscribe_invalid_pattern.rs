//! Negative: a `**` segment that is not the final segment in a
//! subscribe pattern is rejected as `InvalidPatternSegment`
//! (security RFC §5.1).

use rafaello_core::error::ValidationError;
use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

#[test]
fn double_star_not_final_rejected() {
    let src = r#"
schema = 1
name = "subscriber"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[bus]
subscribes = ["core.**.session"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    match validate::manifest_standalone(&m) {
        Err(ValidationError::InvalidPatternSegment { pattern, segment }) => {
            assert_eq!(pattern, "core.**.session");
            assert_eq!(segment, "**");
        }
        other => panic!("expected InvalidPatternSegment, got {other:?}"),
    }
}

#[test]
fn in_segment_glob_rejected() {
    let src = r#"
schema = 1
name = "subscriber"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[bus]
subscribes = ["grep.*foo"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    assert!(matches!(
        validate::manifest_standalone(&m),
        Err(ValidationError::InvalidPatternSegment { .. })
    ));
}
