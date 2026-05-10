use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use fittings_client::Client;
use fittings_core::{
    context::ServiceContext,
    error::FittingsError,
    message::{JsonRpcId, Request, Response},
    service::Service,
    transport::Connector,
};
use fittings_transport::stdio::StdioTransport;
use serde_json::{json, Map, Value};
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{Mutex, Notify};

type FixtureTransport = StdioTransport<OwnedReadHalf, OwnedWriteHalf>;

struct OneShotConnector {
    transport: Arc<Mutex<Option<FixtureTransport>>>,
}

impl OneShotConnector {
    fn new(transport: FixtureTransport) -> Self {
        Self {
            transport: Arc::new(Mutex::new(Some(transport))),
        }
    }
}

#[async_trait]
impl Connector for OneShotConnector {
    type Connection = FixtureTransport;

    async fn connect(&self) -> Result<Self::Connection, FittingsError> {
        Ok(self
            .transport
            .lock()
            .await
            .take()
            .expect("OneShotConnector::connect called twice"))
    }
}

struct RespondPeerCallService;

#[async_trait]
impl Service for RespondPeerCallService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        let id = req.id.unwrap_or(JsonRpcId::Null);
        let result = match req.method.as_str() {
            "core.fixture.start" => Value::Null,
            "core.fixture.echo" => req.params,
            "core.fixture.dump_env" => dump_env(),
            "core.fixture.write_private_state" => write_private_state()?,
            "core.fixture.report_open_result" => report_open_result(),
            "core.fixture.try_write_path" => try_write_path(),
            other => return Err(FittingsError::method_not_found(other)),
        };
        Ok(Response {
            id,
            result,
            metadata: Default::default(),
        })
    }
}

struct StartOnlyService {
    started: Arc<Notify>,
}

#[async_trait]
impl Service for StartOnlyService {
    async fn call(&self, req: Request, _ctx: ServiceContext) -> Result<Response, FittingsError> {
        let id = req.id.unwrap_or(JsonRpcId::Null);
        match req.method.as_str() {
            "core.fixture.start" => {
                self.started.notify_one();
                Ok(Response {
                    id,
                    result: Value::Null,
                    metadata: Default::default(),
                })
            }
            other => Err(FittingsError::method_not_found(other)),
        }
    }
}

fn dump_env() -> Value {
    let keys = std::env::var("RFL_FIXTURE_ENV_KEYS").unwrap_or_default();
    let mut env = Map::new();
    for key in keys.split(',').filter(|s| !s.is_empty()) {
        if let Ok(value) = std::env::var(key) {
            env.insert(key.to_string(), Value::String(value));
        }
    }
    json!({ "env": Value::Object(env) })
}

fn write_private_state() -> Result<Value, FittingsError> {
    let dir = std::env::var("RFL_PRIVATE_STATE_DIR")
        .map_err(|_| FittingsError::internal("RFL_PRIVATE_STATE_DIR not set"))?;
    let path = std::path::Path::new(&dir).join("marker");
    let marker = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let body = json!({ "marker": marker.to_string() }).to_string();
    std::fs::write(&path, body)
        .map_err(|e| FittingsError::internal(format!("write_private_state: {}", e)))?;
    Ok(json!({ "wrote": path.display().to_string() }))
}

fn report_open_result() -> Value {
    let path = std::env::var("RFL_FIXTURE_OPEN_PATH").unwrap_or_default();
    match std::fs::read(&path) {
        Ok(_) => json!({ "ok": true }),
        Err(e) => json!({ "ok": false, "errno": e.raw_os_error().unwrap_or(0) }),
    }
}

fn try_write_path() -> Value {
    let path = std::env::var("RFL_FIXTURE_WRITE_PATH").unwrap_or_default();
    match std::fs::write(&path, b"x") {
        Ok(_) => json!({ "ok": true }),
        Err(e) => json!({ "ok": false, "errno": e.raw_os_error().unwrap_or(0) }),
    }
}

const REAL_BUS_MODES: &[&str] = &[
    "respond_peer_call",
    "publish_one",
    "publish_with_taint",
    "publish_full_params",
    "publish_bad_namespace",
    "publish_bad_grammar",
    "publish_outside_grant",
    "publish_bad_in_reply_to_missing",
    "publish_bad_in_reply_to_empty",
    "publish_bad_in_reply_to_multiple",
    "call_core_then_exit",
    "observer",
    "signal_ready",
    "hold_silent",
    "signal_ready_then_exit_n",
    "frontend_bus_publish",
];

