//! Loading indicators, ports of HMCL's spinner and progress bar widgets.

use egui::{Align2, Pos2, Rect, Stroke, Ui, Vec2};

use crate::theme;

/// A Material-style circular loading spinner.
pub fn spinner(ui: &mut Ui, size: f32) {
    let palette = theme::palette();
    let (rect, _response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    let time = ui.input(|i| i.time);
    let angle = time as f32 * std::f32::consts::TAU * 0.6;
    let radius = size / 2.0 - 2.0;
    let center = rect.center();
    let points = 24;
    let mut last = None;
    for i in 0..=points {
        let a = angle + (i as f32 / points as f32) * std::f32::consts::TAU * 0.8;
        let (sin, cos) = a.sin_cos();
        let p = Pos2::new(center.x + cos * radius, center.y + sin * radius);
        if let Some(prev) = last {
            let alpha = (i as f32 / points as f32) * 200.0 + 40.0;
            ui.painter().line_segment(
                [prev, p],
                Stroke::new(size / 9.0, palette.primary.gamma_multiply(alpha / 255.0)),
            );
        }
        last = Some(p);
    }
}

/// A progress bar with percentage text.
pub fn progress_bar(ui: &mut Ui, fraction: f32, height: f32) {
    let palette = theme::palette();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), height), egui::Sense::hover());
    let radius_u8 = (height / 2.0) as u8;
    ui.painter().rect_filled(
        rect,
        egui::CornerRadius::same(radius_u8),
        palette.surface_container_high,
    );
    let fill_width = (rect.width() * fraction.clamp(0.0, 1.0)).max(0.0);
    if fill_width > 0.0 {
        let fill = Rect::from_min_max(rect.min, Pos2::new(rect.min.x + fill_width, rect.max.y));
        ui.painter().rect_filled(
            fill,
            egui::CornerRadius::same(radius_u8),
            palette.primary,
        );
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        format!("{:.0}%", fraction * 100.0),
        egui::FontId::proportional(11.0),
        palette.on_primary,
    );
}
