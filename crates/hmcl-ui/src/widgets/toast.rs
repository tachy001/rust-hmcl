//! Toast notifications, a port of HMCL's `ui.construct.HintPane`.

use std::time::Instant;

use egui::{Align, Align2, Color32, Context, CornerRadius, Id, Layout, RichText, Vec2};

use crate::theme;
use crate::widgets::icon;

const TOAST_LIFETIME: f64 = 4.0;

/// A single toast message.
#[derive(Clone)]
pub struct Toast {
    id: u64,
    kind: ToastKind,
    text: String,
    created: Instant,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Warning,
    Error,
}

impl ToastKind {
    fn icon(&self) -> &'static str {
        match self {
            ToastKind::Info => "INFO",
            ToastKind::Warning => "WARNING",
            ToastKind::Error => "ERROR",
        }
    }

    fn color(&self, palette: &theme::MonetPalette) -> Color32 {
        match self {
            ToastKind::Info => palette.primary,
            ToastKind::Warning => palette.tertiary,
            ToastKind::Error => palette.error,
        }
    }
}

/// The global toast queue.
#[derive(Default)]
pub struct Toasts {
    list: Vec<Toast>,
    next_id: u64,
}

impl Toasts {
    pub fn info(&mut self, text: impl Into<String>) {
        self.push(ToastKind::Info, text);
    }

    pub fn warning(&mut self, text: impl Into<String>) {
        self.push(ToastKind::Warning, text);
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.push(ToastKind::Error, text);
    }

    fn push(&mut self, kind: ToastKind, text: impl Into<String>) {
        self.list.push(Toast {
            id: self.next_id,
            kind,
            text: text.into(),
            created: Instant::now(),
        });
        self.next_id += 1;
        if self.list.len() > 5 {
            self.list.remove(0);
        }
    }

    /// Draw all live toasts in the bottom-right corner.
    pub fn show(&mut self, ctx: &Context) {
        let now = Instant::now();
        self.list
            .retain(|toast| now.duration_since(toast.created).as_secs_f64() < TOAST_LIFETIME);
        let palette = theme::palette();

        let mut y = ctx.screen_rect().max.y - 16.0;
        for toast in &self.list {
            let text_width = ctx.fonts(|f| {
                f.layout_no_wrap(
                    toast.text.clone(),
                    egui::FontId::proportional(13.0),
                    palette.on_surface,
                )
                .size()
                .x
            });
            let size = Vec2::new((text_width + 64.0).clamp(200.0, 420.0), 44.0);
            let pos = egui::pos2(ctx.screen_rect().max.x - 16.0 - size.x, y - size.y);
            y -= size.y + 8.0;

            egui::Area::new(Id::new(("toast", toast.id)))
                .order(egui::Order::Foreground)
                .fixed_pos(pos)
                .show(ctx, |ui| {
                    ui.set_width(size.x);
                    ui.set_height(size.y);
                    let rect = ui.max_rect();
                    ui.painter().rect_filled(
                        rect,
                        CornerRadius::same(10),
                        palette.surface_container_high,
                    );
                    ui.painter().rect_stroke(
                        rect,
                        CornerRadius::same(10),
                        egui::Stroke::new(1.0_f32, palette.outline_variant),
                        egui::StrokeKind::Inside,
                    );
                    let icon_rect = egui::Rect::from_min_size(
                        egui::pos2(rect.min.x + 14.0, rect.center().y - 10.0),
                        Vec2::splat(20.0),
                    );
                    icon::icon_in_rect(
                        ui.painter(),
                        icon_rect,
                        toast.kind.icon(),
                        toast.kind.color(&palette),
                    );
                    ui.painter().text(
                        egui::pos2(rect.min.x + 42.0, rect.center().y),
                        Align2::LEFT_CENTER,
                        &toast.text,
                        egui::FontId::proportional(13.0),
                        palette.on_surface,
                    );
                });
        }
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
}

/// Convenience: render an inline hint banner (port of `HintPane`).
pub fn hint(ui: &mut egui::Ui, kind: ToastKind, text: &str) {
    let palette = theme::palette();
    let color = kind.color(&palette);
    ui.horizontal(|ui| {
        let _ = icon::icon(ui, kind.icon(), 16.0, color);
        ui.label(RichText::new(text).color(color).size(12.0));
    });
}

/// A translucent rounded "card" container used by pages so the wallpaper
/// shows through slightly (port of HMCL's layered surfaces).
///
/// Contents are laid out vertically regardless of the caller's layout.
pub fn card<R>(
    ui: &mut egui::Ui,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let palette = theme::palette();
    egui::Frame::new()
        .fill(palette.surface.gamma_multiply(0.86))
        .stroke(egui::Stroke::new(
            1.0_f32,
            palette.outline_variant.gamma_multiply(0.5),
        ))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(16, 14))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical(add_contents).inner
        })
}

/// Show a frame helper used by hint banners.
pub fn hint_frame(ui: &mut egui::Ui, add_contents: impl FnOnce(&mut egui::Ui)) {
    let palette = theme::palette();
    egui::Frame::new()
        .fill(palette.surface_container)
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.with_layout(Layout::top_down(Align::Min), add_contents);
        });
}
