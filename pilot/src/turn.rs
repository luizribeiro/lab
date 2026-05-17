//! Per-turn stream and accumulated record.

use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures_core::Stream;
use tokio::sync::mpsc;

use crate::driver::Driver;
use crate::process::ProcessHandle;
use crate::{Error, Event, Result};

/// A completed turn: the accumulated normalized events the driver emitted
/// before the underlying CLI exited.
#[derive(Debug, Clone)]
pub struct Turn {
    pub events: Vec<Event>,
}

/// Item yielded by [`TurnStream`]. Either a streamed [`Event`], or the
/// terminal [`Turn`] containing the full accumulated event list.
#[derive(Debug)]
#[non_exhaustive]
pub enum TurnItem {
    Event(Event),
    Complete(Turn),
}

/// Stream of [`TurnItem`]s for a single turn. Yields each [`Event`] in
/// order, then yields exactly one [`TurnItem::Complete`] when the child
/// exits, then `None`.
///
/// Dropping the stream aborts the underlying child process.
pub struct TurnStream {
    // Held solely so its Drop aborts the reader task, which in turn drops
    // the owned `Child` (spawned with kill_on_drop) and SIGKILLs the CLI.
    // Cleared on terminal paths to kill the child promptly rather than
    // waiting for the outer stream to be dropped.
    #[allow(dead_code)]
    handle: Option<ProcessHandle>,
    rx: mpsc::Receiver<Result<serde_json::Value>>,
    driver: Arc<dyn Driver>,
    events: Vec<Event>,
    pending: VecDeque<Event>,
    finished: bool,
    completed: bool,
}

impl TurnStream {
    #[allow(dead_code)] // wired into Session in a later commit
    pub(crate) fn new(
        handle: ProcessHandle,
        rx: mpsc::Receiver<Result<serde_json::Value>>,
        driver: Arc<dyn Driver>,
    ) -> Self {
        Self {
            handle: Some(handle),
            rx,
            driver,
            events: Vec::new(),
            pending: VecDeque::new(),
            finished: false,
            completed: false,
        }
    }
}

impl Stream for TurnStream {
    type Item = Result<TurnItem>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.finished {
            return Poll::Ready(None);
        }

        loop {
            if let Some(e) = self.pending.pop_front() {
                self.events.push(e.clone());
                return Poll::Ready(Some(Ok(TurnItem::Event(e))));
            }

            match self.rx.poll_recv(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(Ok(value))) => match self.driver.parse(value.clone()) {
                    Ok(events) => self.pending.extend(events),
                    Err(reason) => {
                        self.finished = true;
                        self.handle = None;
                        return Poll::Ready(Some(Err(Error::DriverParse {
                            driver: self.driver.name(),
                            value,
                            reason: reason.to_string(),
                        })));
                    }
                },
                Poll::Ready(Some(Err(err))) => {
                    self.finished = true;
                    self.handle = None;
                    return Poll::Ready(Some(Err(err)));
                }
                Poll::Ready(None) => {
                    if self.completed {
                        self.finished = true;
                        return Poll::Ready(None);
                    }
                    self.completed = true;
                    self.handle = None;
                    let events = self.events.clone();
                    return Poll::Ready(Some(Ok(TurnItem::Complete(Turn { events }))));
                }
            }
        }
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::*;
    use crate::driver::CommandSpec;
    use crate::process::spawn_jsonl;
    use crate::test_support::TestDriver;
    use futures_util::StreamExt;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{Duration, Instant};
    use tempfile::NamedTempFile;

    fn fake_agent() -> PathBuf {
        let mut p = std::env::current_exe().unwrap();
        p.pop();
        if p.ends_with("deps") {
            p.pop();
        }
        p.push(format!("fake_agent{}", std::env::consts::EXE_SUFFIX));
        p
    }

    fn spec(script_path: &std::path::Path) -> CommandSpec {
        CommandSpec {
            program: fake_agent(),
            args: vec!["--script".into(), script_path.to_string_lossy().into()],
            env: vec![],
        }
    }

    #[tokio::test]
    async fn stream_yields_events_then_complete() {
        let mut script = NamedTempFile::new().unwrap();
        writeln!(script, r#"emit {{"n":1}}"#).unwrap();
        writeln!(script, r#"emit {{"n":2}}"#).unwrap();
        script.flush().unwrap();

        let (handle, rx) = spawn_jsonl(spec(script.path()), std::env::temp_dir())
            .await
            .expect("spawn");
        let driver: Arc<dyn Driver> = Arc::new(TestDriver::new("t", fake_agent()));
        let mut stream = TurnStream::new(handle, rx, driver);

        let mut event_count = 0;
        let mut saw_complete = false;
        while let Some(item) = stream.next().await {
            match item.expect("ok") {
                TurnItem::Event(_) => event_count += 1,
                TurnItem::Complete(turn) => {
                    assert_eq!(turn.events.len(), 2);
                    saw_complete = true;
                }
            }
        }
        assert_eq!(event_count, 2);
        assert!(saw_complete);
    }

    #[tokio::test]
    async fn drop_kills_child_no_hang() {
        let mut script = NamedTempFile::new().unwrap();
        writeln!(script, r#"emit {{"n":1}}"#).unwrap();
        writeln!(script, "sleep 30000").unwrap();
        writeln!(script, r#"emit {{"n":2}}"#).unwrap();
        script.flush().unwrap();

        let (handle, rx) = spawn_jsonl(spec(script.path()), std::env::temp_dir())
            .await
            .expect("spawn");
        let driver: Arc<dyn Driver> = Arc::new(TestDriver::new("t", fake_agent()));
        let mut stream = TurnStream::new(handle, rx, driver);

        let first = stream.next().await.expect("first item").expect("ok");
        assert!(matches!(first, TurnItem::Event(_)));

        let start = Instant::now();
        drop(stream);

        tokio::time::sleep(Duration::from_millis(250)).await;
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "drop did not return promptly (would have meant child still blocking)"
        );
    }

    #[tokio::test]
    async fn driver_parse_error_kills_child_promptly() {
        struct AlwaysErrParse;
        impl crate::driver::Driver for AlwaysErrParse {
            fn name(&self) -> &'static str {
                "err"
            }
            fn command(
                &self,
                _: uuid::Uuid,
                _: &str,
                _: &crate::driver::TurnOptions,
            ) -> crate::driver::CommandSpec {
                unreachable!("not invoked in this test")
            }
            fn parse(
                &self,
                _value: serde_json::Value,
            ) -> std::result::Result<Vec<Event>, crate::ParseError> {
                Err(crate::ParseError::Custom("forced".into()))
            }
        }

        let mut script = NamedTempFile::new().unwrap();
        writeln!(script, r#"emit {{"first":true}}"#).unwrap();
        writeln!(script, "sleep 30000").unwrap();
        script.flush().unwrap();

        let (handle, rx) = spawn_jsonl(spec(script.path()), std::env::temp_dir())
            .await
            .expect("spawn");
        let driver: Arc<dyn Driver> = Arc::new(AlwaysErrParse);
        let mut stream = TurnStream::new(handle, rx, driver);

        let item = stream.next().await.expect("first").expect_err("err");
        assert!(matches!(item, crate::Error::DriverParse { .. }));
        assert!(
            stream.handle.is_none(),
            "handle must be dropped on terminal parse error"
        );

        assert!(stream.next().await.is_none());

        let start = Instant::now();
        drop(stream);
        assert!(start.elapsed() < Duration::from_millis(500));
    }
}
