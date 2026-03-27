use std::{
    ffi::{OsStr, OsString},
    process::Stdio,
};

use async_trait::async_trait;
use fittings_core::{
    error::FittingsError,
    transport::{Connector, Transport},
};
use fittings_transport::stdio::StdioTransport;
use serde_json::Value;
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

const DEFAULT_MAX_FRAME_BYTES: usize = 1_048_576;

#[derive(Debug, Clone)]
pub struct ProcessConnector {
    command: OsString,
    config_json: Option<Value>,
    max_frame_bytes: usize,
}

impl ProcessConnector {
    pub fn new(command: impl AsRef<OsStr>) -> Self {
        Self {
            command: command.as_ref().to_os_string(),
            config_json: None,
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
        }
    }

    pub fn with_config_json(mut self, config: Value) -> Self {
        self.config_json = Some(config);
        self
    }

    pub async fn connect(self) -> Result<ProcessTransport, FittingsError> {
        let mut command = Command::new(&self.command);
        command
            .env("FITTINGS", "1")
            .arg("serve")
            .kill_on_drop(true)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        if let Some(config) = self.config_json {
            let config_arg = serde_json::to_string(&config).map_err(|error| {
                FittingsError::internal(format!("failed to encode process config as JSON: {error}"))
            })?;
            command.arg(config_arg);
        }

        let mut child = command.spawn().map_err(|error| {
            FittingsError::transport(format!("failed to spawn process: {error}"))
        })?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| FittingsError::internal("spawned process missing piped stdout"))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| FittingsError::internal("spawned process missing piped stdin"))?;

        Ok(ProcessTransport::new(
            child,
            StdioTransport::new(stdout, stdin, self.max_frame_bytes),
        ))
    }
}

#[async_trait]
impl Connector for ProcessConnector {
    type Connection = ProcessTransport;

    async fn connect(&self) -> Result<Self::Connection, FittingsError> {
        ProcessConnector::connect(self.clone()).await
    }
}

pub struct ProcessTransport {
    child: Option<Child>,
    io: StdioTransport<ChildStdout, ChildStdin>,
}

impl ProcessTransport {
    fn new(child: Child, io: StdioTransport<ChildStdout, ChildStdin>) -> Self {
        Self {
            child: Some(child),
            io,
        }
    }
}

impl Drop for ProcessTransport {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };

        let _ = child.start_kill();

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let _ = child.wait().await;
            });
        } else {
            let _ = child.try_wait();
        }
    }
}

#[async_trait]
impl Transport for ProcessTransport {
    async fn send(&mut self, frame: &[u8]) -> Result<(), FittingsError> {
        self.io.send(frame).await.map_err(normalize_send_error)
    }

    async fn recv(&mut self) -> Result<Vec<u8>, FittingsError> {
        self.io.recv().await.map_err(normalize_recv_error)
    }
}

fn normalize_send_error(error: FittingsError) -> FittingsError {
    match error {
        FittingsError::Transport(_) => FittingsError::transport("child process stdin closed"),
        other => other,
    }
}

