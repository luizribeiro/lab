use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::Value;

use crate::Result;
use crate::error::McpfitError;
use crate::protocol::ProgressNotificationParams;

pub(crate) type ProgressSink = Arc<dyn Fn(ProgressNotificationParams) + Send + Sync>;

#[derive(Clone, Default)]
pub struct Cx {
    cancelled: Arc<AtomicBool>,
    progress: Option<(Value, ProgressSink)>,
}

impl std::fmt::Debug for Cx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cx")
            .field("cancelled", &self.cancelled.load(Ordering::Acquire))
            .field("progress_token", &self.progress.as_ref().map(|(t, _)| t))
            .finish()
    }
}

impl Cx {
    pub fn check_cancelled(&self) -> Result<()> {
        if self.cancelled.load(Ordering::Acquire) {
            Err(McpfitError::Cancelled)
        } else {
            Ok(())
        }
    }

    pub fn progress(&self, progress: f64) -> ProgressBuilder<'_> {
        ProgressBuilder {
            cx: self,
            progress,
            total: None,
            message: None,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn mark_cancelled(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub(crate) fn with_progress(token: Value, sink: ProgressSink) -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            progress: Some((token, sink)),
        }
    }

    pub(crate) fn with_external_cancellation(cancelled: Arc<AtomicBool>) -> Self {
        Self {
            cancelled,
            progress: None,
        }
    }

    pub(crate) fn with_progress_and_cancellation(
        token: Value,
        sink: ProgressSink,
        cancelled: Arc<AtomicBool>,
    ) -> Self {
        Self {
            cancelled,
            progress: Some((token, sink)),
        }
    }
}

pub(crate) fn extract_progress_token(meta: Option<&Value>) -> Option<Value> {
    let token = meta?.get("progressToken")?;
    match token {
        Value::String(_) | Value::Number(_) => Some(token.clone()),
        _ => None,
    }
}

#[must_use = "progress notifications are only sent on .emit()"]
pub struct ProgressBuilder<'a> {
    cx: &'a Cx,
    progress: f64,
    total: Option<f64>,
    message: Option<String>,
}

impl<'a> ProgressBuilder<'a> {
    pub fn total(mut self, total: f64) -> Self {
        self.total = Some(total);
        self
    }

    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    pub fn emit(self) {
        let Some((token, sink)) = self.cx.progress.as_ref() else {
            return;
        };
        sink(ProgressNotificationParams {
            progress_token: token.clone(),
            progress: self.progress,
            total: self.total,
            message: self.message,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    use serde_json::json;

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

    fn capturing_sink() -> (ProgressSink, Arc<Mutex<Vec<ProgressNotificationParams>>>) {
        let captured: Arc<Mutex<Vec<ProgressNotificationParams>>> = Arc::default();
        let sink_captured = Arc::clone(&captured);
        let sink: ProgressSink = Arc::new(move |params| {
            sink_captured.lock().expect("sink mutex").push(params);
        });
        (sink, captured)
    }

    #[test]
    fn progress_emit_is_no_op_without_token() {
        let cx = Cx::default();
        cx.progress(0.5).total(1.0).message("hi").emit();
    }

    #[test]
    fn progress_emit_sends_full_payload_when_token_present() {
        let (sink, captured) = capturing_sink();
        let cx = Cx::with_progress(json!("tok"), sink);
        cx.progress(0.25).total(1.0).message("quarter").emit();
        let events = captured.lock().expect("captured mutex");
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0],
            ProgressNotificationParams {
                progress_token: json!("tok"),
                progress: 0.25,
                total: Some(1.0),
                message: Some("quarter".into()),
            }
        );
    }

    #[test]
    fn extract_progress_token_accepts_string_and_number() {
        let s = json!({"progressToken": "abc"});
        assert_eq!(extract_progress_token(Some(&s)), Some(json!("abc")));
        let n = json!({"progressToken": 7});
        assert_eq!(extract_progress_token(Some(&n)), Some(json!(7)));
    }

    #[test]
    fn extract_progress_token_ignores_other_shapes() {
        for token in [json!(true), json!(null), json!([1]), json!({"x": 1})] {
            let meta = json!({"progressToken": token});
            assert_eq!(extract_progress_token(Some(&meta)), None);
        }
    }

    #[test]
    fn extract_progress_token_returns_none_when_absent() {
        assert_eq!(extract_progress_token(None), None);
        assert_eq!(extract_progress_token(Some(&json!({}))), None);
        assert_eq!(extract_progress_token(Some(&json!("not-an-object"))), None);
    }

    #[test]
    fn progress_emit_omits_optional_fields_by_default() {
        let (sink, captured) = capturing_sink();
        let cx = Cx::with_progress(json!(3), sink);
        cx.progress(2.0).emit();
        let events = captured.lock().expect("captured mutex");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].progress_token, json!(3));
        assert_eq!(events[0].progress, 2.0);
        assert!(events[0].total.is_none());
        assert!(events[0].message.is_none());
    }
}
