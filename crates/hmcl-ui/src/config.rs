//! Launcher configuration persisted to `config.json` in the data directory.
//!
//! Covers the launcher appearance settings mirrored from HMCL's
//! `Config.Settings`: appearance, accent color, background wallpaper and
//! its opacity.

use serde::{Deserialize, Serialize};

use crate::theme::{AccentColor, Appearance};

/// The built-in wallpaper ids, mirroring `BuiltinBackground`.
pub const BUILTIN_WALLPAPERS: &[(&str, &str)] = &[
    ("2021-08-26", "2021-08-26.jpg"),
    ("2016-02-25", "2016-02-25.jpg"),
    ("2015-06-22", "2015-06-22.jpg"),
];

/// The persisted launcher configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LauncherConfig {
    pub appearance: String,
    pub accent: String,
    /// The wallpaper id (builtin id or `none`).
    pub wallpaper: String,
    /// The wallpaper opacity in `[0, 1]`.
    pub background_opacity: f32,
}

impl Default for LauncherConfig {
    fn default() -> Self {
        Self {
            appearance: "light".to_owned(),
            accent: "blue".to_owned(),
            wallpaper: "2021-08-26".to_owned(),
            background_opacity: 1.0,
        }
    }
}

impl LauncherConfig {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        if path.exists() {
            let text = std::fs::read_to_string(path)?;
            Ok(serde_json::from_str(&text)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn appearance(&self) -> Appearance {
        match self.appearance.as_str() {
            "dark" => Appearance::Dark,
            _ => Appearance::Light,
        }
    }

    pub fn accent(&self) -> AccentColor {
        AccentColor::of(&self.accent).unwrap_or_default()
    }
}
