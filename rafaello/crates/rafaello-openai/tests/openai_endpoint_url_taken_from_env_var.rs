//! Scope §OP5: the plugin reads the endpoint URL from
//! `RFL_OPENAI_ENDPOINT_URL` (set via the lock's `env.set` map).

use rafaello_openai::{read_required_endpoint_url, OpenaiConfigError};
use serial_test::serial;

#[serial]
#[test]
fn endpoint_url_taken_from_env_var() {
    std::env::set_var(
        "RFL_OPENAI_ENDPOINT_URL",
        "https://litellm.thepromisedlan.club/v1",
    );
    let got = read_required_endpoint_url().expect("present env should resolve");
    assert_eq!(got, "https://litellm.thepromisedlan.club/v1");
    std::env::remove_var("RFL_OPENAI_ENDPOINT_URL");
}

#[serial]
#[test]
fn missing_endpoint_url_errors() {
    std::env::remove_var("RFL_OPENAI_ENDPOINT_URL");
    let err = read_required_endpoint_url().expect_err("missing env must error");
    assert_eq!(err, OpenaiConfigError::MissingEndpointUrl);
}
