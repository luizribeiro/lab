//! c12 — `CanonicalId` rejects path-traversal-shaped sources
//! (scope §L8 — pi review-2 finding 1, pi-2 commits-finding 7).

use rafaello_core::lock::CanonicalId;
use rafaello_core::LockError;

#[test]
fn rejects_parent_dir_segment() {
    assert!(matches!(
        CanonicalId::parse("../escape:foo@1.0.0"),
        Err(LockError::CanonicalIdSourceDotSegment { segment }) if segment == ".."
    ));
}

#[test]
fn rejects_parent_dir_inside_source() {
    assert!(matches!(
        CanonicalId::parse("a/../b:foo@1.0.0"),
        Err(LockError::CanonicalIdSourceDotSegment { segment }) if segment == ".."
    ));
}

#[test]
fn rejects_dot_segment() {
    assert!(matches!(
        CanonicalId::parse("./here:foo@1.0.0"),
        Err(LockError::CanonicalIdSourceDotSegment { segment }) if segment == "."
    ));
}

#[test]
fn rejects_dot_segment_inside_source() {
    assert!(matches!(
        CanonicalId::parse("a/./b:foo@1.0.0"),
        Err(LockError::CanonicalIdSourceDotSegment { segment }) if segment == "."
    ));
}

#[test]
fn rejects_leading_slash() {
    assert!(matches!(
        CanonicalId::parse("/abs:foo@1.0.0"),
        Err(LockError::CanonicalIdSourceLeadingSlash)
    ));
}

#[test]
fn rejects_trailing_slash() {
    assert!(matches!(
        CanonicalId::parse("a/:foo@1.0.0"),
        Err(LockError::CanonicalIdSourceTrailingSlash)
    ));
}

#[test]
fn rejects_double_slash() {
    assert!(matches!(
        CanonicalId::parse("a//b:foo@1.0.0"),
        Err(LockError::CanonicalIdSourceEmptySegment)
    ));
}

#[test]
fn rejects_empty_source() {
    assert!(matches!(
        CanonicalId::parse(":foo@1.0.0"),
        Err(LockError::CanonicalIdEmptySource)
    ));
}
