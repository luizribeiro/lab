//! c13 — `LoadPolicy` table form round-trips through TOML.

use rafaello_core::lock::LoadPolicy;

#[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
struct Wrap {
    load: LoadPolicy,
}

#[test]
fn lazy_table_round_trips() {
    let original = Wrap {
        load: LoadPolicy::Lazy {
            event: vec!["bus.note.created".to_owned()],
            command: vec!["grep".to_owned()],
            kind: vec!["acme.terminal".to_owned()],
        },
    };
    let s = toml::to_string(&original).expect("serialize");
    let back: Wrap = toml::from_str(&s).expect("parse");
    assert_eq!(original, back);
}

#[test]
fn boot_string_round_trips() {
    let original = Wrap {
        load: LoadPolicy::Boot,
    };
    let s = toml::to_string(&original).expect("serialize");
    assert!(s.contains(r#"load = "boot""#));
    let back: Wrap = toml::from_str(&s).expect("parse");
    assert_eq!(original, back);
}

#[test]
fn manual_string_round_trips() {
    let original = Wrap {
        load: LoadPolicy::Manual,
    };
    let s = toml::to_string(&original).expect("serialize");
    let back: Wrap = toml::from_str(&s).expect("parse");
    assert_eq!(original, back);
}

#[test]
fn lazy_string_decodes_as_empty_lazy() {
    let back: Wrap = toml::from_str(r#"load = "lazy""#).expect("parse");
    assert_eq!(
        back.load,
        LoadPolicy::Lazy {
            event: vec![],
            command: vec![],
            kind: vec![],
        }
    );
}
