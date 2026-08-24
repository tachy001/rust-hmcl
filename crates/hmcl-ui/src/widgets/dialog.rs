//! Modal dialogs, a port of HMCL's `ui.construct.DialogPane` built on
//! `egui::Modal`.
//!
//! Dialogs render a title bar, a content area and a button row
//! (accept/cancel), following the launcher's Material style.

use egui::{Align2, Color32, Context, Id, Pos2, Rect, RichText, Ui, Vec2};

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
    height: f32,
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
            height: 260.0,
            positive_text: Some(crate::i18n::tr("button.ok")),
            negative_text: Some(crate::i18n::tr("button.cancel")),
            positive_enabled: true,
        }
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Set the fixed dialog box height.
    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
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
                // Fixed-size dialog box: the size is fully deterministic so
                // it cannot feed back into the next frame's layout.
                let (rect, _) = ui
                    .allocate_exact_size(Vec2::new(self.width, self.height), egui::Sense::hover());
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

                let mut child = ui
                    .new_child(egui::UiBuilder::new().max_rect(rect.shrink2(Vec2::new(10.0, 6.0))));
                child.set_width(self.width - 20.0);
                // Title bar
                child.horizontal(|ui| {
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new(&self.title)
                            .size(16.0)
                            .color(palette.on_surface),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (rect, response) =
                            ui.allocate_exact_size(Vec2::splat(30.0), egui::Sense::click());
                        if response.hovered() {
                            ui.painter().circle_filled(
                                rect.center(),
                                14.0,
                                palette.surface_container_highest,
                            );
                        }
                        icon::icon_in_rect(ui.painter(), rect, "CLOSE", palette.on_surface_variant);
                        if response.clicked() {
                            result = Some(DialogResult::Dismissed);
                        }
                    });
                });
                child.separator();
                // Content
                egui::ScrollArea::vertical()
                    .id_salt(self.id.with("scroll"))
                    .max_height((self.height - 104.0).max(60.0))
                    .show(&mut child, |ui| {
                        ui.add_space(8.0);
                        content(ui);
                        ui.add_space(4.0);
                    });
                // Button row pinned at the bottom of the dialog box.
                let button_rect = Rect::from_min_max(
                    Pos2::new(rect.min.x + 8.0, rect.max.y - 46.0),
                    Pos2::new(rect.max.x - 8.0, rect.max.y - 6.0),
                );
                let mut buttons = ui.new_child(egui::UiBuilder::new().max_rect(button_rect));
                buttons.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
