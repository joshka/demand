use std::collections::HashSet;
use std::io;

use console::{Alignment, Key, Term};
use fuzzy_matcher::skim::SkimMatcherV2;
use itertools::Itertools;

use crate::host::Host;
use crate::prompt::{KeyPress, Outcome, Prompt};
use crate::view::{Size, View};
use crate::{DemandOption, theme};
use crate::{
    input::{CTRL_N, CTRL_P},
    theme::Theme,
};

/// Select multiple options from a list
///
/// # Example
/// ```rust
/// use demand::{DemandOption, MultiSelect};
///
/// let multiselect = MultiSelect::new("Toppings")
///   .description("Select your toppings")
///   .min(1)
///   .max(4)
///   .filterable(true)
///   .option(DemandOption::new("Lettuce").selected(true))
///   .option(DemandOption::new("Tomatoes").selected(true))
///   .option(DemandOption::new("Charm Sauce"))
///   .option(DemandOption::new("Jalapenos").label("Jalapeños"))
///   .option(DemandOption::new("Cheese"))
///   .option(DemandOption::new("Vegan Cheese"))
///   .option(DemandOption::new("Nutella"));
/// let toppings = match multiselect.run() {
///   Ok(toppings) => toppings,
///   Err(e) => {
///       if e.kind() == std::io::ErrorKind::Interrupted {
///           println!("Input cancelled");
///           return;
///       } else {
///           panic!("Error: {}", e);
///       }
///   }
/// };
/// ```
pub struct MultiSelect<'a, T> {
    /// The title of the selector
    pub title: String,
    /// The colors/style of the selector
    pub theme: &'a Theme,
    /// A description to display after the title
    pub description: String,
    /// The options which can be selected
    pub options: Vec<DemandOption<T>>,
    /// The minimum number of options which must be selected
    pub min: usize,
    /// The maximum number of options which can be selected
    pub max: usize,
    /// Whether the selector can be filtered with a query
    pub filterable: bool,
    /// Whether the selector is currently being filtered
    pub filtering: bool,
    /// A filter query to preset when `filtering` is true
    pub filter: String,
    /// The key used to toggle selection (default: Space)
    pub toggle_key: Key,

    err: Option<String>,
    cursor_x: usize,
    cursor_y: usize,
    cursor: usize,
    pages: usize,
    cur_page: usize,
    capacity: usize,
    description_fit: crate::option::DescriptionFit,
    fuzzy_matcher: SkimMatcherV2,
}

impl<'a, T> MultiSelect<'a, T> {
    /// Create a new multi select with the given title
    pub fn new<S: Into<String>>(title: S) -> Self {
        MultiSelect {
            title: title.into(),
            description: String::new(),
            options: vec![],
            min: 0,
            max: usize::MAX,
            filterable: false,
            theme: &theme::DEFAULT,
            cursor_x: 0,
            cursor_y: 0,
            err: None,
            cursor: 0,
            filter: String::new(),
            filtering: false,
            pages: 0,
            cur_page: 0,
            capacity: crate::option::DEFAULT_CAPACITY,
            description_fit: Default::default(),
            toggle_key: Key::Char(' '),
            fuzzy_matcher: SkimMatcherV2::default().use_cache(true).smart_case(),
        }
    }

