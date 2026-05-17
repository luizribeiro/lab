//! Per-turn stream and accumulated record.

use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::task::{Context, Poll};
use std::time::Duration;

use futures_core::Stream;
use tokio::sync::mpsc;
use tokio::time::{Instant, Sleep};
use uuid::Uuid;

use crate::driver::Driver;
use crate::process::ProcessHandle;
use crate::{Error, Event, Result};

/// RAII guard cleared on drop. Used by `Session::send` to enforce that a
/// session has at most one turn in flight at a time.
pub(crate) struct BusyGuard {
    pub flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.flag.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Cross-session lock for `(driver_name, session_uuid)`. Held by a
/// `TurnStream` for the duration of a turn so two `Session` instances
/// with the same identity cannot run concurrent turns.
pub(crate) struct SessionGuard {
    pub _owned_lock: tokio::sync::OwnedMutexGuard<()>,
}

impl std::fmt::Debug for SessionGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SessionGuard")
    }
}

/// A completed (or cancelled) turn's accumulated events.
///
/// Constructed by [`TurnStream`] and yielded inside [`TurnItem::Complete`]
/// at successful turn end, or returned by [`TurnStream::cancel`].
/// Use [`Turn::final_text`] for the canonical assistant response.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Turn {
    pub events: Vec<Event>,
}

impl Turn {
    /// The canonical final text of this turn — the concatenation of all
    /// `Event::AssistantText` deltas in the order they were observed.
    ///
    /// Drivers without streamed deltas (e.g. claude's error-result path)
    /// emit a synthetic `AssistantText` so this method still returns
    /// usable text. Empty string when the turn produced no text at all
    /// (e.g. a turn that errored before any model output).
    pub fn final_text(&self) -> String {
        self.events
            .iter()
            .filter_map(|e| match e {
                Event::AssistantText { delta } => Some(delta.as_str()),
                _ => None,
            })
            .collect()
    }
}

/// Item yielded by [`TurnStream`]. Either a streamed [`Event`], or the
/// terminal [`Turn`] containing the full accumulated event list.
#[derive(Debug)]
#[non_exhaustive]
pub enum TurnItem {
    Event(Event),
    Complete(Turn),
}

/// Stream of [`TurnItem`]s for a single in-flight turn.
///
/// Yields each [`Event`] in order as the child agent CLI emits it, then
/// exactly one [`TurnItem::Complete`] when the turn ends successfully,
/// then `None`. Errors during the turn are yielded as `Some(Err(_))`
/// items and terminate the stream.
///
/// # Lifecycle guarantees
///
/// * **Drop kills the child.** When a `TurnStream` is dropped — including
///   panic unwind — the underlying child process is terminated via
///   tokio's `kill_on_drop(true)`. There is no zombie process risk.
/// * **Drop releases the busy guard.** The owning [`crate::Session`]
///   becomes available for another [`crate::Session::send`] immediately.
///   Any cross-`Session` lock keyed by `(driver_name, Session::id)` is
///   also released.
/// * **Drop does NOT increment the turns-completed counter.** A
///   `TurnStream` that's dropped before yielding [`TurnItem::Complete`]
///   leaves the owning session's dispatch state unchanged. For a
///   session created via [`crate::Session::new`] whose first turn never
///   completed, that means the next `send` again uses
///   [`crate::Driver::command`]. For a session created via
///   [`crate::Session::resume`] (counter starts at 1), the next `send`
///   continues to use [`crate::Driver::resume_command`].
/// * **[`TurnStream::cancel`] is a controlled drop.** Same as drop, but
///   additionally drains any events already buffered in the channel
///   and returns them inside the partial [`Turn`].
/// * **Stream errors are terminal.** Once `poll_next` yields
///   `Some(Err(_))`, subsequent polls return `None`. The handle has
///   already been released; the stream is safe to keep around or drop.
/// * **[`TurnStream::with_timeout`]-triggered errors are equivalent to
///   errors:** yields `Some(Err(Error::Timeout(_)))`, then `None`.
///   Counter not incremented; session is reusable.
/// * **Successful completion increments the counter.** When `poll_next`
///   yields [`TurnItem::Complete`], the session's turns-completed
///   counter increments BEFORE the busy guard releases. Subsequent
///   [`crate::Session::send`] correctly dispatches via
///   [`crate::Driver::resume_command`].
pub struct TurnStream {
    // Held solely so its Drop aborts the reader task, which in turn drops
    // the owned `Child` (spawned with kill_on_drop) and SIGKILLs the CLI.
    // Cleared on terminal paths to kill the child promptly rather than
    // waiting for the outer stream to be dropped.
    #[allow(dead_code)]
    handle: Option<ProcessHandle>,
    session_id: Uuid,
    rx: mpsc::Receiver<Result<serde_json::Value>>,
    driver: Arc<dyn Driver>,
    events: Vec<Event>,
    pending: VecDeque<Event>,
    finished: bool,
    completed: bool,
    deadline: Option<Instant>,
    timer: Option<Pin<Box<Sleep>>>,
    timeout_dur: Option<Duration>,
    _busy_guard: Option<BusyGuard>,
    _session_guard: Option<SessionGuard>,
    completion_counter: Option<Arc<AtomicUsize>>,
}

