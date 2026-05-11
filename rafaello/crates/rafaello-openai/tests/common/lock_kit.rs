//! Lock+compile fixture helpers shared by the c34 OP4/OP5 tests.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use rafaello_core::compile::{compile_plugin, CompiledPlugin};
use rafaello_core::digest::{content_digest, manifest_digest, RecomputedDigests};
use rafaello_core::lock::{
    Bindings, CanonicalId, Grant, GrantBundle, GrantEnv, GrantNetwork, LoadPolicy, Lock, LockFlags,
    PluginEntry, SessionTable,
};
use rafaello_core::manifest::capabilities::NetworkMode;
use rafaello_core::manifest::{Manifest, SafePath};
use rafaello_core::paths::PathContext;
use rafaello_core::validate::{self, LockValidationContext};

pub const OPENAI_CANONICAL: &str = "builtin:openai@0.0.0";

pub fn openai_fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("m5a-locks")
        .join("rafaello-openai")
}

pub fn m5a_locks_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("m5a-locks")
}

pub fn fixtures_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
}

/// Standard `env.set` projected from the bundled openai manifest +
/// dev lock per scope §OP5.
pub fn standard_env_set() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    m.insert(
        "RFL_OPENAI_API_KEY_ENV".to_string(),
        "LITELLM_API_KEY".to_string(),
    );
    m.insert(
        "RFL_OPENAI_ENDPOINT_URL".to_string(),
        "https://litellm.thepromisedlan.club/v1".to_string(),
    );
    m.insert(
        "RFL_OPENAI_MODEL".to_string(),
        "vllm/qwen3.6-27b".to_string(),
    );
    m
}

pub fn openai_env_grant(pass: Vec<String>, allow_secrets: Vec<String>) -> GrantEnv {
    GrantEnv {
        pass,
        set: standard_env_set(),
        allow_secrets,
    }
}

/// Build a single-plugin lock for the openai fixture with the given
/// env grant. Network bundle is `proxy` against `127.0.0.1` (per
/// the m5a CI lock).
pub fn openai_only_lock(env: GrantEnv) -> (Lock, CanonicalId, PathBuf) {
    let canonical = CanonicalId::parse(OPENAI_CANONICAL).expect("canonical");
    let pdir = openai_fixture_dir();
    let manifest_raw =
        std::fs::read_to_string(pdir.join("rafaello.toml")).expect("read openai manifest");
    let manifest = Manifest::parse(&manifest_raw).expect("parse openai manifest");
    let m_digest = manifest_digest(&manifest.canonical_bytes());
    let c_digest = content_digest(&pdir).expect("content_digest");

    let mut bundles = BTreeMap::new();
    bundles.insert(
        "default".to_string(),
        GrantBundle {
            network: Some(GrantNetwork {
                mode: NetworkMode::Proxy,
                allow_hosts: vec!["127.0.0.1".to_string()],
            }),
            env: Some(env),
            ..GrantBundle::default()
        },
    );

    let entry = PluginEntry {
        entry: SafePath::parse("bin/rfl-openai").expect("safepath"),
        digest: c_digest,
        manifest_digest: m_digest,
        granted_at: Utc::now(),
        grant: Grant {
            bundles,
            subscribes: vec![
                "core.session.user_message".to_string(),
                "core.session.tool_result".to_string(),
            ],
            publishes: vec![
                "provider.openai.tool_request".to_string(),
                "provider.openai.assistant_message".to_string(),
            ],
        },
        bindings: Bindings {
            provider: true,
            provider_id: Some("openai".to_string()),
            tools: Vec::new(),
            renderer_kinds: Vec::new(),
            tool_meta: BTreeMap::new(),
            load: LoadPolicy::Eager,
        },
        flags: LockFlags::default(),
    };

    let mut plugins = BTreeMap::new();
    plugins.insert(canonical.clone(), entry);

    let lock = Lock {
        plugins,
        session: SessionTable {
            provider_active: Some(canonical.to_string()),
            tool_owner: BTreeMap::new(),
        },
    };
    (lock, canonical, pdir)
}

/// Validate `lock` against a tempdir-backed project root and compile
/// the named plugin. The fixture dir is pinned to `pdir`.
pub fn validate_and_compile_one(
    lock: &Lock,
    canonical: &CanonicalId,
    pdir: &Path,
) -> CompiledPlugin {
    let project = tempfile::tempdir().expect("project tempdir");
    let home = tempfile::tempdir().expect("home tempdir");
    let project_root = project.path().to_path_buf();
    let home_root = home.path().to_path_buf();

    let mut plugin_dirs = BTreeMap::new();
    plugin_dirs.insert(canonical.clone(), pdir.to_path_buf());

    let lvc = LockValidationContext {
        project_root: project_root.clone(),
        home: home_root.clone(),
        plugin_dirs: plugin_dirs.clone(),
        cache_root: project_root.clone(),
        state_root: project_root.clone(),
    };
    validate::lock(lock, &lvc).expect("validate::lock");

    let pctx = PathContext {
        project_root,
        home: home_root,
        plugin_dir: pdir.to_path_buf(),
        cache_dir: lvc.cache_root.clone(),
        state_dir: lvc.state_root.clone(),
    };
    let manifest_raw = std::fs::read_to_string(pdir.join("rafaello.toml")).expect("read manifest");
    let recomputed = RecomputedDigests {
        content: content_digest(pdir).expect("content_digest"),
        manifest: manifest_digest(
            &Manifest::parse(&manifest_raw)
                .expect("parse manifest")
                .canonical_bytes(),
        ),
    };
    compile_plugin(lock, canonical, &pctx, &recomputed).expect("compile_plugin")
}
