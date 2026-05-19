//! Modal stack used to render dialogs above the composer.
//!
//! A modal is a small widget rendered inside the inline viewport. The viewport
//! grows by the sum of stacked modal heights so modals occupy rows above the
//! composer without scrolling history away. When a modal dismisses, the viewport
//! shrinks back and the rows the modal occupied are simply repainted (they were
//! viewport rows, not scrollback, so nothing leaks into history).
//!
//! Concrete modals implement [`Modal`] and are pushed onto a [`ModalStack`]
//! held by the app.

use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;

/// Outcome of a key being routed to a modal.
pub enum ModalResult {
    /// Modal handled the key — don't forward to anything below.
    Consumed,
    /// Modal didn't want this key — let the composer (or other handler) see it.
    /// This is what makes the autocomplete modal feel like inline completion:
    /// navigation keys are eaten, character keys flow through to the textarea.
    Forward,
    /// Modal is done — pop it from the stack.
    Dismiss,
}

/// Side effect a modal can request as it dismisses. We keep this small on
/// purpose: anything more elaborate belongs in an app-level command bus, not
/// in the modal primitive.
pub enum ModalEffect {
    /// Replace the composer's text. Used by the slash-command autocomplete to
    /// "complete into" the textarea on Tab/Enter.
    ReplaceComposer(String),
}

/// A modal dialog rendered above the composer.
pub trait Modal {
    /// Desired height in rows at the given viewport width. The stack uses this
    /// to allocate space top-to-bottom.
    fn height(&self, width: u16) -> u16;

    /// Render this modal into `area`.
    fn render(&mut self, area: Rect, frame: &mut Frame);

    /// Handle a key. The top-of-stack modal sees keys first.
    fn handle_key(&mut self, key: KeyEvent) -> ModalResult;

    /// Called whenever the composer text changes. Default: ignore. Modals that
    /// react to live input (autocomplete) override this.
    fn on_composer_change(&mut self, _text: &str) {}

    /// Called once when the modal is being popped. Modals that produce a value
    /// (e.g. autocomplete returning the chosen command) override this.
    fn take_effect(&mut self) -> Option<ModalEffect> {
        None
    }
}

/// Stack of active modals. The last pushed modal is on top — it renders
/// closest to the composer and gets first crack at incoming keys.
#[derive(Default)]
pub struct ModalStack {
    modals: Vec<Box<dyn Modal>>,
}

impl ModalStack {
    pub fn push(&mut self, modal: Box<dyn Modal>) {
        self.modals.push(modal);
    }

    pub fn is_empty(&self) -> bool {
        self.modals.is_empty()
    }

    pub fn len(&self) -> usize {
        self.modals.len()
    }

    /// Total height (rows) all stacked modals want at the given width.
    pub fn total_height(&self, width: u16) -> u16 {
        self.modals.iter().map(|m| m.height(width)).sum()
    }

    /// Render the stack into `area`, top-to-bottom in push order.
    pub fn render(&mut self, area: Rect, frame: &mut Frame) {
        let width = area.width;
        let mut y = area.y;
        for modal in self.modals.iter_mut() {
            let h = modal.height(width).min(area.bottom().saturating_sub(y));
            if h == 0 {
                break;
            }
            let slot = Rect {
                x: area.x,
                y,
                width,
                height: h,
            };
            modal.render(slot, frame);
            y = y.saturating_add(h);
        }
    }

    /// Route a key to the top-of-stack modal. If it dismisses, pop it and
    /// return the effect (if any) it produced on the way out.
    pub fn handle_key(&mut self, key: KeyEvent) -> (ModalResult, Option<ModalEffect>) {
        let Some(top) = self.modals.last_mut() else {
            return (ModalResult::Forward, None);
        };
        let result = top.handle_key(key);
        let effect = match result {
            ModalResult::Dismiss => {
                let mut popped = self.modals.pop().expect("just observed last_mut");
                popped.take_effect()
            }
            _ => None,
        };
        (result, effect)
    }

    /// Broadcast a composer-text change to every modal. The stack is small,
    /// so iterating all of them is fine.
    pub fn on_composer_change(&mut self, text: &str) {
        for modal in self.modals.iter_mut() {
            modal.on_composer_change(text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyModifiers};

    struct DummyModal {
        h: u16,
        dismiss_next: bool,
        effect: Option<String>,
    }

    impl Modal for DummyModal {
        fn height(&self, _w: u16) -> u16 {
            self.h
        }
        fn render(&mut self, _area: Rect, _frame: &mut Frame) {}
        fn handle_key(&mut self, _key: KeyEvent) -> ModalResult {
            if self.dismiss_next {
                ModalResult::Dismiss
            } else {
                ModalResult::Consumed
            }
        }
        fn take_effect(&mut self) -> Option<ModalEffect> {
            self.effect.take().map(ModalEffect::ReplaceComposer)
        }
    }

    fn dummy(h: u16, dismiss_next: bool, effect: Option<&str>) -> Box<dyn Modal> {
        Box::new(DummyModal {
            h,
            dismiss_next,
            effect: effect.map(String::from),
        })
    }

    fn key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)
    }

    #[test]
    fn total_height_sums_modal_heights() {
        let mut stack = ModalStack::default();
        stack.push(dummy(3, false, None));
        stack.push(dummy(7, false, None));
        assert_eq!(stack.total_height(80), 10);
        assert_eq!(stack.len(), 2);
    }

    #[test]
    fn dismiss_pops_top_and_returns_effect() {
        let mut stack = ModalStack::default();
        stack.push(dummy(3, false, None));
        stack.push(dummy(3, true, Some("/help")));
        let (result, effect) = stack.handle_key(key('x'));
        assert!(matches!(result, ModalResult::Dismiss));
        assert!(matches!(effect, Some(ModalEffect::ReplaceComposer(s)) if s == "/help"));
        assert_eq!(stack.len(), 1);
    }

    #[test]
    fn forward_when_empty() {
        let mut stack = ModalStack::default();
        let (result, effect) = stack.handle_key(key('x'));
        assert!(matches!(result, ModalResult::Forward));
        assert!(effect.is_none());
    }
}
