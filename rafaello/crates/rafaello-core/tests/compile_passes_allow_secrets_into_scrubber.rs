//! c06 — scope §OP6: end-to-end via `compile_plugin` against a
//! fixture lock; the compiled `EnvPlan.pass` matches the scrubber's
//! expected output when `allow_secrets` opts a `*_KEY`-pattern
//! name out of the default strip.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::lock::{Grant, GrantBundle, GrantEnv, SessionTable};
use rafaello_core::paths::PathContext;
use rafaello_core::scrubber::strip;

use common::{canonical, entry, lock_with};

#[test]
fn env_plan_pass_matches_scrubber_with_allow_secrets() {
    let tmp = tempfile::tempdir().unwrap();
    let project = std::fs::canonicalize(tmp.path()).unwrap();

    let id = canonical("github.com/acme:openai@1.0.0");
    let mut e = entry(&["chat"], false, None);

    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            env: Some(GrantEnv {
                pass: vec!["LITELLM_API_KEY".to_owned(), "RANDOM_API_KEY".to_owned()],
                set: BTreeMap::new(),
                allow_secrets: vec!["LITELLM_API_KEY".to_owned()],
            }),
            ..GrantBundle::default()
        },
    );
    e.grant = Grant {
        bundles,
        ..Grant::default()
    };

    let lock = lock_with(vec![(id.clone(), e)], SessionTable::default());
    let ctx = PathContext {
        project_root: project.clone(),
        home: PathBuf::from("/tmp/home"),
        plugin_dir: project.join(".rafaello/plugins/openai"),
        cache_dir: PathBuf::from("/tmp/cache"),
        state_dir: PathBuf::from("/tmp/state"),
    };
    common::make_plugin_dir(&ctx.plugin_dir);
    let digests = RecomputedDigests {
        content: "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        manifest: "sha256:1111111111111111111111111111111111111111111111111111111111111111".into(),
    };

    let plan = compile_plugin(&lock, &id, &ctx, &digests).expect("compile");
    let mut sorted_pass = vec!["LITELLM_API_KEY".to_owned(), "RANDOM_API_KEY".to_owned()];
    sorted_pass.sort();
    let expected = strip(&sorted_pass, &["LITELLM_API_KEY".to_owned()], false);
    assert_eq!(plan.env.pass, expected);
    assert_eq!(plan.env.pass, vec!["LITELLM_API_KEY".to_owned()]);
}
