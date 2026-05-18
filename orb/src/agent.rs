use std::path::{Path, PathBuf};

use pilot::{
    Claude, ClaudeConfig, Codex, CodexConfig, Driver, Gemini, GeminiConfig, Pi, PiConfig, Session,
};
use uuid::Uuid;

use crate::utils::orb_data_dir;

#[derive(Clone, Copy)]
pub enum AgentKind {
    Claude,
    Codex,
    Gemini,
    Pi,
}

impl AgentKind {
    pub fn label(self) -> &'static str {
        match self {
            AgentKind::Claude => "claude",
            AgentKind::Codex => "codex",
            AgentKind::Gemini => "gemini",
            AgentKind::Pi => "pi",
        }
    }

    /// `None` for `pi` because its model depends on the configured
    /// provider — hardcoding one here would break user setups.
    pub fn default_model(self) -> Option<&'static str> {
        match self {
            AgentKind::Claude => Some("claude-opus-4-7"),
            AgentKind::Codex => Some("gpt-5.5"),
            AgentKind::Gemini => Some("gemini-3.1-pro-preview"),
            AgentKind::Pi => None,
        }
    }
}

pub fn make_session(
    agent: AgentKind,
    workdir: &Path,
    resume: Option<Uuid>,
    model: Option<String>,
) -> Session {
    match agent {
        AgentKind::Claude => {
            let mut cfg = ClaudeConfig::default();
            cfg.default_model = model;
            start_session(Claude::with_config(cfg), workdir, resume)
        }
        AgentKind::Codex => {
            let mut cfg = CodexConfig::default();
            cfg.state.thread_store_path = Some(codex_thread_store());
            cfg.default_model = model;
            start_session(Codex::with_config(cfg), workdir, resume)
        }
        AgentKind::Gemini => {
            let mut cfg = GeminiConfig::default();
            cfg.default_model = model;
            start_session(Gemini::with_config(cfg), workdir, resume)
        }
        AgentKind::Pi => {
            let mut cfg = PiConfig::default();
            cfg.default_model = model;
            start_session(Pi::with_config(cfg), workdir, resume)
        }
    }
}

fn start_session<D: Driver + 'static>(driver: D, workdir: &Path, resume: Option<Uuid>) -> Session {
    match resume {
        Some(id) => Session::resume(driver, id, workdir),
        None => Session::new(driver, workdir),
    }
}

fn codex_thread_store() -> PathBuf {
    let dir = orb_data_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir.join("codex-threads.json")
}
