//! c21 — scope §Sc1 glob-coverage negative: every glob class in
//! `SECRET_PATTERNS` strips the corresponding entry.

use rafaello_core::scrubber::strip;

#[test]
fn every_glob_class_strips() {
    let env_pass = vec![
        "GITHUB_TOKEN".to_owned(),
        "OPENAI_API_KEY".to_owned(),
        "MY_PASSWORD".to_owned(),
        "AWS_PROFILE".to_owned(),
    ];
    let out = strip(&env_pass, false);
    assert!(
        out.is_empty(),
        "all four entries should be stripped; got {out:?}"
    );
}
