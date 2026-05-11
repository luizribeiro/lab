//! c03 acceptance: the three m5b `AuditKind` variants exist and
//! their `as_str()` arms return the contract strings. Also pins
//! that the round-1 withdrawn name (§"Out of scope" item 18) is
//! not introduced under any of the new variants' string forms.

use rafaello_core::audit::AuditKind;

const M5B_NEW_KIND_STRS: [&str; 3] = [
    "confirm_request_taint_attached",
    "plugin_publish_rejected_taint_superset",
    "tool_request_taint_unioned_from_in_reply_to",
];

#[test]
fn confirm_request_taint_attached_as_str() {
    assert_eq!(
        AuditKind::ConfirmRequestTaintAttached.as_str(),
        "confirm_request_taint_attached",
    );
}

#[test]
fn plugin_publish_rejected_taint_superset_as_str() {
    assert_eq!(
        AuditKind::PluginPublishRejectedTaintSuperset.as_str(),
        "plugin_publish_rejected_taint_superset",
    );
}

#[test]
fn tool_request_taint_unioned_from_in_reply_to_as_str() {
    assert_eq!(
        AuditKind::ToolRequestTaintUnionedFromInReplyTo.as_str(),
        "tool_request_taint_unioned_from_in_reply_to",
    );
}

#[test]
fn m5b_new_kind_strs_literal_is_well_formed() {
    assert_eq!(M5B_NEW_KIND_STRS.len(), 3);
    assert!(!M5B_NEW_KIND_STRS.contains(&"tool_request_rejected_taint_superset"));
}
