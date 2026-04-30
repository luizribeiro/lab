//! Pure parsers from backend-native log formats to `InferEvent`.
//!
//! Each backend (syd on Linux, Seatbelt on Darwin) writes a stream of
//! events in its own format. The parsers here turn one line/record of
//! that format into our cross-platform event model. They are pure
//! functions — testable without spawning the backend.

pub mod seatbelt;
pub mod syd;
