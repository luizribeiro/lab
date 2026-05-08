//! c12 — `CanonicalId` parser/formatter round-trip (scope §L8).

use rafaello_core::lock::CanonicalId;

fn round_trip(input: &str) {
    let id = CanonicalId::parse(input).expect("parse should succeed");
    assert_eq!(id.to_string(), input, "round-trip stable for `{input}`");
}

#[test]
fn round_trips_simple_source() {
    round_trip("local:foo@1.0.0");
}

#[test]
fn round_trips_dotted_source() {
    round_trip("github.com/acme:grep@1.4.2");
}

#[test]
fn round_trips_crates_io_source() {
    round_trip("crates.io:serde@1.0.228");
}

#[test]
fn round_trips_multi_segment_source() {
    round_trip("github.com/acme/sub-org:foo@0.1.0");
}

#[test]
fn round_trips_underscored_source() {
    round_trip("git_lfs.example.io/team_a:foo_bar@2.3.4");
}

#[test]
fn round_trips_pre_release_version() {
    round_trip("local:foo@1.0.0-alpha.1");
}

#[test]
fn round_trips_build_metadata_version() {
    round_trip("local:foo@1.0.0+build.42");
}

#[test]
fn round_trips_pre_release_with_build_metadata() {
    round_trip("github.com/acme:bar@2.0.0-rc.1+build.7");
}

#[test]
fn round_trips_name_with_digits_and_dashes() {
    round_trip("local:foo-2-bar@0.0.1");
}

#[test]
fn round_trips_via_serde() {
    #[derive(serde::Serialize, serde::Deserialize, PartialEq, Debug)]
    struct Wrap {
        id: CanonicalId,
    }
    let w: Wrap = toml::from_str(r#"id = "github.com/acme:grep@1.4.2""#).unwrap();
    assert_eq!(w.id.to_string(), "github.com/acme:grep@1.4.2");

    let toml = toml::to_string(&w).unwrap();
    assert!(toml.contains(r#"id = "github.com/acme:grep@1.4.2""#));
}

#[test]
fn exposes_components() {
    let id = CanonicalId::parse("github.com/acme:grep@1.4.2").unwrap();
    assert_eq!(id.source(), "github.com/acme");
    assert_eq!(id.name(), "grep");
    assert_eq!(id.version().to_string(), "1.4.2");
}