impl std::fmt::Debug for TurnStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TurnStream")
            .field("session_id", &self.session_id)
            .field("finished", &self.finished)
            .finish()
    }
}

impl TurnStream {
    #[allow(dead_code)] // wired into Session in a later commit
    pub(crate) fn new(
        session_id: Uuid,
        handle: ProcessHandle,
        rx: mpsc::Receiver<Result<serde_json::Value>>,
        driver: Arc<dyn Driver>,
    ) -> Self {
        Self {
            session_id,
            handle: Some(handle),
            rx,
            driver,
            events: Vec::new(),
            pending: VecDeque::new(),
            finished: false,
            completed: false,
            deadline: None,
            timer: None,
            timeout_dur: None,
            _busy_guard: None,
            _session_guard: None,
            completion_counter: None,
        }
    }

    /// Attach a counter incremented each time the stream yields
    /// [`TurnItem::Complete`]. Used by `Session` to dispatch the next turn to
    /// `command()` vs `resume_command()` based on observed completions.
    #[allow(dead_code)]
    pub(crate) fn with_completion_counter(mut self, counter: Arc<AtomicUsize>) -> Self {
        self.completion_counter = Some(counter);
        self
    }

    /// Attach a busy guard whose `Drop` releases the owning session's
    /// in-flight flag. Called by `Session::send`.
    #[allow(dead_code)]
    pub(crate) fn with_busy_guard(mut self, guard: BusyGuard) -> Self {
        self._busy_guard = Some(guard);
        self
    }

    /// Attach a cross-session guard whose `Drop` releases the
    /// `(driver, uuid)` lock. Called by `Session::send`.
    #[allow(dead_code)]
    pub(crate) fn with_session_guard(mut self, guard: SessionGuard) -> Self {
        self._session_guard = Some(guard);
        self
    }

    /// Set a per-turn wall-clock deadline.
    ///
    /// When the duration elapses before the stream yields
    /// [`TurnItem::Complete`], the stream yields exactly one
    /// `Err(Error::Timeout(duration))` and then `None`. The child process
    /// is killed, the busy guard is released, and the session is
    /// immediately reusable. The turns-completed counter is NOT
    /// incremented; the next send dispatches via whichever of
    /// [`crate::Driver::command`] / [`crate::Driver::resume_command`]
    /// matches the current counter state (see [`TurnStream`] docs).
    ///
    /// Timeouts do NOT fire after [`TurnItem::Complete`] has been yielded —
    /// once the stream is finished, subsequent polls return `None`
    /// regardless of how much wall time has elapsed.
    #[allow(dead_code)] // wired into Session in a later commit
    pub fn with_timeout(mut self, duration: Duration) -> Self {
        self.deadline = Some(Instant::now() + duration);
        self.timeout_dur = Some(duration);
        self
    }

