//! `.flags` — loud lock-level overrides per scope §L5.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LockFlags {
    #[serde(default)]
    pub i_know_what_im_doing: bool,
    #[serde(default)]
    pub allow_credential_paths: bool,
}
