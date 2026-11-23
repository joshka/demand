use std::io;

use console::{Key, Term};

use crate::host::Host;
use crate::prompt::{CliOptions, KeyPress, Outcome, Prompt};
use crate::theme;
use crate::theme::Theme;
use crate::view::{Size, View};

#[derive(Clone, Debug, Default, PartialEq)]
/// A button to select in a dialog
pub struct DialogButton {
    /// The text to display for the option
    pub label: String,
    /// The key to press to select the option
    pub key: char,
}

impl DialogButton {
    /// Create a new button with the given label.
    /// The key will be the first character of the label, lowercased.
    pub fn new(label: &str) -> Self {
        let label = label.to_string();
        let key = label.to_lowercase().chars().next().unwrap();
        Self { label, key }
    }
    /// Create a new button with the given label and key.
    pub fn with_key(label: &str, key: char) -> Self {
        let label = label.to_string();
        Self { label, key }
    }
}

/// A multi-button dialog
///
/// # Example
/// ```rust
/// use demand::Dialog;
/// use demand::DialogButton;
///
/// let dialog = Dialog::new("Are you sure?")
///   .description("This will do a thing.")
///   .buttons(vec![
///      DialogButton::new("Ok"),
///      DialogButton::new("Not sure"),
///      DialogButton::new("Cancel"),
///   ]);
/// let choice = match dialog.run() {
///   Ok(value) => value,
///   Err(e) => {
///       if (e.kind() == std::io::ErrorKind::Interrupted) {
///           println!("Dialog cancelled");
///           return;
///       } else {
///           panic!("Error: {}", e);
///       }
///   }
/// };
/// ```
pub struct Dialog<'a> {
    /// The title of the dialog
    pub title: String,
    /// The colors/style of the dialog
    pub theme: &'a Theme,
    /// A description to display after the title
    pub description: String,
    /// The buttons to display to the user
    pub buttons: Vec<DialogButton>,

    clear_screen: bool,
    selected_button_idx: usize,
}

impl<'a> Dialog<'a> {
    /// Create a new dialog with the given title
    ///
    /// By default, the dialog will have a single "Ok" button and a "Cancel" button.
    pub fn new<S: Into<String>>(title: S) -> Self {
        Self {
            title: title.into(),
            description: String::new(),
            theme: &*theme::DEFAULT,
            buttons: vec![DialogButton::new("Ok"), DialogButton::new("Cancel")],
            clear_screen: false,
            selected_button_idx: 0,
        }
    }

