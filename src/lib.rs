//! A prompt library for Rust. Based on [huh? for Go](https://github.com/charmbracelet/huh).

pub use confirm::Confirm;
pub use dialog::Dialog;
pub use dialog::DialogButton;
pub use editor::Editor;
pub use grid_select::{GridRow, GridSelect};
pub use handle::PromptHandle;
pub use input::{
    Autocomplete, AutocompleteClone, FnAutocomplete, Input, InputParser, InputValidator,
    NoAutocompletion,
};
pub use list::List;
pub use multiselect::MultiSelect;
pub use option::DemandOption;
pub use prompt::{KeyPress, Outcome};
pub use select::Select;
pub use spinner::Spinner;
pub use spinner::SpinnerStyle;
pub use theme::Theme;
pub use wizard::{Navigation, Steps, Wizard, handle_navigation_key};

mod confirm;
#[cfg_attr(any(windows), path = "ctrlc_stub.rs")]
mod ctrlc;
mod dialog;
mod editor;
mod event;
mod frame;
mod fuzzy;
mod grid_select;
mod handle;
mod height;
mod host;
mod input;
mod list;
mod multiselect;
mod option;
mod prompt;
mod select;
mod spinner;
mod synchronized_output;
mod theme;
mod tty;
mod view;
mod wizard;

#[cfg(test)]
mod test;
