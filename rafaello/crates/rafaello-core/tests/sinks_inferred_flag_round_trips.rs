//! c13 — `tool_meta.sinks_inferred` discriminator round-trips.
//!
//! Per scope §L4 / pi review-2 finding 2: the lock records whether
//! the snapshotted sink list came from the manifest's explicit
//! declaration (`false`) or was inferred at install time (`true`).

use rafaello_core::lock::ToolMeta;

#[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
struct Wrap {
    tool_meta: ToolMeta,
}

#[test]
fn sinks_inferred_true_round_trips() {
    let original = Wrap {
        tool_meta: ToolMeta {
            sinks: vec!["workspace_write".to_owned()],
            sinks_inferred: true,
            grant_match: None,
            always_confirm: false,
        },
    };
    let s = toml::to_string(&original).expect("serialize");
    assert!(s.contains("sinks_inferred = true"));
    let back: Wrap = toml::from_str(&s).expect("parse");
    assert_eq!(original, back);
}

#[test]
fn sinks_inferred_false_round_trips() {
    let original = Wrap {
        tool_meta: ToolMeta {
            sinks: vec!["network".to_owned()],
            sinks_inferred: false,
            grant_match: None,
            always_confirm: true,
        },
    };
    let s = toml::to_string(&original).expect("serialize");
    let back: Wrap = toml::from_str(&s).expect("parse");
    assert_eq!(original, back);
    assert!(!back.tool_meta.sinks_inferred);
    assert!(back.tool_meta.always_confirm);
}
