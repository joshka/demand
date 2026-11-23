//! Every prompt as a widget in a ratatui app.
//!
//! Tab and Shift-Tab move between the prompts, F2 changes the theme, Ctrl-C
//! quits. The other keys are the prompt's.

use std::io;
use std::time::Duration;

use crossterm::event::{self, KeyCode, KeyEvent, KeyModifiers};
use demand::{
    Confirm, DemandOption, Dialog, DialogButton, Editor, GridRow, GridSelect, Input, List,
    MultiSelect, Navigation, Outcome, Select, Spinner, SpinnerStyle, Steps, Theme,
};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Style, Stylize};
use ratatui::text::Line;
use ratatui::widgets::Block;
use ratatui::{DefaultTerminal, Frame};

fn main() -> io::Result<()> {
    let themes = [
        ("charm", Theme::charm()),
        ("dracula", Theme::dracula()),
        ("catppuccin", Theme::catppuccin()),
        ("base16", Theme::base16()),
        ("new", Theme::new()),
    ];
    let spinner_style = SpinnerStyle::dots();
    let mut app = App::new(&themes, &spinner_style);
    ratatui::run(|terminal| app.run(terminal))
}

struct App<'a> {
    themes: &'a [(&'static str, Theme)],
    theme: usize,
    page: Page,
    status: String,
    input: Input<'a>,
    password: Input<'a>,
    autocomplete: Input<'a>,
    select: Select<'a, u32>,
    multi_select: MultiSelect<'a, &'static str>,
    grid_select: GridSelect<'a, &'static str>,
    list: List<'a>,
    confirm: Confirm<'a>,
    dialog: Dialog<'a>,
    editor: Editor<'a>,
    spinner: Spinner<'a>,
    steps: Steps<'a>,
    step_name: Input<'a>,
    step_size: Select<'a, &'static str>,
    step_confirm: Confirm<'a>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Page {
    Input,
    Password,
    Autocomplete,
    Select,
    MultiSelect,
    GridSelect,
    List,
    Confirm,
    Dialog,
    Editor,
    Spinner,
    Wizard,
}

impl Page {
    const ALL: [Page; 12] = [
        Page::Input,
        Page::Password,
        Page::Autocomplete,
        Page::Select,
        Page::MultiSelect,
        Page::GridSelect,
        Page::List,
        Page::Confirm,
        Page::Dialog,
        Page::Editor,
        Page::Spinner,
        Page::Wizard,
    ];

    /// The page `by` pages on from this one.
    fn step(self, by: usize) -> Self {
        Self::ALL[(self as usize + by) % Self::ALL.len()]
    }
}

