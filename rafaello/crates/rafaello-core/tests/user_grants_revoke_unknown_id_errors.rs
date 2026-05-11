use rafaello_core::user_grants::{GrantId, RevokeError, UserGrants};
use ulid::Ulid;

#[test]
fn revoke_unknown_id_errors() {
    let mut grants = UserGrants::new();
    let bogus = GrantId(Ulid::new());
    let err = grants.revoke(bogus).expect_err("expected error");
    match err {
        RevokeError::Unknown(id) => assert_eq!(id, bogus),
    }
}
