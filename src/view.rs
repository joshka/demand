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
/// The CLI host encodes a view as ANSI.
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
                Op::Caret => {}
            }
        }
        String::from_utf8(out.into_inner()).unwrap()
    }

    /// Lay the view out in rows `width` columns wide, wrapping the way a
    /// terminal does.
    pub(crate) fn layout(&self, width: usize) -> Layout {
        let mut layout = Layout { caret: None };
        let (mut row, mut col) = (0, 0);
        let mut buf = [0; 4];
        let mut caret_pending = false;
        for op in &self.ops {
            match op {
                Op::Style(_) | Op::Reset => {}
                Op::Caret => {
                    layout.caret = Some((row, col.min(width.saturating_sub(1))));
                    caret_pending = true;
                }
                Op::Text(text) => {
                    for c in console::strip_ansi_codes(text).chars() {
                        if c == '\n' {
                            row += 1;
                            col = 0;
                            caret_pending = false;
                            continue;
                        }
                        let w = measure_text_width(c.encode_utf8(&mut buf));
                        if w == 0 {
                            caret_pending = false;
                            continue;
                        }
                        if width > 0 && col + w > width {
                            row += 1;
                            col = 0;
                        }
                        if caret_pending {
                            // Where the character under the caret ended up,
                            // which can be on the next row.
                            layout.caret = Some((row, col));
                            caret_pending = false;
                        }
                        col += w;
                    }
                }
            }
        }
        layout
    }
}

/// Where things are in a [`View`] wrapped to a certain width.
#[derive(Debug)]
pub(crate) struct Layout {
    /// The (row, column) of the text caret.
    pub caret: Option<(usize, usize)>,
}