    /// Set the description of the selector
    pub fn description(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    /// Add an option to the selector
    pub fn option(mut self, option: DemandOption<T>) -> Self {
        self.options.push(option);
        self.pages = self.get_pages();
        self
    }

    /// Add multiple options to the selector
    pub fn options(mut self, options: Vec<DemandOption<T>>) -> Self {
        for option in options {
            self.options.push(option);
        }
        self.pages = self.get_pages();
        self
    }

    /// Set the minimum number of options which must be selected
    pub fn min(mut self, min: usize) -> Self {
        self.min = min;
        self
    }

    /// Set the maximum number of options which can be selected
    pub fn max(mut self, max: usize) -> Self {
        self.max = max;
        self
    }

    /// Set whether the selector can be filtered with a query
    pub fn filterable(mut self, filterable: bool) -> Self {
        self.filterable = filterable;
        self
    }

    pub fn filtering(mut self, filtering: bool) -> Self {
        self.filtering = filtering;
        self
    }

    pub fn filter(mut self, filter: &str) -> Self {
        self.filter = filter.to_string();
        self.cursor_x = self.filter.chars().count();
        self.pages = self.get_pages();
        self
    }

    /// Set the key used to toggle selection (default: Space)
    pub fn toggle_key(mut self, key: Key) -> Self {
        self.toggle_key = key;
        self
    }

    /// Set the theme of the selector
    pub fn theme(mut self, theme: &'a Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Displays the selector to the user and returns their selected options
    ///
    /// This function will block until the user submits the input. If the user cancels the input,
    /// an error of type `io::ErrorKind::Interrupted` is returned.
    pub fn run(mut self) -> io::Result<Vec<T>> {
        Host::new(Term::stderr()).run(&mut self, |_| Some(()))?;
        Ok(self.into_selected())
    }

    /// Handle a key, for a prompt that's part of a larger interface rather
    /// than shown with [`run`](Self::run).
    pub fn handle_key(&mut self, key: impl Into<KeyPress>) -> Outcome {
        self.on_key(key.into().0)
    }

    /// The items of the selected options, which are the answer once a key
    /// has been handled with [`Outcome::Submit`].
    pub fn selected(&self) -> Vec<&T> {
        self.options
            .iter()
            .filter(|option| option.selected)
            .map(|option| &option.item)
            .collect()
    }

    /// Like [`selected`](Self::selected), by value.
    pub fn into_selected(self) -> Vec<T> {
        self.options
            .into_iter()
            .filter(|option| option.selected)
            .map(|option| option.item)
            .collect()
    }

    fn filtered_options(&self) -> Vec<&DemandOption<T>> {
        if self.filter.is_empty() {
            return self.options.iter().collect();
        }
        self.options
            .iter()
            .filter_map(|opt| {
                crate::fuzzy::score(&self.fuzzy_matcher, &opt.label, &self.filter)
                    .map(|score| (score, opt))
            })
            .sorted_by_key(|(score, _opt)| -1 * *score)
            .map(|(_score, opt)| opt)
            .collect()
    }

    fn visible_options(&self) -> Vec<&DemandOption<T>> {
        let filtered_options = self.filtered_options();
        let start = self.cur_page * self.capacity;
        filtered_options
            .into_iter()
            .skip(start)
            .take(self.capacity)
            .collect()
    }

    fn handle_down(&mut self) {
        let visible_options = self.visible_options();
        if self.cursor < visible_options.len().max(1) - 1 {
            self.cursor += 1;
        } else if self.pages > 0 && self.cur_page < self.pages - 1 {
            self.cur_page += 1;
            self.cursor = 0;
        }
    }

    fn handle_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        } else if self.cur_page > 0 {
            self.cur_page -= 1;
            self.cursor = self.visible_options().len().max(1) - 1;
        }
    }

    fn handle_left(&mut self) {
        if self.filtering {
            if self.cursor_x > 0 {
                self.cursor_x -= 1;
            }
        } else if self.cur_page > 0 {
            self.cur_page -= 1;
        }
    }

    fn handle_right(&mut self) {
        if self.filtering {
            if self.cursor_x < self.filter.chars().count() {
                self.cursor_x += 1;
            }
        } else if self.pages > 0 && self.cur_page < self.pages - 1 {
            self.cur_page += 1;
            if self.cursor_y > self.visible_options().len() - 1 {
                self.cursor_y = self.visible_options().len() - 1;
            }
        }
    }

    fn handle_toggle(&mut self) {
        self.err = None;
        let visible_options = self.visible_options();
        if visible_options.is_empty() {
            return;
        }
        let cursor = self.cursor.min(visible_options.len().saturating_sub(1));
        let id = visible_options[cursor].id;
        let selected = visible_options[cursor].selected;
        drop(visible_options);
        self.cursor = cursor;
        self.options
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap()
            .selected = !selected;
    }

