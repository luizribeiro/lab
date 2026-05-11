//! c06 — pi-1 B-8 / scope §M1.1: `scrubber::reject_reserved`
//! still rejects each of the live seven reserved names in both
//! `env.pass` and `env.set`; m5a adds **no new names** to
//! `RESERVED_ENV_VARS` (the list size remains 7).

use std::collections::BTreeMap;

use rafaello_core::error::CompileError;
use rafaello_core::scrubber::{reject_reserved, RESERVED_ENV_VARS};

const SEVEN_CORE_NAMES: &[&str] = &[
    "RFL_BUS_FD",
    "RFL_PLUGIN",
    "RFL_HELPER_FD",
    "RFL_TOPIC_ID",
    "RFL_PROJECT_ROOT",
    "RFL_PRIVATE_STATE_DIR",
    "RFL_PROVIDER_ID",
];

#[test]
fn reserved_env_vars_size_is_seven() {
    assert_eq!(RESERVED_ENV_VARS.len(), 7);
    for name in SEVEN_CORE_NAMES {
        assert!(
            RESERVED_ENV_VARS.contains(name),
            "expected `{name}` in RESERVED_ENV_VARS"
        );
    }
}

#[test]
fn reject_reserved_rejects_each_core_name_in_pass_and_set() {
    for name in SEVEN_CORE_NAMES {
        let pass = vec![(*name).to_owned()];
        let err = reject_reserved(&pass, &BTreeMap::new()).expect_err("must reject pass");
        assert!(
            matches!(err, CompileError::ReservedEnvVarRequested),
            "expected ReservedEnvVarRequested for pass `{name}`, got {err:?}"
        );

        let mut set = BTreeMap::new();
        set.insert((*name).to_owned(), "x".to_owned());
        let err = reject_reserved(&[], &set).expect_err("must reject set");
        assert!(
            matches!(err, CompileError::ReservedEnvVarRequested),
            "expected ReservedEnvVarRequested for set `{name}`, got {err:?}"
        );
    }
}
