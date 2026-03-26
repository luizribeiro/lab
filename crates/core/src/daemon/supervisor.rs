use std::marker::PhantomData;
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use super::process::DaemonProcess;
use super::resolve::resolve_daemon_binary;
use super::traits::{DaemonAdapter, DaemonReadiness};

#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    pub readiness_timeout: Duration,
    pub shutdown_timeout: Duration,
    pub poll_interval: Duration,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            readiness_timeout: Duration::from_secs(5),
            shutdown_timeout: Duration::from_secs(2),
            poll_interval: Duration::from_millis(50),
        }
    }
}

pub(crate) trait SpawnBackend: Send + Sync + 'static {
    fn spawn(
        &self,
        program: &Path,
        args: &[String],
        sandbox: &capsa_sandbox::SandboxSpec,
        fd_remaps: &[capsa_sandbox::FdRemap],
    ) -> Result<capsa_sandbox::SandboxedChild>;
}

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct SandboxedSpawnBackend;

impl SpawnBackend for SandboxedSpawnBackend {
    fn spawn(
        &self,
        program: &Path,
        args: &[String],
        sandbox: &capsa_sandbox::SandboxSpec,
        fd_remaps: &[capsa_sandbox::FdRemap],
    ) -> Result<capsa_sandbox::SandboxedChild> {
        capsa_sandbox::spawn_sandboxed_with_fds(program, args, sandbox, fd_remaps)
            .with_context(|| format!("failed to spawn daemon binary {}", program.display()))
    }
}

pub struct DaemonSupervisor<B = SandboxedSpawnBackend> {
    pub config: SupervisorConfig,
    backend: B,
}

impl DaemonSupervisor {
    pub fn new(config: SupervisorConfig) -> Self {
        Self {
            config,
            backend: SandboxedSpawnBackend,
        }
    }
}

impl Default for DaemonSupervisor {
    fn default() -> Self {
        Self::new(SupervisorConfig::default())
    }
}

impl<B: SpawnBackend> DaemonSupervisor<B> {
    #[cfg(test)]
    fn with_backend(config: SupervisorConfig, backend: B) -> Self {
        Self { config, backend }
    }

