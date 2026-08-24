//! Material You (Monet) color palettes.
//!
//! The light palette mirrors `assets/css/blue.css`; the dark palette uses
//! the standard Material 3 dark tonal counterparts of the same scheme.

use egui::Color32;

use super::{AccentColor, Appearance};

/// The Monet palette role colors mapped to egui.
#[derive(Debug, Clone, Copy)]
pub struct MonetPalette {
    pub primary: Color32,
    pub on_primary: Color32,
    pub primary_container: Color32,
    pub on_primary_container: Color32,
    pub secondary: Color32,
    pub on_secondary: Color32,
    pub secondary_container: Color32,
    pub on_secondary_container: Color32,
    pub tertiary: Color32,
    pub error: Color32,
    pub on_error: Color32,
    pub error_container: Color32,
    pub surface: Color32,
    pub on_surface: Color32,
    pub surface_variant: Color32,
    pub on_surface_variant: Color32,
    pub surface_container_lowest: Color32,
    pub surface_container_low: Color32,
    pub surface_container: Color32,
    pub surface_container_high: Color32,
    pub surface_container_highest: Color32,
    pub outline: Color32,
    pub outline_variant: Color32,
}

impl MonetPalette {
    /// Resolve the palette for `appearance` and `accent`.
    ///
    /// The six standard accents ship hand-picked Material 3 palettes; custom
    /// accents fall back to the blue scheme with the accent color as primary.
    pub fn resolve(appearance: Appearance, accent: AccentColor) -> Self {
        match accent {
            AccentColor::Blue => blue(appearance),
            AccentColor::DarkerBlue => darker_blue(appearance),
            AccentColor::Green => green(appearance),
            AccentColor::Orange => orange(appearance),
            AccentColor::Purple => purple(appearance),
            AccentColor::Red => red(appearance),
            AccentColor::Custom(color) => {
                let mut palette = blue(appearance);
                palette.primary = color;
                let [r, g, b, _] = color.to_array();
                let luminance = 0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32;
                palette.on_primary = if luminance > 150.0 {
                    Color32::from_rgb(0x00, 0x10, 0x5B)
                } else {
                    Color32::WHITE
                };
                palette
            }
        }
    }
}

fn blue(appearance: Appearance) -> MonetPalette {
    if appearance.is_dark() {
        MonetPalette {
            primary: Color32::from_rgb(0xBA, 0xC3, 0xFF),
            on_primary: Color32::from_rgb(0x00, 0x10, 0x5B),
            primary_container: Color32::from_rgb(0x2F, 0x3F, 0x92),
            on_primary_container: Color32::from_rgb(0xDE, 0xE0, 0xFF),
            secondary: Color32::from_rgb(0xBF, 0xC4, 0xEC),
            on_secondary: Color32::from_rgb(0x28, 0x2E, 0x4F),
            secondary_container: Color32::from_rgb(0x3F, 0x45, 0x66),
            on_secondary_container: Color32::from_rgb(0xDE, 0xE0, 0xFF),
            tertiary: Color32::from_rgb(0xE2, 0xC0, 0x6D),
            error: Color32::from_rgb(0xFF, 0xB4, 0xAB),
            on_error: Color32::from_rgb(0x69, 0x00, 0x05),
            error_container: Color32::from_rgb(0x93, 0x00, 0x0A),
            surface: Color32::from_rgb(0x12, 0x13, 0x18),
            on_surface: Color32::from_rgb(0xE3, 0xE1, 0xE9),
            surface_variant: Color32::from_rgb(0x45, 0x46, 0x4F),
            on_surface_variant: Color32::from_rgb(0xC6, 0xC5, 0xD0),
            surface_container_lowest: Color32::from_rgb(0x0D, 0x0E, 0x13),
            surface_container_low: Color32::from_rgb(0x1B, 0x1B, 0x21),
            surface_container: Color32::from_rgb(0x1F, 0x1F, 0x25),
            surface_container_high: Color32::from_rgb(0x29, 0x29, 0x30),
            surface_container_highest: Color32::from_rgb(0x34, 0x34, 0x3B),
            outline: Color32::from_rgb(0x90, 0x8F, 0x99),
            outline_variant: Color32::from_rgb(0x45, 0x46, 0x4F),
        }
    } else {
        MonetPalette {
            primary: Color32::from_rgb(0x43, 0x52, 0xA5),
            on_primary: Color32::WHITE,
            primary_container: Color32::from_rgb(0x5C, 0x6B, 0xC0),
            on_primary_container: Color32::from_rgb(0xF8, 0xF6, 0xFF),
            secondary: Color32::from_rgb(0x57, 0x5C, 0x7F),
            on_secondary: Color32::WHITE,
            secondary_container: Color32::from_rgb(0xD0, 0xD5, 0xFD),
            on_secondary_container: Color32::from_rgb(0x56, 0x5B, 0x7D),
            tertiary: Color32::from_rgb(0x77, 0x52, 0x00),
            error: Color32::from_rgb(0xBA, 0x1A, 0x1A),
            on_error: Color32::WHITE,
            error_container: Color32::from_rgb(0xFF, 0xDA, 0xD6),
            surface: Color32::from_rgb(0xFB, 0xF8, 0xFF),
            on_surface: Color32::from_rgb(0x1B, 0x1B, 0x21),
            surface_variant: Color32::from_rgb(0xE2, 0xE1, 0xEF),
            on_surface_variant: Color32::from_rgb(0x45, 0x46, 0x51),
            surface_container_lowest: Color32::WHITE,
            surface_container_low: Color32::from_rgb(0xF5, 0xF2, 0xFA),
            surface_container: Color32::from_rgb(0xEF, 0xED, 0xF5),
            surface_container_high: Color32::from_rgb(0xE9, 0xE7, 0xEF),
            surface_container_highest: Color32::from_rgb(0xE3, 0xE1, 0xE9),
            outline: Color32::from_rgb(0x76, 0x76, 0x80),
            outline_variant: Color32::from_rgb(0xC7, 0xC5, 0xD0),
        }
    }
}

