use std::io;

use crate::prompt::Outcome;
use crate::theme::Theme;
use crate::view::View;
use crate::{ctrlc, theme};
use console::{Key, Term};

/// Navigation result from a wizard section
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Navigation {
    /// Go to the next section
    Next,
    /// Go to the previous section
    Back,
    /// Jump to a specific section by index
    Jump(usize),
    /// Return to the hub (section 0) - for hub-and-spoke patterns
    Hub,
    /// Stay on the current section (re-render)
    Stay,
    /// Complete the wizard
    Done,
}

/// Type alias for the section run function
pub type SectionFn<'a, S> = Box<dyn Fn(&mut S, &Theme) -> io::Result<Navigation> + 'a>;

/// A section in a wizard flow
pub struct Section<'a, S> {
    /// The label shown in the breadcrumb
    pub label: String,
    /// The function to run when this section is active
    run: SectionFn<'a, S>,
}

impl<'a, S> Section<'a, S> {
    /// Create a new section with a label and run function
    pub fn new<L, F>(label: L, run: F) -> Self
    where
        L: Into<String>,
        F: Fn(&mut S, &Theme) -> io::Result<Navigation> + 'a,
    {
        Self {
            label: label.into(),
            run: Box::new(run),
        }
    }
}

/// A multi-step wizard with clickable breadcrumb navigation
///
/// # Example
/// ```ignore
/// use demand::{Wizard, Section, Navigation, Input};
///
/// #[derive(Default)]
/// struct State {
///     name: String,
///     email: String,
/// }
///
/// let result = Wizard::new("Setup")
///     .section("Name", |state: &mut State, theme| {
///         let name = Input::new("Your name")
///             .theme(theme)
///             .run()?;
///         state.name = name;
///         Ok(Navigation::Next)
///     })
///     .section("Email", |state: &mut State, theme| {
///         let email = Input::new("Your email")
///             .theme(theme)
///             .run()?;
///         state.email = email;
///         Ok(Navigation::Next)
///     })
///     .section("Done", |_state, _theme| {
///         Ok(Navigation::Done)
///     })
///     .run(&mut State::default())?;
/// ```
pub struct Wizard<'a, S> {
    steps: Steps<'a>,
    /// The sections in the wizard, one for each step
    sections: Vec<Section<'a, S>>,
}

impl<'a, S> Wizard<'a, S> {
    /// Create a new wizard with the given title
    pub fn new<T: Into<String>>(title: T) -> Self {
        Self {
            steps: Steps::new(title),
            sections: Vec::new(),
        }
    }

    /// Add a section to the wizard
    pub fn section<L, F>(mut self, label: L, run: F) -> Self
    where
        L: Into<String>,
        F: Fn(&mut S, &Theme) -> io::Result<Navigation> + 'a,
    {
        let section = Section::new(label, run);
        self.steps = self.steps.step(section.label.clone());
        self.sections.push(section);
        self
    }

    /// Set the theme for the wizard
    pub fn theme(mut self, theme: &'a Theme) -> Self {
        self.steps.theme = theme;
        self
    }

    /// Run the wizard with the given state
    ///
    /// Returns the final state when Navigation::Done is received.
    /// Returns an error with `io::ErrorKind::Interrupted` if the user cancels.
    pub fn run(mut self, state: &mut S) -> io::Result<()> {
        if self.sections.is_empty() {
            return Ok(());
        }

        let term = Term::stderr();
        let ctrlc_handle = ctrlc::show_cursor_after_ctrlc(&term)?;

        let result = loop {
            let section = &self.sections[self.steps.current()];
            match (section.run)(state, self.steps.theme) {
                Ok(navigation) => {
                    if self.steps.navigate(navigation) == Outcome::Submit {
                        break Ok(());
                    }
                }
                // Escape pressed - go back if possible, otherwise cancel
                Err(e) if e.kind() == io::ErrorKind::Interrupted && self.steps.back() => {}
                Err(e) => break Err(e),
            }
        };
        term.show_cursor()?;
        ctrlc_handle.close();
        result
    }

