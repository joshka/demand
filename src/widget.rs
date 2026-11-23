use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Position, Rect};
use ratatui_core::style::{Color, Modifier, Style};
use ratatui_core::widgets::Widget;
use termcolor::ColorSpec;

use crate::prompt::Prompt;
use crate::view::{Size, View};
use crate::{
    Confirm, Dialog, Editor, GridSelect, Input, List, MultiSelect, Select, Spinner, Steps,
};

macro_rules! widget {
    ($($prompt:ident $(<$t:ident>)?),*) => {$(
        impl<$($t)?> Widget for &mut $prompt<'_ $(, $t)?> {
            fn render(self, area: Rect, buf: &mut Buffer) {
                render(self, area, buf);
            }
        }
    )*};
}

widget!(
    Confirm,
    Dialog,
    Editor,
    GridSelect<T>,
    List,
    MultiSelect<T>,
    Select<T>
);

impl Widget for &mut Input<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.cursor_position = render(self, area, buf);
    }
}

impl Input<'_> {
    /// Where the text caret was when the input was last rendered, to place
    /// the terminal's own cursor on it with `Frame::set_cursor_position`.
    /// The input draws a caret whether or not that's done.
    pub fn cursor_position(&self) -> Option<Position> {
        self.cursor_position
    }
}

/// The spinner turns as time passes, however often it's rendered.
impl Widget for &Spinner<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        render_view(&self.view(), area, buf);
    }
}

impl Widget for &Steps<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        render_view(&self.view(area.width as usize), area, buf);
    }
}

/// Draw `prompt` in `area`, and return where its text caret is.
fn render(prompt: &mut impl Prompt, area: Rect, buf: &mut Buffer) -> Option<Position> {
    let area = area.intersection(buf.area);
    let view = prompt.view(Size {
        // On the command line the cursor rests on a row of its own below
        // the prompt, which isn't needed here.
        rows: area.height as usize + 1,
        width: area.width as usize,
    });
    render_view(&view, area, buf)
}

/// Draw `view` in `area`, and return where its text caret is.
fn render_view(view: &View, area: Rect, buf: &mut Buffer) -> Option<Position> {
    let area = area.intersection(buf.area);
    if area.is_empty() {
        return None;
    }
    let (width, height) = (area.width as usize, area.height as usize);
    let layout = view.layout(width);

    // When there isn't room for everything, scroll to what's focused.
    let focus = layout.caret.map(|(row, _)| row).or(layout.focus);
    let skip = focus.map_or(0, |row| (row + 1).saturating_sub(height));

    for (y, row) in layout.rows.iter().skip(skip).take(height).enumerate() {
        for cell in row {
            if cell.col + cell.width > width {
                // Too wide for the area even at the start of a row.
                continue;
            }
            let (x, y) = (area.x + cell.col as u16, area.y + y as u16);
            let style = style(&cell.color);
            buf[(x, y)].set_symbol(&cell.text).set_style(style);
            for i in 1..cell.width as u16 {
                buf[(x + i, y)].reset();
                buf[(x + i, y)].set_style(style);
            }
        }
    }
    layout.caret.map(|(row, col)| {
        Position::new(
            area.x + col as u16,
            area.y + row.saturating_sub(skip) as u16,
        )
    })
}

fn style(spec: &ColorSpec) -> Style {
    let mut style = Style::new();
    if let Some(fg) = spec.fg() {
        style = style.fg(color(fg, spec.intense()));
    }
    if let Some(bg) = spec.bg() {
        style = style.bg(color(bg, spec.intense()));
    }
    for (set, modifier) in [
        (spec.bold(), Modifier::BOLD),
        (spec.dimmed(), Modifier::DIM),
        (spec.italic(), Modifier::ITALIC),
        (spec.underline(), Modifier::UNDERLINED),
        (spec.strikethrough(), Modifier::CROSSED_OUT),
    ] {
        if set {
            style = style.add_modifier(modifier);
        }
    }
    style
}

fn color(color: &termcolor::Color, intense: bool) -> Color {
    use termcolor::Color as C;
    match (color, intense) {
        (C::Black, false) => Color::Black,
        (C::Black, true) => Color::DarkGray,
        (C::Red, false) => Color::Red,
        (C::Red, true) => Color::LightRed,
        (C::Green, false) => Color::Green,
        (C::Green, true) => Color::LightGreen,
        (C::Yellow, false) => Color::Yellow,
        (C::Yellow, true) => Color::LightYellow,
        (C::Blue, false) => Color::Blue,
        (C::Blue, true) => Color::LightBlue,
        (C::Magenta, false) => Color::Magenta,
        (C::Magenta, true) => Color::LightMagenta,
        (C::Cyan, false) => Color::Cyan,
        (C::Cyan, true) => Color::LightCyan,
        (C::White, false) => Color::Gray,
        (C::White, true) => Color::White,
        (C::Ansi256(n), _) => Color::Indexed(*n),
        (C::Rgb(r, g, b), _) => Color::Rgb(*r, *g, *b),
        _ => Color::Reset,
    }
}

