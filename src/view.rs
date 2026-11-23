use std::fmt;

use console::measure_text_width;
use termcolor::{Buffer, ColorSpec, WriteColor};

/// The size a prompt is laid out for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Size {
    pub rows: usize,
    pub width: usize,
}

/// What a prompt looks like, independent of where it's shown: styled text
/// and the positions in it that a host needs to know about.
///
/// The CLI host encodes a view as ANSI, the ratatui one lays it out into a
/// buffer.
#[derive(Debug, Default, Clone)]
pub(crate) struct View {
    ops: Vec<Op>,
}

#[derive(Debug, Clone, PartialEq)]
enum Op {
    Text(String),
    Style(ColorSpec),
    Reset,
    Caret,
    Focus,
}

impl View {
    /// Style the text that follows.
    pub(crate) fn set_color(&mut self, color: &ColorSpec) {
        self.ops.push(Op::Style(color.clone()));
    }

    /// Stop styling the text that follows.
    pub(crate) fn reset(&mut self) {
        self.ops.push(Op::Reset);
    }

    /// The text caret is on whatever is written next.
    pub(crate) fn mark_caret(&mut self) {
        self.ops.push(Op::Caret);
    }

    /// Whatever is written next has to stay on screen when there isn't
    /// room for the whole view.
    pub(crate) fn mark_focus(&mut self) {
        self.ops.push(Op::Focus);
    }

    /// Lets `write!` and `writeln!` write to a view, which can't fail.
    pub(crate) fn write_fmt(&mut self, args: fmt::Arguments<'_>) {
        let text = args.to_string();
        match self.ops.last_mut() {
            Some(Op::Text(last)) => last.push_str(&text),
            _ => self.ops.push(Op::Text(text)),
        }
    }

    /// Write `text` with a block on the character at index `cursor`, or
    /// after the end of it.
    pub(crate) fn write_with_caret(&mut self, text: &str, cursor: usize, block: &ColorSpec) {
        let start = text
            .char_indices()
            .nth(cursor)
            .map_or(text.len(), |(i, _)| i);
        let end = start + text[start..].chars().next().map_or(0, char::len_utf8);
        write!(self, "{}", &text[..start]);
        self.mark_caret();
        self.set_color(block);
        if start == end {
            write!(self, " ");
        } else {
            write!(self, "{}", &text[start..end]);
        }
        self.reset();
        if end < text.len() {
            write!(self, "{}", &text[end..]);
        }
    }

    /// Write the keys a prompt responds to.
    pub(crate) fn write_help<K: AsRef<str>>(
        &mut self,
        theme: &crate::Theme,
        keys: &[(K, &str)],
        leading_separator: bool,
    ) {
        for (i, (key, desc)) in keys.iter().enumerate() {
            if i > 0 || leading_separator {
                self.set_color(&theme.help_sep);
                write!(self, " • ");
            }
            self.set_color(&theme.help_key);
            write!(self, "{}", key.as_ref());
            self.set_color(&theme.help_desc);
            write!(self, " {desc}");
        }
    }

    pub(crate) fn to_ansi(&self) -> String {
        let mut out = Buffer::ansi();
        for op in &self.ops {
            // Writing to a buffer in memory can't fail.
            match op {
                Op::Text(text) => std::io::Write::write_all(&mut out, text.as_bytes()).unwrap(),
                Op::Style(color) => out.set_color(color).unwrap(),
                Op::Reset => out.reset().unwrap(),
                Op::Caret | Op::Focus => {}
            }
        }
        String::from_utf8(out.into_inner()).unwrap()
    }

    /// Lay the view out in rows `width` columns wide, wrapping the way a
    /// terminal does.
    pub(crate) fn layout(&self, width: usize) -> Layout {
        let mut layout = Layout {
            rows: vec![vec![]],
            caret: None,
            focus: None,
        };
        let mut color = ColorSpec::new();
        let mut col = 0;
        let mut buf = [0; 4];
        let mut caret_pending = false;
        for op in &self.ops {
            match op {
                Op::Style(c) => color = c.clone(),
                Op::Reset => color = ColorSpec::new(),
                Op::Caret => {
                    layout.caret = Some((layout.rows.len() - 1, col.min(width.saturating_sub(1))));
                    caret_pending = true;
                }
                Op::Focus => layout.focus = Some(layout.rows.len() - 1),
                Op::Text(text) => {
                    for c in console::strip_ansi_codes(text).chars() {
                        if c == '\n' {
                            layout.rows.push(vec![]);
                            col = 0;
                            caret_pending = false;
                            continue;
                        }
                        let w = measure_text_width(c.encode_utf8(&mut buf));
                        if w == 0 {
                            // A combining mark belongs to the character
                            // before it, and control codes aren't drawn.
                            if !c.is_control()
                                && let Some(cell) = layout.rows.last_mut().unwrap().last_mut()
                            {
                                cell.text.push(c);
                            }
                            caret_pending = false;
                            continue;
                        }
                        if width > 0 && col + w > width {
                            layout.rows.push(vec![]);
                            col = 0;
                        }
                        if caret_pending {
                            // Where the character under the caret ended up,
                            // which can be on the next row.
                            layout.caret = Some((layout.rows.len() - 1, col));
                            caret_pending = false;
                        }
                        layout.rows.last_mut().unwrap().push(Cell {
                            col,
                            width: w,
                            text: c.to_string(),
                            color: color.clone(),
                        });
                        col += w;
                    }
                }
            }
        }
        layout
    }
}

/// A [`View`] in rows of a certain width.
#[derive(Debug)]
pub(crate) struct Layout {
    pub rows: Vec<Vec<Cell>>,
    /// The (row, column) of the text caret.
    pub caret: Option<(usize, usize)>,
    /// The row to keep on screen.
    #[cfg_attr(not(feature = "ratatui"), allow(dead_code))]
    pub focus: Option<usize>,
}

#[derive(Debug)]
#[cfg_attr(not(feature = "ratatui"), allow(dead_code))]
pub(crate) struct Cell {
    pub col: usize,
    pub width: usize,
    pub text: String,
    pub color: ColorSpec,
}
