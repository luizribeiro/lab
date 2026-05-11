//! c25 — `rfl-tui` in headless test mode must peer-call `frontend.ready`
//! parent-side and exit cleanly via the `RFL_TUI_MAX_LIFETIME` self-timeout.

mod common;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use fittings_core::{
    context::ServiceContext,
    error::FittingsError,
    message::{JsonRpcId, Request, Response},
    service::Service,
};
use serde_json::Value;

use common::{expect_clean_exit, spawn_tui, SpawnOpts};

struct ReadyMock {
    fired: Arc<AtomicBool>,
}

#[async_trait]
impl Service for ReadyMock {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        let id = req.id.clone().unwrap_or(JsonRpcId::Null);
        if req.method == "frontend.ready" && req.id.is_some() {
            self.fired.store(true, Ordering::SeqCst);
            return Ok(Response {
                id,
                result: serde_json::json!({}),
                metadata: Default::default(),
            });
        }
        Ok(Response {
            id,
            result: Value::Null,
            metadata: Default::default(),
        })
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn tui_handler_calls_frontend_ready_then_self_exits() {
    let fired = Arc::new(AtomicBool::new(false));
    let svc = ReadyMock {
        fired: fired.clone(),
    };
    let mut h = spawn_tui(
        SpawnOpts {
            test_mode: true,
            max_lifetime: Some(2),
            ready_delay_ms: None,
            test_message: None,
        },
        svc,
    );

    expect_clean_exit(&mut h.child, Duration::from_millis(2500)).await;
    assert!(
        fired.load(Ordering::SeqCst),
        "frontend.ready never arrived parent-side"
    );
}
