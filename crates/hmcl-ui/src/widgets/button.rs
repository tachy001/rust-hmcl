//! Material-style buttons, ports of HMCL's `jfx-button` styles.
//!
//! Three variants mirroring HMCL's button set: contained (primary),
//! outlined and text buttons, all with the launcher's capsule radius.

use egui::{Align2, CornerRadius, Pos2, Rect, Response, Sense, Ui, Vec2};

use crate::theme;
use crate::widgets::icon;

/// The height of standard buttons.
pub const BUTTON_HEIGHT: f32 = 36.0;

/// A contained (primary) button with an optional leading icon.
pub fn filled_button(ui: &mut Ui, id: egui::Id, label: &str, icon_name: Option<&str>) -> Response {
    let palette = theme::palette();
    let text_width = ui.fonts(|f| {
        f.layout_no_wrap(label.to_owned(), egui::FontId::proportional(14.0), palette.on_primary)
            .size()
            .x
    });
    let icon_space = if icon_name.is_some() { 28.0 } else { 0.0 };
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(text_width + icon_space + 32.0, BUTTON_HEIGHT),
        Sense::click(),
    );
    let (bg, fg) = if response.is_pointer_button_down_on() {
        (palette.primary, palette.on_primary)
    } else if response.hovered() {
        (palette.primary_container, palette.on_primary_container)
    } else {
        (palette.primary, palette.on_primary)
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same((BUTTON_HEIGHT / 2.0) as u8), bg);
    let mut x = rect.min.x + 16.0;
    if let Some(icon_name) = icon_name {
        let icon_rect = Rect::from_min_size(Pos2::new(x, rect.center().y - 10.0), Vec2::splat(20.0));
        icon::icon_in_rect(ui.painter(), icon_rect, icon_name, fg);
        x += icon_space;
    }
    ui.painter().text(
        Pos2::new(x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(14.0),
        fg,
    );
    let _ = id;
    response
}

/// An outlined (secondary) button with an optional leading icon.
pub fn outlined_button(ui: &mut Ui, id: egui::Id, label: &str, icon_name: Option<&str>) -> Response {
    let palette = theme::palette();
    let text_width = ui.fonts(|f| {
        f.layout_no_wrap(label.to_owned(), egui::FontId::proportional(14.0), palette.on_surface)
            .size()
            .x
    });
    let icon_space = if icon_name.is_some() { 28.0 } else { 0.0 };
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(text_width + icon_space + 32.0, BUTTON_HEIGHT),
        Sense::click(),
    );
    let bg = if response.hovered() {
        palette.surface_container_highest
    } else {
        palette.surface_container
    };
    ui.painter()
        .rect_filled(rect, CornerRadius::same((BUTTON_HEIGHT / 2.0) as u8), bg);
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same((BUTTON_HEIGHT / 2.0) as u8),
        egui::Stroke::new(1.0_f32, palette.outline),
        egui::StrokeKind::Inside,
    );
    let mut x = rect.min.x + 16.0;
    if let Some(icon_name) = icon_name {
        let icon_rect = Rect::from_min_size(Pos2::new(x, rect.center().y - 10.0), Vec2::splat(20.0));
        icon::icon_in_rect(ui.painter(), icon_rect, icon_name, palette.primary);
        x += icon_space;
    }
    ui.painter().text(
        Pos2::new(x, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(14.0),
        palette.primary,
    );
    let _ = id;
    response
}

/// A borderless text button (HMCL dialog button style).
pub fn text_button(ui: &mut Ui, id: egui::Id, label: &str, primary: bool) -> Response {
    let palette = theme::palette();
    let text_width = ui.fonts(|f| {
        f.layout_no_wrap(label.to_owned(), egui::FontId::proportional(14.0), palette.on_surface)
            .size()
            .x
    });
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(text_width + 32.0, BUTTON_HEIGHT),
        Sense::click(),
    );
    let fill = if response.hovered() {
        palette.surface_container_highest
    } else {
        egui::Color32::TRANSPARENT
    };
    let fg = if primary { palette.primary } else { palette.on_surface_variant };
    ui.painter()
        .rect_filled(rect, CornerRadius::same((BUTTON_HEIGHT / 2.0) as u8), fill);
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(14.0),
        fg,
    );
    let _ = id;
    response
}

/// A small icon-only round button.
pub fn icon_button(ui: &mut Ui, id: egui::Id, icon_name: &str, size: f32) -> Response {
    let palette = theme::palette();
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let bg = if response.hovered() {
        palette.surface_container_highest
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter()
        .circle_filled(rect.center(), size / 2.0, bg);
    icon::icon_in_rect(ui.painter(), rect, icon_name, palette.on_surface_variant);
    let _ = id;
    response
}

