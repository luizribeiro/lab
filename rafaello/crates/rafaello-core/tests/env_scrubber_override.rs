//! c21 — scope §Sc2: override flag returns input verbatim.

use rafaello_core::scrubber::strip;

#[test]
fn override_flag_preserves_input() {
    let env_pass = vec![
        "GITHUB_TOKEN".to_owned(),
        "OPENAI_API_KEY".to_owned(),
        "MY_PASSWORD".to_owned(),
        "AWS_PROFILE".to_owned(),
    ];
    assert_eq!(strip(&env_pass, &[], true), env_pass);
}