/// Generate a palette for a single accent color using Material 3 baseline
/// neutral tones. Only the primary tones are accent-specific.
fn with_accent(
    appearance: Appearance,
    primary: [u8; 3],
    on_primary: [u8; 3],
    container: [u8; 3],
    on_container: [u8; 3],
) -> MonetPalette {
    let mut palette = blue(appearance);
    palette.primary = Color32::from_rgb(primary[0], primary[1], primary[2]);
    palette.on_primary = Color32::from_rgb(on_primary[0], on_primary[1], on_primary[2]);
    palette.primary_container = Color32::from_rgb(container[0], container[1], container[2]);
    palette.on_primary_container =
        Color32::from_rgb(on_container[0], on_container[1], on_container[2]);
    palette
}

fn darker_blue(appearance: Appearance) -> MonetPalette {
    if appearance.is_dark() {
        with_accent(
            appearance,
            [0x9F, 0xA8, 0xFF],
            [0x00, 0x06, 0x60],
            [0x20, 0x2E, 0x7A],
            [0xDE, 0xE0, 0xFF],
        )
    } else {
        with_accent(
            appearance,
            [0x28, 0x35, 0x93],
            [0xFF, 0xFF, 0xFF],
            [0x1A, 0x27, 0x8E],
            [0xDE, 0xE0, 0xFF],
        )
    }
}

fn green(appearance: Appearance) -> MonetPalette {
    if appearance.is_dark() {
        with_accent(
            appearance,
            [0x89, 0xD9, 0x80],
            [0x00, 0x39, 0x0B],
            [0x14, 0x5F, 0x26],
            [0xB2, 0xF2, 0xB2],
        )
    } else {
        with_accent(
            appearance,
            [0x43, 0xA0, 0x47],
            [0xFF, 0xFF, 0xFF],
            [0x30, 0x87, 0x36],
            [0xE2, 0xF2, 0xDF],
        )
    }
}

fn orange(appearance: Appearance) -> MonetPalette {
    if appearance.is_dark() {
        with_accent(
            appearance,
            [0xFF, 0xB7, 0x75],
            [0x4D, 0x2A, 0x00],
            [0x7A, 0x48, 0x00],
            [0xFF, 0xDC, 0xC2],
        )
    } else {
        with_accent(
            appearance,
            [0xE6, 0x7E, 0x22],
            [0xFF, 0xFF, 0xFF],
            [0xC5, 0x63, 0x00],
            [0xFF, 0xE1, 0xC7],
        )
    }
}

fn purple(appearance: Appearance) -> MonetPalette {
    if appearance.is_dark() {
        with_accent(
            appearance,
            [0xE8, 0xB9, 0xFF],
            [0x49, 0x07, 0x5F],
            [0x62, 0x20, 0x78],
            [0xF5, 0xD9, 0xFF],
        )
    } else {
        with_accent(
            appearance,
            [0x9C, 0x27, 0xB0],
            [0xFF, 0xFF, 0xFF],
            [0x80, 0x00, 0x96],
            [0xF5, 0xD9, 0xFF],
        )
    }
}

fn red(appearance: Appearance) -> MonetPalette {
    if appearance.is_dark() {
        with_accent(
            appearance,
            [0xFF, 0xB3, 0xB3],
            [0x68, 0x00, 0x08],
            [0x8F, 0x0E, 0x1C],
            [0xFF, 0xDA, 0xD9],
        )
    } else {
        with_accent(
            appearance,
            [0xB7, 0x1C, 0x1C],
            [0xFF, 0xFF, 0xFF],
            [0x98, 0x00, 0x0E],
            [0xFF, 0xDA, 0xD9],
        )
    }
}
