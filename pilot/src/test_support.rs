//! Test utilities for downstream crates. Gated behind the `test-support`
//! Cargo feature.

use std::path::PathBuf;

use uuid::Uuid;

use crate::driver::{CommandSpec, Driver, TurnOptions};
use crate::{Event, ParseError};

pub struct TestDriver {
    pub name: &'static str,
    pub program: PathBuf,
}

impl TestDriver {
    pub fn new(name: &'static str, program: impl Into<PathBuf>) -> Self {
        Self {
            name,
            program: program.into(),
        }
    }
}

impl Driver for TestDriver {
    fn name(&self) -> &'static str {
        self.name
    }

    fn command(
        &self,
        session_id: Uuid,
        prompt: &str,
        _opts: &TurnOptions,
    ) -> crate::Result<CommandSpec> {
        Ok(CommandSpec {
            program: self.program.clone(),
            args: vec![
                "--session".into(),
                session_id.to_string(),
                "--prompt".into(),
                prompt.into(),
            ],
            env: Vec::new(),
        })
    }

    fn parse(&self, value: serde_json::Value) -> std::result::Result<Vec<Event>, ParseError> {
        Ok(vec![Event::Raw {
            driver: self.name,
            value,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_carries_session_and_prompt() {
        let d = TestDriver::new("t", "/bin/echo");
        let spec = d
            .command(Uuid::nil(), "hi", &TurnOptions::default())
            .unwrap();
        assert!(spec.args.iter().any(|a| a == &Uuid::nil().to_string()));
        assert!(spec.args.iter().any(|a| a == "hi"));
    }

    #[test]
    fn parse_returns_raw() {
        let d = TestDriver::new("t", "/bin/echo");
        let evs = d.parse(serde_json::json!({"x": 1})).unwrap();
        assert_eq!(evs.len(), 1);
        assert!(matches!(evs[0], Event::Raw { driver: "t", .. }));
    }
}
