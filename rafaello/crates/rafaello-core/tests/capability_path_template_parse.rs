//! Acceptance test for c03: `manifest::CapabilityPathTemplate::parse` (scope §M11).

use rafaello_core::manifest::CapabilityPathTemplate;
use rafaello_core::ManifestError;

#[test]
fn parses_each_placeholder_prefix() {
    for s in [
        "${project}/src",
        "${home}/.config/foo",
        "${plugin}/assets",
        "${cache}/blob",
        "${state}/db",
    ] {
        let t = CapabilityPathTemplate::parse(s).unwrap();
        assert_eq!(t.as_str(), s);
    }
}

#[test]
fn parses_bare_placeholder_root() {
    let t = CapabilityPathTemplate::parse("${project}").unwrap();
    assert_eq!(t.as_str(), "${project}");
}

#[test]
fn parses_absolute_host_path() {
    let t = CapabilityPathTemplate::parse("/usr/bin/rustc").unwrap();
    assert_eq!(t.as_str(), "/usr/bin/rustc");
}

#[test]
fn allows_parent_dir_segments() {
    // `..` is parser-allowed; the post-expansion containment check
    // is c31's resolver responsibility.
    let t = CapabilityPathTemplate::parse("${project}/../etc").unwrap();
    assert_eq!(t.as_str(), "${project}/../etc");
}

#[test]
fn rejects_bare_relative() {
    assert!(matches!(
        CapabilityPathTemplate::parse("relative/path"),
        Err(ManifestError::CapabilityPathBareRelative)
    ));
    assert!(matches!(
        CapabilityPathTemplate::parse(""),
        Err(ManifestError::CapabilityPathBareRelative)
    ));
}

#[test]
fn rejects_unknown_placeholder() {
    assert!(matches!(
        CapabilityPathTemplate::parse("${secret}/x"),
        Err(ManifestError::UnknownPlaceholder)
    ));
}

#[test]
fn rejects_malformed_placeholder_syntax() {
    assert!(matches!(
        CapabilityPathTemplate::parse("${project"),
        Err(ManifestError::CapabilityPathMalformedPlaceholder)
    ));
    // No `/` separator after the closing brace: "${project}foo" is
    // malformed (placeholder must be a path-segment prefix).
    assert!(matches!(
        CapabilityPathTemplate::parse("${project}foo"),
        Err(ManifestError::CapabilityPathMalformedPlaceholder)
    ));
}

#[test]
fn rejects_backslash() {
    assert!(matches!(
        CapabilityPathTemplate::parse("${project}\\foo"),
        Err(ManifestError::CapabilityPathBackslash)
    ));
    assert!(matches!(
        CapabilityPathTemplate::parse("/abs\\path"),
        Err(ManifestError::CapabilityPathBackslash)
    ));
}

#[test]
fn rejects_control_characters() {
    assert!(matches!(
        CapabilityPathTemplate::parse("${project}/a\x00b"),
        Err(ManifestError::CapabilityPathControlChar)
    ));
    assert!(matches!(
        CapabilityPathTemplate::parse("/a\nb"),
        Err(ManifestError::CapabilityPathControlChar)
    ));
}

#[test]
fn deserialize_via_serde_runs_parser() {
    #[derive(serde::Deserialize)]
    struct Wrap {
        path: CapabilityPathTemplate,
    }
    let ok: Wrap = toml::from_str(r#"path = "${project}/src""#).unwrap();
    assert_eq!(ok.path.as_str(), "${project}/src");

    let bad: Result<Wrap, _> = toml::from_str(r#"path = "relative/path""#);
    assert!(bad.is_err());
}
