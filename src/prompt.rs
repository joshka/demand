use console::Key;

use crate::handle::Updates;
use crate::view::{Size, View};

/// What a prompt did with a key.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The prompt has no use for the key.
    Ignored,
    /// The prompt handled the key and is still being answered.
    Handled,
    /// The prompt has been answered.
    Submit,
    /// The user dismissed the prompt.
    Cancel,
    /// The prompt needs the terminal to itself. Restore it, call
    /// [`Editor::edit`](crate::Editor::edit), and take it over again.
    /// Only an [`Editor`](crate::Editor) asks for this.
    Edit,
}

/// A key press for a prompt to handle.
///
/// Converts from [`console::Key`] and, with the `crossterm` feature, from
/// crossterm's `KeyEvent`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPress(pub(crate) Key);

impl From<Key> for KeyPress {
    fn from(key: Key) -> Self {
        Self(key)
    }
}

#[cfg(feature = "crossterm")]
impl From<crossterm::event::KeyEvent> for KeyPress {
    fn from(event: crossterm::event::KeyEvent) -> Self {
        use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

        if event.kind == KeyEventKind::Release {
            return Self(Key::Unknown);
        }
        let ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
        let alt = event.modifiers.contains(KeyModifiers::ALT);
        // The same keys a terminal's escape sequences are read as.
        Self(match event.code {
            // AltGr is reported as ctrl+alt, and types a character.
            KeyCode::Char(c) if ctrl && !alt => match c.to_ascii_lowercase() {
                'a' => Key::Home,
                'c' => Key::CtrlC,
                'e' => Key::End,
                'h' => Key::Backspace,
                'i' => Key::Tab,
                'j' | 'm' => Key::Enter,
                c @ 'a'..='z' => Key::Char((c as u8 - b'a' + 1) as char),
                _ => Key::Unknown,
            },
            KeyCode::Char(c) if alt && !ctrl => Key::UnknownEscSeq(vec![c]),
            KeyCode::Char(c) => Key::Char(c),
            KeyCode::Backspace if alt => Key::UnknownEscSeq(vec!['\x7f']),
            KeyCode::Backspace => Key::Backspace,
            KeyCode::Enter => Key::Enter,
            KeyCode::Left => Key::ArrowLeft,
            KeyCode::Right => Key::ArrowRight,
            KeyCode::Up => Key::ArrowUp,
            KeyCode::Down => Key::ArrowDown,
            KeyCode::Home => Key::Home,
            KeyCode::End => Key::End,
            KeyCode::PageUp => Key::PageUp,
            KeyCode::PageDown => Key::PageDown,
            KeyCode::Tab => Key::Tab,
            KeyCode::BackTab => Key::BackTab,
            KeyCode::Delete => Key::Del,
            KeyCode::Insert => Key::Insert,
            KeyCode::Esc => Key::Escape,
            _ => Key::Unknown,
        })
    }
}

/// What every prompt provides to be hosted.
pub(crate) trait Prompt {
    /// React to a key.
    fn on_key(&mut self, key: Key) -> Outcome;

    /// Lay the prompt out for `size`, and draw it.
    fn view(&mut self, size: Size) -> View;

    /// The line left behind once the prompt has been answered.
    fn success(&mut self) -> View;

    /// Take over the terminal, after [`Outcome::Edit`].
    fn edit(&mut self) {}

    /// How the CLI host shows the prompt.
    fn cli(&self) -> CliOptions {
        CliOptions::default()
    }

    /// Updates from other threads to wake up for.
    fn updates(&self) -> Option<&Updates> {
        None
    }
}

/// Where prompts differ in how the CLI host shows them.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CliOptions {
    /// Start the first frame on a clean line.
    pub clear_line: bool,
    /// Clear the screen before every frame.
    pub clear_screen: bool,
    /// Erase the prompt when it's cancelled.
    pub clear_on_cancel: bool,
    /// Park the terminal's cursor on the text caret.
    pub park_cursor: bool,
}

impl Default for CliOptions {
    fn default() -> Self {
        Self {
            clear_line: false,
            clear_screen: false,
            clear_on_cancel: true,
            park_cursor: false,
        }
    }
}