fn normalize_recv_error(error: FittingsError) -> FittingsError {
    match error {
        FittingsError::Transport(message) if message == "end of input" => {
            FittingsError::transport("child process stdout closed")
        }
        other => other,
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use fittings_core::error::FittingsError;
    use serde_json::json;

    use crate::Client;

    use super::ProcessConnector;

    fn unique_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("fittings-{name}-{}-{nanos}", std::process::id()))
    }

    fn write_executable_script(path: &Path, content: &str) {
        fs::write(path, content).expect("write script fixture");
        let mut perms = fs::metadata(path)
            .expect("read script metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("set executable permissions");
    }

    #[tokio::test]
    async fn process_connector_roundtrips_request_response_over_stdio() {
        let script_path = unique_path("process-echo");
        write_executable_script(
            &script_path,
            r#"#!/bin/sh
if [ "$FITTINGS" != "1" ]; then
  exit 90
fi
if [ "$1" != "serve" ]; then
  exit 91
fi
IFS= read -r _line || exit 1
printf '{"id":"1","result":{"ok":true},"error":null,"metadata":{}}\n'
"#,
        );

        let client = Client::connect(ProcessConnector::new(&script_path))
            .await
            .expect("client should connect");

        let result = client
            .call("ping", json!({}))
            .await
            .expect("call should succeed");
        assert_eq!(result, json!({"ok": true}));

        let _ = fs::remove_file(script_path);
    }

    #[tokio::test]
    async fn process_connector_maps_child_exit_to_deterministic_transport_error() {
        let script_path = unique_path("process-exit");
        write_executable_script(
            &script_path,
            r#"#!/bin/sh
if [ "$FITTINGS" != "1" ]; then
  exit 90
fi
if [ "$1" != "serve" ]; then
  exit 91
fi
exit 0
"#,
        );

        let client = Client::connect(ProcessConnector::new(&script_path))
            .await
            .expect("client should connect");

        let error = client
            .call("ping", json!({}))
            .await
            .expect_err("call should fail after child exits");

        assert!(matches!(
            error,
            FittingsError::Transport(message)
                if message == "child process stdin closed"
                    || message == "child process stdout closed"
        ));

        let _ = fs::remove_file(script_path);
    }

    #[tokio::test]
    async fn dropping_process_transport_kills_child_process() {
        let script_path = unique_path("process-lifecycle");
        let pid_file = unique_path("process-lifecycle-pid");
        let pid_file_escaped = pid_file.to_string_lossy().replace('"', "\\\"");

        write_executable_script(
            &script_path,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$$\" > \"{pid_file}\"\nwhile true; do sleep 1; done\n",
                pid_file = pid_file_escaped
            ),
        );

        let transport = ProcessConnector::new(&script_path)
            .connect()
            .await
            .expect("connector should spawn process");

        let pid = loop {
            if let Ok(text) = fs::read_to_string(&pid_file) {
                break text
                    .trim()
                    .parse::<u32>()
                    .expect("pid file should contain integer");
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        };

        drop(transport);

        let mut alive = true;
        for _ in 0..50 {
            let status = std::process::Command::new("sh")
                .arg("-c")
                .arg(format!("kill -0 {pid} 2>/dev/null"))
                .status()
                .expect("run kill -0 probe");
            alive = status.success();
            if !alive {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        assert!(!alive, "child process should be terminated after drop");

        let _ = fs::remove_file(script_path);
        let _ = fs::remove_file(pid_file);
    }

    #[tokio::test]
    async fn process_connector_passes_config_json_as_single_positional_argument() {
        let script_path = unique_path("process-config");
        let args_file = unique_path("process-config-args");
        let args_file_escaped = args_file.to_string_lossy().replace('"', "\\\"");

        write_executable_script(
            &script_path,
            &format!(
                "#!/bin/sh\nprintf '%s\\n' \"$#\" > \"{args_file}\"\nprintf '%s\\n' \"$1\" >> \"{args_file}\"\nprintf '%s\\n' \"$2\" >> \"{args_file}\"\n",
                args_file = args_file_escaped
            ),
        );

        let config = json!({"name": "Ada", "nested": {"x": 1}});
        let expected_config = serde_json::to_string(&config).expect("serialize config");

        let transport = ProcessConnector::new(&script_path)
            .with_config_json(config)
            .connect()
            .await
            .expect("connector should spawn process");

        let mut args = None;
        for _ in 0..20 {
            if let Ok(text) = fs::read_to_string(&args_file) {
                args = Some(text);
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        drop(transport);

        let args = args.expect("read script args output");
        let mut lines = args.lines();
        assert_eq!(lines.next(), Some("2"));
        assert_eq!(lines.next(), Some("serve"));
        assert_eq!(lines.next(), Some(expected_config.as_str()));

        let _ = fs::remove_file(script_path);
        let _ = fs::remove_file(args_file);
    }
}
