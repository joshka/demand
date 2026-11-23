use std::io;

use console::{Key, Term};

use crate::host::Host;
use crate::prompt::{CliOptions, Outcome, Prompt};
use crate::theme;
use crate::theme::Theme;
use crate::view::{Size, View};

/// A confirmation dialog
///
/// # Example
/// ```rust
/// use demand::Confirm;
///
/// let confirm = Confirm::new("Are you sure?")
///   .affirmative("Yes!")
///   .negative("No.");
/// let choice = match confirm.run() {
///     Ok(confirm) => confirm,
///     Err(e) => {
///         if e.kind() == std::io::ErrorKind::Interrupted {
///             println!("Dialog cancelled");
///             false
///         } else {
///             panic!("Error: {}", e);
///         }
///     }
/// };
/// ```
pub struct Confirm<'a> {
    /// The title of the dialog
    pub title: String,
    /// The colors/style of the dialog
    pub theme: &'a Theme,
    /// A description to display after the title
    pub description: String,
    /// The text to display for the affirmative option
    pub affirmative: String,
    /// The text to display for the negative option
    pub negative: String,
    /// If true, the affirmative option is selected by default
    pub selected: bool,

    clear_screen: bool,
}

impl<'a> Confirm<'a> {
    /// Create a new confirmation dialog with the given title
    pub fn new<S: Into<String>>(title: S) -> Self {
        Self {
            title: title.into(),
            description: String::new(),
            theme: &*theme::DEFAULT,
            affirmative: "Yes".to_string(),
            negative: "No".to_string(),
            selected: true,
            clear_screen: false,
        }
    }

    /// Calculate hint strings and shortcut characters for affirmative/negative options
    /// Returns (affirmative_hint, negative_hint, affirmative_char, negative_char, has_conflict)
    fn calculate_hints(&self) -> (String, String, char, char, bool) {
        let affirmative_lower = self.affirmative.to_lowercase();
        let negative_lower = self.negative.to_lowercase();

        let affirmative_char = affirmative_lower.chars().next().unwrap_or('y');
        let negative_char = negative_lower.chars().next().unwrap_or('n');

        let has_conflict = affirmative_char == negative_char;

        // Find minimum unique prefixes when first characters are the same
        let (affirmative_hint, negative_hint) = if has_conflict {
            // Find shortest unique prefixes
            let mut prefix_len = 1;
            loop {
                let aff_prefix: String = affirmative_lower.chars().take(prefix_len).collect();
                let neg_prefix: String = negative_lower.chars().take(prefix_len).collect();

                if aff_prefix != neg_prefix
                    || prefix_len >= affirmative_lower.len().max(negative_lower.len())
                {
                    break (aff_prefix, neg_prefix);
                }
                prefix_len += 1;
            }
        } else {
            (affirmative_char.to_string(), negative_char.to_string())
        };

        (
            affirmative_hint,
            negative_hint,
            affirmative_char,
            negative_char,
            has_conflict,
        )
    }

    /// Set the description of the dialog
    pub fn description(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    /// Set the label of the affirmative option
    pub fn affirmative<S: Into<String>>(mut self, affirmative: S) -> Self {
        self.affirmative = affirmative.into();
        self
    }

    /// Set the label of the negative option
    pub fn negative<S: Into<String>>(mut self, negative: S) -> Self {
        self.negative = negative.into();
        self
    }

    /// Set whether the affirmative option is selected by default
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
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

    /// Displays the dialog to the user and returns their response
    ///
    /// This function will block until the user submits the input. If the user cancels the input,
    /// an error of type `io::ErrorKind::Interrupted` is returned.
    pub fn run(mut self) -> io::Result<bool> {
        // If not a TTY (e.g., piped input or non-interactive environment),
        // write a simple prompt and read from stdin
        if !crate::tty::is_tty() {
            let (affirmative_hint, negative_hint, _, _, _) = self.calculate_hints();
            let affirmative_lower = self.affirmative.to_lowercase();
            let negative_lower = self.negative.to_lowercase();

            let prompt = format!(
                "{} / {} [{}/{}]: ",
                self.affirmative, self.negative, affirmative_hint, negative_hint
            );

            crate::tty::write_prompt(&self.title, &self.description, &prompt)?;
            let input = crate::tty::read_line()?.trim().to_lowercase();

            // Parse response
            if input.is_empty() {
                // Empty input uses default
                return Ok(self.selected);
            }

            // Check for exact prefix matches first (more specific)
            let aff_matches = affirmative_lower.starts_with(&input);
            let neg_matches = negative_lower.starts_with(&input);

            if aff_matches && !neg_matches {
                return Ok(true);
            } else if neg_matches && !aff_matches {
                return Ok(false);
            } else if aff_matches && neg_matches {
                // Ambiguous - require more characters
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Ambiguous input: please use at least '{}/{}' to disambiguate",
                        affirmative_hint, negative_hint
                    ),
                ));
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Invalid input: expected {}/{}",
                        affirmative_hint, negative_hint
                    ),
                ));
            }
        }

        Host::new(Term::stderr()).run(&mut self, |confirm| Some(confirm.selected))
    }

    fn handle_left(&mut self) {
        if !self.selected {
            self.selected = true;
        }
    }

    fn handle_right(&mut self) {
        if self.selected {
            self.selected = false;
        }
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

        write!(out, " ");
        if self.selected {
            out.set_color(&self.theme.focused_button);
        } else {
            out.set_color(&self.theme.blurred_button);
        }
        write!(out, "  {}  ", self.affirmative);
        out.reset();
        write!(out, " ");
        if self.selected {
            out.set_color(&self.theme.blurred_button);
        } else {
            out.set_color(&self.theme.focused_button);
        }
        write!(out, "  {}  ", self.negative);
        out.reset();
        writeln!(out, "\n");

        let mut help_keys = vec![("←/→", "toggle")];
        let (affirmative_hint, negative_hint, _, _, has_conflict) = self.calculate_hints();
        let submit_keys = if has_conflict {
            "enter".to_string()
        } else {
            format!("{}/{}/enter", affirmative_hint, negative_hint)
        };
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
        if self.selected {
            writeln!(out, " {}", self.affirmative);
        } else {
            writeln!(out, " {}", self.negative);
        }
        out.reset();
        out
    }
}

