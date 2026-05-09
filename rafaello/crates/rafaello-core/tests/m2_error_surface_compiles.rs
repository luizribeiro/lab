//! Build-only assertion that every m2 error variant is reachable
//! through `rafaello_core::Error` (scope §B2 + §SP3 + §E).
//!
//! Variants are constructed cheaply with placeholder source errors
//! (`anyhow::anyhow!`, `std::io::Error::other`, etc.); variants whose
//! source types have no public constructor (`std::process::ExitStatus`)
//! are exercised through exhaustiveness matches.

use std::path::PathBuf;

use rafaello_core::lock::CanonicalId;
use rafaello_core::{
    BrokerError, Error, InReplyToReason, InvalidPlanReason, PathKind, Publisher, ReaperOutcome,
    ShutdownFailure, SpawnError,
};

#[test]
fn module_errors_route_into_top_level() {
    let _: fn(BrokerError) -> Error = Error::from;
    let _: fn(SpawnError) -> Error = Error::from;
}

fn cid() -> CanonicalId {
    CanonicalId::parse("local/test:plugin@0.1.0").expect("test canonical id parses")
}

#[allow(dead_code)]
fn _broker_variants() {
    let _: BrokerError = BrokerError::NotInAcl(cid());
    let _: BrokerError = BrokerError::NotRegistered(cid());
    let _: BrokerError = BrokerError::AlreadyRegistered(cid());
    let _: BrokerError = BrokerError::UnknownNamespace {
        publisher: Publisher::Core,
        topic: "x".into(),
    };
    let _: BrokerError = BrokerError::PublishOnReservedNamespace {
        publisher: Publisher::Plugin(cid()),
        topic: "x".into(),
    };
    let _: BrokerError = BrokerError::PublishOutsideGrant {
        canonical: cid(),
        topic: "x".into(),
    };
    let _: BrokerError = BrokerError::InvalidTopic {
        publisher: Publisher::Core,
        topic: "x".into(),
        reason: "bad".into(),
    };
    let _: BrokerError = BrokerError::InvalidPattern {
        reason: "bad".into(),
    };
    let _: BrokerError = BrokerError::InvalidPayload {
        publisher: Publisher::Core,
        reason: "bad".into(),
    };
    let _: BrokerError = BrokerError::InvalidInReplyTo {
        canonical: cid(),
        topic: "x".into(),
        reason: InReplyToReason::Missing,
    };
    let _: InReplyToReason = InReplyToReason::EmptyArray;
    let _: InReplyToReason = InReplyToReason::UnexpectedMultiple;
    let _: BrokerError = BrokerError::Internal {
        detail: "boom".into(),
    };
}

#[allow(dead_code)]
fn _spawn_variants() {
    let _: SpawnError = SpawnError::NotInAcl(cid());
    let _: SpawnError = SpawnError::AlreadyRegistered(cid());
    let _: SpawnError = SpawnError::InvalidPlan {
        canonical: cid(),
        reason: InvalidPlanReason::NonAbsolutePath {
            kind: PathKind::ReadPath,
            path: PathBuf::from("rel"),
        },
    };
    let _: InvalidPlanReason = InvalidPlanReason::ControlCharsInPath {
        kind: PathKind::WritePath,
        path: PathBuf::from("/x"),
    };
    let _: InvalidPlanReason = InvalidPlanReason::TopicIdMismatch {
        expected: "a".into(),
        got: "b".into(),
    };
    let _: InvalidPlanReason = InvalidPlanReason::ProviderNotInM2 {
        provider_id: "p".into(),
    };
    let _: SpawnError = SpawnError::EntryNotExecutable {
        canonical: cid(),
        path: PathBuf::from("/x"),
    };
    let _: SpawnError = SpawnError::SandboxBuild {
        canonical: cid(),
        source: anyhow::anyhow!("test"),
    };
    let _: SpawnError = SpawnError::Spawn {
        canonical: cid(),
        source: std::io::Error::other("test"),
    };
    let _: SpawnError = SpawnError::ProxyStart {
        canonical: cid(),
        source: std::io::Error::other("test"),
    };
    let _: SpawnError = SpawnError::Socketpair {
        canonical: cid(),
        source: nix::errno::Errno::EAGAIN,
    };
    let _: SpawnError = SpawnError::ReservedEnvInPlan {
        canonical: cid(),
        var: "PATH".into(),
    };
    let _: SpawnError = SpawnError::TransportSetup {
        canonical: cid(),
        source: std::io::Error::other("test"),
    };
    let _: SpawnError = SpawnError::PrivateStateDirCreate {
        canonical: cid(),
        path: PathBuf::from("/x"),
        source: std::io::Error::other("test"),
    };
}

#[allow(dead_code, unreachable_patterns)]
fn _invalid_plan_reason_variants(r: InvalidPlanReason) {
    match r {
        InvalidPlanReason::NonAbsolutePath { .. } => {}
        InvalidPlanReason::ControlCharsInPath { .. } => {}
        InvalidPlanReason::TopicIdMismatch { .. } => {}
        InvalidPlanReason::NetworkAllowHostsInvalid { .. } => {}
        InvalidPlanReason::ProviderNotInM2 { .. } => {}
        _ => {}
    }
}

#[allow(dead_code, unreachable_patterns)]
fn _path_kind_variants(k: PathKind) {
    match k {
        PathKind::ReadPath => {}
        PathKind::ReadDir => {}
        PathKind::WritePath => {}
        PathKind::WriteDir => {}
        PathKind::ExecPath => {}
        PathKind::ExecDir => {}
        PathKind::EntryAbsolute => {}
        PathKind::ProjectRoot => {}
        PathKind::PrivateStateDir => {}
        _ => {}
    }
}

#[allow(dead_code, unreachable_patterns)]
fn _reaper_outcome_variants(o: ReaperOutcome) {
    match o {
        ReaperOutcome::Exited(_) => {}
        ReaperOutcome::WaitFailed(_) => {}
        ReaperOutcome::ReaperPanicked => {}
        _ => {}
    }
}

#[allow(dead_code)]
fn _shutdown_failure_variants() {
    let _: ShutdownFailure = ShutdownFailure::SignalSendFailed(nix::errno::Errno::EAGAIN);
    let _: ShutdownFailure = ShutdownFailure::WaitFailed {
        kind: std::io::ErrorKind::Other,
        message: "x".into(),
    };
    let _: ShutdownFailure = ShutdownFailure::ReaperPanicked;
}
