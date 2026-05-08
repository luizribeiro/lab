//! c12 — `CanonicalId::parse` rejects malformed inputs (scope §L8).

use rafaello_core::lock::CanonicalId;
use rafaello_core::LockError;

#[test]
fn rejects_uppercase_in_name() {
    // pi review-2: `[plugin."github:acme/Grep@1.4"]` — uppercase
    // `G` violates the topic-segment grammar.
    assert!(matches!(
        CanonicalId::parse("github.com/acme:Grep@1.4.2"),
        Err(LockError::CanonicalIdIllegalName { .. })
    ));
}

#[test]
fn rejects_missing_patch_in_version() {
    // pi review-2: `1.4` is not a valid semver Version.
    assert!(matches!(
        CanonicalId::parse("github.com/acme:grep@1.4"),
        Err(LockError::CanonicalIdInvalidVersion { .. })
    ));
}

#[test]
fn rejects_missing_name_separator() {
    assert!(matches!(
        CanonicalId::parse("local-foo@1.0.0"),
        Err(LockError::CanonicalIdMissingNameSeparator { .. })
    ));
}

#[test]
fn rejects_missing_version_separator() {
    assert!(matches!(
        CanonicalId::parse("local:foo-1.0.0"),
        Err(LockError::CanonicalIdMissingVersionSeparator { .. })
    ));
}

#[test]
fn rejects_empty_name() {
    assert!(matches!(
        CanonicalId::parse("local:@1.0.0"),
        Err(LockError::CanonicalIdIllegalName { .. })
    ));
}

#[test]
fn rejects_name_starting_with_dash() {
    assert!(matches!(
        CanonicalId::parse("local:-foo@1.0.0"),
        Err(LockError::CanonicalIdIllegalName { .. })
    ));
}

#[test]
fn rejects_name_with_dot() {
    assert!(matches!(
        CanonicalId::parse("local:foo.bar@1.0.0"),
        Err(LockError::CanonicalIdIllegalName { .. })
    ));
}

#[test]
fn rejects_garbage_version() {
    assert!(matches!(
        CanonicalId::parse("local:foo@not-a-version"),
        Err(LockError::CanonicalIdInvalidVersion { .. })
    ));
}

#[test]
fn rejects_uppercase_in_source() {
    assert!(matches!(
        CanonicalId::parse("GitHub.com/acme:grep@1.0.0"),
        Err(LockError::CanonicalIdIllegalSourceSegment { .. })
    ));
}

#[test]
fn rejects_source_with_space() {
    assert!(matches!(
        CanonicalId::parse("git hub:foo@1.0.0"),
        Err(LockError::CanonicalIdIllegalSourceSegment { .. })
    ));
}
