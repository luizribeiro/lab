//! Lock-side types per scope §L. C12 lands `CanonicalId`; the
//! lock schema itself follows in c13+.

pub mod canonical_id;

pub use canonical_id::CanonicalId;