    /// Cancel the running turn.
    ///
    /// Kills the child process and returns a [`Turn`] holding whatever
    /// events were accumulated before cancellation, including any that
    /// arrived in the channel but had not yet been polled.
    ///
    /// Semantics (consumed-self):
    ///
    /// * Returns synchronously after a small drain — no waiting on the
    ///   child to actually exit (`kill_on_drop` handles that asynchronously).
    /// * The returned [`Turn`] may have ZERO [`Event::AssistantText`](crate::Event)
    ///   events if the agent had not emitted any yet.
    /// * The session this stream came from is immediately available for
    ///   another [`crate::Session::send`].
    /// * The turns-completed counter is NOT incremented; the next send
    ///   dispatches via whichever of [`crate::Driver::command`] /
    ///   [`crate::Driver::resume_command`] matches the current counter
    ///   state (see [`TurnStream`] docs).
    /// * Equivalent to dropping the stream, plus the channel drain and
    ///   the returned [`Turn`] value.
    #[allow(dead_code)] // wired into Session in a later commit
    pub async fn cancel(mut self) -> Turn {
        self.handle = None;
        self._busy_guard = None;
        self._session_guard = None;
        for e in self.pending.drain(..) {
            self.events.push(e);
        }
        while let Ok(item) = self.rx.try_recv() {
            if let Ok(value) = item {
                if let Ok(events) = self.driver.parse(value) {
                    self.events.extend(events);
                }
            }
        }
        Turn {
            events: self.events,
        }
    }
}

impl Stream for TurnStream {
    type Item = Result<TurnItem>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        if this.finished {
            return Poll::Ready(None);
        }

        loop {
            if let Some(e) = this.pending.pop_front() {
                this.events.push(e.clone());
                return Poll::Ready(Some(Ok(TurnItem::Event(e))));
            }

            if let Some(deadline) = this.deadline {
                if this.timer.is_none() {
                    this.timer = Some(Box::pin(tokio::time::sleep_until(deadline)));
                }
            }

            if let Some(timer) = this.timer.as_mut() {
                if timer.as_mut().poll(cx).is_ready() {
                    this.handle = None;
                    this.finished = true;
                    this._busy_guard = None;
                    this._session_guard = None;
                    let d = this.timeout_dur.unwrap_or_default();
                    return Poll::Ready(Some(Err(Error::Timeout(d))));
                }
            }

            match this.rx.poll_recv(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Some(Ok(value))) => {
                    this.driver.observe(this.session_id, &value);
                    match this.driver.parse(value.clone()) {
                        Ok(events) => this.pending.extend(events),
                        Err(reason) => {
                            this.handle = None;
                            this.finished = true;
                            this._busy_guard = None;
                            this._session_guard = None;
                            return Poll::Ready(Some(Err(Error::DriverParse {
                                driver: this.driver.name(),
                                value,
                                reason: reason.to_string(),
                            })));
                        }
                    }
                }
                Poll::Ready(Some(Err(err))) => {
                    this.handle = None;
                    this.finished = true;
                    this._busy_guard = None;
                    this._session_guard = None;
                    return Poll::Ready(Some(Err(err)));
                }
                Poll::Ready(None) => {
                    this.handle = None;
                    if this.completed {
                        this.finished = true;
                        this._busy_guard = None;
                        this._session_guard = None;
                        return Poll::Ready(None);
                    }
                    this.completed = true;
                    this.finished = true;
                    // Increment completion counter BEFORE releasing the busy guard so
                    // any concurrent send past the busy CAS observes the completion
                    // and dispatches resume_command, not command.
                    if let Some(counter) = &this.completion_counter {
                        counter.fetch_add(1, Ordering::SeqCst);
                    }
                    this._busy_guard = None;
                    this._session_guard = None;
                    let events = this.events.clone();
                    return Poll::Ready(Some(Ok(TurnItem::Complete(Turn { events }))));
                }
            }
        }
    }
}

#[cfg(test)]
mod final_text_tests {
    use super::*;

