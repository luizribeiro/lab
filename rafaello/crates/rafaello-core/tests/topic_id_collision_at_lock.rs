//! c22 — Two distinct canonical ids whose pre-computed prefixes match
//! surface as `CollisionError` via the public `collisions_with_prefixes`
//! seam that `validate::lock` delegates to (T2).

use rafaello_core::error::CollisionError;
use rafaello_core::lock::CanonicalId;
use rafaello_core::topic_id;

#[test]
fn synthetic_prefix_collision_at_lock_level() {
    let a = CanonicalId::parse("github.com/acme:alpha@1.0.0").unwrap();
    let b = CanonicalId::parse("github.com/other:beta@1.0.0").unwrap();
    let pairs = vec![
        (a, "id_collide_at_lock".to_owned()),
        (b, "id_collide_at_lock".to_owned()),
    ];
    let err = topic_id::collisions_with_prefixes(&pairs).unwrap_err();
    assert!(matches!(err, CollisionError::TopicIdCollision { .. }));
}
