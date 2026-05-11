//! Scope §OP6 M-6 / pi-2 M-6: `RFL_OPENAI_MODEL` is required;
//! missing → `OpenaiConfigError::MissingModel` returned at plugin
//! startup *before* any HTTP call is attempted.

mod common;

use rafaello_openai::{read_required_model, OpenaiConfigError};
use serial_test::serial;

#[serial]
#[test]
fn missing_model_env_errors_before_request() {
    // SAFETY: serial_test pins this test against any other test
    // that touches `RFL_OPENAI_MODEL`.
    std::env::remove_var("RFL_OPENAI_MODEL");
    let err = read_required_model().expect_err("missing env must error");
    assert_eq!(err, OpenaiConfigError::MissingModel);
}

#[serial]
#[test]
fn empty_model_env_errors_before_request() {
    std::env::set_var("RFL_OPENAI_MODEL", "");
    let err = read_required_model().expect_err("empty env must error");
    assert_eq!(err, OpenaiConfigError::MissingModel);
    std::env::remove_var("RFL_OPENAI_MODEL");
}

#[serial]
#[test]
fn present_model_env_resolves() {
    std::env::set_var("RFL_OPENAI_MODEL", "vllm/qwen3.6-27b");
    assert_eq!(
        read_required_model().expect("present env should resolve"),
        "vllm/qwen3.6-27b"
    );
    std::env::remove_var("RFL_OPENAI_MODEL");
}
