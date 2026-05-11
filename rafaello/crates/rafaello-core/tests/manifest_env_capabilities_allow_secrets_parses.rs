//! c06 — scope §OP6: manifest `[capabilities.default.env]`
//! `allow_secrets` parses and round-trips through serialization.

use rafaello_core::manifest::Manifest;

#[test]
fn allow_secrets_round_trips() {
    let src = r#"
schema = 1
name = "openai"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[capabilities.default.env]
allow_secrets = ["LITELLM_API_KEY", "OPENAI_API_KEY"]
"#;
    let m = Manifest::parse(src).expect("parse should succeed");
    let caps = m.capabilities.as_ref().expect("capabilities present");
    let env = caps
        .get("default")
        .and_then(|b| b.env.as_ref())
        .expect("env present");
    assert_eq!(
        env.allow_secrets,
        vec!["LITELLM_API_KEY".to_owned(), "OPENAI_API_KEY".to_owned()]
    );

    let toml_out = toml::to_string(&m).expect("serialise");
    let m2 = Manifest::parse(&toml_out).expect("re-parse");
    let env2 = m2
        .capabilities
        .as_ref()
        .and_then(|c| c.get("default"))
        .and_then(|b| b.env.as_ref())
        .expect("env present");
    assert_eq!(env2.allow_secrets, env.allow_secrets);
}
