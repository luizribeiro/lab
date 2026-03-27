use std::{future::Future, io::Write};

use fittings_core::{error::FittingsError, service::Service};
use fittings_server::Server;
use fittings_transport::stdio::from_process_stdio;
use serde_json::Value;

use crate::{
    config::parse_server_config,
    mode::{detect_mode, SpawnMode, SpawnModeError},
    schema::{validate_service_schema, ServiceSchema},
};

const DEFAULT_MAX_FRAME_BYTES: usize = 1_048_576;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    Normal,
    Exit(i32),
}

pub struct SpawnRunner {
    schema: ServiceSchema,
    max_frame_bytes: usize,
    max_in_flight: usize,
}

impl SpawnRunner {
    pub fn new(schema: ServiceSchema) -> Self {
        Self {
            schema,
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
            max_in_flight: 64,
        }
    }

    pub fn with_max_frame_bytes(mut self, max_frame_bytes: usize) -> Self {
        self.max_frame_bytes = max_frame_bytes.max(1);
        self
    }

    pub fn with_max_in_flight(mut self, max_in_flight: usize) -> Self {
        self.max_in_flight = max_in_flight.max(1);
        self
    }

    pub async fn run_with<S, Fut, WOut, WErr>(
        &self,
        env_fittings: Option<&str>,
        args: &[String],
        stdout: &mut WOut,
        stderr: &mut WErr,
        serve: S,
    ) -> RunOutcome
    where
        S: FnOnce(Option<Value>) -> Fut,
        Fut: Future<Output = Result<(), FittingsError>>,
        WOut: Write,
        WErr: Write,
    {
        let mode = match detect_mode(env_fittings, args) {
            Ok(mode) => mode,
            Err(error) => {
                render_mode_error(stderr, error);
                return RunOutcome::Exit(1);
            }
        };

        if matches!(mode, SpawnMode::Schema | SpawnMode::Serve { .. }) {
            if let Err(error) = validate_service_schema(&self.schema) {
                let _ = writeln!(stderr, "invalid service schema: {error}");
                return RunOutcome::Exit(1);
            }
        }

        match mode {
            SpawnMode::Normal => RunOutcome::Normal,
            SpawnMode::Schema => {
                if emit_schema(stdout, &self.schema, stderr) {
                    RunOutcome::Exit(0)
                } else {
                    RunOutcome::Exit(1)
                }
            }
            SpawnMode::Serve { config_json } => {
                let config = match parse_server_config(config_json.as_deref(), &self.schema) {
                    Ok(config) => config,
                    Err(error) => {
                        let _ = writeln!(stderr, "{error}");
                        return RunOutcome::Exit(1);
                    }
                };

                match serve(config).await {
                    Ok(()) => RunOutcome::Exit(0),
                    Err(error) => {
                        let _ = writeln!(stderr, "serve failed: {error}");
                        RunOutcome::Exit(1)
                    }
                }
            }
        }
    }

    pub async fn run_with_stdio_service<T, F>(
        &self,
        env_fittings: Option<&str>,
        args: &[String],
        make_service: F,
    ) -> RunOutcome
    where
        T: Service + 'static,
        F: FnOnce(Option<Value>) -> T,
    {
        let mut stdout = std::io::stdout();
        let mut stderr = std::io::stderr();
        let max_in_flight = self.max_in_flight;
        let max_frame_bytes = self.max_frame_bytes;

        self.run_with(
            env_fittings,
            args,
            &mut stdout,
            &mut stderr,
            move |config| async move {
                let service = make_service(config);
                let transport = from_process_stdio(max_frame_bytes);
                let server = Server::new(service, transport).with_max_in_flight(max_in_flight);
                server.serve().await
            },
        )
        .await
    }
}

fn render_mode_error<WErr: Write>(stderr: &mut WErr, error: SpawnModeError) {
    let _ = writeln!(stderr, "{error}");
}

