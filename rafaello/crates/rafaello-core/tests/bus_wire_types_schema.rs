use rafaello_core::bus::{BusEvent, PublishMsg, PublisherIdentity, TaintEntry};
use serde_json::{json, Value};

#[test]
fn publish_msg_decodes_all_fields() {
    let raw = json!({
        "topic": "plugin.id_x.foo",
        "payload": {"k": 1},
        "in_reply_to": ["a"],
        "taint": [{"source": "web", "detail": "x"}],
    });

    let msg: PublishMsg = serde_json::from_value(raw).expect("decode");
    assert_eq!(msg.topic, "plugin.id_x.foo");
    assert_eq!(msg.payload, json!({"k": 1}));
    let in_reply = msg.in_reply_to.expect("in_reply_to present");
    assert_eq!(in_reply.len(), 1);
    assert_eq!(in_reply[0].as_str(), Some("a"));
    let taint = msg.taint.expect("taint present");
    assert_eq!(
        taint,
        vec![TaintEntry {
            source: "web".to_string(),
            detail: Some("x".to_string()),
        }]
    );
}

#[test]
fn publish_msg_decodes_null_payload() {
    let raw = json!({"topic": "a.b", "payload": null});
    let msg: PublishMsg = serde_json::from_value(raw).expect("decode");
    assert_eq!(msg.topic, "a.b");
    assert_eq!(msg.payload, Value::Null);
    assert!(msg.in_reply_to.is_none());
    assert!(msg.taint.is_none());
}

#[test]
fn publish_msg_rejects_unknown_top_level_field() {
    let raw = json!({"topic": "a.b", "payload": null, "unknown": 1});
    let err = serde_json::from_value::<PublishMsg>(raw).expect_err("must reject unknown field");
    assert!(
        err.to_string().contains("unknown"),
        "error did not mention 'unknown': {err}"
    );
}

#[test]
fn taint_entry_rejects_unknown_field() {
    let raw = json!({
        "topic": "a.b",
        "payload": null,
        "taint": [{"source": "web", "detail": "x", "extra": 1}],
    });
    let err = serde_json::from_value::<PublishMsg>(raw).expect_err("must reject unknown field");
    assert!(
        err.to_string().contains("unknown"),
        "error did not mention 'unknown': {err}"
    );
}

#[derive(serde::Deserialize)]
#[allow(dead_code)]
struct BusEventReceived {
    topic: String,
    payload: Value,
    publisher: Value,
    #[serde(default)]
    in_reply_to: Option<Vec<Value>>,
    #[serde(default)]
    taint: Option<Vec<TaintEntry>>,
}

#[test]
fn bus_event_encodes_with_core_publisher_omits_optionals() {
    let ev = BusEvent {
        topic: "core.lifecycle.boot".to_string(),
        payload: json!({"version": "0.1"}),
        publisher: PublisherIdentity::Core,
        in_reply_to: None,
        taint: None,
        request_id: None,
    };

    let value = serde_json::to_value(&ev).expect("serialise");
    assert_eq!(
        value,
        json!({
            "topic": "core.lifecycle.boot",
            "payload": {"version": "0.1"},
            "publisher": {"kind": "core"},
        })
    );
    let obj = value.as_object().expect("object");
    assert!(!obj.contains_key("in_reply_to"));
    assert!(!obj.contains_key("taint"));
}

#[test]
fn bus_event_encodes_with_plugin_publisher_and_taint() {
    let ev = BusEvent {
        topic: "plugin.id_x.foo".to_string(),
        payload: json!(null),
        publisher: PublisherIdentity::Plugin {
            canonical: "plugin@1".to_string(),
            topic_id: "id_x".to_string(),
        },
        in_reply_to: Some(vec!["a".into()]),
        taint: Some(vec![TaintEntry {
            source: "web".to_string(),
            detail: None,
        }]),
        request_id: None,
    };

    let value = serde_json::to_value(&ev).expect("serialise");
    assert_eq!(
        value,
        json!({
            "topic": "plugin.id_x.foo",
            "payload": null,
            "publisher": {
                "kind": "plugin",
                "canonical": "plugin@1",
                "topic_id": "id_x",
            },
            "in_reply_to": ["a"],
            "taint": [{"source": "web", "detail": null}],
        })
    );
}

#[test]
fn bus_event_round_trips_through_permissive_struct() {
    let ev = BusEvent {
        topic: "plugin.id_x.foo".to_string(),
        payload: json!({"x": 1}),
        publisher: PublisherIdentity::Plugin {
            canonical: "plugin@1".to_string(),
            topic_id: "id_x".to_string(),
        },
        in_reply_to: Some(vec!["a".into()]),
        taint: Some(vec![TaintEntry {
            source: "web".to_string(),
            detail: Some("x".to_string()),
        }]),
        request_id: None,
    };

    let value = serde_json::to_value(&ev).expect("serialise");
    let decoded: BusEventReceived = serde_json::from_value(value).expect("decode permissive");
    assert_eq!(decoded.topic, "plugin.id_x.foo");
    assert_eq!(decoded.payload, json!({"x": 1}));
    assert_eq!(
        decoded.publisher,
        json!({
            "kind": "plugin",
            "canonical": "plugin@1",
            "topic_id": "id_x",
        })
    );
    assert_eq!(decoded.in_reply_to.as_deref(), Some(&[json!("a")][..]));
    assert_eq!(
        decoded.taint,
        Some(vec![TaintEntry {
            source: "web".to_string(),
            detail: Some("x".to_string()),
        }])
    );
}