    fn handle_toggle_all(&mut self) {
        self.err = None;
        let filtered_options = self.filtered_options();
        if filtered_options.is_empty() {
            return;
        }
        let select = !filtered_options.iter().all(|o| o.selected);
        let ids = filtered_options
            .into_iter()
            .map(|o| o.id)
            .collect::<HashSet<_>>();
        for opt in &mut self.options {
            if ids.contains(&opt.id) {
                opt.selected = select;
            }
        }
    }

    fn handle_start_filtering(&mut self) {
        self.err = None;
        self.filtering = true;
    }

    /// Check that an allowed number of options is selected, and show what's
    /// wrong if not.
    fn try_confirm(&mut self) -> bool {
        let max = self.max.min(self.options.len());
        let min = self.min.min(max);
        let selected = self.options.iter().filter(|o| o.selected).count();
        self.err = if selected < min {
            Some(match min {
                1 => "Please select an option".to_string(),
                _ => format!("Please select at least {min} options"),
            })
        } else if selected > max {
            Some(match max {
                1 => "Please select only one option".to_string(),
                _ => format!("Please select at most {max} options"),
            })
        } else {
            return true;
        };
        false
    }

    fn handle_stop_filtering(&mut self, save: bool) {
        self.filtering = false;

        let visible_options = self.visible_options();
        if !visible_options.is_empty() {
            self.cursor = self.cursor.min(self.visible_options().len() - 1);
        }
        if !save {
            self.filter.clear();
            self.reset_paging();
        }
    }

    fn handle_filter_key(&mut self, c: char) {
        let idx = self.get_char_idx(&self.filter, self.cursor_x);
        self.filter.insert(idx, c);
        self.cursor_x += 1;
        self.cursor = 0;
        self.cursor_y = 0;
        self.err = None;
        self.reset_paging();
    }

    fn handle_filter_backspace(&mut self) {
        let chars_count = self.filter.chars().count();
        if chars_count > 0 && self.cursor_x > 0 {
            let idx = self.get_char_idx(&self.filter, self.cursor_x - 1);
            self.filter.remove(idx);
        }
        if self.cursor_x > 0 {
            self.cursor_x -= 1;
        }
        self.cursor = 0;
        self.cursor_y = 0;
        self.err = None;
        self.reset_paging();
    }

    fn reset_paging(&mut self) {
        self.cur_page = 0;
        self.pages = self.get_pages();
    }

    fn resize_layout(&mut self, rows: usize, width: usize) {
        // The fixed 6 rows assume a one-row title and description. A
        // longer one wraps onto more, and those come out of the options.
        // The title's row also carries the error marker, when there is one.
        let marker = if self.err.is_some() { " *" } else { "" };
        let header_rows = crate::height::rendered_rows(&format!("{}{marker}", self.title), width)
            + if self.description.is_empty() {
                0
            } else {
                crate::height::rendered_rows(&self.description, width)
            };
        let available = (rows.max(8) - 6).saturating_sub(header_rows.saturating_sub(2));
        // An option whose label or description is wider than the terminal
        // wraps too.
        let prefix = console::measure_text_width(&self.theme.selected_prefix)
            .max(console::measure_text_width(&self.theme.unselected_prefix));
        let filtered_options = self.filtered_options();
        let option_count = filtered_options.len();
        let (capacity, fit) =
            crate::option::layout_options(&filtered_options, 2 + prefix, width, available);
        drop(filtered_options);
        self.description_fit = fit;
        if capacity == self.capacity {
            return;
        }
        let cursor =
            (self.cur_page * self.capacity + self.cursor).min(option_count.saturating_sub(1));
        self.capacity = capacity;
        self.pages = self.get_pages();
        self.cur_page = cursor / self.capacity;
        self.cursor = cursor % self.capacity;
    }

    fn get_pages(&self) -> usize {
        if self.filtering || !self.filter.is_empty() {
            ((self.filtered_options().len() as f64) / self.capacity as f64).ceil() as usize
        } else {
            ((self.options.len() as f64) / self.capacity as f64).ceil() as usize
        }
    }

