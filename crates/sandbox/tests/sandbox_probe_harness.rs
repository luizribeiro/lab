mod common;

use common::{run_probe, TestDir};

use capsa_sandbox::Sandbox;

#[test]
fn probe_can_run_under_sandbox_with_basic_read_allow() {
    let temp = TestDir::new("probe-harness");
    let file = temp.join("allowed.txt");
    std::fs::write(&file, b"ok").expect("failed to write fixture file");

    assert!(run_probe(
        Sandbox::builder().read_only_path(file.clone()),
        &["can-read", &file.display().to_string()]
    ));
}
