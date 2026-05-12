mod common;
use common::EnvGuard;
use rafaello_fetch::maybe_write_invocation_log;

#[tracing_test::traced_test]
#[test]
fn log_unwritable_path_warns_and_continues() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Path whose parent directory does not exist — create+open fails.
    let unwritable = dir.path().join("missing-dir/log.txt");
    let _g = EnvGuard::set("RFL_FETCH_TEST_LOG_PATH", unwritable.to_str().unwrap());

    maybe_write_invocation_log("https://example.com/x");

    assert!(
        logs_contain("failed to write invocation log"),
        "expected warn log from rafaello-fetch; none captured"
    );
}
