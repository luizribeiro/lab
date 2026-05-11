//! Scope §OP5/§OP6 + pi-2 N-4: stripping happens in `compile_plugin`
//! via `scrubber::strip`, NOT in `validate::lock`. When the lock's
//! `env.allow_secrets` (snapshotted from the manifest) covers the
//! `LITELLM_API_KEY` pass entry, the compiled `EnvPlan.pass` retains
//! it.

mod common;

use common::lock_kit::{openai_env_grant, openai_only_lock, validate_and_compile_one};

#[test]
fn litellm_api_key_pass_honoured_via_manifest_allow_secrets() {
    let env = openai_env_grant(
        vec!["LITELLM_API_KEY".to_string()],
        vec![
            "LITELLM_API_KEY".to_string(),
            "OPENAI_API_KEY".to_string(),
            "ANTHROPIC_API_KEY".to_string(),
        ],
    );
    let (lock, canonical, pdir) = openai_only_lock(env);
    let plan = validate_and_compile_one(&lock, &canonical, &pdir);

    assert!(
        plan.env.pass.iter().any(|n| n == "LITELLM_API_KEY"),
        "compiled plan retains LITELLM_API_KEY pass entry: {:?}",
        plan.env.pass
    );
}