    fn render(&self) -> View {
        let mut out = View::default();

        out.set_color(&self.theme.title);
        write!(out, "{}", self.title);

        if self.err.is_some() {
            out.set_color(&self.theme.error_indicator);
            writeln!(out, " *");
        } else {
            writeln!(out);
        }
        if !self.description.is_empty() || self.pages > 1 {
            out.set_color(&self.theme.description);
            write!(out, "{}", self.description);
            writeln!(out);
        }
        // Padded to the widest label in the list, not just on this page:
        // it's what the layout measured, and descriptions line up across
        // pages.
        let filtered_options = self.filtered_options();
        let max_label_len = crate::option::label_width(&filtered_options);
        let start = self.cur_page * self.capacity;
        for (i, option) in filtered_options
            .into_iter()
            .skip(start)
            .take(self.capacity)
            .enumerate()
        {
            if self.cursor == i {
                out.mark_focus();
                out.set_color(&self.theme.cursor);
                write!(out, " >");
            } else {
                write!(out, "  ");
            }
            if option.selected {
                out.set_color(&self.theme.selected_prefix_fg);
                write!(out, "{}", self.theme.selected_prefix);
                out.set_color(&self.theme.selected_option);
                self.print_option_label(&mut out, option, max_label_len);
            } else {
                out.set_color(&self.theme.unselected_prefix_fg);
                write!(out, "{}", self.theme.unselected_prefix);
                out.set_color(&self.theme.unselected_option);
                self.print_option_label(&mut out, option, max_label_len);
            }
        }

        if self.pages > 1 {
            out.set_color(&self.theme.description);
            writeln!(out, " (page {}/{})", self.cur_page + 1, self.pages);
        }

        if self.filtering {
            out.set_color(&self.theme.input_cursor);

            write!(out, "/");
            out.reset();

            out.write_with_caret(
                &self.filter,
                self.cursor_x,
                &self.theme.real_cursor_color(None),
            );
            writeln!(out);
            out.reset();
        } else if !self.filter.is_empty() {
            out.set_color(&self.theme.description);
            write!(out, "/{}", self.filter);
        } else if let Some(err) = &self.err {
            out.set_color(&self.theme.error_indicator);
            write!(out, " {err}");
        }

        self.print_help_keys(&mut out);

        writeln!(out);
        out.reset();
        out
    }

    fn print_option_label(&self, out: &mut View, option: &DemandOption<T>, max_label_len: usize) {
        if let Some(desc) = &option.description {
            let label = console::pad_str(&option.label, max_label_len, Alignment::Left, None);
            if self.filtering && !self.filter.is_empty() {
                self.highlight_matches(out, &label);
            } else {
                write!(out, " {label}");
            }
            out.set_color(&self.theme.description);
            let prefix = if option.selected {
                &self.theme.selected_prefix
            } else {
                &self.theme.unselected_prefix
            };
            let indent = 2 + console::measure_text_width(prefix);
            let desc = self
                .description_fit
                .fit(&format!("{} {label}  ", " ".repeat(indent)), desc);
            writeln!(out, "  {desc}");
        } else if self.filtering && !self.filter.is_empty() {
            self.highlight_matches(out, &option.label);
            writeln!(out);
        } else {
            writeln!(out, " {}", option.label);
        }
    }

    fn toggle_key_label(&self) -> String {
        match self.toggle_key {
            Key::Char(' ') => "space".to_string(),
            Key::Tab => "tab".to_string(),
            Key::Char(c) => c.to_string(),
            _ => "?".to_string(),
        }
    }

    fn print_help_keys(&self, out: &mut View) {
        let toggle_label = self.toggle_key_label();
        let mut help_keys: Vec<(String, &str)> = vec![("↑/↓/k/j".to_string(), "up/down")];
        if self.pages > 1 {
            help_keys.push(("←/→/h/l".to_string(), "prev/next page"));
        }
        help_keys.push((format!("x/{}", toggle_label), "toggle"));
        help_keys.push(("a".to_string(), "toggle all"));
        if self.filterable {
            if self.filtering {
                help_keys = vec![
                    ("↑/↓".to_string(), "up/down"),
                    (toggle_label, "toggle"),
                    ("esc".to_string(), "clear filter"),
                    ("enter".to_string(), "confirm"),
                ];
            } else {
                help_keys.push(("/".to_string(), "filter"));
                if !self.filter.is_empty() {
                    help_keys.push(("esc".to_string(), "clear filter"));
                }
            }
        }
        if !self.filtering {
            help_keys.push(("enter".to_string(), "confirm"));
        }
        out.write_help(
            self.theme,
            &help_keys,
            !self.filtering && !self.filter.is_empty(),
        );
    }