fn main() {
    let mode = std::env::var("RFL_FIXTURE_MODE").unwrap_or_default();
    if mode == "scaffold_only" {
        return;
    }
    if mode == "exit_immediately" {
        std::process::exit(0);
    }
    if mode == "probe_fd_closed" {
        run_probe_fd_closed();
    }
    if !REAL_BUS_MODES.contains(&mode.as_str()) {
        eprintln!("rfl-bus-fixture: unknown mode '{}'", mode);
        std::process::exit(64);
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_io()
        .enable_time()
        .build()
        .expect("build tokio runtime");
    runtime.block_on(run_bus_backed(&mode));
}

async fn run_bus_backed(mode: &str) {
    install_max_lifetime();
    maybe_install_sigterm_trap();
    let transport = build_bus_transport();
    let client = Client::connect(OneShotConnector::new(transport))
        .await
        .expect("client connect");

    match mode {
        "respond_peer_call" => {
            let client = client.with_service(RespondPeerCallService);
            ack_ready(&client, mode).await;
            std::future::pending::<()>().await;
        }
        "signal_ready" => {
            client
                .peer()
                .call("frontend.ready", json!({}))
                .await
                .expect("frontend.ready ack");
            std::future::pending::<()>().await;
        }
        "hold_silent" => {
            std::future::pending::<()>().await;
        }
        "signal_ready_then_exit_n" => {
            client
                .peer()
                .call("frontend.ready", json!({}))
                .await
                .expect("frontend.ready ack");
            tokio::time::sleep(Duration::from_millis(200)).await;
            let n: i32 = std::env::var("RFL_FIXTURE_EXIT_CODE")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(7);
            std::process::exit(n);
        }
        "frontend_bus_publish" => {
            client
                .peer()
                .call("frontend.ready", json!({}))
                .await
                .expect("frontend.ready ack");
            let topic = std::env::var("RFL_FIXTURE_PUBLISH_TOPIC")
                .expect("RFL_FIXTURE_PUBLISH_TOPIC not set");
            client
                .notify("bus.publish", json!({"topic": topic, "payload": {}}))
                .await
                .expect("bus.publish notify");
            std::future::pending::<()>().await;
        }
        "observer" => {
            let peer = client.peer();
            let started = Arc::new(Notify::new());
            let client = client
                .with_service(StartOnlyService {
                    started: started.clone(),
                })
                .with_notification_handler(move |method, params| {
                    if method == "bus.event" {
                        let peer = peer.clone();
                        tokio::spawn(async move {
                            let _ = peer.call("core.fixture.observed", params).await;
                        });
                    }
                });
            ack_ready(&client, mode).await;
            std::future::pending::<()>().await;
        }
        "call_core_then_exit" => {
            let started = Arc::new(Notify::new());
            let client = client.with_service(StartOnlyService {
                started: started.clone(),
            });
            ack_ready(&client, mode).await;
            started.notified().await;
            match client
                .peer()
                .call("core.fixture.ping", json!({"n": 42}))
                .await
            {
                Ok(_) => std::process::exit(0),
                Err(_) => std::process::exit(2),
            }
        }
        m if m.starts_with("publish_") => {
            let started = Arc::new(Notify::new());
            let client = client.with_service(StartOnlyService {
                started: started.clone(),
            });
            ack_ready(&client, mode).await;
            started.notified().await;
            let params = build_publish_params(mode);
            client
                .notify("bus.publish", params)
                .await
                .expect("bus.publish notify");
            client
                .call("core.fixture.after_publish", json!({}))
                .await
                .expect("after_publish ack");
            std::process::exit(0);
        }
        _ => unreachable!("mode dispatch already validated"),
    }
}

fn install_max_lifetime() {
    let secs: u64 = std::env::var("RFL_FIXTURE_MAX_LIFETIME")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(secs)).await;
        std::process::exit(0);
    });
}

fn run_probe_fd_closed() -> ! {
    let mut args = std::env::args().skip(1);
    let mut probe_fd: Option<RawFd> = None;
    while let Some(a) = args.next() {
        if a == "--probe-fd" {
            probe_fd = args.next().and_then(|s| s.parse().ok());
        }
    }
    let fd = probe_fd.expect("--probe-fd <N> required");
    match nix::fcntl::fcntl(fd, nix::fcntl::FcntlArg::F_GETFD) {
        Err(nix::errno::Errno::EBADF) => std::process::exit(0),
        _ => std::process::exit(1),
    }
}