    /// Render the breadcrumb navigation bar as a string.
    ///
    /// This method is available for consumers who want to display the breadcrumb
    /// in custom UI layouts. Note that sections receive only `(&mut S, &Theme)`
    /// and don't have direct access to the Wizard instance.
    ///
    /// Returns the rendered breadcrumb as a string that can be printed.
    pub fn render_breadcrumb(&self) -> io::Result<String> {
        let width = Term::stderr().size().1 as usize;
        Ok(self.steps.view(width).to_ansi())
    }
}

/// The steps of a multi-step flow, where the user is in them, and a
/// breadcrumb showing it.
///
/// [`Wizard`] runs one prompt after another on the command line. In a
/// larger interface, where prompts are shown by the interface instead, this
/// keeps track of which one it's the turn of.
///
/// # Example
/// ```rust
/// use demand::{Navigation, Outcome, Steps};
///
/// let mut steps = Steps::new("Setup").step("Name").step("Tools");
/// assert_eq!(steps.navigate(Navigation::Next), Outcome::Handled);
/// assert_eq!(steps.current(), 1);
/// assert!(steps.back());
/// // There's nothing before the first step.
/// assert!(!steps.back());
/// ```
pub struct Steps<'a> {
    /// The title displayed above the breadcrumb
    pub title: String,
    /// The colors/style of the breadcrumb
    pub theme: &'a Theme,
    labels: Vec<String>,
    current: usize,
    /// Which steps have been visited
    visited: Vec<bool>,
}

impl<'a> Steps<'a> {
    /// Create steps with the given title
    pub fn new<T: Into<String>>(title: T) -> Self {
        Self {
            title: title.into(),
            theme: &theme::DEFAULT,
            labels: Vec::new(),
            current: 0,
            visited: Vec::new(),
        }
    }

    /// Add a step
    pub fn step(mut self, label: impl Into<String>) -> Self {
        self.labels.push(label.into());
        self.visited.push(self.visited.is_empty());
        self
    }

    /// Set the theme of the breadcrumb
    pub fn theme(mut self, theme: &'a Theme) -> Self {
        self.theme = theme;
        self
    }

    /// The index of the current step
    pub fn current(&self) -> usize {
        self.current
    }

    /// Move to another step. [`Outcome::Submit`] means the steps are done.
    pub fn navigate(&mut self, navigation: Navigation) -> Outcome {
        match navigation {
            Navigation::Next => self.go_to(self.current + 1),
            Navigation::Back => {
                self.back();
            }
            Navigation::Jump(step) => self.go_to(step),
            Navigation::Hub => self.current = 0,
            Navigation::Stay => {}
            Navigation::Done => return Outcome::Submit,
        }
        Outcome::Handled
    }

    /// Move to the previous step, if there is one.
    pub fn back(&mut self) -> bool {
        if self.current == 0 {
            return false;
        }
        // Marks it as visited too, in case it was jumped over.
        self.go_to(self.current - 1);
        true
    }

    fn go_to(&mut self, step: usize) {
        if step < self.labels.len() {
            self.current = step;
            self.visited[step] = true;
        }
    }

    pub(crate) fn view(&self, width: usize) -> View {
        let mut out = View::default();

        // Title
        out.set_color(&self.theme.title);
        writeln!(out, "{}", self.title);

        // Breadcrumb: [1:Section] > 2:Section > 3:Section
        for (i, label) in self.labels.iter().enumerate() {
            if i > 0 {
                out.set_color(&self.theme.description);
                write!(out, "{}", self.theme.breadcrumb_separator);
            }
            if i == self.current {
                out.mark_focus();
                out.set_color(&self.theme.breadcrumb_active);
                write!(out, "[{}:{label}]", i + 1);
            } else if self.visited[i] {
                out.set_color(&self.theme.breadcrumb_clickable);
                write!(out, "{}:{label}", i + 1);
            } else {
                out.set_color(&self.theme.breadcrumb_future);
                write!(out, "{}:{label}", i + 1);
            }
        }
        writeln!(out);

        // Separator line
        out.set_color(&self.theme.description);
        writeln!(out, "{}", "─".repeat(width.min(50)));
        writeln!(out);

        out.reset();
        out
    }
}