    fn get_char_idx(&self, input: &str, cursor: usize) -> usize {
        input
            .char_indices()
            .nth(cursor)
            .map(|(i, _)| i)
            .unwrap_or(input.len())
    }

    fn highlight_matches(&self, out: &mut View, label: &str) {
        if let Some(indices) = crate::fuzzy::indices(&self.fuzzy_matcher, label, &self.filter) {
            for (j, c) in label.chars().enumerate() {
                if indices.contains(&j) {
                    out.set_color(&self.theme.selected_option);
                } else {
                    out.set_color(&self.theme.unselected_option);
                }
                if j == 0 {
                    write!(out, " ");
                }
                write!(out, "{c}");
            }
        } else {
            write!(out, " {label}");
        }
    }

    fn render_success(&self, selected: &[String]) -> View {
        let mut out = View::default();
        out.set_color(&self.theme.title);
        write!(out, "{}", self.title);
        out.set_color(&self.theme.selected_option);
        writeln!(out, " {}", selected.join(", "));
        out.reset();
        out
    }
}

impl<T> Prompt for MultiSelect<'_, T> {
    fn on_key(&mut self, key: Key) -> Outcome {
        if self.filtering {
            match key {
                Key::ArrowDown | Key::Char(CTRL_N) => self.handle_down(),
                Key::ArrowUp | Key::Char(CTRL_P) => self.handle_up(),
                Key::ArrowLeft => self.handle_left(),
                Key::ArrowRight => self.handle_right(),
                Key::Enter if self.try_confirm() => return Outcome::Submit,
                Key::Enter => {}
                Key::Escape => self.handle_stop_filtering(false),
                Key::Backspace => self.handle_filter_backspace(),
                key if key == self.toggle_key => self.handle_toggle(),
                Key::Char(c) if !c.is_control() => self.handle_filter_key(c),
                _ => return Outcome::Ignored,
            }
        } else {
            match key {
                Key::ArrowDown | Key::Char('j') | Key::Char(CTRL_N) => self.handle_down(),
                Key::ArrowUp | Key::Char('k') | Key::Char(CTRL_P) => self.handle_up(),
                Key::ArrowLeft | Key::Char('h') => self.handle_left(),
                Key::ArrowRight | Key::Char('l') => self.handle_right(),
                key if key == self.toggle_key || key == Key::Char('x') => self.handle_toggle(),
                Key::Char('a') => self.handle_toggle_all(),
                Key::Char('/') if self.filterable => self.handle_start_filtering(),
                Key::Escape if self.filter.is_empty() => return Outcome::Cancel,
                Key::Escape => self.handle_stop_filtering(false),
                Key::Enter if self.try_confirm() => return Outcome::Submit,
                Key::Enter => {}
                _ => return Outcome::Ignored,
            }
        }
        Outcome::Handled
    }

    fn view(&mut self, size: Size) -> View {
        self.resize_layout(size.rows, size.width);
        self.render()
    }

    fn success(&mut self) -> View {
        let selected: Vec<String> = self
            .options
            .iter()
            .filter(|option| option.selected)
            .map(|option| option.label.clone())
            .collect();
        self.render_success(&selected)
    }
}

#[cfg(test)]
mod tests {
    use crate::test::without_ansi;
    #[cfg(unix)]
    use crate::test::{Parser, capture_term, replay, snapshot};

    use super::*;
    use indoc::indoc;

    #[test]
    fn test_render() {
        let select = MultiSelect::new("Toppings")
            .description("Select your toppings")
            .option(DemandOption::new("Lettuce").selected(true))
            .option(DemandOption::new("Tomatoes").selected(true))
            .option(DemandOption::new("Charm Sauce"))
            .option(DemandOption::new("Jalapenos").label("Jalapeños"))
            .option(DemandOption::new("Cheese"))
            .option(DemandOption::new("Vegan Cheese"))
            .option(DemandOption::new("Nutella"));

        assert_eq!(
            indoc! {
              "Toppings
            Select your toppings
             >[•] Lettuce
              [•] Tomatoes
              [ ] Charm Sauce
              [ ] Jalapeños
              [ ] Cheese
              [ ] Vegan Cheese
              [ ] Nutella
            ↑/↓/k/j up/down • x/space toggle • a toggle all • enter confirm
            "
            },
            without_ansi(select.render().to_ansi().as_str())
        );
    }

