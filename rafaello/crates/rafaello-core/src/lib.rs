// crate doc placeholder; modules land in subsequent m1 commits.

pub mod broker_acl;
pub mod bus;
pub mod carveout;
pub mod compile;
pub mod digest;
pub mod entry;
pub mod error;
pub mod frontend;
pub mod lock;
pub mod manifest;
pub mod paths;
pub mod renderer;
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

pub use renderer::{
    Capabilities, ColorClass, RenderPipeline, Renderer, RendererError, RendererRegistry,
    ScrollbackClass, UnicodeClass,
};

pub use broker_acl::{AttachId, BrokerAcl, FrontendAcl, PluginAcl};
pub use compile::{
    compile_plugin, CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan,
    NetworkPlan, ToolMeta,
};
pub use error::{
    AttachIdParseError, BrokerError, CarveOutError, CollisionError, CompileError, DigestError,
    Error, FrontendSpawnError, InReplyToReason, InvalidFrontendPlanReason, InvalidPlanReason,
    LockError, ManifestError, PathError, PathKind, Publisher, ReaperOutcome, ShutdownFailure,
    SpawnError, TrifectaError, ValidationError,
};

pub use frontend::{
    CompiledFrontend, FrontendBusPublishService, FrontendConfig, FrontendExtraServiceFactory,
    FrontendHandle, FrontendPaths, FrontendReadyError, FrontendReadyService, FrontendSupervisor,
    PaintError, ShutdownReport,
};
