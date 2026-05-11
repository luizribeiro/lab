//! c06 — scope §OP6: `scrubber::strip` retains a secret-pattern
//! name when it is listed in `allow_secrets`, without forcing
//! `i_know_what_im_doing`.

use rafaello_core::scrubber::strip;

#[test]
fn allow_secrets_retains_listed_name() {
    let env_pass = vec!["LITELLM_API_KEY".to_owned()];
    let allow_secrets = vec!["LITELLM_API_KEY".to_owned()];
    let out = strip(&env_pass, &allow_secrets, false);
    assert_eq!(out, vec!["LITELLM_API_KEY".to_owned()]);
}