    /// Set the description of the dialog
    pub fn description(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    /// Set the buttons of the dialog
    pub fn buttons(mut self, buttons: Vec<DialogButton>) -> Self {
        self.buttons = buttons;
        self
    }

    /// Set the index of the initially selected button.
    ///
    /// The `idx` is the index of the button in the `buttons` vector and is 0-indexed.
    ///
    /// # Errors
    ///
    /// This will panic if there are no buttons to select or if the index is out of bounds.
    pub fn selected_button(mut self, idx: usize) -> Self {
        if self.buttons.is_empty() {
            panic!("No buttons to select");
        }
        if idx >= self.buttons.len() {
            panic!("Selected button index out of bounds");
        }
        self.selected_button_idx = idx;
        self
    }

    /// Set the theme of the dialog
    pub fn theme(mut self, theme: &'a Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Clear the screen before the dialog is (re-)rendered
    pub fn clear_screen(mut self, clear: bool) -> Self {
        self.clear_screen = clear;
        self
    }

    /// Displays the dialog to the user and returns their response.
    ///
    /// The response will be the label of the selected button.
    ///
    /// This function will block until the user submits the input. If the user cancels the input,
    /// an error of type `io::ErrorKind::Interrupted` is returned.
    pub fn run(mut self) -> io::Result<String> {
        Host::new(Term::stderr()).run(&mut self, |dialog| {
            Some(dialog.selected_label().unwrap_or_default().to_string())
        })
    }

    /// Handle a key, for a prompt that's part of a larger interface rather
    /// than shown with [`run`](Self::run).
    pub fn handle_key(&mut self, key: impl Into<KeyPress>) -> Outcome {
        self.on_key(key.into().0)
    }

    /// The label of the selected button, if there are any buttons.
    pub fn selected_label(&self) -> Option<&str> {
        self.buttons
            .get(self.selected_button_idx)
            .map(|button| button.label.as_str())
    }

    fn handle_left(&mut self) {
        self.selected_button_idx =
            (self.selected_button_idx + self.buttons.len() - 1) % self.buttons.len();
    }

    fn handle_right(&mut self) {
        self.selected_button_idx = (self.selected_button_idx + 1) % self.buttons.len();
    }

    fn render(&self) -> View {
        let mut out = View::default();

        out.set_color(&self.theme.title);
        writeln!(out, "{}", self.title);

        if !self.description.is_empty() {
            out.set_color(&self.theme.description);
            write!(out, "{}", self.description);
        }

        writeln!(out, "\n");

        out.mark_focus();
        for (i, button) in self.buttons.iter().enumerate() {
            write!(out, " ");
            if self.selected_button_idx == i {
                out.set_color(&self.theme.focused_button);
            } else {
                out.set_color(&self.theme.blurred_button);
            }
            write!(out, "  {}  ", button.label);
            out.reset();
        }

        writeln!(out, "\n");

        let mut help_keys = vec![("←/→", "toggle")];
        let button_keys = self
            .buttons
            .clone()
            .iter()
            .fold(String::new(), |mut output, button| {
                output.push_str(button.key.to_string().as_str());
                output.push('/');
                output
            });
        let submit_keys = format!("{button_keys}enter");
        help_keys.push((&submit_keys, "submit"));
        out.write_help(self.theme, &help_keys, false);
        writeln!(out);

        out.reset();
        out
    }

    fn render_success(&self) -> View {
        let mut out = View::default();
        out.set_color(&self.theme.title);
        write!(out, "{}", self.title);
        out.set_color(&self.theme.selected_option);
        writeln!(
            out,
            " {}",
            if !self.buttons.is_empty() {
                self.buttons[self.selected_button_idx].label.clone()
            } else {
                "".to_string()
            }
        );
        out.reset();
        out
    }
}

impl Prompt for Dialog<'_> {
    fn on_key(&mut self, key: Key) -> Outcome {
        match key {
            Key::ArrowLeft | Key::Char('h') if !self.buttons.is_empty() => self.handle_left(),
            Key::ArrowRight | Key::Char('l') if !self.buttons.is_empty() => self.handle_right(),
            Key::Char(c) => {
                let Some(idx) = self.buttons.iter().position(|b| b.key == c) else {
                    return Outcome::Ignored;
                };
                self.selected_button_idx = idx;
                return Outcome::Submit;
            }
            Key::Enter => return Outcome::Submit,
            Key::Escape => return Outcome::Cancel,
            _ => return Outcome::Ignored,
        }
        Outcome::Handled
    }

    fn view(&mut self, _size: Size) -> View {
        self.render()
    }

    fn success(&mut self) -> View {
        self.render_success()
    }

    fn cli(&self) -> CliOptions {
        CliOptions {
            clear_screen: self.clear_screen,
            clear_on_cancel: false,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test::without_ansi;
    #[cfg(unix)]
    use crate::test::{Parser, capture_term, replay, snapshot};
    use indoc::indoc;

    #[test]
    fn test_render() {
        let dialog = Dialog::new("Are you sure?")
            .description("This will do a thing.")
            .buttons(vec![
                DialogButton::new("Ok"),
                DialogButton::new("Not sure"),
                DialogButton::new("Cancel"),
            ]);

        assert_eq!(
            indoc! {
              "Are you sure?
            This will do a thing.

               Ok     Not sure     Cancel  

            ←/→ toggle • o/n/c/enter submit
            "
            },
            without_ansi(dialog.render().to_ansi().as_str())
        );
    }
    /// Regression for jdx/demand#7: help text or a description wider than
    /// the terminal wraps, and redrawing must clear every wrapped row or
    /// the prompt stacks a copy of itself on each keypress.
    #[cfg(unix)]
    #[test]
    fn a_wrapped_description_does_not_stack_copies_of_the_frame() {
        let (term, buf) = capture_term();
        let mut prompt = Dialog::new("Proceed?").description(&"d".repeat(200));
        let mut host = Host::new(term);
        let width = host.term.size().1 as usize;
        assert!(
            width < 200,
            "test needs a description wider than the terminal"
        );

        for _ in 0..3 {
            host.draw(&mut prompt).unwrap();
            prompt.handle_right();
        }

        let mut parser = Parser::new(24, width as u16, 0);
        replay(&mut parser, &snapshot(&buf));
        let screen = parser.screen().contents();
        assert_eq!(
            screen.matches("Proceed?").count(),
            1,
            "prompt drawn more than once:\n{screen}"
        );
    }
}
