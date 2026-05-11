//! c21 — scope §Sc2 positive: `strip` removes literal-and-glob
//! secret matches and preserves non-matching entries.

use rafaello_core::scrubber::strip;

#[test]
fn strips_known_secrets_keeps_path() {
    let env_pass = vec![
        "GITHUB_TOKEN".to_owned(),
        "OPENAI_API_KEY".to_owned(),
        "AWS_REGION".to_owned(),
        "PATH".to_owned(),
    ];
    assert_eq!(strip(&env_pass, &[], false), vec!["PATH".to_owned()]);
}
