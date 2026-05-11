//! c06 — scope §OP6 / §C2: `effective_grant` unions and dedups
//! `allow_secrets` across bundles, just like `env.pass`.

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use rafaello_core::compile::compile_plugin;
use rafaello_core::digest::RecomputedDigests;
use rafaello_core::lock::{Grant, GrantBundle, GrantEnv, SessionTable};
use rafaello_core::paths::PathContext;

use common::{canonical, entry, lock_with};

#[test]
fn effective_grant_unions_allow_secrets_across_bundles() {
    let tmp = tempfile::tempdir().unwrap();
    let project = std::fs::canonicalize(tmp.path()).unwrap();

    let id = canonical("github.com/acme:mailer@1.0.0");
    let mut e = entry(&["send-mail"], false, None);

    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_owned(),
        GrantBundle {
            env: Some(GrantEnv {
                pass: vec!["LITELLM_API_KEY".to_owned(), "OPENAI_API_KEY".to_owned()],
                set: BTreeMap::new(),
                allow_secrets: vec!["LITELLM_API_KEY".to_owned()],
            }),
            ..GrantBundle::default()
        },
    );
    bundles.insert(
        "send-mail".to_owned(),
        GrantBundle {
            env: Some(GrantEnv {
                pass: Vec::new(),
                set: BTreeMap::new(),
                allow_secrets: vec!["OPENAI_API_KEY".to_owned(), "LITELLM_API_KEY".to_owned()],
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
        plugin_dir: project.join(".rafaello/plugins/mailer"),
        cache_dir: PathBuf::from("/tmp/cache"),
        state_dir: PathBuf::from("/tmp/state"),
    };
    common::make_plugin_dir(&ctx.plugin_dir);
    let digests = RecomputedDigests {
        content: "sha256:0000000000000000000000000000000000000000000000000000000000000000".into(),
        manifest: "sha256:1111111111111111111111111111111111111111111111111111111111111111".into(),
    };

    let plan = compile_plugin(&lock, &id, &ctx, &digests).expect("compile");
    assert_eq!(
        plan.env.pass,
        vec!["LITELLM_API_KEY".to_owned(), "OPENAI_API_KEY".to_owned()]
    );
}
