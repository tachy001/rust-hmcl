//! Accent colors, mirroring `ThemeColor.STANDARD_COLORS`.

use egui::Color32;

/// A named or custom accent color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AccentColor {
    #[default]
    Blue,
    DarkerBlue,
    Green,
    Orange,
    Purple,
    Red,
    Custom(Color32),
}

impl AccentColor {
    pub fn name(&self) -> &'static str {
        match self {
            AccentColor::Blue => "blue",
            AccentColor::DarkerBlue => "darker_blue",
            AccentColor::Green => "green",
            AccentColor::Orange => "orange",
            AccentColor::Purple => "purple",
            AccentColor::Red => "red",
            AccentColor::Custom(_) => "custom",
        }
    }

    /// Parse a color name or `#RRGGBB` hex string, returning `None` when invalid.
    pub fn of(name: &str) -> Option<Self> {
        if !name.starts_with('#') {
            for color in STANDARD_COLORS {
                if name.eq_ignore_ascii_case(color.name()) {
                    return Some(*color);
                }
            }
        }
        let hex = name.strip_prefix('#')?;
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let value = u32::from_str_radix(hex, 16).ok()?;
        Some(AccentColor::Custom(Color32::from_rgb(
            ((value >> 16) & 0xFF) as u8,
            ((value >> 8) & 0xFF) as u8,
            (value & 0xFF) as u8,
        )))
    }

    pub fn color(&self) -> Color32 {
        match self {
            AccentColor::Blue => Color32::from_rgb(0x5C, 0x6B, 0xC0),
            AccentColor::DarkerBlue => Color32::from_rgb(0x28, 0x35, 0x93),
            AccentColor::Green => Color32::from_rgb(0x43, 0xA0, 0x47),
            AccentColor::Orange => Color32::from_rgb(0xE6, 0x7E, 0x22),
            AccentColor::Purple => Color32::from_rgb(0x9C, 0x27, 0xB0),
            AccentColor::Red => Color32::from_rgb(0xB7, 0x1C, 0x1C),
            AccentColor::Custom(color) => *color,
        }
    }
}

/// The six standard accent colors, same as `ThemeColor.STANDARD_COLORS`.
pub const STANDARD_COLORS: &[AccentColor] = &[
    AccentColor::Blue,
    AccentColor::DarkerBlue,
    AccentColor::Green,
    AccentColor::Orange,
    AccentColor::Purple,
    AccentColor::Red,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_of() {
        assert_eq!(AccentColor::of("blue"), Some(AccentColor::Blue));
        assert_eq!(AccentColor::of("BLUE"), Some(AccentColor::Blue));
        assert_eq!(
            AccentColor::of("#FF0000"),
            Some(AccentColor::Custom(Color32::from_rgb(0xFF, 0, 0)))
        );
        assert_eq!(AccentColor::of("invalid"), None);
        assert_eq!(AccentColor::of("#12345"), None);
    }
}