    #[test]
    fn non_display() {
        struct Thing {
            num: u32,
            _thing: Option<()>,
        }
        let things = [
            Thing {
                num: 1,
                _thing: Some(()),
            },
            Thing {
                num: 2,
                _thing: None,
            },
            Thing {
                num: 3,
                _thing: None,
            },
        ];
        let select = MultiSelect::new("things")
            .description("pick a thing")
            .options(
                things
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        if i == 0 {
                            DemandOption::with_label("First", t)
                        } else {
                            DemandOption::new(t.num).item(t).selected(true)
                        }
                    })
                    .collect(),
            );
        assert_eq!(
            indoc! {
              "things
            pick a thing
             >[ ] First
              [•] 2
              [•] 3
            ↑/↓/k/j up/down • x/space toggle • a toggle all • enter confirm
            "
            },
            without_ansi(select.render().to_ansi().as_str())
        );
    }

    /// Regression for the off-by-one in the old line-by-line repositioning
    /// code (jdx/demand#129), which made the menu drift up on every keypress.
    ///
    /// Drives the redraw path through a `Term::read_write_pair` whose writer is
    /// a `SharedBuf`, replays the captured bytes through a terminal emulator, and
    /// asserts that the cursor row is stable across iterations (i.e. the render
    /// origin doesn't drift).
    #[cfg(unix)]
    #[test]
    fn redraw_does_not_drift_upward() {
        let (term, buf) = capture_term();
        let mut ms = MultiSelect::new("Toppings")
            .option(DemandOption::new("Lettuce"))
            .option(DemandOption::new("Tomatoes"))
            .option(DemandOption::new("Cheese"))
            .option(DemandOption::new("Nutella"));
        let mut host = Host::new(term);

        // Start the emulator cursor well below the top of the screen, so that an
        // upward off-by-one would actually shift the rendered area visibly
        // rather than getting silently clamped at row 0.
        let mut parser = Parser::new(40, 120, 0);
        parser.process(&b"\n".repeat(20));

        let mut cursor_rows = Vec::new();
        for _ in 0..6 {
            let before = snapshot(&buf).len();
            host.draw(&mut ms).unwrap();

            let new_bytes = snapshot(&buf)[before..].to_vec();
            replay(&mut parser, &new_bytes);
            cursor_rows.push(parser.screen().cursor_position().0);

            // Move the in-menu cursor so the renders aren't byte-identical and
            // the redraw path actually has work to do (clear + rewrite).
            ms.handle_down();
        }

        // After iter 1 the emulator cursor settles at the end of the frame.
        // With the bug, every subsequent iter pulls it up by another row; with
        // the fix it stays put.
        let first = cursor_rows[0];
        for (i, &row) in cursor_rows.iter().enumerate().skip(1) {
            assert_eq!(
                row, first,
                "iteration {i}: cursor row drifted from {first} to {row} (full trace: {cursor_rows:?})",
            );
        }
    }

    /// When the rendered output shrinks, redraw must clear the rows that are
    /// no longer occupied and leave the cursor at the end of the new frame.
    #[cfg(unix)]
    #[test]
    fn redraw_clears_rows_when_wrapped_frame_shrinks() {
        let (term, buf) = capture_term();
        let mut ms = MultiSelect::new("T")
            .option(DemandOption::new("long").label(&"x".repeat(140)))
            .option(DemandOption::new("b"))
            .option(DemandOption::new("c"));
        let mut host = Host::new(term);

        let mut parser = Parser::new(40, 120, 0);
        parser.process(&b"\n".repeat(20));

        // First render with all 3 options.
        let before = snapshot(&buf).len();
        host.draw(&mut ms).unwrap();
        replay(&mut parser, &snapshot(&buf)[before..]);
        let row_after_full = parser.screen().cursor_position().0;

        // Remove the wrapped option and one short option, forcing the next
        // render to shrink by both logical and physical rows.
        ms.options.remove(0);
        ms.options.truncate(1);

        let before = snapshot(&buf).len();
        host.draw(&mut ms).unwrap();
        replay(&mut parser, &snapshot(&buf)[before..]);
        let row_after_short = parser.screen().cursor_position().0;

        assert!(row_after_short < row_after_full);
        assert!(!parser.screen().contents().contains("xxxxxxxx"));

        // The third render must again be stable — a fresh redraw on top of
        // the shrunken state without further drift.
        let before = snapshot(&buf).len();
        host.draw(&mut ms).unwrap();
        replay(&mut parser, &snapshot(&buf)[before..]);
        assert_eq!(parser.screen().cursor_position().0, row_after_short);
    }

    /// Regression for jdx/mise#11326: a wrapped option used to make each
    /// redraw start below stale physical rows, stacking another copy of the
    /// prompt on every keypress.
    #[cfg(unix)]
    #[test]
    fn wrapped_option_does_not_stack_copies_of_the_frame() {
        let (term, buf) = capture_term();
        let mut ms = MultiSelect::new("mise upgrade")
            .option(DemandOption::new("long").label(&"x".repeat(140)))
            .option(DemandOption::new("short"));
        let mut host = Host::new(term);
        let width = host.term.size().1 as usize;
        assert!(width < 140, "test needs a label wider than the terminal");

        let mut parser = Parser::new(40, width as u16, 0);
        parser.process(&b"\n".repeat(20));

        for _ in 0..3 {
            let before = snapshot(&buf).len();
            host.draw(&mut ms).unwrap();

            replay(&mut parser, &snapshot(&buf)[before..]);
            ms.handle_down();
        }

        let screen = parser.screen().contents();
        assert_eq!(
            screen.matches("mise upgrade").count(),
            1,
            "prompt drawn more than once:\n{screen}"
        );
    }

    /// jdx/demand#235: options whose descriptions wrap take more than one
    /// row each, so fewer of them fit on a page.
    #[test]
    fn wrapping_descriptions_take_rows_from_the_page() {
        let mut ms = MultiSelect::new("Pick").options(
            (0..40)
                .map(|i| DemandOption::new(i.to_string()).description("short"))
                .collect(),
        );
        ms.resize_layout(24, 80);
        assert_eq!(ms.capacity, 18);

        ms.options[7].description = Some("d".repeat(80));
        ms.resize_layout(24, 80);
        assert_eq!(ms.capacity, 9);
        assert_eq!(ms.pages, 5);

        // A title filling two rows takes nothing: one row each was already
        // set aside for it and the description.
        ms.options[7].description = None;
        ms.title = "t".repeat(80 * 2);
        ms.resize_layout(24, 80);
        assert_eq!(ms.capacity, 18);

        // The error marker pushes it onto a third, which does.
        ms.err = Some("pick one".to_string());
        ms.resize_layout(24, 80);
        assert_eq!(ms.capacity, 17);
    }

    #[test]
    fn resize_preserves_focused_option() {
        let mut select = MultiSelect::new("Pick").options(
            (0..20)
                .map(|value| DemandOption::new(value.to_string()))
                .collect(),
        );
        select.capacity = 10;
        select.pages = 2;
        select.cur_page = 1;
        select.cursor = 5;

        select.resize_layout(10, 80);

        assert_eq!(select.capacity, 4);
        assert_eq!(select.pages, 5);
        assert_eq!(select.cur_page, 3);
        assert_eq!(select.cursor, 3);
        assert_eq!(select.visible_options()[select.cursor].label, "15");
    }

    /// The caret's block used to slice one byte off the filter, which
    /// panics when the char under it is multi-byte.
    #[test]
    fn the_filter_caret_can_be_on_a_multibyte_char() {
        let mut select = MultiSelect::new("Pick").option(DemandOption::new("a"));
        select.filtering = true;
        select.filter = "éa".to_string();
        select.cursor_x = 0;
        let rendered = select.render().to_ansi();
        assert!(without_ansi(&rendered).contains("/éa"));
    }
}
