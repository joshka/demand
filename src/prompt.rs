use console::Key;

use crate::handle::Updates;
use crate::view::{Size, View};

/// What a prompt did with a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// The prompt has no use for the key.
    Ignored,
    /// The prompt handled the key and is still being answered.
    Handled,
    /// The prompt has been answered.
    Submit,
    /// The user dismissed the prompt.
    Cancel,
    /// The prompt needs the terminal to itself.
    Edit,
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
