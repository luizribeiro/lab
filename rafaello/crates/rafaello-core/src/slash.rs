//! Core slash-command handler (scope §SL3 + §SL0).
//!
//! Subscribes internally to `frontend.tui.slash_command`, mutates the
//! in-process [`UserGrants`], and publishes
//! `core.session.command_result` correlated to the inbound slash via
//! envelope `in_reply_to` (cardinality exactly one — c11 enforces).
//! Default plugin resolution for `/grant` uses
//! [`BrokerAcl::tool_route`] (pi-4 B-2): the live dispatch-target map
//! populated by m4 at compile time, regardless of whether
//! `session.tool_owner` is set.

use std::collections::BTreeMap;
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::{json, Map, Value};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use ulid::Ulid;

use crate::audit::{AuditKind, AuditWriter};
use crate::broker_acl::BrokerAcl;
use crate::bus::{
    Broker, BusEvent, JsonRpcId, CORE_SESSION_COMMAND_RESULT, FRONTEND_TUI_SLASH_COMMAND,
};
use crate::lock::canonical_id::CanonicalId;
use crate::user_grants::{GrantId, GrantMatcher, GrantSource, RevokeError, UserGrant, UserGrants};

const SLASH_CHANNEL_CAPACITY: usize = 256;

pub struct SlashHandler {
    broker: Broker,
    acl: Arc<BrokerAcl>,
    user_grants: Arc<Mutex<UserGrants>>,
    audit: Arc<AuditWriter>,
    tool_grant_match_schemas: BTreeMap<String, Value>,
    shutdown_rx: watch::Receiver<bool>,
}

impl SlashHandler {
    pub fn new(
        broker: Broker,
        acl: Arc<BrokerAcl>,
        user_grants: Arc<Mutex<UserGrants>>,
        audit: Arc<AuditWriter>,
        tool_grant_match_schemas: BTreeMap<String, Value>,
        shutdown_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            broker,
            acl,
            user_grants,
            audit,
            tool_grant_match_schemas,
            shutdown_rx,
        }
    }

    pub fn start(self) -> JoinHandle<()> {
        let (rx, subscription) = self.broker.subscribe_internal(
            vec![FRONTEND_TUI_SLASH_COMMAND.to_string()],
            SLASH_CHANNEL_CAPACITY,
        );
        let broker = self.broker.clone();
        let acl = self.acl.clone();
        let user_grants = self.user_grants.clone();
        let audit = self.audit.clone();
        let schemas = self.tool_grant_match_schemas.clone();
        let mut shutdown_rx = self.shutdown_rx;
        tokio::spawn(async move {
            let _subscription = subscription;
            let mut rx = rx;
            loop {
                tokio::select! {
                    biased;
                    res = shutdown_rx.changed() => {
                        if res.is_err() || *shutdown_rx.borrow() {
                            break;
                        }
                    }
                    maybe_event = rx.recv() => {
                        match maybe_event {
                            Some(event) => handle_event(
                                &broker,
                                &acl,
                                &user_grants,
                                &audit,
                                &schemas,
                                &event,
                            ),
                            None => break,
                        }
                    }
                }
            }
        })
    }
}

fn handle_event(
    broker: &Broker,
    acl: &BrokerAcl,
    user_grants: &Arc<Mutex<UserGrants>>,
    audit: &AuditWriter,
    schemas: &BTreeMap<String, Value>,
    event: &BusEvent,
) {
    let Some(slash_request_id) = event.request_id.clone() else {
        return;
    };
    let Some(payload_obj) = event.payload.as_object() else {
        reject_malformed(broker, audit, &slash_request_id, "payload not an object");
        return;
    };
    let command = payload_obj.get("command").and_then(|v| v.as_str());
    let args = payload_obj.get("args").and_then(|v| v.as_object());
    let (Some(command), Some(args)) = (command, args) else {
        reject_malformed(
            broker,
            audit,
            &slash_request_id,
            "missing `command`/`args` fields",
        );
        return;
    };
    match command {
        "grant" => handle_grant(
            broker,
            acl,
            user_grants,
            audit,
            schemas,
            &slash_request_id,
            args,
        ),
        "list_grants" => handle_list_grants(broker, user_grants, audit, &slash_request_id),
        "revoke" => handle_revoke(broker, user_grants, audit, &slash_request_id, args),
        "unknown" => handle_unknown(broker, audit, &slash_request_id, args),
        other => reject_malformed(
            broker,
            audit,
            &slash_request_id,
            &format!("unrecognised command kind `{other}`"),
        ),
    }
}

