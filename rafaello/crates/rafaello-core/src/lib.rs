// crate doc placeholder; modules land in subsequent m1 commits.

pub mod broker_acl;
pub mod bus;
pub mod carveout;
pub mod compile;
pub mod digest;
pub mod entry;
pub mod error;
pub mod lock;
pub mod manifest;
pub mod paths;
pub mod scrubber;
pub mod sinks;
pub mod supervisor;
pub mod topic_id;
pub mod trifecta;
pub mod validate;

pub use entry::{
    Entry, EntryAuthor, EntryFallback, EntryMetadata, RawFormat, RenderNode, StreamState,
    ToolCallStatus,
};

pub use compile::{
    compile_plugin, CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan,
    NetworkPlan, ToolMeta,
};
pub use error::{
    BrokerError, CarveOutError, CollisionError, CompileError, DigestError, Error, InReplyToReason,
    InvalidPlanReason, LockError, ManifestError, PathError, PathKind, Publisher, ReaperOutcome,
    ShutdownFailure, SpawnError, TrifectaError, ValidationError,
};