    pub fn spawn<D: DaemonAdapter>(
        &self,
        spec: D::Spec,
        mut handoff: D::Handoff,
    ) -> Result<DaemonHandle<D>> {
        let binary_info = D::binary_info();
        let binary_path = resolve_daemon_binary(binary_info.binary_name, binary_info.env_override)
            .with_context(|| {
                format!(
                    "failed to resolve {} daemon binary ({})",
                    binary_info.daemon_name, binary_info.binary_name
                )
            })?;

        let spawn_spec = match D::spawn_spec(&spec, &handoff, &binary_path) {
            Ok(spec) => spec,
            Err(primary_error) => {
                let mut error = primary_error.context(format!(
                    "failed to build spawn spec for {} daemon",
                    binary_info.daemon_name
                ));

                if let Err(cleanup_error) = D::on_spawn_failed(&spec, handoff) {
                    error = attach_cleanup_error(
                        error,
                        cleanup_error.context(format!(
                            "{} adapter spawn-failure cleanup failed",
                            binary_info.daemon_name
                        )),
                    );
                }

                return Err(error);
            }
        };

        let readiness = match D::readiness(&spec, &mut handoff) {
            Ok(readiness) => readiness,
            Err(primary_error) => {
                let mut error = primary_error.context(format!(
                    "failed to prepare readiness for {} daemon",
                    binary_info.daemon_name
                ));

                if let Err(cleanup_error) = D::on_spawn_failed(&spec, handoff) {
                    error = attach_cleanup_error(
                        error,
                        cleanup_error.context(format!(
                            "{} adapter spawn-failure cleanup failed",
                            binary_info.daemon_name
                        )),
                    );
                }

                return Err(error);
            }
        };

        let child = self
            .backend
            .spawn(
                &binary_path,
                &spawn_spec.args,
                &spawn_spec.sandbox,
                &spawn_spec.fd_remaps,
            )
            .with_context(|| format!("failed to spawn {} daemon", binary_info.daemon_name));

        let child = match child {
            Ok(child) => child,
            Err(primary_error) => {
                let mut error = primary_error;
                if let Err(cleanup_error) = D::on_spawn_failed(&spec, handoff) {
                    error = attach_cleanup_error(
                        error,
                        cleanup_error.context(format!(
                            "{} adapter spawn-failure cleanup failed",
                            binary_info.daemon_name
                        )),
                    );
                }

                return Err(error);
            }
        };

        let mut process = DaemonProcess::new(binary_info.daemon_name, child);

        if let Err(primary_error) = readiness.wait_ready(self.config.readiness_timeout) {
            let mut error = primary_error.context(format!(
                "{} daemon readiness check failed",
                binary_info.daemon_name
            ));

            if let Err(cleanup_error) = teardown_process(&mut process) {
                error = attach_cleanup_error(error, cleanup_error);
            }

            if let Err(cleanup_error) = D::on_spawn_failed(&spec, handoff) {
                error = attach_cleanup_error(
                    error,
                    cleanup_error.context(format!(
                        "{} adapter spawn-failure cleanup failed",
                        binary_info.daemon_name
                    )),
                );
            }

            return Err(error);
        }

        if let Err(primary_error) = D::on_spawned(&spec, &mut handoff) {
            let mut error = primary_error.context(format!(
                "{} adapter post-spawn hook failed",
                binary_info.daemon_name
            ));

            if let Err(cleanup_error) = teardown_process(&mut process) {
                error = attach_cleanup_error(error, cleanup_error);
            }

            if let Err(cleanup_error) = D::on_spawn_failed(&spec, handoff) {
                error = attach_cleanup_error(
                    error,
                    cleanup_error.context(format!(
                        "{} adapter spawn-failure cleanup failed",
                        binary_info.daemon_name
                    )),
                );
            }

            return Err(error);
        }

        Ok(DaemonHandle {
            process,
            spec,
            handoff: Some(handoff),
            shutdown_timeout: self.config.shutdown_timeout,
            poll_interval: self.config.poll_interval,
            _marker: PhantomData,
        })
    }
}

pub struct DaemonHandle<D: DaemonAdapter> {
    process: DaemonProcess,
    spec: D::Spec,
    handoff: Option<D::Handoff>,
    shutdown_timeout: Duration,
    poll_interval: Duration,
    _marker: PhantomData<D>,
}

impl<D: DaemonAdapter> DaemonHandle<D> {
    pub fn daemon_name(&self) -> &'static str {
        self.process.name()
    }

    pub fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>> {
        self.process.try_wait()
    }

    pub fn kill(&mut self) -> Result<()> {
        self.process.kill()
    }

    pub fn wait_blocking(&mut self) -> Result<std::process::ExitStatus> {
        self.process.wait_blocking()
    }

    pub fn shutdown(mut self) -> Result<()> {
        let mut primary_error = shutdown_process_with_timeout(
            &mut self.process,
            self.shutdown_timeout,
            self.poll_interval,
        )
        .err();

        if let Some(handoff) = self.handoff.take() {
            if let Err(cleanup_error) = D::on_shutdown(&self.spec, handoff) {
                primary_error = Some(match primary_error {
                    Some(primary_error) => attach_cleanup_error(
                        primary_error,
                        cleanup_error.context(format!(
                            "{} adapter shutdown hook failed",
                            self.process.name()
                        )),
                    ),
                    None => cleanup_error.context(format!(
                        "{} adapter shutdown hook failed",
                        self.process.name()
                    )),
                });
            }
        }

        if let Some(error) = primary_error {
            return Err(error);
        }

        Ok(())
    }
}

