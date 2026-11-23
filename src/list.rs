use std::io;

use console::{Key, Term};

use crate::{
    Theme,
    host::Host,
    input::{CTRL_N, CTRL_P},
    prompt::{CliOptions, KeyPress, Outcome, Prompt},
    theme,
    view::{Size, View},
};

/// Display a list of options
///
/// # Example
/// ```rust
/// use demand::{DemandOption, List};
///
/// let list = List::new("Toppings")
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
/// let toppings = match list.run() {
///   Ok(_) => {},
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
pub struct List<'a> {
    /// Title of the list
    pub title: String,
    /// A description to display after the title
    pub description: String,
    /// Number of items to show on each page
    pub success_items: usize,
    /// Colors/style of the list
    pub theme: &'a Theme,

    items: Vec<&'a str>,
    capacity: usize,
    filtering: bool,
    filterable: bool,
    filter: String,
    cur_page: usize,
    pages: usize,
    scroll: usize,
}

impl<'a> List<'a> {
    /// Creates a new list with a title
    pub fn new<S: Into<String>>(title: S) -> Self {
        Self {
            title: title.into(),
            description: String::new(),
            theme: &theme::DEFAULT,
            items: Vec::new(),
            capacity: 24 - 5,
            filtering: false,
            filterable: false,
            filter: String::new(),
            cur_page: 0,
            pages: 0,
            success_items: 4,
            scroll: 0,
        }
    }

    /// Sets the description of the list
    pub fn description(mut self, description: &str) -> Self {
        self.description = description.to_string();
        self
    }

    /// Adds an item to the list
    pub fn item(mut self, entry: &'a str) -> Self {
        self.items.push(entry);
        self.pages = self.get_pages();
        self
    }

    /// Adds multiple items to the list
    pub fn items(mut self, entries: &[&'a str]) -> Self {
        self.items.extend_from_slice(entries);
        self.pages = self.get_pages();
        self
    }

    /// Sets the number of items to show on confirmation
    pub fn success_items(mut self, items: usize) -> Self {
        self.success_items = items;
        self
    }

    /// Sets the list to be filterable
    pub fn filterable(mut self, filterable: bool) -> Self {
        self.filterable = filterable;
        self
    }

    /// Sets the theme of the list
    pub fn theme(mut self, theme: &'a Theme) -> Self {
        self.theme = theme;
        self
    }

    /// Displays the input to the user and returns the response
    ///
    /// This function will block until the user submits the input. If the user cancels the input,
    /// an error of type `io::ErrorKind::Interrupted` is returned.
    pub fn run(mut self) -> Result<(), io::Error> {
        Host::new(Term::stderr()).run(&mut self, |_| Some(()))
    }

    /// Handle a key, for a prompt that's part of a larger interface rather
    /// than shown with [`run`](Self::run).
    pub fn handle_key(&mut self, key: impl Into<KeyPress>) -> Outcome {
        self.on_key(key.into().0)
    }

    fn handle_up(&mut self) {
        if self.scroll > 0 {
            self.scroll -= 1;
            self.pages = self.get_pages();
        }
    }

    fn handle_down(&mut self) {
        let saturating_sub = self.filtered_entries().len().saturating_sub(self.capacity);
        if self.scroll < saturating_sub {
            self.scroll += 1;
            self.pages = self.get_pages();
        }
    }

    fn handle_left(&mut self) {
        if self.cur_page > 0 {
            self.cur_page -= 1;
        }
    }

    fn handle_right(&mut self) {
        if self.pages > 0 && self.cur_page < self.pages - 1 {
            self.cur_page += 1;
        }
    }

    fn handle_start_filtering(&mut self) {
        self.filtering = true;
    }

    fn handle_stop_filtering(&mut self, save: bool) {
        self.filtering = false;
        self.cur_page = 0;
        if !save {
            self.filter.clear();
            self.pages = self.get_pages();
        }
    }

    fn handle_filter_backspace(&mut self) {
        self.filter.pop();
        self.scroll = 0;
        self.pages = self.get_pages();
    }

    fn handle_filter_key(&mut self, key: char) {
        self.filter.push(key);
        self.scroll = 0;
        self.pages = self.get_pages();
    }

    fn filtered_entries(&self) -> Vec<&&'a str> {
        self.items
            .iter()
            .filter(|e| {
                self.filter.is_empty() || e.to_lowercase().contains(&self.filter.to_lowercase())
            })
            .collect()
    }

    fn get_pages(&self) -> usize {
        if self.filtering {
            ((self.filtered_entries().len() - self.scroll) as f64 / self.capacity as f64).ceil()
                as usize
        } else {
            ((self.items.len() - self.scroll) as f64 / self.capacity as f64).ceil() as usize
        }
    }

    fn resize_layout(&mut self, rows: usize) {
        let capacity = rows.max(8) - 5;
        if capacity == self.capacity {
            return;
        }
        let entry_count = self.filtered_entries().len();
        let start =
            (self.cur_page * self.capacity + self.scroll).min(entry_count.saturating_sub(capacity));
        self.capacity = capacity;
        self.cur_page = start / self.capacity;
        self.scroll = start % self.capacity;
        self.pages = self.get_pages();
    }

