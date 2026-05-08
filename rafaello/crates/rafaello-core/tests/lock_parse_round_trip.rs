//! c13 — Full lock TOML parse → reserialise → reparse round-trip.

use rafaello_core::lock::Lock;

const FIXTURE: &str = r#"
[plugin."github.com/acme:grep@1.4.2"]
entry = "bin/grep.js"
digest = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
manifest_digest = "sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
granted_at = "2026-01-15T08:30:00Z"

[plugin."github.com/acme:grep@1.4.2".grant]
subscribes = ["core.cmd.grep"]
publishes = ["plugin.grep.result"]

[plugin."github.com/acme:grep@1.4.2".grant.bundles.default.filesystem]
read_paths = ["${project}/**"]

[plugin."github.com/acme:grep@1.4.2".grant.bundles.default.network]
mode = "proxy"
allow_hosts = ["api.example.com"]

[plugin."github.com/acme:grep@1.4.2".bindings]
provider = false
tools = ["grep"]
renderer_kinds = []
load = "manual"

[plugin."github.com/acme:grep@1.4.2".bindings.tool_meta.grep]
sinks = ["workspace_write"]
sinks_inferred = false
always_confirm = false

[plugin."github.com/acme:grep@1.4.2".flags]
i_know_what_im_doing = false
allow_credential_paths = false

[session]
provider_active = "local:foo@1.0.0"

[session.tool_owner]
grep = "github.com/acme:grep@1.4.2"
"#;

#[test]
fn round_trip_full_lock() {
    let lock = Lock::from_toml(FIXTURE).expect("parse");
    let reserialised = lock.to_toml();
    let reparsed = Lock::from_toml(&reserialised).expect("reparse");
    assert_eq!(lock, reparsed, "lock round-trips through to_toml/from_toml");
}

#[test]
fn empty_lock_round_trips() {
    let lock = Lock::default();
    let s = lock.to_toml();
    let back = Lock::from_toml(&s).expect("parse empty");
    assert_eq!(lock, back);
}
