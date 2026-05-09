//! T1: pinned-fixture topic-id derivation.

use rafaello_core::topic_id;

#[test]
fn derive_pins_known_canonical_id() {
    // sha256("github:acme/grep@1.4.2")[..10] base32-no-pad-lower
    assert_eq!(
        topic_id::derive("github:acme/grep@1.4.2"),
        "id_lpeaauhytsptc7qq"
    );
}

#[test]
fn derive_is_deterministic() {
    let a = topic_id::derive("github:acme/grep@1.4.2");
    let b = topic_id::derive("github:acme/grep@1.4.2");
    assert_eq!(a, b);
}

#[test]
fn derive_changes_with_input() {
    assert_ne!(
        topic_id::derive("github:acme/grep@1.4.2"),
        topic_id::derive("github:acme/grep@1.4.3")
    );
}

#[test]
fn derive_prefix_shape() {
    let id = topic_id::derive("github:acme/grep@1.4.2");
    assert!(id.starts_with("id_"));
    let body = &id[3..];
    assert_eq!(body.len(), 16);
    assert!(body
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
}
