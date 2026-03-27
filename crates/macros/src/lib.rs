//! Compile-test harness crate for fittings proc macros.

/// Small helper used by compile tests to ensure fixtures resolve this crate.
pub fn ui_helper(value: u32) -> u32 {
    value
}