impl<'a> App<'a> {
    fn new(themes: &'a [(&'static str, Theme)], spinner_style: &'a SpinnerStyle) -> Self {
        let input = Input::new("What's your name?")
            .description("It can't be empty.")
            .placeholder("Ferris")
            .validation(|s| match s {
                "" => Err("required"),
                _ => Ok(()),
            });
        let password = Input::new("Password").password(true);
        let autocomplete = Input::new("Language").autocomplete_fn(|text| {
            let languages = ["C", "Go", "Haskell", "Python", "Ruby", "Rust", "Zig"];
            let text = text.to_lowercase();
            Ok(languages
                .iter()
                .filter(|language| language.to_lowercase().starts_with(&text))
                .map(|language| language.to_string())
                .collect())
        });

        let squares = (1..=40).map(|n| {
            DemandOption::new(n)
                .label(&format!("Option {n}"))
                .description(&format!("its square is {}", n * n))
        });
        let select = Select::new("Pick a number")
            .description("With more options than fit on a page")
            .filterable(true)
            .options(squares.collect());
        let toppings = ["Basil", "Olives", "Mushrooms", "Peppers", "Pineapple"];
        let multi_select = MultiSelect::new("Toppings")
            .description("Pick one to three")
            .min(1)
            .max(3)
            .filterable(true)
            .options(toppings.map(DemandOption::new).into());
        let grid_select = GridSelect::new("Upgrades")
            .columns(["Current", "Range", "Latest"])
            .filterable(true)
            .row(GridRow::new("react").cell("18.2").cell("18.3").cell("19.0"))
            .row(GridRow::new("chalk").cell("4.1").empty_cell().cell("5.0"))
            .row(GridRow::new("jest").cell("27.4").cell("27.5"));
        let list = List::new("Changes")
            .description("Something to read")
            .filterable(true)
            .items(&["Added widgets", "Fixed a panic", "Removed seven loops"]);

        let confirm = Confirm::new("Are you sure?").description("This can be undone.");
        let dialog = Dialog::new("Unsaved changes")
            .description("What should happen to them?")
            .buttons(vec![
                DialogButton::new("Save"),
                DialogButton::new("Discard"),
                DialogButton::new("Cancel"),
            ]);
        let editor = Editor::new("Release notes")
            .description("Opens in $EDITOR")
            .default_value("## Changes\n\n- \n")
            .extension("md");
        let spinner = Spinner::new("Loading...").style(spinner_style);

        let steps = Steps::new("New project")
            .step("Name")
            .step("Size")
            .step("Create");
        let step_name = Input::new("Project name").placeholder("my-project");
        let step_size = Select::new("How big will it get?")
            .options(["Small", "Large"].map(DemandOption::new).into());
        let step_confirm = Confirm::new("Create it?");

        let mut app = Self {
            themes,
            theme: 0,
            page: Page::Input,
            status: String::new(),
            input,
            password,
            autocomplete,
            select,
            multi_select,
            grid_select,
            list,
            confirm,
            dialog,
            editor,
            spinner,
            steps,
            step_name,
            step_size,
            step_confirm,
        };
        app.set_theme(0);
        app
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;
            // The spinner has to keep turning without any input.
            if !event::poll(Duration::from_millis(100))? {
                continue;
            }
            let Some(key) = event::read()?.as_key_press_event() else {
                continue;
            };
            if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
                return Ok(());
            }
            match self.handle_key(key) {
                Outcome::Submit => self.status = format!("Answer: {}", self.answer()),
                Outcome::Cancel => self.status = "Cancelled".to_string(),
                Outcome::Edit => {
                    ratatui::restore();
                    self.editor.edit();
                    *terminal = ratatui::init();
                    terminal.clear()?;
                }
                // Keys a prompt doesn't use are the app's.
                Outcome::Ignored => self.handle_ignored(key),
                _ => self.status.clear(),
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let rows = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]);
        let columns = Layout::horizontal([Constraint::Length(16), Constraint::Fill(1)]);
        let [main, footer] = rows.areas(frame.area());
        let [menu, content] = columns.areas(main);

        let block = Block::bordered().title("Prompts");
        let pages = Page::ALL.map(|page| {
            let style = match page == self.page {
                true => Style::new().reversed(),
                false => Style::new(),
            };
            Line::styled(format!("{page:?}"), style)
        });
        for (row, page) in block.inner(menu).rows().zip(pages) {
            frame.render_widget(page, row);
        }
        frame.render_widget(block, menu);

        let block = Block::bordered().title(self.themes[self.theme].0);
        self.draw_page(frame, block.inner(content));
        frame.render_widget(block, content);

        let help = "tab/shift-tab prompt • F2 theme • ctrl-c quit";
        let footer_line = match self.status.as_str() {
            "" => Line::from(help).dim(),
            status => Line::from(status),
        };
        frame.render_widget(footer_line, footer);
    }

    fn draw_page(&mut self, frame: &mut Frame, area: Rect) {
        match self.page {
            Page::Input => frame.render_widget(&mut self.input, area),
            Page::Password => frame.render_widget(&mut self.password, area),
            Page::Autocomplete => frame.render_widget(&mut self.autocomplete, area),
            Page::Select => frame.render_widget(&mut self.select, area),
            Page::MultiSelect => frame.render_widget(&mut self.multi_select, area),
            Page::GridSelect => frame.render_widget(&mut self.grid_select, area),
            Page::List => frame.render_widget(&mut self.list, area),
            Page::Confirm => frame.render_widget(&mut self.confirm, area),
            Page::Dialog => frame.render_widget(&mut self.dialog, area),
            Page::Editor => frame.render_widget(&mut self.editor, area),
            Page::Spinner => frame.render_widget(&self.spinner, area),
            Page::Wizard => self.draw_wizard(frame, area),
        }
    }

    fn draw_wizard(&mut self, frame: &mut Frame, area: Rect) {
        let rows = Layout::vertical([Constraint::Length(4), Constraint::Fill(1)]);
        let [steps, step] = rows.areas(area);
        frame.render_widget(&self.steps, steps);
        match self.steps.current() {
            0 => frame.render_widget(&mut self.step_name, step),
            1 => frame.render_widget(&mut self.step_size, step),
            _ => frame.render_widget(&mut self.step_confirm, step),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        match self.page {
            Page::Input => self.input.handle_key(key),
            Page::Password => self.password.handle_key(key),
            Page::Autocomplete => self.autocomplete.handle_key(key),
            Page::Select => self.select.handle_key(key),
            Page::MultiSelect => self.multi_select.handle_key(key),
            Page::GridSelect => self.grid_select.handle_key(key),
            Page::List => self.list.handle_key(key),
            Page::Confirm => self.confirm.handle_key(key),
            Page::Dialog => self.dialog.handle_key(key),
            Page::Editor => self.editor.handle_key(key),
            Page::Spinner => Outcome::Ignored,
            Page::Wizard => self.handle_wizard_key(key),
        }
    }

    fn handle_wizard_key(&mut self, key: KeyEvent) -> Outcome {
        let outcome = match self.steps.current() {
            0 => self.step_name.handle_key(key),
            1 => self.step_size.handle_key(key),
            _ => self.step_confirm.handle_key(key),
        };
        match outcome {
            Outcome::Submit if self.steps.current() == 2 => self.steps.navigate(Navigation::Done),
            Outcome::Submit => self.steps.navigate(Navigation::Next),
            Outcome::Cancel if self.steps.back() => Outcome::Handled,
            outcome => outcome,
        }
    }

    fn handle_ignored(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab => self.go_to(self.page.step(1)),
            KeyCode::BackTab => self.go_to(self.page.step(Page::ALL.len() - 1)),
            KeyCode::F(2) => self.set_theme((self.theme + 1) % self.themes.len()),
            _ => {}
        }
    }

    fn go_to(&mut self, page: Page) {
        self.page = page;
        self.status.clear();
    }

    fn set_theme(&mut self, theme: usize) {
        self.theme = theme;
        let theme = &self.themes[theme].1;
        self.input.theme = theme;
        self.password.theme = theme;
        self.autocomplete.theme = theme;
        self.select.theme = theme;
        self.multi_select.theme = theme;
        self.grid_select.theme = theme;
        self.list.theme = theme;
        self.confirm.theme = theme;
        self.dialog.theme = theme;
        self.editor.theme = theme;
        self.spinner.theme = theme;
        self.steps.theme = theme;
        self.step_name.theme = theme;
        self.step_size.theme = theme;
        self.step_confirm.theme = theme;
    }

    fn answer(&self) -> String {
        match self.page {
            Page::Input => self.input.input.clone(),
            Page::Password => format!("{} characters", self.password.input.chars().count()),
            Page::Autocomplete => self.autocomplete.input.clone(),
            Page::Select => format!("{:?}", self.select.selected()),
            Page::MultiSelect => format!("{:?}", self.multi_select.selected()),
            Page::GridSelect => format!("{:?}", self.grid_select.selected()),
            Page::List => "done".to_string(),
            Page::Confirm => self.confirm.selected.to_string(),
            Page::Dialog => format!("{:?}", self.dialog.selected_label()),
            Page::Editor => format!("{:?}", self.editor.text()),
            Page::Spinner => String::new(),
            Page::Wizard => format!(
                "{:?}, {:?}, {}",
                self.step_name.input,
                self.step_size.selected(),
                self.step_confirm.selected
            ),
        }
    }
}
