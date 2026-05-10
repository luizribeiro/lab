//! Manifest-side: a publish on a top-level namespace outside
//! `{core, frontend, plugin, provider}` is rejected with
//! `PublishUnknownNamespace`. Verifies the existing reserved/
//! frontend rejections still fire and that `provider.<own-id>.*`
//! is accepted while `provider.<other-id>.*` mismatches
//! (scope §M1.1 / §M1.2, m2 retro §2.8).

use rafaello_core::error::ValidationError;
use rafaello_core::lock::CanonicalId;
use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

fn manifest_with_publish(extra_provides: &str, publish: &str) -> Manifest {
    let src = format!(
        r#"
schema = 1
name = "naughty"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[provides]
{extra_provides}

[bus]
publishes = ["{publish}"]
"#
    );
    Manifest::parse(&src).expect("parse should succeed")
}

#[test]
fn publishes_unknown_namespace_rejected() {
    // evil.foo → PublishUnknownNamespace
    let m = manifest_with_publish("", "evil.foo");
    match validate::manifest_standalone(&m) {
        Err(ValidationError::PublishUnknownNamespace { topic, namespace }) => {
            assert_eq!(topic, "evil.foo");
            assert_eq!(namespace, "evil");
        }
        other => panic!("expected PublishUnknownNamespace for `evil.foo`, got {other:?}"),
    }

    // core.foo → PublishOnReservedNamespace (existing behavior preserved)
    let m = manifest_with_publish("", "core.foo");
    match validate::manifest_standalone(&m) {
        Err(ValidationError::PublishOnReservedNamespace { topic }) => {
            assert_eq!(topic, "core.foo");
        }
        other => panic!("expected PublishOnReservedNamespace for `core.foo`, got {other:?}"),
    }

    // frontend.foo → PublishOnFrontendNamespace (existing behavior preserved)
    let m = manifest_with_publish("", "frontend.foo");
    match validate::manifest_standalone(&m) {
        Err(ValidationError::PublishOnFrontendNamespace { topic }) => {
            assert_eq!(topic, "frontend.foo");
        }
        other => panic!("expected PublishOnFrontendNamespace for `frontend.foo`, got {other:?}"),
    }

    // provider.<own-id>.foo accepted
    let canonical = CanonicalId::parse("github/acme:naughty@0.1.0").unwrap();
    let m = manifest_with_publish(r#"provider = "myprov""#, "provider.myprov.foo");
    validate::manifest_standalone(&m).expect("standalone passes for provider.<own-id>.foo");
    validate::manifest_with_id(&m, &canonical)
        .expect("manifest_with_id passes for provider.<own-id>.foo");

    // provider.<other-id>.foo → ProviderNamespaceMismatch
    let m = manifest_with_publish(r#"provider = "myprov""#, "provider.someoneelse.foo");
    validate::manifest_standalone(&m).expect("standalone passes for provider.<other>.foo");
    match validate::manifest_with_id(&m, &canonical) {
        Err(ValidationError::ProviderNamespaceMismatch) => {}
        other => panic!("expected ProviderNamespaceMismatch, got {other:?}"),
    }
}
