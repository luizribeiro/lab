use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use scope::PluginRunner;
use scope::plugin::protocol::ReaderOptions;
use scope::protocol::{
    ReaderRequest, ReaderResponse, SearchRequest, SearchResponse, SCHEMA_VERSION,
};

fn python3_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples")
}

fn script_runner(script_name: &str) -> Option<PluginRunner> {
    if !python3_available() {
        eprintln!("skipping: python3 not on PATH");
        return None;
    }
    let script = examples_dir().join(script_name);
    Some(PluginRunner::new(
        vec![
            "python3".to_string(),
            script.to_string_lossy().into_owned(),
        ],
        Duration::from_secs(10),
    ))
}

#[tokio::test]
async fn reader_plugin_example_round_trip() {
    let Some(runner) = script_runner("reader_plugin.py") else {
        return;
    };

    let req = ReaderRequest::new(
        "https://example.com/page",
        ReaderOptions {
            timeout_secs: Some(20),
        },
    );

    let res: ReaderResponse = runner.run(&req).await.unwrap();
    assert_eq!(res.schema_version, SCHEMA_VERSION);
    assert!(res.ok);
    assert_eq!(res.title.as_deref(), Some("Example Plugin Page"));
    assert_eq!(res.url.as_deref(), Some("https://example.com/page"));
    let markdown = res.markdown.expect("markdown present");
    assert!(markdown.contains("# Example"));
    assert!(markdown.contains("Fetched: https://example.com/page"));
}

fn run_selftest(script_name: &str) {
    if !python3_available() {
        eprintln!("skipping: python3 not on PATH");
        return;
    }
    let script = examples_dir().join(script_name);
    let status = Command::new("python3")
        .arg(&script)
        .arg("--selftest")
        .status()
        .expect("spawn python3");
    assert!(status.success(), "{script_name} selftest failed");
}

#[tokio::test]
async fn wikipedia_plugin_selftest_passes() {
    run_selftest("wikipedia_plugin.py");
}

#[tokio::test]
async fn wikipedia_search_plugin_selftest_passes() {
    run_selftest("wikipedia_search_plugin.py");
}

#[tokio::test]
async fn search_plugin_example_round_trip() {
    let Some(runner) = script_runner("search_plugin.py") else {
        return;
    };

    let req = SearchRequest::new("rust async", Some(3));
    let res: SearchResponse = runner.run(&req).await.unwrap();

    assert_eq!(res.schema_version, SCHEMA_VERSION);
    assert!(res.ok);
    assert_eq!(res.results.len(), 3);
    assert!(res.results[0].title.contains("rust async"));
    assert_eq!(res.results[0].url, "https://example.com/1");
    assert_eq!(res.results[2].url, "https://example.com/3");
    assert!(res.results[0]
        .snippet
        .as_deref()
        .unwrap()
        .contains("rust async"));
}
