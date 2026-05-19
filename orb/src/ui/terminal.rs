use std::io;

use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

/// Height of the composer block: top bar + 1 textarea row + bottom bar.
/// When a turn is in flight, the spinner label is embedded *inside* the
/// top border (codex-style title), so the status doesn't need a row of
/// its own.
pub const COMPOSER_HEIGHT: u16 = 3;

/// Default live viewport height when no modals are stacked. Equal to
/// COMPOSER_HEIGHT; modals push the viewport taller on demand.
pub const LIVE_VIEWPORT_HEIGHT: u16 = COMPOSER_HEIGHT;

pub type Term = Terminal<CrosstermBackend<io::Stdout>>;

/// Build a fresh `Terminal` with an inline viewport of `height` rows,
/// anchored at the current cursor position.
pub fn make_terminal(height: u16) -> io::Result<Term> {
    let backend = CrosstermBackend::new(io::stdout());
    Terminal::with_options(
        backend,
        ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Inline(height),
        },
    )
}
