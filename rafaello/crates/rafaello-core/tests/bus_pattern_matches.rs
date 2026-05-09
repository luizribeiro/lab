//! Re-tests m1's `pattern_matches_topic` per scope §B7 example
//! matrix plus the §B7 / pi-1 §91 zero-trailing `**` negatives
//! (commits c07 / pi-1 §B4). Pure unit test — no tokio.

use rafaello_core::validate::topic::pattern_matches_topic;

#[test]
fn matches_examples_from_scope_section_b7() {
    let cases: &[(&str, &str, bool)] = &[
        ("plugin.id_x.tool_request", "plugin.id_x.tool_request", true),
        ("plugin.id_x.*", "plugin.id_x.tool_request", true),
        ("plugin.id_x.*", "plugin.id_x.foo.bar", false),
        ("plugin.**", "plugin.id_x.tool_request", true),
        ("plugin.**", "plugin.id_x.foo.bar.baz", true),
        ("plugin.id_x.**", "plugin.id_x.foo", true),
        ("plugin.id_x.**", "plugin.id_x.foo.bar", true),
        ("plugin.id_x.**", "plugin.id_y.foo", false),
        ("core.session.*", "core.session.tool_request", true),
        ("core.lifecycle.**", "core.lifecycle.boot", true),
        ("core.lifecycle.**", "core.lifecycle.publish_rejected", true),
        ("plugin.id_x.tool_request", "plugin.id_x.tool_result", false),
        ("plugin.id_x.foo", "plugin.id_y.foo", false),
    ];
    for (pattern, topic, expected) in cases {
        assert_eq!(
            pattern_matches_topic(pattern, topic),
            *expected,
            "pattern={pattern:?} topic={topic:?}",
        );
    }
}

#[test]
fn double_star_requires_at_least_one_trailing_segment() {
    // pi-1 §91 / scope §B7: `**` matches one-or-more trailing segments,
    // not zero. The tail `**` must consume something.
    assert!(!pattern_matches_topic("core.session.**", "core.session"));
    assert!(!pattern_matches_topic("plugin.id_x.**", "plugin.id_x"));
}

#[test]
fn double_star_matches_one_or_more_trailing_segments() {
    assert!(pattern_matches_topic("core.session.**", "core.session.x"));
    assert!(pattern_matches_topic(
        "core.session.**",
        "core.session.x.y.z",
    ));
}
