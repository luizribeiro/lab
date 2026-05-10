//! Build-only assertion that every m3 frontend error variant is
//! reachable through `rafaello_core::Error` (scope §F2 + §E).

use std::path::PathBuf;

use rafaello_core::broker_acl::AttachId;
use rafaello_core::{Error, FrontendSpawnError, InvalidFrontendPlanReason};

#[test]
fn frontend_errors_route_into_top_level() {
    let _: fn(FrontendSpawnError) -> Error = Error::from;
}

fn aid() -> AttachId {
    AttachId::new("tui").expect("test attach id parses")
}

#[allow(dead_code)]
fn _frontend_spawn_variants() {
    let _: FrontendSpawnError = FrontendSpawnError::InvalidPlan {
        reason: InvalidFrontendPlanReason::AttachIdInvalid {
            attach_id: "BAD".into(),
        },
    };
    let _: FrontendSpawnError = FrontendSpawnError::Io {
        source: std::io::Error::other("test"),
    };
    let _: FrontendSpawnError = FrontendSpawnError::Spawn {
        source: std::io::Error::other("test"),
    };
    let _: FrontendSpawnError = FrontendSpawnError::Transport {
        source: anyhow::anyhow!("test"),
    };
}

#[allow(dead_code, unreachable_patterns)]
fn _invalid_frontend_plan_reason_variants(r: InvalidFrontendPlanReason) {
    match r {
        InvalidFrontendPlanReason::AttachIdInvalid { .. } => {}
        InvalidFrontendPlanReason::EntryNotAbsolute { .. } => {}
        InvalidFrontendPlanReason::EntryNotExecutable { .. } => {}
        InvalidFrontendPlanReason::ControlCharsInPath { .. } => {}
        InvalidFrontendPlanReason::ReservedEnvName { .. } => {}
        InvalidFrontendPlanReason::AttachIdNotInAcl { .. } => {}
        InvalidFrontendPlanReason::AttachIdAlreadyRegistered { .. } => {}
        _ => {}
    }
}

#[allow(dead_code)]
fn _construct_aid_paths() {
    let _: InvalidFrontendPlanReason = InvalidFrontendPlanReason::EntryNotAbsolute {
        path: PathBuf::from("rel"),
    };
    let _: InvalidFrontendPlanReason = InvalidFrontendPlanReason::EntryNotExecutable {
        path: PathBuf::from("/x"),
    };
    let _: InvalidFrontendPlanReason = InvalidFrontendPlanReason::ControlCharsInPath {
        path: PathBuf::from("/x"),
    };
    let _: InvalidFrontendPlanReason =
        InvalidFrontendPlanReason::ReservedEnvName { var: "PATH".into() };
    let _: InvalidFrontendPlanReason =
        InvalidFrontendPlanReason::AttachIdNotInAcl { attach_id: aid() };
    let _: InvalidFrontendPlanReason =
        InvalidFrontendPlanReason::AttachIdAlreadyRegistered { attach_id: aid() };
}
