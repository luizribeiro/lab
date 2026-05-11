//! c06 — scope §OP6: `scrubber::strip` strips secret-pattern names
//! that are **not** in `allow_secrets`, even when `allow_secrets`
//! is non-empty. The opt-in is per-name, not per-list.

use rafaello_core::scrubber::strip;

#[test]
fn unlisted_secret_is_stripped_when_allow_secrets_present() {
    let env_pass = vec!["LITELLM_API_KEY".to_owned(), "RANDOM_API_KEY".to_owned()];
    let allow_secrets = vec!["LITELLM_API_KEY".to_owned()];
    let out = strip(&env_pass, &allow_secrets, false);
    assert_eq!(out, vec!["LITELLM_API_KEY".to_owned()]);
}
