//! Slash command registry.
//!
//! A slash command is a named function the user invokes by typing `/<name>`
//! at the composer. Handlers receive `&mut App` and return a [`CommandResult`]
//! describing the side effect they want (push a modal, redraw, quit, …).
//!
//! Commands are declared with the [`slash_command!`] macro and bundled into
//! the static [`registry`].

use crate::app::App;
use crate::help_modal::HelpModal;

/// Outcome of a slash-command handler. The run loop dispatches on this after
/// the handler returns so command bodies stay short and side-effect-free.
pub enum CommandResult {
    /// Handler is done — no further action needed beyond what it already did
    /// (e.g. it pushed a modal directly).
    Continue,
    /// Tear down the current session and start a fresh one (`/new`).
    StartNewSession,
    /// Exit orb (`/exit`).
    Quit,
    /// Force a full viewport rebuild (`/redraw`). Useful as a manual escape
    /// hatch when terminal-resize repainting goes sideways.
    Redraw,
}

pub type CommandHandler = fn(&mut App) -> CommandResult;

pub struct Command {
    pub name: &'static str,
    pub description: &'static str,
    pub handler: CommandHandler,
}

/// Declare a slash-command entry. The macro is intentionally thin — it just
/// gives every entry the same shape so the registry stays uniform. Define
/// the handler function in the usual way and reference it by path:
///
/// ```ignore
/// fn cmd_help(_app: &mut App) -> CommandResult { CommandResult::Continue }
///
/// static HELP: Command = slash_command!("help", "Show available commands", cmd_help);
/// ```
#[macro_export]
macro_rules! slash_command {
    ($name:literal, $description:literal, $handler:path $(,)?) => {
        $crate::commands::Command {
            name: $name,
            description: $description,
            handler: $handler,
        }
    };
}

fn cmd_new(_app: &mut App) -> CommandResult {
    CommandResult::StartNewSession
}

fn cmd_exit(_app: &mut App) -> CommandResult {
    CommandResult::Quit
}

fn cmd_redraw(_app: &mut App) -> CommandResult {
    CommandResult::Redraw
}

fn cmd_help(app: &mut App) -> CommandResult {
    app.modals.push(Box::new(HelpModal::new()));
    CommandResult::Continue
}

static NEW: Command = slash_command!("new", "Start a fresh session", cmd_new);
static EXIT: Command = slash_command!("exit", "Quit orb", cmd_exit);
static REDRAW: Command = slash_command!("redraw", "Force a full viewport rebuild", cmd_redraw);
static HELP: Command = slash_command!("help", "Show available commands", cmd_help);

pub fn registry() -> &'static [&'static Command] {
    static REGISTRY: &[&Command] = &[&NEW, &EXIT, &REDRAW, &HELP];
    REGISTRY
}

/// Parse a composer line as a slash command invocation. Returns the matching
/// [`Command`] when the line starts with `/` and exactly matches a registered
/// command name. Arguments aren't supported yet — the lookup is name-only.
pub fn parse(line: &str) -> Option<&'static Command> {
    let rest = line.strip_prefix('/')?.trim();
    if rest.is_empty() {
        return None;
    }
    let name = rest.split_whitespace().next()?;
    registry().iter().copied().find(|c| c.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_finds_registered_command_by_name() {
        let cmd = parse("/exit").expect("registered");
        assert_eq!(cmd.name, "exit");
    }

    #[test]
    fn parse_ignores_extra_whitespace_and_args() {
        let cmd = parse("/redraw   now please").expect("registered");
        assert_eq!(cmd.name, "redraw");
    }

    #[test]
    fn parse_rejects_unknown_command() {
        assert!(parse("/nonsense").is_none());
    }

    #[test]
    fn parse_rejects_non_slash_input() {
        assert!(parse("hello there").is_none());
    }

    #[test]
    fn parse_rejects_bare_slash() {
        assert!(parse("/").is_none());
        assert!(parse("/   ").is_none());
    }

    #[test]
    fn registry_names_are_unique() {
        let mut names: Vec<&str> = registry().iter().map(|c| c.name).collect();
        names.sort_unstable();
        let dedup_len = {
            let mut clone = names.clone();
            clone.dedup();
            clone.len()
        };
        assert_eq!(names.len(), dedup_len, "duplicate command names: {names:?}");
    }
}
