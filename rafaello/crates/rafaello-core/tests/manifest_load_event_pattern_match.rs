//! Table-driven: a `[load] event = [...]` trigger is accepted iff
//! at least one `bus.subscribes` pattern matches it under §5.1
//! (pi review-3 finding 9 — pattern match, not literal equality).

use rafaello_core::error::ValidationError;
use rafaello_core::manifest::Manifest;
use rafaello_core::validate;

fn manifest_with(subscribes: &str, event: &str) -> Manifest {
    let src = format!(
        r#"
schema = 1
name = "loader"
version = "0.1.0"
entry = "main.py"
rafaello = ">=0.1, <0.2"

[bus]
subscribes = [{subscribes}]

[load]
event = [{event}]
"#
    );
    Manifest::parse(&src).expect("parse should succeed")
}

#[test]
fn load_event_pattern_match_table() {
    struct Case {
        subscribes: &'static str,
        event: &'static str,
        accept: bool,
    }
    let cases = [
        Case {
            subscribes: r#""core.session.**""#,
            event: r#""core.session.started""#,
            accept: true,
        },
        Case {
            subscribes: r#""core.session.**""#,
            event: r#""unrelated.x""#,
            accept: false,
        },
        Case {
            subscribes: r#""tool.*""#,
            event: r#""tool.invoked""#,
            accept: true,
        },
        Case {
            subscribes: r#""tool.*""#,
            event: r#""tool.invoked.deeply""#,
            accept: false,
        },
    ];
    for c in cases {
        let m = manifest_with(c.subscribes, c.event);
        let res = validate::manifest_standalone(&m);
        if c.accept {
            assert!(
                res.is_ok(),
                "expected accept for subscribes={} event={}, got {res:?}",
                c.subscribes,
                c.event
            );
        } else {
            match res {
                Err(ValidationError::LoadTriggerUnmatchedEvent { .. }) => {}
                other => panic!(
                    "expected LoadTriggerUnmatchedEvent for subscribes={} event={}, got {other:?}",
                    c.subscribes, c.event
                ),
            }
        }
    }
}
