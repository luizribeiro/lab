//! c06 §B2 — in-tree `rafaello.toml` parses via `Manifest::parse`
//! and the sibling `openrpc.json` deserialises as JSON. Pins the
//! bundled-plugin tree shape that Phase F2 copies to
//! `$out/share/rafaello/plugins/rfl-mailcat/`.

use std::path::PathBuf;

use rafaello_core::manifest::Manifest;

#[test]
fn bundled_manifest_parses() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest_raw = std::fs::read_to_string(dir.join("rafaello.toml")).unwrap();
    Manifest::parse(&manifest_raw).expect("in-tree rafaello.toml parses");
    let openrpc_raw = std::fs::read_to_string(dir.join("openrpc.json")).unwrap();
    serde_json::from_str::<serde_json::Value>(&openrpc_raw)
        .expect("in-tree openrpc.json deserialises");
}
