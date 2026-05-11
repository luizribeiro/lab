//! Typed-error enums for rafaello-core (scope §E1).
//!
//! Variants are skeleton placeholders; structured fields land in
//! the commits that construct them. The variant *names* are the
//! contract here so subsequent commits can `?`-propagate through
//! the top-level [`enum@Error`] without churn.

use std::path::PathBuf;

use fittings_core::error::FittingsError;
use fittings_core::message::JsonRpcId;
use thiserror::Error;

use crate::broker_acl::AttachId;
use crate::bus::TaintEntry;
use crate::lock::CanonicalId;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ManifestError {
    #[error("reserved field `{field}`: {hint}")]
    ReservedField { field: String, hint: &'static str },
    #[error("unknown field")]
    UnknownField,
    #[error("safepath: leading slash")]
    SafePathLeadingSlash,
    #[error("safepath: empty segment")]
    SafePathEmptySegment,
    #[error("safepath: parent-dir traversal")]
    SafePathParentDir,
    #[error("safepath: backslash separator")]
    SafePathBackslash,
    #[error("safepath: control character")]
    SafePathControlChar,
    #[error("safepath: empty path")]
    SafePathEmpty,
    #[error("capability path: bare relative path")]
    CapabilityPathBareRelative,
    #[error("capability path: backslash separator")]
    CapabilityPathBackslash,
    #[error("capability path: control character")]
    CapabilityPathControlChar,
    #[error("capability path: malformed placeholder")]
    CapabilityPathMalformedPlaceholder,
    #[error("unknown placeholder in path")]
    UnknownPlaceholder,
    #[error("malformed placeholder syntax")]
    MalformedPlaceholder,
    #[error("missing openrpc.json sibling")]
    MissingOpenRpc,
    #[error("entry path escapes package_dir")]
    EntryEscape,
    #[error("entry path not found")]
    EntryNotFound,
    #[error("entry path is not a regular file")]
    EntryNotFile,
    #[error("grant_match path escapes package_dir")]
    GrantMatchEscape,
    #[error("grant_match path not found")]
    GrantMatchNotFound,
    #[error("grant_match path is not a regular file")]
    GrantMatchNotFile,
    #[error("exec_path resolves inside ${{project}}")]
    ExecPathInsideProject,
    #[error(transparent)]
    Validation(ValidationError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("toml parse error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("serde error: {0}")]
    Serde(#[from] serde_json::Error),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum LockError {
    #[error("lock entry missing required field")]
    MissingEntry,
    #[error("toml parse error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("canonical id `{input}` missing `:` between source and name")]
    CanonicalIdMissingNameSeparator { input: String },
    #[error("canonical id `{input}` missing `@` between name and version")]
    CanonicalIdMissingVersionSeparator { input: String },
    #[error("canonical id source is empty")]
    CanonicalIdEmptySource,
    #[error("canonical id source has leading `/`")]
    CanonicalIdSourceLeadingSlash,
    #[error("canonical id source has trailing `/`")]
    CanonicalIdSourceTrailingSlash,
    #[error("canonical id source has empty segment")]
    CanonicalIdSourceEmptySegment,
    #[error("canonical id source segment `{segment}` is `.` or `..`")]
    CanonicalIdSourceDotSegment { segment: String },
    #[error("canonical id source segment `{segment}` contains illegal character")]
    CanonicalIdIllegalSourceSegment { segment: String },
    #[error("canonical id name `{name}` violates topic-segment grammar")]
    CanonicalIdIllegalName { name: String },
    #[error("canonical id version `{version}` is not valid semver: {source}")]
    CanonicalIdInvalidVersion {
        version: String,
        #[source]
        source: semver::Error,
    },
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ValidationError {
    #[error("illegal manifest name `{name}`")]
    IllegalManifestName { name: String },
    #[error("publish on reserved namespace: `{topic}`")]
    PublishOnReservedNamespace { topic: String },
    #[error("publish on frontend namespace from non-frontend plugin: `{topic}`")]
    PublishOnFrontendNamespace { topic: String },
    #[error("publish on unknown top-level namespace `{top}`: `{topic}`")]
    PublishUnknownNamespace { topic: String, top: String },
    #[error("publish on foreign topic id")]
    PublishOnForeignTopicId,
    #[error("provider namespace mismatch")]
    ProviderNamespaceMismatch,
    #[error("pattern in publish position: `{topic}`")]
    PatternInPublishPosition { topic: String },
    #[error("invalid pattern segment `{segment}` in pattern `{pattern}`")]
    InvalidPatternSegment { pattern: String, segment: String },
    #[error("illegal topic segment `{segment}` in topic `{topic}`")]
    IllegalTopicSegment { topic: String, segment: String },
    #[error("topic `{topic}` has too few segments")]
    TopicTooFewSegments { topic: String },
    #[error("illegal tool name `{name}`")]
    IllegalToolName { name: String },
    #[error("illegal sink class `{class}`")]
    IllegalSinkClass { class: String },
    #[error("unknown tool table `{tool}`")]
    UnknownToolTable { tool: String },
    #[error("unknown bundle key `{bundle}`")]
    UnknownBundleKey { bundle: String },
    #[error("reserved renderer kind `{kind}`")]
    ReservedRendererKind { kind: String },
    #[error("unprefixed renderer kind `{kind}`")]
    UnprefixedRendererKind { kind: String },
    #[error("load trigger references unknown command `{command}`")]
    LoadTriggerUnknownCommand { command: String },
    #[error("load trigger event `{event}` not matched by any bus subscribe pattern")]
    LoadTriggerUnmatchedEvent { event: String },
    #[error("load trigger references unknown renderer kind `{kind}`")]
    LoadTriggerUnknownKind { kind: String },
    #[error("allow_hosts set outside proxy mode in bundle `{bundle}`")]
    AllowHostsOutsideProxy { bundle: String },
    #[error("exec_path inside project")]
    ExecPathInsideProject,
    #[error("session.provider_active references unknown plugin")]
    ProviderActiveUnknown,
    #[error("session.provider_active references non-provider plugin")]
    ProviderActiveNotProvider,
    #[error("conflicting tool names across plugins")]
    ConflictingToolName,
    #[error("session.tool_owner references unknown plugin")]
    ToolOwnerUnknownPlugin,
    #[error("session.tool_owner plugin does not declare tool")]
    ToolOwnerPluginDoesNotDeclareTool,
    #[error("session.tool_owner is redundant (no conflict)")]
    ToolOwnerRedundant,
    #[error("plugin_dirs missing entry for installed plugin")]
    MissingPluginDir,
    #[error("topic-id collision: {0}")]
    TopicIdCollision(#[from] CollisionError),
    #[error("trifecta refused (reads_untrusted={reads_untrusted}, has_outbound={has_outbound}, has_workspace_write={has_workspace_write})")]
    TrifectaRefused {
        reads_untrusted: bool,
        has_outbound: bool,
        has_workspace_write: bool,
    },
    #[error("carve-out refused")]
    CarveOutRefused,
    #[error("carve-out decomposition exceeds cap")]
    CarveOutTooLarge,
    #[error("sink inference drift for tool `{tool}` (expected={expected:?}, found={found:?})")]
    SinkInferenceDrift {
        tool: String,
        expected: Vec<String>,
        found: Vec<String>,
    },
    #[error("lock publish on reserved namespace")]
    LockPublishOnReservedNamespace,
    #[error("lock publish on frontend namespace from non-frontend plugin")]
    LockPublishOnFrontendNamespace,
    #[error("lock publish on foreign topic id")]
    LockPublishOnForeignTopicId,
    #[error("lock provider namespace mismatch")]
    LockProviderNamespaceMismatch,
    #[error("lock allow_hosts set outside proxy mode")]
    LockAllowHostsOutsideProxy,
    #[error("lock unknown bundle key")]
    LockUnknownBundleKey,
    #[error("lock capability path is relative")]
    LockCapabilityPathRelative,
    #[error("orphan tool_meta entry")]
    OrphanToolMeta,
    #[error("provider id inconsistent with bindings.provider")]
    ProviderIdInconsistent,
    #[error("allow_secrets entry has invalid env-var name shape: `{name}`")]
    AllowSecretsInvalidName { name: String },
    #[error("allow_secrets entry reserves a core-owned env var: `{name}`")]
    AllowSecretsReservesCoreName { name: String },
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CompileError {
    #[error("validate::lock was not run before compile")]
    ValidationNotRun,
    #[error("unknown placeholder in path")]
    UnknownPlaceholder,
    #[error("path escapes its root after expansion")]
    PathEscape,
    #[error("symlink target escapes its root")]
    SymlinkEscape,
    #[error("reserved env var requested")]
    ReservedEnvVarRequested,
    #[error("invalid allow_hosts entry")]
    InvalidAllowHosts,
    #[error("content digest mismatch")]
    ContentDigestMismatch,
    #[error("manifest digest mismatch")]
    ManifestDigestMismatch,
    #[error("entry path escapes plugin_dir")]
    EntryEscape,
    #[error("entry path not found")]
    EntryNotFound,
    #[error("entry path is not a regular file")]
    EntryNotFile,
    #[error("carve-out refused")]
    CarveOutRefused,
    #[error("carve-out decomposition exceeds cap")]
    CarveOutTooLarge,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum DigestError {
    #[error("symlink target escapes package root")]
    SymlinkEscape,
    #[error("symlink cycle detected")]
    SymlinkCycle,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PathError {
    #[error("unknown placeholder in path")]
    UnknownPlaceholder,
    #[error("malformed placeholder syntax")]
    MalformedPlaceholder,
    #[error("path is not absolute after expansion")]
    NotAbsolute,
    #[error("path escapes its root after expansion")]
    PathEscape,
    #[error("symlink target escapes its root")]
    SymlinkEscape,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CarveOutError {
    #[error(transparent)]
    Compile(#[from] CompileError),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TrifectaError {}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CollisionError {
    #[error("topic-id collision between `{a}` and `{b}` on prefix `{prefix}`")]
    TopicIdCollision {
        a: String,
        b: String,
        prefix: String,
    },
}

/// Identifies the source of a publish/subscribe call (scope §B2).
#[derive(Debug)]
#[non_exhaustive]
pub enum Publisher {
    Core,
    Plugin(CanonicalId),
    Frontend(AttachId),
    Provider {
        canonical: CanonicalId,
        provider_id: String,
    },
}

/// Why an [`AttachId`] failed to parse (scope §B1).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AttachIdParseError {
    #[error("attach id is empty")]
    Empty,
    #[error("attach id length {len} exceeds 32 bytes")]
    TooLong { len: usize },
    #[error("attach id must start with `[a-z]`, got `{ch}`")]
    IllegalLeadChar { ch: char },
    #[error("attach id contains illegal character `{ch}`")]
    IllegalChar { ch: char },
}

/// Why an `in_reply_to` field on a publish was rejected (scope §B2).
#[derive(Debug)]
#[non_exhaustive]
pub enum InReplyToReason {
    Missing,
    EmptyArray,
    UnexpectedMultiple,
    StaleRequestId { id: JsonRpcId },
}

/// Why a `taint` field on a provider publish was rejected (scope §B4, §B7b).
#[derive(Debug)]
#[non_exhaustive]
pub enum TaintReason {
    Missing,
    EmptyArray,
    UnknownSource { source: String },
}

/// Errors raised by the in-process broker (scope §B2).
///
/// Intentionally `Debug + Error` only — not `Clone`, not `PartialEq`,
/// because future variants may carry non-cloneable sources.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BrokerError {
    #[error("plugin `{0}` not in broker ACL")]
    NotInAcl(CanonicalId),
    #[error("plugin `{0}` not registered with broker")]
    NotRegistered(CanonicalId),
    #[error("plugin `{0}` already registered with broker")]
    AlreadyRegistered(CanonicalId),
    #[error("frontend `{0}` not in broker ACL")]
    FrontendNotInAcl(AttachId),
    #[error("frontend `{0}` not registered with broker")]
    FrontendNotRegistered(AttachId),
    #[error("frontend `{0}` already registered with broker")]
    FrontendAlreadyRegistered(AttachId),
    #[error("publisher {publisher:?} subscribed to unknown namespace: `{topic}`")]
    UnknownNamespace { publisher: Publisher, topic: String },
    #[error("publisher {publisher:?} attempted publish on reserved namespace: `{topic}`")]
    PublishOnReservedNamespace { publisher: Publisher, topic: String },
    #[error("publisher {publisher:?} published outside its grant: `{topic}`")]
    PublishOutsideGrant { publisher: Publisher, topic: String },
    #[error("publisher {publisher:?} sent invalid topic `{topic}`: {reason}")]
    InvalidTopic {
        publisher: Publisher,
        topic: String,
        reason: String,
    },
    #[error("invalid subscribe pattern: {reason}")]
    InvalidPattern { reason: String },
    #[error("publisher {publisher:?} sent invalid payload: {reason}")]
    InvalidPayload {
        publisher: Publisher,
        reason: String,
    },
    #[error("publisher {publisher:?} sent invalid in_reply_to on `{topic}`: {reason:?}")]
    InvalidInReplyTo {
        publisher: Publisher,
        topic: String,
        reason: InReplyToReason,
    },
    #[error("provider `{0}` not in broker ACL")]
    ProviderNotInAcl(CanonicalId),
    #[error("provider `{0}` not registered with broker")]
    ProviderNotRegistered(CanonicalId),
    #[error("provider `{0}` already registered with broker")]
    ProviderAlreadyRegistered(CanonicalId),
    #[error("publisher {publisher:?} missing request_id on `{topic}`")]
    MissingRequestId { publisher: Publisher, topic: String },
    #[error("plugin `{canonical}` published tool_result citing stale request_id `{id:?}`")]
    StaleRequestId {
        canonical: CanonicalId,
        id: JsonRpcId,
    },
    #[error("publisher {publisher:?} sent invalid taint on `{topic}`: {reason:?}")]
    InvalidTaint {
        publisher: Publisher,
        topic: String,
        reason: TaintReason,
    },
    #[error(
        "publisher {publisher:?} published taint on `{topic}` that is not a superset of \
         in_reply_to ancestry; missing entries: {missing:?}"
    )]
    TaintSupersetViolated {
        publisher: Publisher,
        topic: String,
        missing: Vec<TaintEntry>,
    },
    #[error("internal broker error: {detail}")]
    Internal { detail: String },
}

/// Which path-shaped field of a compiled plan was malformed (scope §SP3).
#[derive(Debug)]
#[non_exhaustive]
pub enum PathKind {
    ReadPath,
    ReadDir,
    WritePath,
    WriteDir,
    ExecPath,
    ExecDir,
    EntryAbsolute,
    ProjectRoot,
    PrivateStateDir,
}

/// Why a compiled plan failed structural validation at spawn time
/// (scope §SP3).
#[derive(Debug)]
#[non_exhaustive]
pub enum InvalidPlanReason {
    NonAbsolutePath {
        kind: PathKind,
        path: PathBuf,
    },
    ControlCharsInPath {
        kind: PathKind,
        path: PathBuf,
    },
    TopicIdMismatch {
        expected: String,
        got: String,
    },
    NetworkAllowHostsInvalid {
        source: outpost::DomainPatternParseError,
    },
}

/// How a previously-spawned plugin process exited (scope §SP3).
#[derive(Debug)]
#[non_exhaustive]
pub enum ReaperOutcome {
    Exited(std::process::ExitStatus),
    WaitFailed(std::io::Error),
    ReaperPanicked,
}

/// Cloneable shareable shape of a shutdown failure (scope §B2 / pi-2).
///
/// Mirrors `std::io::Error` into kind + message so the value can be
/// passed across `tokio::sync::watch` channels.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum ShutdownFailure {
    SignalSendFailed(nix::errno::Errno),
    WaitFailed {
        kind: std::io::ErrorKind,
        message: String,
    },
    ReaperPanicked,
}

/// Errors raised by the spawn pipeline (scope §SP3).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SpawnError {
    #[error("plugin `{0}` not in spawn ACL")]
    NotInAcl(CanonicalId),
    #[error("plugin `{0}` already registered with spawn supervisor")]
    AlreadyRegistered(CanonicalId),
    #[error("invalid compiled plan for `{canonical}`: {reason:?}")]
    InvalidPlan {
        canonical: CanonicalId,
        reason: InvalidPlanReason,
    },
    #[error("entry path `{path}` for `{canonical}` is not executable")]
    EntryNotExecutable {
        canonical: CanonicalId,
        path: PathBuf,
    },
    #[error("sandbox build failed for `{canonical}`: {source}")]
    SandboxBuild {
        canonical: CanonicalId,
        #[source]
        source: anyhow::Error,
    },
    #[error("spawn failed for `{canonical}`: {source}")]
    Spawn {
        canonical: CanonicalId,
        #[source]
        source: std::io::Error,
    },
    #[error("proxy start failed for `{canonical}`: {source}")]
    ProxyStart {
        canonical: CanonicalId,
        #[source]
        source: std::io::Error,
    },
    #[error("socketpair failed for `{canonical}`: {source}")]
    Socketpair {
        canonical: CanonicalId,
        #[source]
        source: nix::errno::Errno,
    },
    #[error("fittings build failed for `{canonical}`: {source}")]
    FittingsBuild {
        canonical: CanonicalId,
        #[source]
        source: FittingsError,
    },
    #[error("compiled plan for `{canonical}` requested reserved env var `{var}`")]
    ReservedEnvInPlan { canonical: CanonicalId, var: String },
    #[error("transport setup failed for `{canonical}`: {source}")]
    TransportSetup {
        canonical: CanonicalId,
        #[source]
        source: std::io::Error,
    },
    #[error("private state dir `{path}` create failed for `{canonical}`: {source}")]
    PrivateStateDirCreate {
        canonical: CanonicalId,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Why a [`CompiledFrontend`](crate::frontend::CompiledFrontend) failed
/// structural validation at frontend spawn time (scope §F2).
#[derive(Debug)]
#[non_exhaustive]
pub enum InvalidFrontendPlanReason {
    AttachIdInvalid { attach_id: String },
    EntryNotAbsolute { path: PathBuf },
    EntryNotExecutable { path: PathBuf },
    ControlCharsInPath { path: PathBuf },
    ReservedEnvName { var: String },
    AttachIdNotInAcl { attach_id: AttachId },
    AttachIdAlreadyRegistered { attach_id: AttachId },
}

/// Errors raised by the frontend spawn pipeline (scope §F2).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FrontendSpawnError {
    #[error("invalid compiled frontend plan: {reason:?}")]
    InvalidPlan { reason: InvalidFrontendPlanReason },
    #[error("io error: {source}")]
    Io {
        #[source]
        source: std::io::Error,
    },
    #[error("frontend spawn failed: {source}")]
    Spawn {
        #[source]
        source: std::io::Error,
    },
    #[error("frontend transport setup failed: {source}")]
    Transport {
        #[source]
        source: anyhow::Error,
    },
    #[error("frontend broker registration failed: {source}")]
    BrokerRegister {
        #[source]
        source: BrokerError,
    },
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error(transparent)]
    Lock(#[from] LockError),
    #[error(transparent)]
    Validation(#[from] ValidationError),
    #[error(transparent)]
    Compile(#[from] CompileError),
    #[error(transparent)]
    Digest(#[from] DigestError),
    #[error(transparent)]
    CarveOut(#[from] CarveOutError),
    #[error(transparent)]
    Trifecta(#[from] TrifectaError),
    #[error(transparent)]
    Path(#[from] PathError),
    #[error(transparent)]
    Collision(#[from] CollisionError),
    #[error(transparent)]
    Broker(#[from] BrokerError),
    #[error(transparent)]
    Spawn(#[from] SpawnError),
    #[error(transparent)]
    FrontendSpawn(#[from] FrontendSpawnError),
}
