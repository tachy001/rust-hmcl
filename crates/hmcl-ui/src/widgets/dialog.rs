//! Modal dialogs, a port of HMCL's `ui.construct.DialogPane` built on
//! `egui::Modal`.
//!
//! Dialogs render a title bar, a content area and a button row
//! (accept/cancel), following the launcher's Material style.

use egui::{Align2, Color32, Context, Id, RichText, Ui, Vec2};

use crate::theme;
use crate::widgets::icon;

/// The outcome of showing a dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogResult {
    /// The user pressed the accept (positive) button.
    Accept,
    /// The user pressed the cancel (negative) button.
    Cancel,
    /// The user dismissed the dialog (backdrop click or Escape).
    Dismissed,
}

/// Builder for a modal dialog.
pub struct Dialog {
    id: Id,
    title: String,
    width: f32,
    positive_text: Option<String>,
    negative_text: Option<String>,
    positive_enabled: bool,
}

impl Dialog {
    pub fn new(id: Id, title: impl Into<String>) -> Self {
        Self {
            id,
            title: title.into(),
            width: 420.0,
            positive_text: Some(crate::i18n::tr("button.ok")),
            negative_text: Some(crate::i18n::tr("button.cancel")),
            positive_enabled: true,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Set the accept button label (`None` hides it).
    pub fn positive_text(mut self, text: Option<String>) -> Self {
        self.positive_text = text;
        self
    }

    /// Set the cancel button label (`None` hides it).
    pub fn negative_text(mut self, text: Option<String>) -> Self {
        self.negative_text = text;
        self
    }

    /// Whether the accept button is clickable.
    pub fn positive_enabled(mut self, enabled: bool) -> Self {
        self.positive_enabled = enabled;
        self
    }

    /// Show the dialog with `content` rendered in the content area.
    ///
    /// Returns `None` while the dialog is still open, and the result once
    /// it is closed.
    pub fn show<T>(
        self,
        ctx: &Context,
        content: impl FnOnce(&mut Ui) -> T,
    ) -> Option<DialogResult> {
        let palette = theme::palette();
        let mut result: Option<DialogResult> = None;

        let mut modal = egui::Modal::new(self.id)
            .area(egui::Area::new(self.id.with("area")).anchor(Align2::CENTER_CENTER, Vec2::ZERO))
            .backdrop_color(Color32::from_black_alpha(96))
            .show(ctx, |ui| {
                ui.set_width(self.width);
                ui.set_max_width(self.width);
                dialog_frame(ui, palette);
                ui.scope_builder(
                    egui::UiBuilder::new().max_rect(ui.max_rect().shrink(8.0)),
                    |ui| {
                        ui.set_width(self.width - 16.0);
                        // Title bar
                        ui.horizontal(|ui| {
                            ui.add_space(10.0);
                            ui.label(
                                RichText::new(&self.title)
                                    .size(16.0)
                                    .color(palette.on_surface),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let (rect, response) = ui.allocate_exact_size(
                                        Vec2::splat(30.0),
                                        egui::Sense::click(),
                                    );
                                    if response.hovered() {
                                        ui.painter().circle_filled(
                                            rect.center(),
                                            14.0,
                                            palette.surface_container_highest,
                                        );
                                    }
                                    icon::icon_in_rect(
                                        ui.painter(),
                                        rect,
                                        "CLOSE",
                                        palette.on_surface_variant,
                                    );
                                    if response.clicked() {
                                        result = Some(DialogResult::Dismissed);
                                    }
                                },
                            );
                        });
                        ui.separator();
                        // Content
                        egui::ScrollArea::vertical()
                            .id_salt(self.id.with("scroll"))
                            .max_height(360.0)
                            .show(ui, |ui| {
                                ui.add_space(8.0);
                                content(ui);
                                ui.add_space(4.0);
                            });
                        // Button row
                        ui.add_space(8.0);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if let Some(text) = &self.negative_text
                                && crate::widgets::text_button(ui, self.id.with("neg"), text, false)
                                    .clicked()
                            {
                                result = Some(DialogResult::Cancel);
                            }
                            if let Some(text) = &self.positive_text
                                && crate::widgets::text_button(ui, self.id.with("pos"), text, true)
                                    .clicked()
                                && self.positive_enabled
                            {
                                result = Some(DialogResult::Accept);
                            }
                        });
                        ui.add_space(8.0);
                    },
                );
            });

        if result.is_some() {
            modal.response.set_close();
        }
        if modal.should_close() {
            result.or(Some(DialogResult::Dismissed))
        } else {
            None
        }
    }
}

/// Draw the rounded dialog background with a border.
fn dialog_frame(ui: &mut Ui, palette: theme::MonetPalette) {
    let rect = ui.max_rect();
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::same(12),
        palette.surface_container_high,
    );
    ui.painter().rect_stroke(
        rect,
        egui::CornerRadius::same(12),
        egui::Stroke::new(1.0_f32, palette.outline_variant),
        egui::StrokeKind::Inside,
    );
}

/// Show a simple message dialog with an OK button.
pub fn message(ctx: &Context, title: impl Into<String>, text: impl Into<String>) {
    Dialog::new(Id::new("__message__"), title)
        .negative_text(None)
        .show(ctx, |ui| {
            ui.label(text.into());
        });
}

/// Show a confirmation dialog with accept/cancel buttons.
pub fn confirm(
    ctx: &Context,
    title: impl Into<String>,
    text: impl Into<String>,
) -> Option<DialogResult> {
    Dialog::new(Id::new("__confirm__"), title).show(ctx, |ui| {
        ui.label(text.into());
    })
}
