//! Acceptance test for c03: `manifest::SafePath::parse` (scope §M11).

use rafaello_core::manifest::SafePath;
use rafaello_core::ManifestError;

#[test]
fn parses_simple_relative_path() {
    let p = SafePath::parse("src/main.rs").unwrap();
    assert_eq!(p.as_str(), "src/main.rs");
}

#[test]
fn parses_single_segment() {
    let p = SafePath::parse("entry.py").unwrap();
    assert_eq!(p.as_str(), "entry.py");
}

#[test]
fn parses_nested_relative_path() {
    let p = SafePath::parse("schemas/tools/foo.json").unwrap();
    assert_eq!(p.as_str(), "schemas/tools/foo.json");
}

#[test]
fn rejects_leading_slash() {
    assert!(matches!(
        SafePath::parse("/abs/path"),
        Err(ManifestError::SafePathLeadingSlash)
    ));
}

#[test]
fn rejects_parent_dir_anywhere() {
    assert!(matches!(
        SafePath::parse("../escape"),
        Err(ManifestError::SafePathParentDir)
    ));
    assert!(matches!(
        SafePath::parse("a/../b"),
        Err(ManifestError::SafePathParentDir)
    ));
    assert!(matches!(
        SafePath::parse("a/b/.."),
        Err(ManifestError::SafePathParentDir)
    ));
}

#[test]
fn rejects_empty_segments() {
    assert!(matches!(
        SafePath::parse("a//b"),
        Err(ManifestError::SafePathEmptySegment)
    ));
    assert!(matches!(
        SafePath::parse("a/"),
        Err(ManifestError::SafePathEmptySegment)
    ));
    assert!(matches!(
        SafePath::parse(""),
        Err(ManifestError::SafePathEmpty)
    ));
}

#[test]
fn rejects_backslash() {
    assert!(matches!(
        SafePath::parse("a\\b"),
        Err(ManifestError::SafePathBackslash)
    ));
}

#[test]
fn rejects_control_characters() {
    assert!(matches!(
        SafePath::parse("a\x00b"),
        Err(ManifestError::SafePathControlChar)
    ));
    assert!(matches!(
        SafePath::parse("a\nb"),
        Err(ManifestError::SafePathControlChar)
    ));
    assert!(matches!(
        SafePath::parse("a\x7fb"),
        Err(ManifestError::SafePathControlChar)
    ));
}

#[test]
fn deserialize_via_serde_runs_parser() {
    // Deserialise a SafePath wrapped as a TOML string.
    #[derive(serde::Deserialize)]
    struct Wrap {
        entry: SafePath,
    }
    let ok: Wrap = toml::from_str(r#"entry = "src/main.rs""#).unwrap();
    assert_eq!(ok.entry.as_str(), "src/main.rs");

    let bad: Result<Wrap, _> = toml::from_str(r#"entry = "../escape""#);
    assert!(bad.is_err());
}
