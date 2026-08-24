//! Reusable widgets, ports of HMCL's `ui.construct` package and `SVG` icons.

pub mod button;
pub mod dialog;
pub mod icon;
pub mod icons_data;
pub mod list_item;
pub mod spinner;
pub mod tab;
pub mod text_field;
pub mod toast;
pub mod validator;

pub use button::{BUTTON_HEIGHT, filled_button, icon_button, outlined_button, text_button};
pub use dialog::{Dialog, DialogResult, confirm, message};
pub use icon::{icon, icon_in_rect, icon_shapes, parse_path};
pub use list_item::{list_item, two_line_list_item};
pub use spinner::{progress_bar, spinner};
pub use tab::tab_bar;
pub use text_field::{rounded_text_edit_multiline, rounded_text_edit_singleline};
pub use toast::{ToastKind, Toasts, card, hint, hint_frame};
pub use validator::{NumberValidator, RequiredValidator, UrlValidator, Validator, validate_all};
