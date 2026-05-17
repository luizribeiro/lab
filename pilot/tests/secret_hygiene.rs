//! Integration tests that enforce no-secret policies.
//! Lives at the workspace `tests/` level so it runs as a single crate-wide
//! check rather than per-module.

use pilot::{Auth, ClaudeConfig, GeminiConfig, PiConfig};
use secrecy::SecretString;

const TEST_SECRET: &str = "sk-pilot-test-DO-NOT-LEAK-XYZ123";

/// Returns the configs we cover. Add new drivers' configs here.
fn redactable_configs_debug() -> Vec<String> {
    let secret = SecretString::from(TEST_SECRET);
    let claude = ClaudeConfig {
        auth: Auth::ApiKey(secret.clone()),
        ..Default::default()
    };
    let gemini = GeminiConfig {
        auth: Auth::ApiKey(secret.clone()),
        ..Default::default()
    };
    let pi = PiConfig {
        auth: Auth::ApiKey(secret),
        ..Default::default()
    };
    vec![
        format!("{claude:?}"),
        format!("{gemini:?}"),
        format!("{pi:?}"),
    ]
}

#[test]
fn apikey_secret_never_appears_in_any_config_debug() {
    for rendered in redactable_configs_debug() {
        assert!(
            !rendered.contains(TEST_SECRET),
            "secret leaked through Debug: {rendered}"
        );
    }
}

/// Patterns we refuse to ship in fixture files. Add new prefixes as new
/// providers/agents enter the ecosystem.
const FORBIDDEN_FIXTURE_PATTERNS: &[&str] = &[
    "sk-",      // OpenAI / generic vendor "sk-..." keys
    "sk-ant-",  // Anthropic (subset of sk- but listed for clarity)
    "sk_live_", // Stripe-style
    "sk-proj-", // OpenAI project keys
    "AIza",     // Google API keys
    "ghp_",     // GitHub personal access tokens
    "ghs_",     // GitHub server-side tokens
    "gho_",     // GitHub OAuth tokens
    "ya29.",    // Google OAuth access tokens
];

fn walk_files(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            walk_files(&path, files);
        } else if ft.is_file() {
            files.push(path);
        }
    }
}

#[test]
fn fixtures_contain_no_known_api_key_patterns() {
    let fixture_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let mut files = Vec::new();
    walk_files(&fixture_dir, &mut files);
    assert!(
        !files.is_empty(),
        "no fixture files found at {fixture_dir:?} — bug in the test?"
    );

    for file in files {
        let Ok(content) = std::fs::read_to_string(&file) else {
            continue;
        };
        for pat in FORBIDDEN_FIXTURE_PATTERNS {
            assert!(
                !content.contains(pat),
                "fixture {} contains forbidden secret pattern {:?} — possible leaked credential; sanitize before committing",
                file.display(),
                pat
            );
        }
    }
}
