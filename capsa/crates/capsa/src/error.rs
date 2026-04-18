use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BuildError {
    InvalidHostPattern { pattern: String, reason: String },
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHostPattern { pattern, reason } => {
                write!(f, "invalid host pattern '{pattern}': {reason}")
            }
        }
    }
}

impl std::error::Error for BuildError {}

type BoxedError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug)]
#[non_exhaustive]
pub enum StartError {
    /// Failed to spawn the network daemon.
    NetworkSpawn(BoxedError),
    /// Failed to spawn the virtual machine monitor.
    VmSpawn(BoxedError),
    /// Failed to attach an interface to a running network daemon.
    Attach(BoxedError),
    /// Failed to allocate a host/guest socketpair for an attachment.
    Socketpair(std::io::Error),
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NetworkSpawn(e) => write!(f, "failed to spawn network daemon: {e}"),
            Self::VmSpawn(e) => write!(f, "failed to spawn VM: {e}"),
            Self::Attach(e) => write!(f, "failed to attach interface: {e}"),
            Self::Socketpair(e) => write!(f, "failed to allocate host/guest socketpair: {e}"),
        }
    }
}

impl std::error::Error for StartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NetworkSpawn(e) | Self::VmSpawn(e) | Self::Attach(e) => Some(e.as_ref()),
            Self::Socketpair(e) => Some(e),
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum RuntimeError {
    /// The VM failed to start before it began running.
    Start(StartError),
    /// Reaping the VMM process failed, or it exited with a non-zero
    /// status.
    Wait(BoxedError),
    /// Sending SIGKILL to the VMM process failed.
    Kill(std::io::Error),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(e) => write!(f, "VM failed to start: {e}"),
            Self::Wait(e) => write!(f, "VM did not exit cleanly: {e}"),
            Self::Kill(e) => write!(f, "failed to kill VM: {e}"),
        }
    }
}

impl std::error::Error for RuntimeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Start(e) => Some(e),
            Self::Wait(e) => Some(e.as_ref()),
            Self::Kill(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_host_pattern_display_includes_pattern_and_reason() {
        let err = BuildError::InvalidHostPattern {
            pattern: "*example.com".into(),
            reason: "wildcard host pattern must use only a leading '*.' prefix".into(),
        };

        let msg = err.to_string();
        assert!(
            msg.contains("*example.com"),
            "message missing pattern: {msg}"
        );
        assert!(msg.contains("wildcard"), "message missing reason: {msg}");
    }

    #[test]
    fn build_error_implements_std_error() {
        fn assert_error<E: std::error::Error>(_: &E) {}

        let err = BuildError::InvalidHostPattern {
            pattern: "x".into(),
            reason: "y".into(),
        };
        assert_error(&err);
    }

    #[test]
    fn start_error_network_spawn_display_names_the_daemon() {
        use std::error::Error;

        let cause = std::io::Error::new(std::io::ErrorKind::NotFound, "binary missing");
        let err = StartError::NetworkSpawn(Box::new(cause));

        let msg = err.to_string();
        assert!(msg.contains("network daemon"), "unexpected: {msg}");
        assert!(msg.contains("binary missing"), "unexpected: {msg}");
        assert!(err.source().is_some(), "source should be set");
    }

    #[test]
    fn start_error_vm_spawn_display_names_the_vm() {
        let err = StartError::VmSpawn(Box::new(std::io::Error::other("vmm missing")));
        let msg = err.to_string();
        assert!(msg.contains("failed to spawn VM"), "unexpected: {msg}");
        assert!(msg.contains("vmm missing"), "unexpected: {msg}");
    }

    #[test]
    fn start_error_socketpair_preserves_io_error_source() {
        use std::error::Error;

        let io_err = std::io::Error::from(std::io::ErrorKind::AddrInUse);
        let err = StartError::Socketpair(io_err);
        assert!(err.to_string().contains("socketpair"), "{}", err);
        assert!(err.source().is_some());
    }

    #[test]
    fn runtime_error_wait_preserves_source() {
        use std::error::Error;

        let cause = std::io::Error::other("reaper bailed");
        let err = RuntimeError::Wait(Box::new(cause));

        let msg = err.to_string();
        assert!(msg.contains("reaper bailed"), "unexpected: {msg}");
        assert!(err.source().is_some(), "source should be set");
    }

    #[test]
    fn runtime_error_start_wraps_start_error_source_chain() {
        use std::error::Error;

        let inner = StartError::Socketpair(std::io::Error::other("eperm"));
        let err = RuntimeError::Start(inner);

        let chain_head = err.source().expect("Start should expose StartError source");
        let second = chain_head
            .source()
            .expect("StartError should itself expose its io::Error source");
        assert!(second.to_string().contains("eperm"));
    }
}