impl<D: DaemonAdapter> Drop for DaemonHandle<D> {
    fn drop(&mut self) {
        let _ = shutdown_process_with_timeout(
            &mut self.process,
            self.shutdown_timeout,
            self.poll_interval,
        );

        if let Some(handoff) = self.handoff.take() {
            let _ = D::on_shutdown(&self.spec, handoff);
        }
    }
}

trait ProcessControl {
    fn name(&self) -> &'static str;
    fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>>;
    fn kill(&mut self) -> Result<()>;
    fn wait_blocking(&mut self) -> Result<std::process::ExitStatus>;
}

impl ProcessControl for DaemonProcess {
    fn name(&self) -> &'static str {
        DaemonProcess::name(self)
    }

    fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>> {
        DaemonProcess::try_wait(self)
    }

    fn kill(&mut self) -> Result<()> {
        DaemonProcess::kill(self)
    }

    fn wait_blocking(&mut self) -> Result<std::process::ExitStatus> {
        DaemonProcess::wait_blocking(self)
    }
}

fn teardown_process<P: ProcessControl>(process: &mut P) -> Result<()> {
    let mut error: Option<anyhow::Error> = None;

    if let Err(kill_error) = process.kill() {
        error = Some(kill_error.context(format!(
            "failed to terminate {} during teardown",
            process.name()
        )));
    }

    if let Err(wait_error) = process.wait_blocking() {
        error = Some(match error {
            Some(primary_error) => attach_cleanup_error(
                primary_error,
                wait_error.context(format!("failed to reap {} during teardown", process.name())),
            ),
            None => {
                wait_error.context(format!("failed to reap {} during teardown", process.name()))
            }
        });
    }

    if let Some(error) = error {
        return Err(error);
    }

    Ok(())
}

fn shutdown_process_with_timeout<P: ProcessControl>(
    process: &mut P,
    shutdown_timeout: Duration,
    poll_interval: Duration,
) -> Result<()> {
    let deadline = Instant::now() + shutdown_timeout;

    loop {
        if process.try_wait()?.is_some() {
            return Ok(());
        }

        if Instant::now() >= deadline {
            break;
        }

        std::thread::sleep(poll_interval);
    }

    if let Err(kill_error) = process.kill() {
        if process.try_wait()?.is_some() {
            return Ok(());
        }

        return Err(kill_error).with_context(|| {
            format!(
                "failed to terminate {} after shutdown timeout",
                process.name()
            )
        });
    }

    process
        .wait_blocking()
        .with_context(|| format!("failed to reap {} after forced shutdown", process.name()))?;

    Ok(())
}