/// Helper to check if a key is a navigation key and return the navigation action.
///
/// This utility function is provided for library consumers who want to implement
/// custom widgets that support wizard navigation shortcuts. It checks for:
/// - Number keys (1-9): Jump to that section
/// - Escape: Go back to the previous section (returns `None` if at first section)
///
/// # Example
/// ```ignore
/// use demand::{handle_navigation_key, Navigation};
/// use console::Key;
///
/// // In your custom widget's key handling loop:
/// if let Some(nav) = handle_navigation_key(key, current_section, total_sections) {
///     return Ok(nav);
/// }
/// // ... handle other keys
/// ```
pub fn handle_navigation_key(key: Key, current: usize, section_count: usize) -> Option<Navigation> {
    match key {
        Key::Escape => {
            if current > 0 {
                Some(Navigation::Back)
            } else {
                None // Let the widget handle cancel
            }
        }
        Key::Char(c) if c.is_ascii_digit() && c != '0' => {
            let idx = (c as usize) - ('1' as usize);
            if idx < section_count {
                Some(Navigation::Jump(idx))
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_navigation_enum() {
        assert_eq!(Navigation::Next, Navigation::Next);
        assert_eq!(Navigation::Back, Navigation::Back);
        assert_eq!(Navigation::Jump(0), Navigation::Jump(0));
        assert_ne!(Navigation::Jump(0), Navigation::Jump(1));
        assert_eq!(Navigation::Hub, Navigation::Hub);
        assert_eq!(Navigation::Stay, Navigation::Stay);
        assert_eq!(Navigation::Done, Navigation::Done);
    }

    #[test]
    fn test_handle_navigation_key() {
        // Test number keys
        assert_eq!(
            handle_navigation_key(Key::Char('1'), 2, 4),
            Some(Navigation::Jump(0))
        );
        assert_eq!(
            handle_navigation_key(Key::Char('3'), 0, 4),
            Some(Navigation::Jump(2))
        );
        assert_eq!(
            handle_navigation_key(Key::Char('5'), 0, 4),
            None // Index out of range
        );
        assert_eq!(
            handle_navigation_key(Key::Char('0'), 0, 4),
            None // 0 is not a valid navigation key
        );

        // Test escape
        assert_eq!(
            handle_navigation_key(Key::Escape, 2, 4),
            Some(Navigation::Back)
        );
        assert_eq!(
            handle_navigation_key(Key::Escape, 0, 4),
            None // Can't go back from first section
        );

        // Test other keys
        assert_eq!(handle_navigation_key(Key::Enter, 0, 4), None);
        assert_eq!(handle_navigation_key(Key::ArrowDown, 0, 4), None);
    }

    #[test]
    fn test_section_creation() {
        let section: Section<'_, i32> = Section::new("Test", |_state, _theme| Ok(Navigation::Next));
        assert_eq!(section.label, "Test");
    }

    #[test]
    fn test_wizard_builder() {
        let wizard: Wizard<'_, i32> = Wizard::new("Test Wizard")
            .section("Step 1", |_state, _theme| Ok(Navigation::Next))
            .section("Step 2", |_state, _theme| Ok(Navigation::Done));

        assert_eq!(wizard.sections.len(), 2);
        assert_eq!(wizard.sections[0].label, "Step 1");
        assert_eq!(wizard.sections[1].label, "Step 2");
    }
}