fn maybe_install_sigterm_trap() {
    if std::env::var("RFL_FIXTURE_TRAP_SIGTERM").as_deref() != Ok("1") {
        return;
    }
    let mut stream = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .expect("install SIGTERM handler");
    tokio::spawn(async move {
        loop {
            if stream.recv().await.is_none() {
                return;
            }
        }
    });
}

fn build_bus_transport() -> FixtureTransport {
    let fd_str = match std::env::var("RFL_BUS_FD") {
        Ok(v) => v,
        Err(_) => {
            eprintln!("rfl-bus-fixture: RFL_BUS_FD not set");
            std::process::exit(3);
        }
    };
    let fd: RawFd = match fd_str.parse() {
        Ok(n) if n >= 0 => n,
        _ => {
            eprintln!("rfl-bus-fixture: invalid RFL_BUS_FD '{}'", fd_str);
            std::process::exit(3);
        }
    };

    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    let std_stream = std::os::unix::net::UnixStream::from(owned);
    if let Err(err) = std_stream.set_nonblocking(true) {
        eprintln!("rfl-bus-fixture: set_nonblocking failed: {}", err);
        std::process::exit(3);
    }
    let stream = match tokio::net::UnixStream::from_std(std_stream) {
        Ok(s) => s,
        Err(err) => {
            eprintln!(
                "rfl-bus-fixture: tokio UnixStream conversion failed: {}",
                err
            );
            std::process::exit(3);
        }
    };
    let (reader, writer) = stream.into_split();
    StdioTransport::new(reader, writer, 1 << 20)
}

async fn ack_ready<C>(client: &Client<C>, mode: &str)
where
    C: Connector + Send + Sync + 'static,
{
    client
        .call("core.fixture.ready", json!({ "mode": mode }))
        .await
        .expect("ready ack");
}

fn build_publish_params(mode: &str) -> Value {
    if mode == "publish_full_params" {
        let raw = std::env::var("RFL_FIXTURE_FULL_PARAMS_JSON")
            .expect("RFL_FIXTURE_FULL_PARAMS_JSON not set");
        return serde_json::from_str(&raw).expect("invalid RFL_FIXTURE_FULL_PARAMS_JSON");
    }

    let topic = resolve_publish_topic(mode);
    let payload: Value = std::env::var("RFL_FIXTURE_PAYLOAD_JSON")
        .ok()
        .map(|s| serde_json::from_str(&s).expect("invalid RFL_FIXTURE_PAYLOAD_JSON"))
        .unwrap_or_else(|| json!({}));

    let mut params = Map::new();
    params.insert("topic".to_string(), Value::String(topic));
    params.insert("payload".to_string(), payload);

    if let Ok(raw) = std::env::var("RFL_FIXTURE_TAINT_JSON") {
        let taint: Value = serde_json::from_str(&raw).expect("invalid RFL_FIXTURE_TAINT_JSON");
        params.insert("taint".to_string(), taint);
    }

    match mode {
        "publish_bad_in_reply_to_empty" => {
            params.insert("in_reply_to".to_string(), json!([]));
        }
        "publish_bad_in_reply_to_multiple" => {
            params.insert("in_reply_to".to_string(), json!(["a", "b"]));
        }
        _ => {}
    }

    Value::Object(params)
}

fn resolve_publish_topic(mode: &str) -> String {
    let topic_id = std::env::var("RFL_TOPIC_ID").unwrap_or_default();
    match mode {
        "publish_bad_namespace" => std::env::var("RFL_FIXTURE_TOPIC")
            .unwrap_or_else(|_| "core.session.user_message".to_string()),
        "publish_bad_grammar" => format!("plugin.{}.UPPERCASE", topic_id),
        "publish_outside_grant" => format!("plugin.{}.ungranted", topic_id),
        "publish_bad_in_reply_to_missing"
        | "publish_bad_in_reply_to_empty"
        | "publish_bad_in_reply_to_multiple" => format!("plugin.{}.tool_result", topic_id),
        _ => std::env::var("RFL_FIXTURE_TOPIC").expect("RFL_FIXTURE_TOPIC not set"),
    }
}