    fn visible_entries(&self) -> Vec<&&'a str> {
        let filtered = self.filtered_entries();
        let start = (self.cur_page * self.capacity) + self.scroll;
        filtered
            .into_iter()
            .skip(start)
            .take(self.capacity)
            .collect()
    }

    fn render(&self) -> View {
        let mut out = View::default();

        out.set_color(&self.theme.title);
        write!(out, "{}", self.title);

        writeln!(out);
        if !self.description.is_empty() {
            out.set_color(&self.theme.description);
            write!(out, "{}", self.description);
            writeln!(out);
        }
        for entry in self.visible_entries().iter() {
            out.set_color(&self.theme.unselected_option);
            writeln!(out, "  {entry}");
        }
        if self.pages > 1 {
            out.set_color(&self.theme.description);
            writeln!(out, " (page {}/{})", self.cur_page + 1, self.pages);
        }
        if self.filtering {
            out.set_color(&self.theme.input_cursor);
            write!(out, "/");
            out.reset();
            write!(out, "{}", self.filter);
            out.set_color(&self.theme.real_cursor_color(None));
            writeln!(out, " ");
        } else if !self.filter.is_empty() {
            out.set_color(&self.theme.description);
            write!(out, "/{}", self.filter);
        }
        let mut help_keys = vec![("↑/↓/k/j", "up/down")];
        if self.pages > 1 {
            help_keys.push(("←/→/h/l", "prev/next page"));
        }
        if self.filterable {
            if self.filtering {
                help_keys = vec![("esc", "clear filter"), ("enter", "save filter")]
            } else {
                help_keys.push(("/", "filter"));
            }
        }
        if !self.filtering {
            help_keys.push(("enter", "done"));
        }
        out.write_help(
            self.theme,
            &help_keys,
            !self.filtering && !self.filter.is_empty(),
        );

        writeln!(out);

        out.reset();
        out
    }

    fn render_success(&self) -> View {
        let mut out = View::default();

        out.set_color(&self.theme.title);
        write!(out, "{}", self.title);

        for entry in self.items.iter().take(self.success_items) {
            out.set_color(&self.theme.unselected_option);
            write!(out, "  {entry},");
        }
        if self.items.len() > self.success_items {
            write!(out, " ...");
        }

        writeln!(out);

        out.reset();
        out
    }
}

impl Prompt for List<'_> {
    fn on_key(&mut self, key: Key) -> Outcome {
        if self.filtering {
            match key {
                Key::ArrowDown | Key::Char(CTRL_N) => self.handle_down(),
                Key::ArrowUp | Key::Char(CTRL_P) => self.handle_up(),
                Key::Enter => self.handle_stop_filtering(true),
                Key::Escape => self.handle_stop_filtering(false),
                Key::Backspace => self.handle_filter_backspace(),
                Key::Char(c) if !c.is_control() => self.handle_filter_key(c),
                _ => return Outcome::Ignored,
            }
        } else {
            match key {
                Key::ArrowDown | Key::Char('j') | Key::Char(CTRL_N) => self.handle_down(),
                Key::ArrowUp | Key::Char('k') | Key::Char(CTRL_P) => self.handle_up(),
                Key::ArrowLeft | Key::Char('h') => self.handle_left(),
                Key::ArrowRight | Key::Char('l') => self.handle_right(),
                Key::Char('/') if self.filterable => self.handle_start_filtering(),
                Key::Escape => return Outcome::Cancel,
                Key::Enter => return Outcome::Submit,
                _ => return Outcome::Ignored,
            }
        }
        Outcome::Handled
    }

    fn view(&mut self, size: Size) -> View {
        self.resize_layout(size.rows);
        self.render()
    }

    fn success(&mut self) -> View {
        self.render_success()
    }

    fn cli(&self) -> CliOptions {
        CliOptions {
            clear_line: true,
            clear_on_cancel: false,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::test::without_ansi;

    use super::*;
    use indoc::indoc;

    #[test]
    fn test_render() {
        let list = List::new("Foods")
            .description("yummy thingos")
            .item("chips")
            .item("burger")
            .item("sandwich")
            .item("cupcakes");
        assert_eq!(
            indoc! {
                "Foods
                 yummy thingos
                   chips
                   burger
                   sandwich
                   cupcakes
                 ↑/↓/k/j up/down • enter done
                ",

            },
            without_ansi(list.render().to_ansi().as_str())
        )
    }

    #[test]
    fn resize_preserves_first_visible_entry() {
        let items = (0..20).map(|value| value.to_string()).collect::<Vec<_>>();
        let item_refs = items.iter().map(String::as_str).collect::<Vec<_>>();
        let mut list = List::new("Items").items(&item_refs);
        list.capacity = 10;
        list.cur_page = 1;
        list.scroll = 2;

        list.resize_layout(9);

        assert_eq!(list.capacity, 4);
        assert_eq!(*list.visible_entries()[0], "12");
    }

    #[test]
    fn resize_clamps_scroll_to_last_full_view() {
        let items = (0..20).map(|value| value.to_string()).collect::<Vec<_>>();
        let item_refs = items.iter().map(String::as_str).collect::<Vec<_>>();
        let mut list = List::new("Items").items(&item_refs);
        list.capacity = 4;
        list.cur_page = 4;
        list.scroll = 2;

        list.resize_layout(20);

        assert_eq!(list.capacity, 15);
        assert_eq!(*list.visible_entries()[0], "5");
    }
}
