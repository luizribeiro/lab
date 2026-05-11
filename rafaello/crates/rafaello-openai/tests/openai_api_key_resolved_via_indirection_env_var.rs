//! Scope §OP5 / pi-1 B-5: the plugin reads `RFL_OPENAI_API_KEY_ENV`
//! to learn the *name* of the env var that holds the API key value,
//! then reads that env var. No rename syntax.

use rafaello_openai::{read_required_api_key, OpenaiConfigError};
use serial_test::serial;

#[serial]
#[test]
fn api_key_resolved_via_indirection_env_var() {
    std::env::set_var("RFL_OPENAI_API_KEY_ENV", "LITELLM_API_KEY");
    std::env::set_var("LITELLM_API_KEY", "sk-test-1234");
    let got = read_required_api_key().expect("indirection should resolve");
    assert_eq!(got, "sk-test-1234");
    std::env::remove_var("LITELLM_API_KEY");
    std::env::remove_var("RFL_OPENAI_API_KEY_ENV");
}

#[serial]
#[test]
fn missing_api_key_env_name_errors() {
    std::env::remove_var("RFL_OPENAI_API_KEY_ENV");
    let err = read_required_api_key().expect_err("missing indirection name must error");
    assert_eq!(err, OpenaiConfigError::MissingApiKeyEnvName);
}

#[serial]
#[test]
fn missing_target_api_key_errors() {
    std::env::set_var("RFL_OPENAI_API_KEY_ENV", "OPENAI_API_KEY");
    std::env::remove_var("OPENAI_API_KEY");
    let err = read_required_api_key().expect_err("missing target var must error");
    assert_eq!(
        err,
        OpenaiConfigError::MissingApiKey {
            name: "OPENAI_API_KEY".to_string()
        }
    );
    std::env::remove_var("RFL_OPENAI_API_KEY_ENV");
}
