//! Scope §M1.1 + pi-1 B-8: an openai lock that sets
//! `RFL_OPENAI_ENDPOINT_URL` / `RFL_OPENAI_MODEL` /
//! `RFL_OPENAI_API_KEY_ENV` via `env.set` compiles cleanly. Live
//! `scrubber::reject_reserved` accepts these names because none of
//! them are in `RESERVED_ENV_VARS`.

mod common;

use common::lock_kit::{openai_env_grant, openai_only_lock, validate_and_compile_one};

#[test]
fn compile_openai_lock_with_rfl_openai_envset_keys_succeeds() {
    let env = openai_env_grant(
        Vec::new(),
        vec![
            "LITELLM_API_KEY".to_string(),
            "OPENAI_API_KEY".to_string(),
            "ANTHROPIC_API_KEY".to_string(),
        ],
    );
    let (lock, canonical, pdir) = openai_only_lock(env);
    let plan = validate_and_compile_one(&lock, &canonical, &pdir);

    assert_eq!(
        plan.env
            .set
            .get("RFL_OPENAI_ENDPOINT_URL")
            .map(String::as_str),
        Some("https://litellm.thepromisedlan.club/v1"),
    );
    assert_eq!(
        plan.env.set.get("RFL_OPENAI_MODEL").map(String::as_str),
        Some("vllm/qwen3.6-27b"),
    );
    assert_eq!(
        plan.env
            .set
            .get("RFL_OPENAI_API_KEY_ENV")
            .map(String::as_str),
        Some("LITELLM_API_KEY"),
    );
}
