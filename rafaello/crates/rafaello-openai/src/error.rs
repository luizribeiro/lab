//! Error types and `map_to_assistant` helper (scope §OP1a).

#[derive(Debug, thiserror::Error)]
pub enum OpenaiError {
    #[error("client error {status}: {body_excerpt}")]
    ClientError { status: u16, body_excerpt: String },
    #[error("server error {status}")]
    ServerError { status: u16 },
    #[error("auth failed ({status})")]
    AuthFailed { status: u16 },
    #[error("transport error: {0}")]
    Transport(String),
    #[error("malformed response: {0}")]
    Malformed(String),
    #[error("empty choices")]
    EmptyChoices,
    #[error("invalid tool args from model: {0}")]
    InvalidToolArgs(String),
    #[error("model proposed unknown tool '{0}'")]
    UnknownTool(String),
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum OpenaiConfigError {
    #[error("RFL_OPENAI_MODEL is required (no plugin-source default)")]
    MissingModel,
}

/// Read `RFL_OPENAI_MODEL` from the environment.
///
/// Returns `MissingModel` if unset (scope §OP6 M-6 / pi-2 M-6).
/// Resolved at plugin startup *before* any HTTP path runs.
pub fn read_required_model() -> Result<String, OpenaiConfigError> {
    match std::env::var("RFL_OPENAI_MODEL") {
        Ok(v) if !v.is_empty() => Ok(v),
        _ => Err(OpenaiConfigError::MissingModel),
    }
}

/// Map an [`OpenaiError`] to the deterministic assistant_message
/// text enumerated in scope §OP1 / §OP1a.
pub fn map_to_assistant(err: &OpenaiError) -> String {
    match err {
        OpenaiError::AuthFailed { status } => {
            format!("openai: auth failed ({status}); check API key env var")
        }
        OpenaiError::ClientError {
            status,
            body_excerpt,
        } => format!("openai: client error {status}: {body_excerpt}"),
        OpenaiError::ServerError { status } => format!("openai: server error {status}"),
        OpenaiError::Transport(s) => format!("openai: transport error: {s}"),
        OpenaiError::Malformed(s) => format!("openai: malformed response: {s}"),
        OpenaiError::EmptyChoices => "(no response)".to_string(),
        OpenaiError::InvalidToolArgs(s) => format!("openai: invalid tool args from model: {s}"),
        OpenaiError::UnknownTool(name) => format!("openai: model proposed unknown tool '{name}'"),
    }
}
