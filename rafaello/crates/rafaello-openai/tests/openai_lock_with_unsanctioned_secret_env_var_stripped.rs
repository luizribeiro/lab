//! Scope §OP5/§OP6 + pi-2 N-4: a `*_KEY`-pattern env name not
//! covered by `allow_secrets` is dropped from the compiled
//! `EnvPlan.pass` by `scrubber::strip` (live `compile_plugin`).

mod common;

use common::lock_kit::{openai_env_grant, openai_only_lock, validate_and_compile_one};

#[test]
fn unsanctioned_secret_env_var_stripped() {
    let env = openai_env_grant(
        vec!["RANDOM_API_KEY".to_string()],
        vec![
            "LITELLM_API_KEY".to_string(),
            "OPENAI_API_KEY".to_string(),
            "ANTHROPIC_API_KEY".to_string(),
        ],
    );
    let (lock, canonical, pdir) = openai_only_lock(env);
    let plan = validate_and_compile_one(&lock, &canonical, &pdir);

    assert!(
        !plan.env.pass.iter().any(|n| n == "RANDOM_API_KEY"),
        "unsanctioned secret should be stripped: {:?}",
        plan.env.pass
    );
}
