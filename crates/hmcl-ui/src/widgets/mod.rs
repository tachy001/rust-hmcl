//! Reusable widgets, ports of HMCL's `ui.construct` package and `SVG` icons.

pub mod dialog;
pub mod icon;
pub mod icons_data;
pub mod list_item;
pub mod spinner;
pub mod tab;
pub mod toast;
pub mod validator;

pub use dialog::{confirm, message, Dialog, DialogResult};
pub use icon::{icon, icon_in_rect, icon_shapes, parse_path};
pub use list_item::{list_item, two_line_list_item};
pub use spinner::{progress_bar, spinner};
pub use tab::tab_bar;
pub use toast::{card, hint, hint_frame, ToastKind, Toasts};
pub use validator::{validate_all, NumberValidator, RequiredValidator, UrlValidator, Validator};
