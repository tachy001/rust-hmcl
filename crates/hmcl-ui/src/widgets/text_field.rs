//! Rounded text fields, a port of HMCL's `jfx-text-field` style.

use egui::{CornerRadius, Response, Sense, Ui, Vec2};

use crate::theme;

/// A single-line text field with a rounded, bordered container.
pub fn rounded_text_edit_singleline(
    ui: &mut Ui,
    text: &mut String,
    hint: &str,
    width: f32,
) -> Response {
    let palette = theme::palette();
    let height = 36.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(8),
        palette.surface_container_high,
    );
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(8),
        egui::Stroke::new(1.0_f32, palette.outline_variant),
        egui::StrokeKind::Inside,
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(10.0, 0.0)))
            .layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight)),
    );
    child.style_mut().visuals.widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
    child.style_mut().visuals.widgets.hovered.bg_fill = egui::Color32::TRANSPARENT;
    child.style_mut().visuals.widgets.active.bg_fill = egui::Color32::TRANSPARENT;
    child.style_mut().visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    child.style_mut().visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    child.style_mut().visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
    child.add(egui::TextEdit::singleline(text).hint_text(hint))
}

/// A multi-line text field with a rounded, bordered container.
pub fn rounded_text_edit_multiline(
    ui: &mut Ui,
    text: &mut String,
    width: f32,
    height: f32,
) -> Response {
    let palette = theme::palette();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(8),
        palette.surface_container_high,
    );
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(8),
        egui::Stroke::new(1.0_f32, palette.outline_variant),
        egui::StrokeKind::Inside,
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new().max_rect(rect.shrink2(Vec2::new(10.0, 8.0))),
    );
    child.style_mut().visuals.widgets.inactive.bg_fill = egui::Color32::TRANSPARENT;
    child.style_mut().visuals.widgets.hovered.bg_fill = egui::Color32::TRANSPARENT;
    child.style_mut().visuals.widgets.active.bg_fill = egui::Color32::TRANSPARENT;
    child.style_mut().visuals.widgets.inactive.bg_stroke = egui::Stroke::NONE;
    child.style_mut().visuals.widgets.hovered.bg_stroke = egui::Stroke::NONE;
    child.style_mut().visuals.widgets.active.bg_stroke = egui::Stroke::NONE;
    child.add(egui::TextEdit::multiline(text).desired_rows(3))
}
