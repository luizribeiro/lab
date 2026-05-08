//! c26 — Lock TOML with `bindings.tool_meta.<n>.grant_match =
//! "../../escape.json"` is rejected by `SafePath` re-validation
//! during lock parse (and therefore never reaches V3 with a
//! traversal-bearing grant_match).

use rafaello_core::lock::Lock;

const FIXTURE: &str = r#"
[plugin."github.com/acme:grep@1.4.2"]
entry = "bin/grep.js"
digest = "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
manifest_digest = "sha256:fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
granted_at = "2026-01-15T08:30:00Z"

[plugin."github.com/acme:grep@1.4.2".bindings]
provider = false
tools = ["grep"]

[plugin."github.com/acme:grep@1.4.2".bindings.tool_meta.grep]
sinks = []
sinks_inferred = false
grant_match = "../../escape.json"
always_confirm = false

[plugin."github.com/acme:grep@1.4.2".flags]
i_know_what_im_doing = false
allow_credential_paths = false

[session]
"#;

#[test]
fn grant_match_traversal_rejected_by_lock_parse() {
    assert!(Lock::from_toml(FIXTURE).is_err());
}
