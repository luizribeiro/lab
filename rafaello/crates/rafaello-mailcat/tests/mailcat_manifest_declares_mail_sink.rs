//! c30 / scope §TP1: the in-tree mailcat manifest parses under
//! the live m1 schema and declares `sinks = ["mail"]` for its
//! `send-mail` tool.

use std::path::PathBuf;

use rafaello_core::manifest::Manifest;

fn manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rafaello.toml")
}

#[test]
fn manifest_declares_mail_sink() {
    let raw = std::fs::read_to_string(manifest_path()).expect("read manifest");
    let parsed = Manifest::parse(&raw).expect("parse manifest");

    let provides = parsed.provides.as_ref().expect("provides present");
    assert_eq!(provides.tools, vec!["send-mail".to_string()]);

    let meta = provides
        .tool
        .get("send-mail")
        .expect("[provides.tool.send-mail] entry present");
    assert_eq!(
        meta.sinks.as_deref(),
        Some(&["mail".to_string()][..]),
        "send-mail must declare sinks = [\"mail\"]"
    );
    assert!(!meta.always_confirm);
    assert_eq!(
        meta.grant_match.as_ref().map(|p| p.as_str()),
        Some("schemas/send-mail-grant.json")
    );
}