    #[test]
    fn final_text_accumulates_assistant_text_deltas() {
        let turn = Turn {
            events: vec![
                Event::AssistantText {
                    delta: "hello ".into(),
                },
                Event::Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                },
                Event::AssistantText {
                    delta: "world".into(),
                },
                Event::TurnComplete { ok: true },
            ],
        };
        assert_eq!(turn.final_text(), "hello world");
    }

    #[test]
    fn final_text_empty_when_no_assistant_deltas() {
        let turn = Turn {
            events: vec![Event::TurnComplete { ok: false }],
        };
        assert_eq!(turn.final_text(), "");
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
        let mut stream = TurnStream::new(Uuid::nil(), handle, rx, driver);

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
        let mut stream = TurnStream::new(Uuid::nil(), handle, rx, driver);

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
                _: &crate::driver::TurnInput,
                _: &crate::driver::TurnOptions,
            ) -> crate::Result<crate::driver::CommandSpec> {
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
        let mut stream = TurnStream::new(Uuid::nil(), handle, rx, driver);

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

    #[tokio::test]
    async fn cancel_before_any_events_returns_empty_turn() {
        let mut script = NamedTempFile::new().unwrap();
        writeln!(script, "sleep 30000").unwrap();
        writeln!(script, r#"emit {{"n":1}}"#).unwrap();
        script.flush().unwrap();

        let (handle, rx) = spawn_jsonl(spec(script.path()), std::env::temp_dir())
            .await
            .expect("spawn");
        let driver: Arc<dyn Driver> = Arc::new(TestDriver::new("t", fake_agent()));
        let stream = TurnStream::new(Uuid::nil(), handle, rx, driver);

        let start = std::time::Instant::now();
        let turn = stream.cancel().await;
        assert!(turn.events.is_empty());
        assert!(start.elapsed() < std::time::Duration::from_secs(2));
    }

    #[tokio::test]
    async fn cancel_after_some_events_returns_partial_turn() {
        let mut script = NamedTempFile::new().unwrap();
        writeln!(script, r#"emit {{"n":1}}"#).unwrap();
        writeln!(script, "sleep 30000").unwrap();
        writeln!(script, r#"emit {{"n":2}}"#).unwrap();
        script.flush().unwrap();

        let (handle, rx) = spawn_jsonl(spec(script.path()), std::env::temp_dir())
            .await
            .expect("spawn");
        let driver: Arc<dyn Driver> = Arc::new(TestDriver::new("t", fake_agent()));
        let mut stream = TurnStream::new(Uuid::nil(), handle, rx, driver);

        let first = stream.next().await.expect("first").expect("ok");
        assert!(matches!(first, TurnItem::Event(_)));

        let turn = stream.cancel().await;
        assert_eq!(turn.events.len(), 1);
    }

    #[tokio::test]
    async fn cancel_after_natural_completion_returns_full_turn() {
        let mut script = NamedTempFile::new().unwrap();
        writeln!(script, r#"emit {{"n":1}}"#).unwrap();
        writeln!(script, r#"emit {{"n":2}}"#).unwrap();
        writeln!(script, "exit 0").unwrap();
        script.flush().unwrap();

        let (handle, rx) = spawn_jsonl(spec(script.path()), std::env::temp_dir())
            .await
            .expect("spawn");
        let driver: Arc<dyn Driver> = Arc::new(TestDriver::new("t", fake_agent()));
        let mut stream = TurnStream::new(Uuid::nil(), handle, rx, driver);

        let mut event_count = 0;
        while let Some(item) = stream.next().await {
            match item.expect("ok") {
                TurnItem::Event(_) => event_count += 1,
                TurnItem::Complete(_) => {}
            }
        }
        assert_eq!(event_count, 2);

        let turn = stream.cancel().await;
        assert_eq!(turn.events.len(), 2);
    }

    #[tokio::test]
    async fn cancel_includes_queued_but_unpolled_events() {
        let mut script = NamedTempFile::new().unwrap();
        writeln!(script, r#"emit {{"n":1}}"#).unwrap();
        writeln!(script, r#"emit {{"n":2}}"#).unwrap();
        writeln!(script, r#"emit {{"n":3}}"#).unwrap();
        writeln!(script, "exit 0").unwrap();
        script.flush().unwrap();

        let (handle, rx) = spawn_jsonl(spec(script.path()), std::env::temp_dir())
            .await
            .expect("spawn");
        let driver: Arc<dyn Driver> = Arc::new(TestDriver::new("t", fake_agent()));
        let stream = TurnStream::new(Uuid::nil(), handle, rx, driver);

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while stream.rx.len() < 3 {
            if std::time::Instant::now() > deadline {
                panic!(
                    "timed out waiting for 3 events to queue; got {}",
                    stream.rx.len()
                );
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }

        let turn = stream.cancel().await;

        assert_eq!(
            turn.events.len(),
            3,
            "cancel must include channel-buffered events; got {:?}",
            turn.events.len()
        );
    }

    #[tokio::test]
    async fn timeout_fires_when_child_blocks() {
        let mut script = NamedTempFile::new().unwrap();
        writeln!(script, "sleep 30000").unwrap();
        script.flush().unwrap();

        let (handle, rx) = spawn_jsonl(spec(script.path()), std::env::temp_dir())
            .await
            .expect("spawn");
        let driver: Arc<dyn Driver> = Arc::new(TestDriver::new("t", fake_agent()));
        let mut stream = TurnStream::new(Uuid::nil(), handle, rx, driver)
            .with_timeout(std::time::Duration::from_millis(150));

        let start = std::time::Instant::now();
        let item = stream.next().await.expect("first").expect_err("timeout");
        assert!(matches!(item, Error::Timeout(_)));
        assert!(start.elapsed() < std::time::Duration::from_secs(2));

        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    async fn timeout_does_not_fire_when_completion_is_fast() {
        let mut script = NamedTempFile::new().unwrap();
        writeln!(script, r#"emit {{"n":1}}"#).unwrap();
        writeln!(script, "exit 0").unwrap();
        script.flush().unwrap();

        let (handle, rx) = spawn_jsonl(spec(script.path()), std::env::temp_dir())
            .await
            .expect("spawn");
        let driver: Arc<dyn Driver> = Arc::new(TestDriver::new("t", fake_agent()));
        let mut stream = TurnStream::new(Uuid::nil(), handle, rx, driver)
            .with_timeout(std::time::Duration::from_secs(30));

        let mut events = 0;
        let mut completed = false;
        while let Some(item) = stream.next().await {
            match item.expect("ok") {
                TurnItem::Event(_) => events += 1,
                TurnItem::Complete(_) => completed = true,
            }
        }
        assert_eq!(events, 1);
        assert!(completed);
    }

    #[tokio::test]
    async fn slow_consumer_does_not_drop_events() {
        let mut script = NamedTempFile::new().unwrap();
        for i in 0..50 {
            writeln!(script, r#"emit {{"n":{i}}}"#).unwrap();
        }
        writeln!(script, "exit 0").unwrap();
        script.flush().unwrap();

        let (handle, rx) = spawn_jsonl(spec(script.path()), std::env::temp_dir())
            .await
            .expect("spawn");
        let driver: Arc<dyn Driver> = Arc::new(TestDriver::new("t", fake_agent()));
        let mut stream = TurnStream::new(Uuid::nil(), handle, rx, driver);

        let mut events = 0;
        while let Some(item) = stream.next().await {
            match item.expect("ok") {
                TurnItem::Event(_) => {
                    events += 1;
                    tokio::time::sleep(std::time::Duration::from_millis(2)).await;
                }
                TurnItem::Complete(_) => {}
            }
        }
        assert_eq!(events, 50);
    }

    #[tokio::test]
    async fn cancel_during_stderr_does_not_hang() {
        let mut script = NamedTempFile::new().unwrap();
        writeln!(script, "stderr noise on stderr").unwrap();
        writeln!(script, "sleep 30000").unwrap();
        script.flush().unwrap();

        let (handle, rx) = spawn_jsonl(spec(script.path()), std::env::temp_dir())
            .await
            .expect("spawn");
        let driver: Arc<dyn Driver> = Arc::new(TestDriver::new("t", fake_agent()));
        let stream = TurnStream::new(Uuid::nil(), handle, rx, driver);

        let start = std::time::Instant::now();
        let _turn = stream.cancel().await;
        assert!(
            start.elapsed() < std::time::Duration::from_secs(2),
            "cancel hung"
        );
    }

    #[tokio::test]
    async fn finished_is_set_when_yielding_complete() {
        let mut script = NamedTempFile::new().unwrap();
        writeln!(script, r#"emit {{"n":1}}"#).unwrap();
        writeln!(script, "exit 0").unwrap();
        script.flush().unwrap();

        let (handle, rx) = spawn_jsonl(spec(script.path()), std::env::temp_dir())
            .await
            .expect("spawn");
        let driver: Arc<dyn Driver> = Arc::new(TestDriver::new("t", fake_agent()));
        let mut stream = TurnStream::new(Uuid::nil(), handle, rx, driver)
            .with_timeout(std::time::Duration::from_secs(60));

        let mut saw_complete = false;
        while let Some(item) = stream.next().await {
            match item.expect("ok") {
                TurnItem::Event(_) => {}
                TurnItem::Complete(_) => {
                    saw_complete = true;
                    break;
                }
            }
        }
        assert!(saw_complete);

        assert!(
            stream.finished,
            "finished must be true immediately after yielding Complete"
        );

        assert!(stream.next().await.is_none());
    }
}