fn attach_cleanup_error(primary: anyhow::Error, cleanup: anyhow::Error) -> anyhow::Error {
    primary.context(format!("cleanup error: {cleanup:#}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::daemon::traits::{DaemonBinaryInfo, DaemonReadiness, DaemonSpawnSpec, NoReadiness};
    use std::collections::VecDeque;
    use std::process::Command;
    use std::sync::{Mutex, OnceLock};

    #[derive(Debug, Clone)]
    struct FakeBackend {
        mode: BackendMode,
    }

    #[derive(Debug, Clone)]
    enum BackendMode {
        SpawnError,
        SpawnSleep,
    }

    impl SpawnBackend for FakeBackend {
        fn spawn(
            &self,
            program: &Path,
            args: &[String],
            sandbox: &capsa_sandbox::SandboxSpec,
            fd_remaps: &[capsa_sandbox::FdRemap],
        ) -> Result<capsa_sandbox::SandboxedChild> {
            match self.mode {
                BackendMode::SpawnError => anyhow::bail!("backend spawn error"),
                BackendMode::SpawnSleep => {
                    capsa_sandbox::spawn_sandboxed_with_fds(program, args, sandbox, fd_remaps)
                        .context("fake backend failed to spawn")
                }
            }
        }
    }

    #[derive(Debug, Default, Clone)]
    struct AdapterState {
        spawn_spec_fails: bool,
        readiness_fails: bool,
        on_spawned_fails: bool,
        on_spawn_failed_fails: bool,
        on_shutdown_fails: bool,
        on_spawned_calls: usize,
        on_spawn_failed_calls: usize,
        on_shutdown_calls: usize,
    }

    fn adapter_state() -> &'static Mutex<AdapterState> {
        static STATE: OnceLock<Mutex<AdapterState>> = OnceLock::new();
        STATE.get_or_init(|| Mutex::new(AdapterState::default()))
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn reset_adapter_state() {
        *adapter_state().lock().expect("state lock") = AdapterState::default();
    }

    struct EnvVarGuard {
        key: &'static str,
        old: Option<std::ffi::OsString>,
    }

    impl EnvVarGuard {
        fn set_path(key: &'static str, value: &Path) -> Self {
            let old = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, old }
        }

        fn set_raw(key: &'static str, value: &str) -> Self {
            let old = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, old }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(old) = self.old.take() {
                std::env::set_var(self.key, old);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    #[derive(Debug)]
    struct TestReadiness {
        should_fail: bool,
    }

    impl DaemonReadiness for TestReadiness {
        fn wait_ready(self, _timeout: Duration) -> Result<()> {
            if self.should_fail {
                anyhow::bail!("readiness failed")
            }
            Ok(())
        }
    }

    struct TestAdapter;

    impl DaemonAdapter for TestAdapter {
        type Spec = String;
        type Handoff = ();
        type Ready = TestReadiness;

        fn binary_info() -> DaemonBinaryInfo {
            DaemonBinaryInfo {
                daemon_name: "test",
                binary_name: "capsa-test-daemon",
                env_override: "CAPSA_TEST_DAEMON_PATH",
            }
        }

        fn spawn_spec(
            _spec: &Self::Spec,
            _handoff: &Self::Handoff,
            _binary_path: &Path,
        ) -> Result<DaemonSpawnSpec> {
            let state = adapter_state().lock().expect("state lock").clone();
            if state.spawn_spec_fails {
                anyhow::bail!("spawn_spec failed")
            }

            Ok(DaemonSpawnSpec {
                args: vec!["-c".into(), "while true; do :; done".into()],
                sandbox: capsa_sandbox::SandboxSpec::default(),
                fd_remaps: vec![],
            })
        }

        fn readiness(_spec: &Self::Spec, _handoff: &mut Self::Handoff) -> Result<Self::Ready> {
            let state = adapter_state().lock().expect("state lock").clone();
            Ok(TestReadiness {
                should_fail: state.readiness_fails,
            })
        }

        fn on_spawned(_spec: &Self::Spec, _handoff: &mut Self::Handoff) -> Result<()> {
            let mut state = adapter_state().lock().expect("state lock");
            state.on_spawned_calls += 1;
            if state.on_spawned_fails {
                anyhow::bail!("on_spawned failed")
            }
            Ok(())
        }

        fn on_spawn_failed(_spec: &Self::Spec, _handoff: Self::Handoff) -> Result<()> {
            let mut state = adapter_state().lock().expect("state lock");
            state.on_spawn_failed_calls += 1;
            if state.on_spawn_failed_fails {
                anyhow::bail!("on_spawn_failed failed")
            }
            Ok(())
        }

        fn on_shutdown(_spec: &Self::Spec, _handoff: Self::Handoff) -> Result<()> {
            let mut state = adapter_state().lock().expect("state lock");
            state.on_shutdown_calls += 1;
            if state.on_shutdown_fails {
                anyhow::bail!("on_shutdown failed")
            }
            Ok(())
        }
    }

    #[test]
    fn spawn_spec_failure_triggers_on_spawn_failed_without_on_spawned() {
        let _env_lock = env_lock().lock().expect("env lock");
        reset_adapter_state();
        adapter_state().lock().expect("state lock").spawn_spec_fails = true;

        let _env_guard = EnvVarGuard::set_path("CAPSA_TEST_DAEMON_PATH", Path::new("/bin/sh"));

        let supervisor = DaemonSupervisor::with_backend(
            SupervisorConfig::default(),
            FakeBackend {
                mode: BackendMode::SpawnSleep,
            },
        );

        let err = match supervisor.spawn::<TestAdapter>("spec".to_string(), ()) {
            Ok(_) => panic!("spawn should fail"),
            Err(err) => err,
        };

        let state = adapter_state().lock().expect("state lock").clone();
        assert_eq!(state.on_spawn_failed_calls, 1);
        assert_eq!(state.on_spawned_calls, 0);
        assert!(format!("{err:#}").contains("spawn_spec failed"));
    }

    #[test]
    fn readiness_failure_triggers_on_spawn_failed() {
        let _env_lock = env_lock().lock().expect("env lock");
        reset_adapter_state();
        adapter_state().lock().expect("state lock").readiness_fails = true;

        let _env_guard = EnvVarGuard::set_path("CAPSA_TEST_DAEMON_PATH", Path::new("/bin/sh"));
        let _sandbox_guard = EnvVarGuard::set_raw("CAPSA_DISABLE_SANDBOX", "1");

        let supervisor = DaemonSupervisor::with_backend(
            SupervisorConfig::default(),
            FakeBackend {
                mode: BackendMode::SpawnSleep,
            },
        );

        let err = match supervisor.spawn::<TestAdapter>("spec".to_string(), ()) {
            Ok(_) => panic!("spawn should fail on readiness"),
            Err(err) => err,
        };

        let state = adapter_state().lock().expect("state lock").clone();
        assert_eq!(state.on_spawn_failed_calls, 1);
        assert_eq!(state.on_spawned_calls, 0);
        assert!(format!("{err:#}").contains("readiness failed"));
    }

    #[test]
    fn on_spawned_failure_triggers_teardown() {
        let _env_lock = env_lock().lock().expect("env lock");
        reset_adapter_state();
        adapter_state().lock().expect("state lock").on_spawned_fails = true;

        let _env_guard = EnvVarGuard::set_path("CAPSA_TEST_DAEMON_PATH", Path::new("/bin/sh"));
        let _sandbox_guard = EnvVarGuard::set_raw("CAPSA_DISABLE_SANDBOX", "1");

        let supervisor = DaemonSupervisor::with_backend(
            SupervisorConfig::default(),
            FakeBackend {
                mode: BackendMode::SpawnSleep,
            },
        );

        let err = match supervisor.spawn::<TestAdapter>("spec".to_string(), ()) {
            Ok(_) => panic!("spawn should fail on on_spawned"),
            Err(err) => err,
        };

        let state = adapter_state().lock().expect("state lock").clone();
        assert_eq!(state.on_spawned_calls, 1);
        assert_eq!(state.on_spawn_failed_calls, 1);
        assert!(format!("{err:#}").contains("on_spawned failed"));
    }

    #[test]
    fn shutdown_calls_on_shutdown_once() {
        let _env_lock = env_lock().lock().expect("env lock");
        reset_adapter_state();

        let _env_guard = EnvVarGuard::set_path("CAPSA_TEST_DAEMON_PATH", Path::new("/bin/sh"));
        let _sandbox_guard = EnvVarGuard::set_raw("CAPSA_DISABLE_SANDBOX", "1");

        let supervisor = DaemonSupervisor::with_backend(
            SupervisorConfig {
                readiness_timeout: Duration::from_secs(1),
                shutdown_timeout: Duration::from_millis(20),
                poll_interval: Duration::from_millis(5),
            },
            FakeBackend {
                mode: BackendMode::SpawnSleep,
            },
        );

        let handle = supervisor
            .spawn::<TestAdapter>("spec".to_string(), ())
            .expect("spawn should succeed");

        handle.shutdown().expect("shutdown should succeed");

        let state = adapter_state().lock().expect("state lock").clone();
        assert_eq!(state.on_shutdown_calls, 1);
    }

    #[test]
    fn drop_calls_on_shutdown_once_without_explicit_shutdown() {
        let _env_lock = env_lock().lock().expect("env lock");
        reset_adapter_state();

        let _env_guard = EnvVarGuard::set_path("CAPSA_TEST_DAEMON_PATH", Path::new("/bin/sh"));
        let _sandbox_guard = EnvVarGuard::set_raw("CAPSA_DISABLE_SANDBOX", "1");

        let supervisor = DaemonSupervisor::with_backend(
            SupervisorConfig {
                readiness_timeout: Duration::from_secs(1),
                shutdown_timeout: Duration::from_millis(20),
                poll_interval: Duration::from_millis(5),
            },
            FakeBackend {
                mode: BackendMode::SpawnSleep,
            },
        );

        let handle = supervisor
            .spawn::<TestAdapter>("spec".to_string(), ())
            .expect("spawn should succeed");
        drop(handle);

        let state = adapter_state().lock().expect("state lock").clone();
        assert_eq!(state.on_shutdown_calls, 1);
    }

    #[test]
    fn primary_error_precedence_is_preserved_over_cleanup_errors() {
        let _env_lock = env_lock().lock().expect("env lock");
        reset_adapter_state();
        {
            let mut state = adapter_state().lock().expect("state lock");
            state.spawn_spec_fails = true;
            state.on_spawn_failed_fails = true;
        }

        let _env_guard = EnvVarGuard::set_path("CAPSA_TEST_DAEMON_PATH", Path::new("/bin/sh"));

        let supervisor = DaemonSupervisor::with_backend(
            SupervisorConfig::default(),
            FakeBackend {
                mode: BackendMode::SpawnError,
            },
        );

        let err = match supervisor.spawn::<TestAdapter>("spec".to_string(), ()) {
            Ok(_) => panic!("spawn should fail"),
            Err(err) => err,
        };
        let rendered = format!("{err:#}");

        assert!(rendered.contains("spawn_spec failed"));
        assert!(rendered.contains("on_spawn_failed failed"));
    }

    #[test]
    fn no_readiness_wait_ready_is_noop() {
        NoReadiness
            .wait_ready(Duration::from_millis(1))
            .expect("NoReadiness should always succeed");
    }

    #[test]
    fn daemon_name_reports_adapter_name() {
        let _env_lock = env_lock().lock().expect("env lock");
        reset_adapter_state();

        let _env_guard = EnvVarGuard::set_path("CAPSA_TEST_DAEMON_PATH", Path::new("/bin/sh"));
        let _sandbox_guard = EnvVarGuard::set_raw("CAPSA_DISABLE_SANDBOX", "1");

        let supervisor = DaemonSupervisor::with_backend(
            SupervisorConfig::default(),
            FakeBackend {
                mode: BackendMode::SpawnSleep,
            },
        );

        let handle = supervisor
            .spawn::<TestAdapter>("spec".to_string(), ())
            .expect("spawn should succeed");
        assert_eq!(handle.daemon_name(), "test");

        handle.shutdown().expect("shutdown should succeed");
    }

    #[test]
    fn binary_resolution_error_bubbles_with_context() {
        let _env_lock = env_lock().lock().expect("env lock");
        reset_adapter_state();

        let _clear_env =
            EnvVarGuard::set_path("CAPSA_TEST_DAEMON_PATH", Path::new("/definitely/missing"));

        let supervisor = DaemonSupervisor::with_backend(
            SupervisorConfig::default(),
            FakeBackend {
                mode: BackendMode::SpawnSleep,
            },
        );

        let err = match supervisor.spawn::<TestAdapter>("spec".to_string(), ()) {
            Ok(_) => panic!("missing binary should fail"),
            Err(err) => err,
        };
        assert!(format!("{err:#}").contains("failed to resolve test daemon binary"));
    }

    #[test]
    fn spawn_backend_failure_calls_on_spawn_failed() {
        let _env_lock = env_lock().lock().expect("env lock");
        reset_adapter_state();

        let _env_guard = EnvVarGuard::set_path("CAPSA_TEST_DAEMON_PATH", Path::new("/bin/sh"));

        let supervisor = DaemonSupervisor::with_backend(
            SupervisorConfig::default(),
            FakeBackend {
                mode: BackendMode::SpawnError,
            },
        );

        let err = match supervisor.spawn::<TestAdapter>("spec".to_string(), ()) {
            Ok(_) => panic!("spawn should fail on backend error"),
            Err(err) => err,
        };

        let state = adapter_state().lock().expect("state lock").clone();
        assert_eq!(state.on_spawn_failed_calls, 1);
        assert!(format!("{err:#}").contains("backend spawn error"));
    }

    #[derive(Debug)]
    enum TryWaitStep {
        Running,
        Exited,
    }

    #[derive(Debug)]
    struct FakeProcess {
        name: &'static str,
        try_wait_steps: VecDeque<TryWaitStep>,
        kill_result: Result<()>,
        wait_result: Result<std::process::ExitStatus>,
        kill_calls: usize,
        wait_calls: usize,
    }

    impl ProcessControl for FakeProcess {
        fn name(&self) -> &'static str {
            self.name
        }

        fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>> {
            match self
                .try_wait_steps
                .pop_front()
                .unwrap_or(TryWaitStep::Running)
            {
                TryWaitStep::Running => Ok(None),
                TryWaitStep::Exited => Ok(Some(success_status())),
            }
        }

        fn kill(&mut self) -> Result<()> {
            self.kill_calls += 1;
            self.kill_result
                .as_ref()
                .map(|_| ())
                .map_err(|err| anyhow::anyhow!("{err}"))
        }

        fn wait_blocking(&mut self) -> Result<std::process::ExitStatus> {
            self.wait_calls += 1;
            self.wait_result
                .as_ref()
                .map(|_| success_status())
                .map_err(|err| anyhow::anyhow!("{err}"))
        }
    }

    fn success_status() -> std::process::ExitStatus {
        Command::new("/bin/sh")
            .arg("-c")
            .arg("exit 0")
            .status()
            .expect("status should be available")
    }

    #[test]
    fn shutdown_timeout_forces_kill_and_wait() {
        let mut process = FakeProcess {
            name: "fake",
            try_wait_steps: VecDeque::from([TryWaitStep::Running]),
            kill_result: Ok(()),
            wait_result: Ok(success_status()),
            kill_calls: 0,
            wait_calls: 0,
        };

        shutdown_process_with_timeout(&mut process, Duration::from_millis(0), Duration::ZERO)
            .expect("forced shutdown should succeed");

        assert_eq!(process.kill_calls, 1);
        assert_eq!(process.wait_calls, 1);
    }

    #[test]
    fn shutdown_treats_kill_error_as_success_if_process_already_exited() {
        let mut process = FakeProcess {
            name: "fake",
            try_wait_steps: VecDeque::from([TryWaitStep::Running, TryWaitStep::Exited]),
            kill_result: Err(anyhow::anyhow!("no such process")),
            wait_result: Ok(success_status()),
            kill_calls: 0,
            wait_calls: 0,
        };

        shutdown_process_with_timeout(&mut process, Duration::from_millis(0), Duration::ZERO)
            .expect("already-exited process should not fail shutdown");

        assert_eq!(process.kill_calls, 1);
        assert_eq!(process.wait_calls, 0);
    }
}
