//! Prompts as widgets in a ratatui app.
//!
//! Tab and Shift-Tab move between the prompts, Enter answers one, Ctrl-C quits.

use std::io;

use crossterm::event::{self, KeyCode, KeyEvent, KeyModifiers};
use demand::{Confirm, DemandOption, Input, MultiSelect, Outcome, Select, Theme};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::{DefaultTerminal, Frame};

fn main() -> io::Result<()> {
    let theme = Theme::charm();
    let mut app = App::new(&theme);
    let ordered = ratatui::run(|terminal| app.run(terminal))?;

    if ordered {
        println!(
            "{}: {}\" with {}",
            app.name.input,
            app.size.selected().unwrap(),
            app.toppings.into_selected().join(", ")
        );
    }
    Ok(())
}

struct App<'a> {
    name: Input<'a>,
    size: Select<'a, u32>,
    toppings: MultiSelect<'a, &'static str>,
    confirm: Confirm<'a>,
    focus: usize,
}

impl<'a> App<'a> {
    fn new(theme: &'a Theme) -> Self {
        let name = Input::new("Name")
            .placeholder("Who's it for?")
            .theme(theme)
            .validation(|s| match s {
                "" => Err("required"),
                _ => Ok(()),
            });
        let size = Select::new("Size")
            .filterable(true)
            .theme(theme)
            .option(DemandOption::with_label("Small", 9))
            .option(DemandOption::with_label("Medium", 12).selected(true))
            .option(DemandOption::with_label("Large", 16));
        let toppings = ["Basil", "Olives", "Mushrooms", "Peppers", "Pineapple"];
        let toppings = MultiSelect::new("Toppings")
            .min(1)
            .filterable(true)
            .theme(theme)
            .options(toppings.map(DemandOption::new).into());
        let confirm = Confirm::new("Place the order?").theme(theme);

        Self {
            name,
            size,
            toppings,
            confirm,
            focus: 0,
        }
    }

    /// Run until the order is placed or abandoned, and return which.
    fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<bool> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;
            let Some(key) = event::read()?.as_key_press_event() else {
                continue;
            };
            match self.handle_key(key) {
                Outcome::Submit if self.focus == 3 => return Ok(self.confirm.selected),
                Outcome::Submit => self.focus += 1,
                Outcome::Cancel => return Ok(false),
                // Keys a prompt doesn't use are the app's.
                Outcome::Ignored => match key.code {
                    KeyCode::Tab => self.focus = (self.focus + 1) % 4,
                    KeyCode::BackTab => self.focus = (self.focus + 3) % 4,
                    _ => {}
                },
                _ => {}
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let rows = Layout::vertical([Constraint::Fill(1), Constraint::Length(9)]);
        let columns = Layout::horizontal([Constraint::Fill(1); 2]);
        let left_rows = Layout::vertical([Constraint::Length(6), Constraint::Fill(1)]);
        let [top, bottom] = rows.areas(frame.area());
        let [left, right] = columns.areas(top);
        let [name, size] = left_rows.areas(left);
        let mut inner = [name, size, right, bottom]
            .into_iter()
            .enumerate()
            .map(|(i, area)| {
                let block = Block::bordered().border_style(if i == self.focus {
                    Style::new().magenta()
                } else {
                    Style::new().dark_gray()
                });
                let inner = block.inner(area);
                frame.render_widget(block, area);
                inner
            });
        let areas: [_; 4] = std::array::from_fn(|_| inner.next().unwrap());
        drop(inner);
        frame.render_widget(&mut self.name, areas[0]);
        frame.render_widget(&mut self.size, areas[1]);
        frame.render_widget(&mut self.toppings, areas[2]);
        frame.render_widget(&mut self.confirm, areas[3]);
    }

    fn handle_key(&mut self, key: KeyEvent) -> Outcome {
        if key.code == KeyCode::Char('c') && key.modifiers == KeyModifiers::CONTROL {
            return Outcome::Cancel;
        }
        match self.focus {
            0 => self.name.handle_key(key),
            1 => self.size.handle_key(key),
            2 => self.toppings.handle_key(key),
            _ => self.confirm.handle_key(key),
        }
    }
}
