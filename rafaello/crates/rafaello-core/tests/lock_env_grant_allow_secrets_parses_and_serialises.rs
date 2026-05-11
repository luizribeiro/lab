//! c06 — scope §OP6: lock-side `GrantEnv.allow_secrets` parses and
//! round-trips through `toml::to_string` / `toml::from_str`.

use rafaello_core::lock::GrantEnv;

#[test]
fn allow_secrets_round_trips_via_toml() {
    let src = r#"
allow_secrets = ["LITELLM_API_KEY", "OPENAI_API_KEY"]
"#;
    let env: GrantEnv = toml::from_str(src).expect("parse");
    assert_eq!(
        env.allow_secrets,
        vec!["LITELLM_API_KEY".to_owned(), "OPENAI_API_KEY".to_owned()]
    );
    let s = toml::to_string(&env).expect("serialise");
    let env2: GrantEnv = toml::from_str(&s).expect("re-parse");
    assert_eq!(env2.allow_secrets, env.allow_secrets);
}

#[test]
fn empty_allow_secrets_is_skipped_on_serialise() {
    let env = GrantEnv::default();
    let s = toml::to_string(&env).expect("serialise");
    assert!(
        !s.contains("allow_secrets"),
        "empty allow_secrets should be skipped, got: {s}"
    );
}
