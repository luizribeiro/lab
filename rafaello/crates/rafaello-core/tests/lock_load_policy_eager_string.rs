//! c13 — `LoadPolicy` eager-shorthand string decodes correctly.

use rafaello_core::lock::LoadPolicy;

#[derive(serde::Deserialize, Debug, PartialEq)]
struct Wrap {
    load: LoadPolicy,
}

#[test]
fn eager_string_decodes_as_eager() {
    let w: Wrap = toml::from_str(r#"load = "eager""#).expect("parse");
    assert_eq!(w.load, LoadPolicy::Eager);
}

#[test]
fn unknown_string_rejected() {
    let err = toml::from_str::<Wrap>(r#"load = "ferocious""#).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("ferocious") || msg.contains("unknown"), "got: {msg}");
}