impl Prompt for Confirm<'_> {
    fn on_key(&mut self, key: Key) -> Outcome {
        let (_, _, affirmative_char, negative_char, has_conflict) = self.calculate_hints();
        match key {
            Key::ArrowLeft | Key::Char('h') => self.handle_left(),
            Key::ArrowRight | Key::Char('l') => self.handle_right(),
            Key::Char(c) if !has_conflict && c == affirmative_char => {
                self.selected = true;
                return Outcome::Submit;
            }
            Key::Char(c) if !has_conflict && c == negative_char => {
                self.selected = false;
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
            clear_line: true,
            clear_screen: self.clear_screen,
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
    use insta::assert_snapshot;

    #[test]
    fn test_render() {
        let confirm = Confirm::new("Are you sure?")
            .description("This will do a thing.")
            .affirmative("Yes!")
            .negative("No.");

        let rendered = confirm.render().to_ansi();
        let actual = without_ansi(rendered.as_str());
        assert_snapshot!(actual.as_ref(), @r"
        Are you sure?
        This will do a thing.

           Yes!     No.  

        ←/→ toggle • y/n/enter submit
        ");
    }

    #[test]
    fn test_render_custom_labels_with_conflict() {
        let confirm = Confirm::new("Deploy to production?")
            .affirmative("Confirm")
            .negative("Cancel");

        let rendered = confirm.render().to_ansi();
        let actual = without_ansi(rendered.as_str());
        assert_snapshot!(actual.as_ref(), @r"
        Deploy to production?


           Confirm     Cancel  

        ←/→ toggle • enter submit
        ");
    }

    #[test]
    fn test_render_custom_labels_no_conflict() {
        let confirm = Confirm::new("Delete file?")
            .affirmative("Proceed")
            .negative("Abort");

        let rendered = confirm.render().to_ansi();
        let actual = without_ansi(rendered.as_str());
        assert_snapshot!(actual.as_ref(), @r"
        Delete file?


           Proceed     Abort  

        ←/→ toggle • p/a/enter submit
        ");
    }

    #[test]
    fn test_calculate_hints_no_conflict() {
        let confirm = Confirm::new("Test").affirmative("Yes").negative("No");
        let (aff_hint, neg_hint, aff_char, neg_char, has_conflict) = confirm.calculate_hints();

        assert_eq!(aff_hint, "y");
        assert_eq!(neg_hint, "n");
        assert_eq!(aff_char, 'y');
        assert_eq!(neg_char, 'n');
        assert!(!has_conflict);
    }

    #[test]
    fn test_calculate_hints_with_conflict() {
        let confirm = Confirm::new("Test")
            .affirmative("Confirm")
            .negative("Cancel");
        let (aff_hint, neg_hint, aff_char, neg_char, has_conflict) = confirm.calculate_hints();

        assert_eq!(aff_hint, "co");
        assert_eq!(neg_hint, "ca");
        assert_eq!(aff_char, 'c');
        assert_eq!(neg_char, 'c');
        assert!(has_conflict);
    }

    #[test]
    fn test_calculate_hints_identical_prefixes() {
        let confirm = Confirm::new("Test")
            .affirmative("Complete")
            .negative("Completed");
        let (aff_hint, neg_hint, _, _, has_conflict) = confirm.calculate_hints();

        // Should find minimum unique prefix
        assert_eq!(aff_hint, "complete");
        assert_eq!(neg_hint, "completed");
        assert!(has_conflict);
    }
    /// Regression for jdx/demand#7: help text or a description wider than
    /// the terminal wraps, and redrawing must clear every wrapped row or
    /// the prompt stacks a copy of itself on each keypress.
    #[cfg(unix)]
    #[test]
    fn a_wrapped_description_does_not_stack_copies_of_the_frame() {
        let (term, buf) = capture_term();
        let mut prompt = Confirm::new("Proceed?").description(&"d".repeat(200));
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
