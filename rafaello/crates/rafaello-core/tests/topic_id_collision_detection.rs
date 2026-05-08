//! T2/T3: forced collision via the public `collisions_with_prefixes` helper.

use rafaello_core::error::CollisionError;
use rafaello_core::lock::CanonicalId;
use rafaello_core::topic_id;

#[test]
fn collisions_with_prefixes_rejects_duplicate_prefix() {
    let a = CanonicalId::parse("github/acme:grep@1.4.2").unwrap();
    let b = CanonicalId::parse("github/other:ripgrep@2.0.0").unwrap();
    let pairs = vec![
        (a.clone(), "id_synthetic_collide".to_owned()),
        (b.clone(), "id_synthetic_collide".to_owned()),
    ];
    let err = topic_id::collisions_with_prefixes(&pairs).unwrap_err();
    match err {
        CollisionError::TopicIdCollision { prefix, .. } => {
            assert_eq!(prefix, "id_synthetic_collide");
        }
        _ => panic!("expected TopicIdCollision"),
    }
}

#[test]
fn collisions_with_prefixes_accepts_distinct_prefixes() {
    let a = CanonicalId::parse("github/acme:grep@1.4.2").unwrap();
    let b = CanonicalId::parse("github/other:ripgrep@2.0.0").unwrap();
    let pairs = vec![
        (a, "id_aaaaaaaaaaaaaaaa".to_owned()),
        (b, "id_bbbbbbbbbbbbbbbb".to_owned()),
    ];
    assert!(topic_id::collisions_with_prefixes(&pairs).is_ok());
}

#[test]
fn collisions_for_real_canonical_ids_are_distinct() {
    let a = CanonicalId::parse("github/acme:grep@1.4.2").unwrap();
    let b = CanonicalId::parse("github/acme:grep@1.4.3").unwrap();
    assert!(topic_id::collisions(&[a, b]).is_ok());
}
