use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::Result;
use crate::error::McpfitError;


#[derive(Debug, Clone, Default)]
pub struct Cx {
    cancelled: Arc<AtomicBool>,
}

impl Cx {
    pub fn check_cancelled(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Acquire) {
            Err(McpfitError::Cancelled)
        } else {
            Ok(())
        }
    }

    #[allow(dead_code)]
    pub(crate) fn mark_cancelled(&self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_cancelled_is_ok_by_default() {
        let cx = Cx::default();
        assert!(cx.check_cancelled().is_ok());
    }

    #[test]
    fn check_cancelled_returns_cancelled_after_marking() {
        let cx = Cx::default();
        cx.mark_cancelled();
        assert_eq!(cx.check_cancelled(), Err(McpfitError::Cancelled));
    }

    #[test]
    fn cancellation_is_shared_across_clones() {
        let cx = Cx::default();
        let other = cx.clone();
        cx.mark_cancelled();
        assert_eq!(other.check_cancelled(), Err(McpfitError::Cancelled));
    }
}