fn emit_schema<WOut: Write, WErr: Write>(
    stdout: &mut WOut,
    schema: &ServiceSchema,
    stderr: &mut WErr,
) -> bool {
    match serde_json::to_writer(&mut *stdout, schema) {
        Ok(()) => {
            if writeln!(stdout).is_err() {
                let _ = writeln!(stderr, "failed to write schema newline to stdout");
                return false;
            }
            true
        }
        Err(error) => {
            let _ = writeln!(stderr, "failed to serialize schema: {error}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    use serde_json::{json, Value};

    use crate::{MethodSchema, ServiceSchema};

    use super::{RunOutcome, SpawnRunner};

    fn schema() -> ServiceSchema {
        ServiceSchema {
            name: "hello".to_string(),
            methods: vec![MethodSchema {
                name: "ping".to_string(),
                description: Some("health check".to_string()),
                params_schema: Some(json!({"type": "object"})),
                result_schema: Some(json!({"type": "object"})),
            }],
            config_schema: Some(json!({"type": "object"})),
        }
    }

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|item| item.to_string()).collect()
    }

    #[tokio::test]
    async fn normal_mode_returns_without_invoking_spawn_paths() {
        let invalid_schema = ServiceSchema {
            name: " ".to_string(),
            methods: vec![],
            config_schema: None,
        };
        let runner = SpawnRunner::new(invalid_schema);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let outcome = runner
            .run_with(None, &args(&[]), &mut stdout, &mut stderr, |_| async {
                panic!("serve callback must not run in normal mode");
            })
            .await;

        assert_eq!(outcome, RunOutcome::Normal);
        assert!(stdout.is_empty());
        assert!(stderr.is_empty());
    }

    #[tokio::test]
    async fn schema_mode_writes_only_stdout_and_exits_zero() {
        let runner = SpawnRunner::new(schema());
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let outcome = runner
            .run_with(
                Some("1"),
                &args(&["schema"]),
                &mut stdout,
                &mut stderr,
                |_| async { Ok(()) },
            )
            .await;

        assert_eq!(outcome, RunOutcome::Exit(0));
        assert!(stderr.is_empty());

        let stdout_text = String::from_utf8(stdout).expect("schema output should be utf-8");
        let parsed: Value = serde_json::from_str(stdout_text.trim()).expect("valid schema json");
        assert_eq!(parsed["name"], json!("hello"));
    }

    #[tokio::test]
    async fn serve_mode_parses_config_and_invokes_callback() {
        let runner = SpawnRunner::new(schema());
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let invoked = Arc::new(AtomicBool::new(false));
        let invoked_clone = Arc::clone(&invoked);

        let outcome = runner
            .run_with(
                Some("1"),
                &args(&["serve", "{\"log_level\":\"debug\"}"]),
                &mut stdout,
                &mut stderr,
                move |config| {
                    invoked_clone.store(true, Ordering::SeqCst);
                    async move {
                        assert_eq!(config, Some(json!({"log_level": "debug"})));
                        Ok(())
                    }
                },
            )
            .await;

        assert_eq!(outcome, RunOutcome::Exit(0));
        assert!(stdout.is_empty());
        assert!(stderr.is_empty());
        assert!(invoked.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn invalid_config_fails_fast_without_invoking_serve() {
        let runner = SpawnRunner::new(schema());
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let invoked = Arc::new(AtomicBool::new(false));
        let invoked_clone = Arc::clone(&invoked);

        let outcome = runner
            .run_with(
                Some("1"),
                &args(&["serve", "{"]),
                &mut stdout,
                &mut stderr,
                move |_| {
                    invoked_clone.store(true, Ordering::SeqCst);
                    async { Ok(()) }
                },
            )
            .await;

        assert_eq!(outcome, RunOutcome::Exit(1));
        assert!(stdout.is_empty());
        assert!(!stderr.is_empty());
        assert!(!invoked.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn usage_and_unsupported_version_errors_go_to_stderr_only() {
        let runner = SpawnRunner::new(schema());
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let missing_command = runner
            .run_with(Some("1"), &args(&[]), &mut stdout, &mut stderr, |_| async {
                Ok(())
            })
            .await;

        assert_eq!(missing_command, RunOutcome::Exit(1));
        assert!(stdout.is_empty());
        assert!(!stderr.is_empty());

        stdout.clear();
        stderr.clear();

        let unsupported = runner
            .run_with(
                Some("9"),
                &args(&["schema"]),
                &mut stdout,
                &mut stderr,
                |_| async { Ok(()) },
            )
            .await;

        assert_eq!(unsupported, RunOutcome::Exit(1));
        assert!(stdout.is_empty());
        assert!(!stderr.is_empty());
    }

    #[tokio::test]
    async fn serve_callback_failure_returns_exit_one_and_stderr_message() {
        let runner = SpawnRunner::new(schema());
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let outcome = runner
            .run_with(
                Some("1"),
                &args(&["serve"]),
                &mut stdout,
                &mut stderr,
                |_| async { Err(fittings_core::error::FittingsError::internal("boom")) },
            )
            .await;

        assert_eq!(outcome, RunOutcome::Exit(1));
        assert!(stdout.is_empty());
        let stderr_text = String::from_utf8(stderr).expect("stderr should be utf-8");
        assert!(stderr_text.contains("serve failed"));
    }
}
