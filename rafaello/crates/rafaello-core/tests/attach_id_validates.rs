//! c13 — `AttachId::new` validates against `^[a-z][a-z0-9-]{0,31}$`
//! per scope §B1.

use rafaello_core::{AttachId, AttachIdParseError};

#[test]
fn valid_attach_ids_round_trip() {
    let valid = [
        "tui",
        "ide-1",
        "a",
        "ab",
        "a0",
        "a-",
        "abcdefghijklmnopqrstuvwxyz012345",
    ];
    for s in valid {
        let id = AttachId::new(s).expect("valid attach id");
        assert_eq!(id.as_str(), s);
        assert_eq!(id.to_string(), s);
    }
}

#[test]
fn empty_rejected() {
    assert!(matches!(AttachId::new(""), Err(AttachIdParseError::Empty)));
}

#[test]
fn leading_uppercase_rejected() {
    assert!(matches!(
        AttachId::new("Tui"),
        Err(AttachIdParseError::IllegalLeadChar { ch: 'T' })
    ));
}

#[test]
fn leading_digit_rejected() {
    assert!(matches!(
        AttachId::new("1tui"),
        Err(AttachIdParseError::IllegalLeadChar { ch: '1' })
    ));
}

#[test]
fn leading_dash_rejected() {
    assert!(matches!(
        AttachId::new("-tui"),
        Err(AttachIdParseError::IllegalLeadChar { ch: '-' })
    ));
}

#[test]
fn too_long_rejected() {
    let s = "a".repeat(33);
    assert!(matches!(
        AttachId::new(&s),
        Err(AttachIdParseError::TooLong { len: 33 })
    ));
}

#[test]
fn punctuation_rejected() {
    assert!(matches!(
        AttachId::new("tui!"),
        Err(AttachIdParseError::IllegalChar { ch: '!' })
    ));
}

#[test]
fn space_rejected() {
    assert!(matches!(
        AttachId::new("tui id"),
        Err(AttachIdParseError::IllegalChar { ch: ' ' })
    ));
}

#[test]
fn uppercase_inside_rejected() {
    assert!(matches!(
        AttachId::new("tuI"),
        Err(AttachIdParseError::IllegalChar { ch: 'I' })
    ));
}

#[test]
fn underscore_rejected() {
    assert!(matches!(
        AttachId::new("tui_id"),
        Err(AttachIdParseError::IllegalChar { ch: '_' })
    ));
}
