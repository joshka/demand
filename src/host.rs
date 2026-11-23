use std::io::{self, Write};

use console::Term;

use crate::ctrlc;
use crate::event::{Event, EventReader};
use crate::frame::Frame;
use crate::prompt::{Outcome, Prompt};
use crate::view::Size;

/// Shows prompts on the command line. This is the only part of an
/// interactive prompt that touches the terminal.
pub(crate) struct Host {
    pub(crate) term: Term,
    pub(crate) frame: Frame,
    /// Rows between where the cursor is parked and the end of the frame, so
    /// it can be moved back down. Recorded rather than recomputed: by the
    /// time it's needed the next view has been laid out.
    rows_below_cursor: usize,
}

impl Host {
    pub(crate) fn new(term: Term) -> Self {
        Self {
            term,
            frame: Frame::default(),
            rows_below_cursor: 0,
        }
    }

    /// Show `prompt` until it's answered, and return what `accept` makes of
    /// the answer. The prompt stays up while that's `None`.
    ///
    /// If the user cancels, an error of type `io::ErrorKind::Interrupted`
    /// is returned.
    pub(crate) fn run<P: Prompt, T>(
        mut self,
        prompt: &mut P,
        mut accept: impl FnMut(&mut P) -> Option<T>,
    ) -> io::Result<T> {
        let ctrlc_handle = ctrlc::show_cursor_after_ctrlc(&self.term)?;
        let mut events = EventReader::with_updates(prompt.updates())?;
        let mut reset_viewport = false;

        self.term.hide_cursor()?;
        loop {
            let term = self.term.clone();
            crate::synchronized_output::run(&term, || {
                if self.frame.is_empty() && prompt.cli().clear_line {
                    self.term.clear_line()?;
                }
                if reset_viewport {
                    self.reset()?;
                    reset_viewport = false;
                }
                self.draw(prompt)
            })?;
            let key = match events.read(&self.term)? {
                Event::Key(key) => key,
                Event::Resize => {
                    reset_viewport = true;
                    continue;
                }
                Event::Update => continue,
            };
            match prompt.on_key(key) {
                Outcome::Ignored | Outcome::Handled => {}
                Outcome::Submit => {
                    let Some(answer) = accept(prompt) else {
                        continue;
                    };
                    let output = prompt.success().to_ansi();
                    crate::synchronized_output::run(&term, || {
                        self.clear(prompt)?;
                        self.term.show_cursor()?;
                        self.term.write_all(output.as_bytes())?;
                        self.term.clear_to_end_of_screen()
                    })?;
                    ctrlc_handle.close();
                    return Ok(answer);
                }
                Outcome::Edit => {
                    self.clear(prompt)?;
                    self.term.show_cursor()?;
                    prompt.edit();
                    self.term.hide_cursor()?;
                }
                Outcome::Cancel => {
                    if prompt.cli().clear_on_cancel {
                        self.clear(prompt)?;
                    }
                    self.term.show_cursor()?;
                    ctrlc_handle.close();
                    return Err(io::Error::new(io::ErrorKind::Interrupted, "user cancelled"));
                }
            }
        }
    }

    /// Draw `prompt` over the previous frame. A frame identical to the one
    /// on screen leaves everything, cursor included, where it is.
    pub(crate) fn draw(&mut self, prompt: &mut impl Prompt) -> io::Result<()> {
        let (rows, width) = self.term.size();
        let (rows, width) = (rows as usize, width as usize);
        let view = prompt.view(Size { rows, width });
        let output = view.to_ansi();
        let cli = prompt.cli();
        if cli.clear_screen {
            self.clear(prompt)?;
        }
        if self.frame.is_current(&self.term, &output) {
            return Ok(());
        }
        if self.frame.resized(&self.term) {
            // The terminal has reflowed the frame's rows, so their count at
            // the old width can't be used to find it.
            self.reset()?;
        } else {
            self.cursor_to_end()?;
        }
        self.frame.update(&self.term, output)?;
        if cli.park_cursor
            && let Some((row, col)) = view.layout(width).caret
        {
            let up = self.frame.height(&self.term).saturating_sub(row);
            if up > 0 {
                self.term.move_cursor_up(up)?;
            }
            self.term.move_cursor_left(usize::MAX)?;
            // Cursor movement stops at the right edge rather than
            // wrapping, which is why the row had to be picked explicitly.
            if col > 0 {
                self.term.move_cursor_right(col)?;
            }
            self.rows_below_cursor = up;
        }
        Ok(())
    }

    pub(crate) fn clear(&mut self, prompt: &impl Prompt) -> io::Result<()> {
        if !self.frame.is_empty() {
            self.cursor_to_end()?;
            self.frame.clear(&self.term)?;
        }
        if prompt.cli().clear_screen {
            self.term.clear_screen()?;
        }
        Ok(())
    }

    fn cursor_to_end(&mut self) -> io::Result<()> {
        if self.rows_below_cursor > 0 {
            self.term.move_cursor_down(self.rows_below_cursor)?;
        }
        self.rows_below_cursor = 0;
        Ok(())
    }

    fn reset(&mut self) -> io::Result<()> {
        self.term.clear_screen()?;
        self.frame.forget();
        self.rows_below_cursor = 0;
        Ok(())
    }
}
