//! Theme system.
//!
//! Port of HMCL's `org.jackhuang.hmcl.theme` package adapted to egui:
//! Material You (Monet) palettes mapped onto `egui::Visuals`.
//!
//! The light palette values come from `assets/css/blue.css`; dark values are
//! the standard Material 3 dark tonal counterparts (same as HMCL's generated
//! dark scheme).

mod accent;
mod palette;

pub use accent::{AccentColor, STANDARD_COLORS};
pub use palette::MonetPalette;

/// Light or dark appearance, mirroring `ThemeAppearance.Brightness`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Appearance {
    #[default]
    Light,
    Dark,
}

impl Appearance {
    pub fn is_dark(&self) -> bool {
        matches!(self, Appearance::Dark)
    }
}

/// Build egui `Visuals` for the given appearance and accent color.
pub fn visuals(appearance: Appearance, accent: AccentColor) -> egui::Visuals {
    let palette = MonetPalette::resolve(appearance, accent);
    let mut visuals = if appearance.is_dark() {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    let egui::Visuals {
        dark_mode: _,
        override_text_color,
        widgets,
        selection,
        hyperlink_color,
        window_corner_radius,
        window_shadow,
        ..
    } = &mut visuals;

    *window_corner_radius = egui::CornerRadius::same(10);
    *window_shadow = egui::epaint::Shadow::NONE;

    *override_text_color = Some(palette.on_surface);

    widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, palette.on_surface_variant);
    widgets.noninteractive.bg_fill = palette.surface_container;
    widgets.noninteractive.weak_bg_fill = palette.surface_container_low;
    widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0_f32, palette.outline_variant);

    widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, palette.on_surface);
    widgets.inactive.bg_fill = palette.surface_container_high;
    widgets.inactive.weak_bg_fill = palette.surface_container_high;
    widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, palette.outline_variant);

    widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, palette.on_surface);
    widgets.hovered.bg_fill = palette.surface_container_highest;
    widgets.hovered.weak_bg_fill = palette.surface_container_highest;
    widgets.hovered.bg_stroke = egui::Stroke::new(1.0_f32, palette.outline);

    widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, palette.on_primary);
    widgets.active.bg_fill = palette.primary;
    widgets.active.weak_bg_fill = palette.primary;
    widgets.active.bg_stroke = egui::Stroke::new(1.0_f32, palette.primary);

    widgets.open.fg_stroke = egui::Stroke::new(1.0_f32, palette.on_primary);
    widgets.open.bg_fill = palette.primary;
    widgets.open.weak_bg_fill = palette.primary;
    widgets.open.bg_stroke = egui::Stroke::new(1.0_f32, palette.primary);

    *selection = egui::style::Selection {
        bg_fill: palette.primary_container,
        stroke: egui::Stroke::new(1.0_f32, palette.primary),
    };
    *hyperlink_color = palette.primary;

    visuals.panel_fill = palette.surface;
    visuals.extreme_bg_color = palette.surface;
    visuals.faint_bg_color = palette.surface_container;
    visuals.window_fill = palette.surface_container_high;
    visuals.window_stroke = egui::Stroke::new(1.0_f32, palette.outline_variant);
    visuals.slider_trailing_fill = false;
    visuals
}

/// Apply the theme to a full egui `Style` (fonts untouched).
pub fn apply_style(style: &mut egui::Style, appearance: Appearance, accent: AccentColor) {
    style.visuals = visuals(appearance, accent);
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.spacing.button_padding = egui::vec2(12.0, 6.0);
    style.spacing.interact_size.y = 30.0;
    style.spacing.scroll = egui::style::ScrollStyle::solid();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_palette_light() {
        let palette = MonetPalette::resolve(Appearance::Light, AccentColor::Blue);
        assert_eq!(palette.primary.to_hex(), "#4352a5ff");
        assert_eq!(palette.on_primary.to_hex(), "#ffffffff");
        assert_eq!(palette.surface.to_hex(), "#fbf8ffff");
        assert_eq!(palette.on_surface.to_hex(), "#1b1b21ff");
    }

    #[test]
    fn test_palette_dark() {
        let palette = MonetPalette::resolve(Appearance::Dark, AccentColor::Blue);
        assert_eq!(palette.primary.to_hex(), "#bac3ffff");
        assert_eq!(palette.surface.to_hex(), "#121318ff");
    }
}


