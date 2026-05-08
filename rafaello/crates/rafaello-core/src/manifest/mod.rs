//! Manifest types and parsing primitives (scope §M).
//!
//! c03 lands the path-vocabulary and placeholder-expansion
//! infrastructure (§M8, §M11). The full `Manifest` top-level type
//! and table parsers land in later commits in Group 1.

pub mod capability_path_template;
pub mod placeholders;
pub mod provides;
pub mod safepath;
pub mod top_level;

pub use capability_path_template::CapabilityPathTemplate;
pub use provides::{Provides, ToolMetaManifest};
pub use safepath::SafePath;
pub use top_level::Manifest;
