// crate doc placeholder; modules land in subsequent m1 commits.

pub mod carveout;
pub mod compile;
pub mod digest;
pub mod error;
pub mod lock;
pub mod manifest;
pub mod paths;
pub mod scrubber;
pub mod sinks;
pub mod topic_id;
pub mod trifecta;
pub mod validate;

pub use compile::{
    compile_plugin, CompiledFlags, CompiledPlugin, EnvPlan, FilesystemPlan, LimitsPlan,
    NetworkPlan, ToolMeta,
};
pub use error::{
    CarveOutError, CollisionError, CompileError, DigestError, Error, LockError, ManifestError,
    PathError, TrifectaError, ValidationError,
};
