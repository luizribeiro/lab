//! Typed-error enums for rafaello-core (scope §E1).
//!
//! Variants are skeleton placeholders; structured fields land in
//! the commits that construct them. The variant *names* are the
//! contract here so subsequent commits can `?`-propagate through
//! the top-level [`Error`] without churn.

use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ManifestError {
    #[error("reserved field")]
    ReservedField,
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
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ValidationError {
    #[error("publish on reserved namespace")]
    PublishOnReservedNamespace,
    #[error("publish on frontend namespace from non-frontend plugin")]
    PublishOnFrontendNamespace,
    #[error("publish on foreign topic id")]
    PublishOnForeignTopicId,
    #[error("provider namespace mismatch")]
    ProviderNamespaceMismatch,
    #[error("pattern in publish position")]
    PatternInPublishPosition,
    #[error("invalid pattern segment")]
    InvalidPatternSegment,
    #[error("illegal topic segment")]
    IllegalTopicSegment,
    #[error("topic has too few segments")]
    TopicTooFewSegments,
    #[error("illegal tool name")]
    IllegalToolName,
    #[error("unknown tool table")]
    UnknownToolTable,
    #[error("unknown bundle key")]
    UnknownBundleKey,
    #[error("reserved renderer kind")]
    ReservedRendererKind,
    #[error("unprefixed renderer kind")]
    UnprefixedRendererKind,
    #[error("load trigger references unknown command")]
    LoadTriggerUnknownCommand,
    #[error("allow_hosts set outside proxy mode")]
    AllowHostsOutsideProxy,
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
    #[error("trifecta refused")]
    TrifectaRefused,
    #[error("sink inference drift")]
    SinkInferenceDrift,
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
}