fn handle_grant(
    broker: &Broker,
    acl: &BrokerAcl,
    user_grants: &Arc<Mutex<UserGrants>>,
    audit: &AuditWriter,
    schemas: &BTreeMap<String, Value>,
    slash_request_id: &JsonRpcId,
    args: &Map<String, Value>,
) {
    let Some(tool) = args
        .get("tool")
        .and_then(|v| v.as_str())
        .map(str::to_string)
    else {
        reject_malformed(broker, audit, slash_request_id, "grant: missing `tool`");
        return;
    };
    let plugin = match args.get("plugin").and_then(|v| v.as_str()) {
        Some(s) => match CanonicalId::parse(s) {
            Ok(c) => c,
            Err(_) => {
                let msg = format!("invalid plugin canonical id `{s}`");
                publish_result(broker, slash_request_id, false, "grant", &msg, json!({}));
                let _ = audit.record(
                    AuditKind::SlashUnknown,
                    Some(slash_request_id),
                    &json!({"details": msg}),
                );
                return;
            }
        },
        None => match acl.tool_route(&tool) {
            Some(c) => c.clone(),
            None => {
                let msg = format!("no plugin provides tool '{tool}'");
                publish_result(broker, slash_request_id, false, "grant", &msg, json!({}));
                let _ = audit.record(
                    AuditKind::SlashUnknown,
                    Some(slash_request_id),
                    &json!({"details": msg}),
                );
                return;
            }
        },
    };
    let template_map: BTreeMap<String, Value> = match args.get("template") {
        Some(Value::Object(m)) => m.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        Some(_) => {
            reject_malformed(
                broker,
                audit,
                slash_request_id,
                "grant: `template` not an object",
            );
            return;
        }
        None => BTreeMap::new(),
    };
    let schema = schemas.get(&tool);
    let matcher = match UserGrants::compile_template(&tool, template_map, schema) {
        Ok(m) => m,
        Err(e) => {
            let msg = format!("template schema mismatch: {e}");
            publish_result(broker, slash_request_id, false, "grant", &msg, json!({}));
            let _ = audit.record(
                AuditKind::SlashUnknown,
                Some(slash_request_id),
                &json!({"details": msg}),
            );
            return;
        }
    };
    let grant = UserGrant {
        tool: tool.clone(),
        plugin: plugin.clone(),
        matcher,
        added_at: chrono::Utc::now(),
        source: GrantSource::SlashCommand,
    };
    let grant_id = user_grants.lock().add(grant);
    let _ = audit.record(
        AuditKind::GrantAdded,
        Some(slash_request_id),
        &json!({
            "source": "SlashCommand",
            "tool": tool,
            "plugin": plugin.to_string(),
            "grant_id": grant_id.0.to_string(),
        }),
    );
    publish_result(
        broker,
        slash_request_id,
        true,
        "grant",
        "",
        json!({"grant_id": grant_id.0.to_string()}),
    );
}

fn handle_list_grants(
    broker: &Broker,
    user_grants: &Arc<Mutex<UserGrants>>,
    audit: &AuditWriter,
    slash_request_id: &JsonRpcId,
) {
    let entries: Vec<Value> = {
        let g = user_grants.lock();
        g.list()
            .into_iter()
            .map(|(id, grant)| {
                let matcher = match &grant.matcher {
                    GrantMatcher::Any => json!({"kind": "any"}),
                    GrantMatcher::Structural { template } => {
                        json!({"kind": "structural", "template": template})
                    }
                };
                json!({
                    "grant_id": id.0.to_string(),
                    "tool": grant.tool,
                    "plugin": grant.plugin.to_string(),
                    "matcher": matcher,
                })
            })
            .collect()
    };
    let _ = audit.record(AuditKind::GrantList, Some(slash_request_id), &json!({}));
    publish_result(
        broker,
        slash_request_id,
        true,
        "list_grants",
        "",
        json!({"entries": entries}),
    );
}

fn handle_revoke(
    broker: &Broker,
    user_grants: &Arc<Mutex<UserGrants>>,
    audit: &AuditWriter,
    slash_request_id: &JsonRpcId,
    args: &Map<String, Value>,
) {
    let Some(raw_id) = args.get("grant_id").and_then(|v| v.as_str()) else {
        reject_malformed(
            broker,
            audit,
            slash_request_id,
            "revoke: missing `grant_id`",
        );
        return;
    };
    let Ok(ulid) = Ulid::from_string(raw_id) else {
        let msg = format!("unknown grant_id '{raw_id}'");
        publish_result(broker, slash_request_id, false, "revoke", &msg, json!({}));
        return;
    };
    let grant_id = GrantId(ulid);
    match user_grants.lock().revoke(grant_id) {
        Ok(()) => {
            let _ = audit.record(
                AuditKind::GrantRevoked,
                Some(slash_request_id),
                &json!({"grant_id": raw_id}),
            );
            publish_result(
                broker,
                slash_request_id,
                true,
                "revoke",
                "",
                json!({"grant_id": raw_id}),
            );
        }
        Err(RevokeError::Unknown(_)) => {
            let msg = format!("unknown grant_id '{raw_id}'");
            publish_result(broker, slash_request_id, false, "revoke", &msg, json!({}));
        }
    }
}

fn handle_unknown(
    broker: &Broker,
    audit: &AuditWriter,
    slash_request_id: &JsonRpcId,
    args: &Map<String, Value>,
) {
    let raw = args.get("raw").and_then(|v| v.as_str()).unwrap_or("");
    let msg = format!("unknown command: {raw}");
    publish_result(broker, slash_request_id, false, "unknown", &msg, json!({}));
    let _ = audit.record(
        AuditKind::SlashUnknown,
        Some(slash_request_id),
        &json!({"raw": raw}),
    );
}

fn reject_malformed(
    broker: &Broker,
    audit: &AuditWriter,
    slash_request_id: &JsonRpcId,
    details: &str,
) {
    publish_result(
        broker,
        slash_request_id,
        false,
        "unknown",
        "malformed payload",
        json!({"details": details}),
    );
    let _ = audit.record(
        AuditKind::SlashUnknown,
        Some(slash_request_id),
        &json!({"details": details}),
    );
}

fn publish_result(
    broker: &Broker,
    slash_request_id: &JsonRpcId,
    ok: bool,
    kind: &str,
    message: &str,
    details: Value,
) {
    let payload = json!({
        "ok": ok,
        "kind": kind,
        "message": message,
        "details": details,
    });
    let fresh = JsonRpcId::from(Ulid::new().to_string());
    if let Err(e) = broker.publish_core_with_taint(
        CORE_SESSION_COMMAND_RESULT,
        payload,
        Some(fresh),
        Some(vec![slash_request_id.clone()]),
        None,
        None,
    ) {
        tracing::error!(error = %e, "slash: command_result publish failed");
    }
}
