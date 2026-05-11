//! Scope §OP5: the plugin reads the model name from
//! `RFL_OPENAI_MODEL` (set via the lock's `env.set` map).

use rafaello_openai::read_required_model;
use serial_test::serial;

#[serial]
#[test]
fn model_taken_from_env_var() {
    std::env::set_var("RFL_OPENAI_MODEL", "vllm/qwen3.6-27b");
    let got = read_required_model().expect("present env should resolve");
    assert_eq!(got, "vllm/qwen3.6-27b");
    std::env::remove_var("RFL_OPENAI_MODEL");
}
