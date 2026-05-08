// crate doc placeholder; modules land in subsequent m1 commits.

pub mod error;
pub mod manifest;
pub mod paths;
pub mod validate;

pub use error::{
    CarveOutError, CompileError, DigestError, Error, LockError, ManifestError, PathError,
    TrifectaError, ValidationError,
};
