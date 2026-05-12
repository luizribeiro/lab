mod common;
use common::EnvGuard;
use rafaello_fetch::maybe_write_invocation_log;

#[test]
fn log_unset_path_does_not_fail() {
    let _g = EnvGuard::clear("RFL_FETCH_TEST_LOG_PATH");
    maybe_write_invocation_log("https://example.com/anything");
}
