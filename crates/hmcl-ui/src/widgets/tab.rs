//! Tab bar widget, a port of HMCL's `ui.construct.TabControl`.

use egui::{Align2, Id, Pos2, Rect, Sense, Ui, Vec2};

use crate::theme;

/// A horizontally scrolling row of tabs with an animated underline indicator.
///
/// Returns `true` when the selection changed.
pub fn tab_bar(ui: &mut Ui, id: Id, labels: &[String], selected: &mut usize) -> bool {
    let palette = theme::palette();
    let height = 40.0;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::hover());

    let mut changed = false;
    let mut x = rect.min.x;
    let mut selected_rect = None;
    for (index, label) in labels.iter().enumerate() {
        let text_width = ui.fonts(|f| {
            f.layout_no_wrap(
                label.clone(),
                egui::FontId::proportional(14.0),
                palette.on_surface,
            )
            .size()
            .x
        });
        let tab_rect = Rect::from_min_max(
            Pos2::new(x, rect.min.y),
            Pos2::new(x + text_width + 32.0, rect.max.y),
        );
        let tab_response = ui.interact(tab_rect, id.with(index), Sense::click());
        if tab_response.hovered() && index != *selected {
            ui.painter().rect_filled(
                tab_rect,
                egui::CornerRadius::same(6),
                palette.surface_container_high,
            );
        }
        let fg = if index == *selected {
            palette.primary
        } else {
            palette.on_surface_variant
        };
        ui.painter().text(
            tab_rect.center(),
            Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(14.0),
            fg,
        );
        if tab_response.clicked() {
            *selected = index;
            changed = true;
        }
        if index == *selected {
            selected_rect = Some(tab_rect);
        }
        x = tab_rect.max.x;
    }

    if let Some(tab_rect) = selected_rect {
        // Material-style underline indicator.
        let indicator = Rect::from_min_max(
            Pos2::new(tab_rect.min.x + 16.0, tab_rect.max.y - 3.0),
            Pos2::new(tab_rect.max.x - 16.0, tab_rect.max.y - 1.0),
        );
        ui.painter()
            .rect_filled(indicator, egui::CornerRadius::same(1), palette.primary);
    }

    // Bottom hairline.
    ui.painter().hline(
        rect.min.x..=rect.max.x,
        rect.max.y - 1.0,
        egui::Stroke::new(1.0_f32, palette.outline_variant),
    );

    let _ = response;
    changed
}
