//! Build-only assertion that every module-local error enum is
//! reachable through `rafaello_core::Error` via `#[from]`, and
//! that the variant names enumerated in scope §E1 exist.
//!
//! No variants are *constructed* here — that's the job of the
//! commits that produce these errors. This test only proves the
//! type-level surface compiles.

use rafaello_core::{
    CarveOutError, CompileError, DigestError, Error, LockError, ManifestError, TrifectaError,
    ValidationError,
};

#[test]
fn module_errors_route_into_top_level() {
    let _: fn(ManifestError) -> Error = Error::from;
    let _: fn(LockError) -> Error = Error::from;
    let _: fn(ValidationError) -> Error = Error::from;
    let _: fn(CompileError) -> Error = Error::from;
    let _: fn(DigestError) -> Error = Error::from;
    let _: fn(CarveOutError) -> Error = Error::from;
    let _: fn(TrifectaError) -> Error = Error::from;
    let _: fn(CompileError) -> CarveOutError = CarveOutError::from;
}

// Variant-name reachability: pattern-match against every name
// scope §E1 enumerates. Wrapped behind `cfg(any())` so it never
// runs, but the compiler still type-checks each pattern.
#[allow(dead_code, unreachable_patterns)]
fn _manifest_variant_names(e: ManifestError) {
    match e {
        ManifestError::ReservedField { .. } => {}
        ManifestError::UnknownField => {}
        ManifestError::MissingOpenRpc => {}
        ManifestError::EntryEscape => {}
        ManifestError::EntryNotFound => {}
        ManifestError::EntryNotFile => {}
        ManifestError::GrantMatchEscape => {}
        ManifestError::GrantMatchNotFound => {}
        ManifestError::GrantMatchNotFile => {}
        ManifestError::Toml(_) => {}
        ManifestError::Serde(_) => {}
        _ => {}
    }
}

#[allow(dead_code, unreachable_patterns)]
fn _lock_variant_names(e: LockError) {
    match e {
        LockError::MissingEntry => {}
        LockError::Toml(_) => {}
        _ => {}
    }
}

#[allow(dead_code, unreachable_patterns)]
fn _validation_variant_names(e: ValidationError) {
    match e {
        ValidationError::PublishOnReservedNamespace => {}
        ValidationError::PublishOnFrontendNamespace => {}
        ValidationError::PublishOnForeignTopicId => {}
        ValidationError::ProviderNamespaceMismatch => {}
        ValidationError::PatternInPublishPosition => {}
        ValidationError::InvalidPatternSegment => {}
        ValidationError::IllegalTopicSegment => {}
        ValidationError::TopicTooFewSegments => {}
        ValidationError::IllegalToolName => {}
        ValidationError::UnknownToolTable => {}
        ValidationError::UnknownBundleKey => {}
        ValidationError::ReservedRendererKind => {}
        ValidationError::UnprefixedRendererKind => {}
        ValidationError::LoadTriggerUnknownCommand => {}
        ValidationError::AllowHostsOutsideProxy => {}
        ValidationError::ExecPathInsideProject => {}
        ValidationError::ProviderActiveUnknown => {}
        ValidationError::ProviderActiveNotProvider => {}
        ValidationError::ConflictingToolName => {}
        ValidationError::ToolOwnerUnknownPlugin => {}
        ValidationError::ToolOwnerPluginDoesNotDeclareTool => {}
        ValidationError::ToolOwnerRedundant => {}
        ValidationError::MissingPluginDir => {}
        ValidationError::TrifectaRefused => {}
        ValidationError::SinkInferenceDrift => {}
        ValidationError::LockPublishOnReservedNamespace => {}
        ValidationError::LockPublishOnFrontendNamespace => {}
        ValidationError::LockPublishOnForeignTopicId => {}
        ValidationError::LockProviderNamespaceMismatch => {}
        ValidationError::LockAllowHostsOutsideProxy => {}
        ValidationError::LockUnknownBundleKey => {}
        ValidationError::LockCapabilityPathRelative => {}
        ValidationError::OrphanToolMeta => {}
        ValidationError::ProviderIdInconsistent => {}
        _ => {}
    }
}

#[allow(dead_code, unreachable_patterns)]
fn _compile_variant_names(e: CompileError) {
    match e {
        CompileError::ValidationNotRun => {}
        CompileError::UnknownPlaceholder => {}
        CompileError::PathEscape => {}
        CompileError::SymlinkEscape => {}
        CompileError::ReservedEnvVarRequested => {}
        CompileError::InvalidAllowHosts => {}
        CompileError::ContentDigestMismatch => {}
        CompileError::ManifestDigestMismatch => {}
        CompileError::EntryEscape => {}
        CompileError::EntryNotFound => {}
        CompileError::EntryNotFile => {}
        CompileError::CarveOutRefused => {}
        CompileError::CarveOutTooLarge => {}
        _ => {}
    }
}

#[allow(dead_code, unreachable_patterns)]
fn _digest_variant_names(e: DigestError) {
    match e {
        DigestError::SymlinkEscape => {}
        DigestError::SymlinkCycle => {}
        DigestError::Io(_) => {}
        _ => {}
    }
}

#[allow(dead_code)]
fn _carveout_variant_names(e: CarveOutError) {
    if let CarveOutError::Compile(_) = e {}
}

#[allow(dead_code)]
fn _trifecta_variant_names(_e: TrifectaError) {
    // intentionally empty: TrifectaError is empty in v1.
}
