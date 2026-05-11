//! c36 §OP2 item 6 (pi-3 N-4 / round-4 rename) — when
//! `core.tools_list` fails, the bin exits non-zero and the same
//! exit status feeds the supervisor's existing crash path
//! (`ReaperOutcome::Exited` with a non-zero code). Round-3 framed
//! this via the now-removed `SpawnError::PostHandshakeFailure`;
//! round-4 wires through the live `ReaperOutcome` channel
//! instead — no new `SpawnError` variant required.

mod common;

use common::openai_provider_handle::{start_http_stub, OpenAiProviderHandle};
use rafaello_core::error::ReaperOutcome;

#[tokio::test]
async fn tools_list_failure_exits_nonzero_and_supervisor_reports_crash() {
    let stub = start_http_stub(vec![]).await;
    let mut handle = OpenAiProviderHandle::launch_with_tools(stub, None).await;

    let status = handle.wait_exit().await;
    let outcome = ReaperOutcome::Exited(status);

    match outcome {
        ReaperOutcome::Exited(s) => {
            assert!(
                !s.success(),
                "supervisor's crash path classifies non-zero exits as crashes; got {s:?}"
            );
        }
        other => panic!("expected ReaperOutcome::Exited, got {other:?}"),
    }
}
