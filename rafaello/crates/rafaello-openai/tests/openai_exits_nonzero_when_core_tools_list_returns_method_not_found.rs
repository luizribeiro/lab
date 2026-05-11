//! c36 §OP2 item 6 — when `core.tools_list` returns
//! `MethodNotFound` (the natural fittings behaviour for a
//! non-provider connection service composed without
//! `CorePluginService`), `rfl-openai` exits non-zero.

mod common;

use common::openai_provider_handle::{start_http_stub, OpenAiProviderHandle};

#[tokio::test]
async fn exits_nonzero_when_core_tools_list_returns_method_not_found() {
    let stub = start_http_stub(vec![]).await;
    let mut handle = OpenAiProviderHandle::launch_with_tools(stub, None).await;

    let status = handle.wait_exit().await;
    assert!(!status.success(), "expected non-zero exit, got {status:?}");
}