#[cfg(test)]
mod tests {
    use console::Key;

    use super::*;
    use crate::{DemandOption, GridRow, Outcome, Theme};

    fn draw(widget: impl Widget, width: u16, height: u16) -> Vec<String> {
        let mut buf = Buffer::empty(Rect::new(0, 0, width, height));
        widget.render(buf.area, &mut buf);
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buf[(x, y)].symbol())
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect()
    }

    fn countries(n: usize) -> Select<'static, usize> {
        Select::new("Pick").options(
            (0..n)
                .map(|i| DemandOption::new(i).label(&format!("item {i}")))
                .collect(),
        )
    }

    #[test]
    fn a_select_returns_its_typed_item() {
        let mut select = countries(3);
        // Keys work before the first render.
        assert_eq!(select.handle_key(Key::ArrowDown), Outcome::Handled);
        assert_eq!(
            draw(&mut select, 40, 8),
            [
                "Pick",
                "  item 0",
                "❯ item 1",
                "  item 2",
                "↑/↓/k/j up/down • enter confirm",
                "",
                "",
                ""
            ]
        );
        assert_eq!(select.handle_key(Key::Enter), Outcome::Submit);
        assert_eq!(select.into_selected(), Some(1));
    }

    #[test]
    fn the_focused_option_stays_in_a_small_area() {
        for height in 1..12 {
            let mut select = countries(30);
            for i in 0..30 {
                let lines = draw(&mut select, 30, height);
                assert!(
                    lines.iter().any(|l| l == &format!("❯ item {i}")),
                    "item {i} not visible in {height} rows: {lines:?}"
                );
                select.handle_key(Key::ArrowDown);
            }
        }
    }

    #[test]
    fn escape_clears_a_filter_before_cancelling() {
        let mut select = countries(3).filterable(true);
        for key in ['/', '2'] {
            assert_eq!(select.handle_key(Key::Char(key)), Outcome::Handled);
        }
        assert_eq!(select.selected(), Some(&2));
        assert_eq!(select.handle_key(Key::Escape), Outcome::Handled);
        assert_eq!(select.handle_key(Key::Tab), Outcome::Ignored);
        assert_eq!(select.handle_key(Key::Escape), Outcome::Cancel);
    }

    #[test]
    fn an_input_reports_its_caret_and_validates() {
        let mut input = Input::new("Name").validation(|s| match s {
            "" => Err("required"),
            _ => Ok(()),
        });
        assert_eq!(input.handle_key(Key::Enter), Outcome::Handled);
        assert_eq!(input.error(), Some("required"));
        assert_eq!(draw(&mut input, 20, 6)[4], "✗ required");
        for c in "日本".chars() {
            input.handle_key(Key::Char(c));
        }
        let mut buf = Buffer::empty(Rect::new(3, 2, 20, 5));
        (&mut input).render(Rect::new(3, 2, 20, 5), &mut buf);
        assert_eq!(input.cursor_position(), Some(Position::new(9, 3)));
        assert_eq!(input.handle_key(Key::Enter), Outcome::Submit);
        assert_eq!(input.input, "日本");
    }

    #[test]
    fn a_wide_character_wraps_whole() {
        let mut input = Input::new("N").default_value("ab日");
        // "> ab" leaves one column, too few for the last character.
        assert_eq!(draw(&mut input, 5, 3), ["N", "> ab", "日"]);
        assert_eq!(input.cursor_position(), Some(Position::new(2, 2)));
    }

    #[test]
    fn nothing_is_drawn_outside_the_area() {
        let area = Rect::new(2, 2, 7, 4);
        let outer = Rect::new(0, 0, 20, 10);
        let check = |render: &mut dyn FnMut(&mut Buffer)| {
            let mut buf = Buffer::empty(outer);
            render(&mut buf);
            for pos in outer.positions().filter(|p| !area.contains(*p)) {
                assert_eq!(buf[pos].symbol(), " ", "{pos}");
            }
        };
        let long = "日本語 wide and long";
        let mut confirm = Confirm::new(long);
        check(&mut |b| (&mut confirm).render(area, b));
        let mut dialog = Dialog::new(long);
        check(&mut |b| (&mut dialog).render(area, b));
        let mut select = Select::new(long).option(DemandOption::new(long));
        check(&mut |b| (&mut select).render(area, b));
        let mut multi = MultiSelect::new(long).option(DemandOption::new(long));
        check(&mut |b| (&mut multi).render(area, b));
        let mut input = Input::new(long).default_value(long);
        check(&mut |b| (&mut input).render(area, b));
        let mut list = List::new(long).item(long);
        check(&mut |b| (&mut list).render(area, b));
        let mut grid = GridSelect::new(long).row(GridRow::new(long).cell(long));
        check(&mut |b| (&mut grid).render(area, b));
        // Zero-sized and off-screen areas are fine too.
        check(&mut |b| (&mut grid).render(Rect::new(2, 2, 0, 0), b));
        check(&mut |b| (&mut grid).render(Rect::new(90, 90, 5, 5), b));
        check(&mut |b| (&mut input).render(Rect::new(2, 2, 1, 1), b));
    }

    #[test]
    fn every_prompt_answers() {
        let mut confirm = Confirm::new("?");
        assert_eq!(confirm.handle_key(Key::Char('n')), Outcome::Submit);
        assert!(!confirm.selected);
        let mut dialog = Dialog::new("?");
        dialog.handle_key(Key::ArrowRight);
        assert_eq!(dialog.handle_key(Key::Enter), Outcome::Submit);
        assert_eq!(dialog.selected_label(), Some("Cancel"));
        let mut multi = MultiSelect::new("?")
            .min(1)
            .option(DemandOption::new("a"))
            .option(DemandOption::new("b"));
        assert_eq!(multi.handle_key(Key::Enter), Outcome::Handled);
        multi.handle_key(Key::Char('a'));
        assert_eq!(multi.handle_key(Key::Enter), Outcome::Submit);
        assert_eq!(multi.selected(), [&"a", &"b"]);
        assert_eq!(multi.into_selected(), ["a", "b"]);
        let mut grid = GridSelect::new("?").row(GridRow::new("a").cell("1").cell("2"));
        assert_eq!(grid.handle_key(Key::ArrowRight), Outcome::Handled);
        assert_eq!(grid.handle_key(Key::Enter), Outcome::Submit);
        assert_eq!(grid.into_selected(), [("a", 1)]);
        let mut list = List::new("?").item("a");
        assert_eq!(list.handle_key(Key::Enter), Outcome::Submit);
    }

    #[test]
    fn an_editor_asks_for_the_terminal() {
        let mut editor = Editor::new("Notes")
            .default_value("one")
            .editor_command("printf 'two' >");
        assert_eq!(draw(&mut editor, 40, 5)[1], "  one");
        assert_eq!(editor.handle_key(Key::Char('e')), Outcome::Edit);
        // Nothing has been run until the host is ready for it.
        assert_eq!(editor.text(), "one");
        assert_eq!(editor.handle_key(Key::Enter), Outcome::Submit);
        assert_eq!(editor.handle_key(Key::Escape), Outcome::Cancel);
    }

    #[test]
    fn a_spinner_turns_with_time() {
        let style = crate::SpinnerStyle {
            frames: vec!["a", "b"],
            fps: std::time::Duration::from_millis(20),
        };
        let spinner = Spinner::new("Working").style(&style);
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..20 {
            seen.insert(draw(&spinner, 20, 1).remove(0));
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(Vec::from_iter(seen), ["a Working", "b Working"]);
    }

    #[test]
    fn steps_show_where_the_user_is() {
        use crate::Navigation;
        let mut steps = Steps::new("Setup").step("A").step("B").step("C");
        assert_eq!(steps.navigate(Navigation::Next), Outcome::Handled);
        let separator = steps.theme.breadcrumb_separator.clone();
        assert_eq!(
            draw(&steps, 40, 2),
            [
                "Setup".to_string(),
                format!("1:A{separator}[2:B]{separator}3:C")
            ]
        );
        assert!(steps.back());
        assert!(!steps.back());
        steps.navigate(Navigation::Jump(9));
        assert_eq!(steps.current(), 0);
        assert_eq!(steps.navigate(Navigation::Done), Outcome::Submit);
    }

    #[test]
    fn theme_styles_carry_over() {
        let theme = Theme::dracula();
        let mut confirm = Confirm::new("Title").theme(&theme);
        let mut buf = Buffer::empty(Rect::new(0, 0, 40, 8));
        (&mut confirm).render(buf.area, &mut buf);
        assert_eq!(buf[(0, 0)].style().fg, style(&theme.title).fg);
        assert_eq!(buf[(3, 3)].style().bg, style(&theme.focused_button).bg);
    }

    #[test]
    fn tab_is_left_to_the_app_once_completed() {
        let mut input = Input::new("?").autocomplete_fn(|_| Ok(vec!["rust".to_string()]));
        assert_eq!(input.handle_key(Key::Tab), Outcome::Handled);
        assert_eq!(input.input, "rust");
        assert_eq!(input.handle_key(Key::Tab), Outcome::Ignored);
    }

    #[cfg(feature = "crossterm")]
    #[test]
    fn crossterm_keys() {
        use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
        let mut input = Input::new("?").default_value("one two");
        input.handle_key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL));
        assert_eq!(input.input, "one ");
        input.handle_key(KeyEvent::new(KeyCode::Char('A'), KeyModifiers::SHIFT));
        assert_eq!(input.input, "one A");
        input.handle_key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::ALT));
        input.handle_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::ALT));
        assert_eq!(input.input, "A");
        let mut release = KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE);
        release.kind = KeyEventKind::Release;
        assert_eq!(input.handle_key(release), Outcome::Ignored);
        assert_eq!(input.input, "A");
    }
}
