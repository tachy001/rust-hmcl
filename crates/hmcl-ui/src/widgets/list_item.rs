//! List item widgets, ports of HMCL's `ui.construct.TwoLineListItem` and
//! `AdvancedListItem`.

use egui::{Align2, Color32, Pos2, Rect, Response, Sense, Ui, Vec2};

use crate::theme;

/// A two-line list item: icon, title, subtitle and an optional trailing
/// widget. Returns the click response.
pub fn two_line_list_item(
    ui: &mut Ui,
    id: egui::Id,
    icon_name: Option<&str>,
    title: &str,
    subtitle: &str,
    selected: bool,
    clickable: bool,
) -> Response {
    let palette = theme::palette();
    let height = 56.0;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(ui.available_width(), height),
        if clickable {
            Sense::click()
        } else {
            Sense::hover()
        },
    );

    let bg = if selected {
        palette.primary_container
    } else if response.hovered() && clickable {
        palette.surface_container_high
    } else {
        Color32::TRANSPARENT
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(8), bg);

    let mut x = rect.min.x + 14.0;
    if let Some(icon_name) = icon_name {
        let icon_rect =
            Rect::from_min_size(Pos2::new(x, rect.center().y - 12.0), Vec2::splat(24.0));
        crate::widgets::icon::icon_in_rect(
            ui.painter(),
            icon_rect,
            icon_name,
            if selected {
                palette.on_primary_container
            } else {
                palette.on_surface_variant
            },
        );
        x += 38.0;
    }

    let title_color = if selected {
        palette.on_primary_container
    } else {
        palette.on_surface
    };
    let subtitle_color = if selected {
        palette.on_primary_container
    } else {
        palette.on_surface_variant
    };
    let text_max_width = (rect.max.x - x - 16.0).max(40.0);

    if subtitle.is_empty() {
        ui.painter().text(
            Pos2::new(x, rect.center().y),
            Align2::LEFT_CENTER,
            title,
            egui::FontId::proportional(15.0),
            title_color,
        );
    } else {
        ui.painter().text(
            Pos2::new(x, rect.min.y + 11.0),
            Align2::LEFT_CENTER,
            title,
            egui::FontId::proportional(15.0),
            title_color,
        );
        ui.painter().text(
            Pos2::new(x, rect.min.y + 32.0),
            Align2::LEFT_CENTER,
            subtitle,
            egui::FontId::proportional(12.0),
            subtitle_color,
        );
    }
    let _ = text_max_width;

    let id_ = id;
    let _ = id_;
    response
}

/// A single-line list item with optional leading icon (port of `AdvancedListItem`).
pub fn list_item(ui: &mut Ui, id: egui::Id, icon_name: Option<&str>, title: &str) -> Response {
    let palette = theme::palette();
    let height = 44.0;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
    let bg = if response.hovered() {
        palette.surface_container_high
    } else {
        Color32::TRANSPARENT
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(8), bg);

    let mut x = rect.min.x + 14.0;
    if let Some(icon_name) = icon_name {
        let icon_rect =
            Rect::from_min_size(Pos2::new(x, rect.center().y - 11.0), Vec2::splat(22.0));
        crate::widgets::icon::icon_in_rect(
            ui.painter(),
            icon_rect,
            icon_name,
            palette.on_surface_variant,
        );
        x += 36.0;
    }
    ui.painter().text(
        Pos2::new(x, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        egui::FontId::proportional(14.0),
        palette.on_surface,
    );
    let _ = id;
    response
}
