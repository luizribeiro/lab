//! c06 — scope §OP6: `validate::lock` rejects an `allow_secrets`
//! entry that overlaps `RESERVED_ENV_VARS`. The
//! `reject_reserved` step (which only checks `pass` and `set`)
//! is not sufficient for `allow_secrets` per pi-4 M-4.

mod common;

use std::collections::BTreeMap;

use rafaello_core::error::ValidationError;
use rafaello_core::lock::{Grant, GrantBundle, GrantEnv, SessionTable};
use rafaello_core::validate;

use common::{canonical, ctx_for, entry, lock_with};

#[test]
fn reserved_name_in_allow_secrets_is_rejected() {
    let id = canonical("github.com/acme:writer@1.0.0");
    let mut e = entry(&["writer"], false, None);
    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            env: Some(GrantEnv {
                pass: Vec::new(),
                set: BTreeMap::new(),
                allow_secrets: vec!["RFL_BUS_FD".to_owned()],
            }),
            ..GrantBundle::default()
        },
    );
    e.grant = Grant {
        bundles,
        ..Grant::default()
    };

    let lock = lock_with(vec![(id.clone(), e)], SessionTable::default());
    let ctx = ctx_for(&[&id]);
    let err = validate::lock(&lock, &ctx).expect_err("must reject");
    assert!(
        matches!(
            err,
            ValidationError::AllowSecretsReservesCoreName { ref name } if name == "RFL_BUS_FD"
        ),
        "got {err:?}"
    );
}
