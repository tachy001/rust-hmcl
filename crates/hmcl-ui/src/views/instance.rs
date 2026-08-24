//! The instance management, launch and settings pages.
//!
//! Ports of HMCL's `ui.instances.GameListPage`, the launch view of
//! `MainPage` and `main.SettingsPage` (appearance section).

use std::time::{Duration, Instant};

use egui::{Context, RichText};

use hmcl_core::auth::AccountStorage;

use crate::app::HmclApp;
use crate::config::BUILTIN_WALLPAPERS;
use crate::theme::{self, Appearance};

/// One installed version entry.
#[derive(Debug, Clone)]
pub struct InstalledVersion {
    pub id: String,
    pub version_type: String,
}

/// Persistent state of the instance list page.
#[derive(Default)]
pub struct InstancePage {
    versions: Vec<InstalledVersion>,
    last_scan: Option<Instant>,
}

impl InstancePage {
    /// Scan the versions directory (throttled to once per second).
    fn scan(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last_scan
            && now.duration_since(last) < Duration::from_secs(1)
        {
            return;
        }
        self.last_scan = Some(now);

        let versions_dir =
            hmcl_core::download::default_game_dir(&crate::data_dir()).join("versions");
        let mut versions = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&versions_dir) {
            for entry in entries.flatten() {
                let json_path = entry
                    .path()
                    .join(format!("{}.json", entry.file_name().to_string_lossy()));
                if !json_path.exists() {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&json_path) else {
                    continue;
                };
                let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) else {
                    continue;
                };
                let id = json
                    .get("id")
                    .and_then(|value| value.as_str())
                    .unwrap_or_default()
                    .to_owned();
                let version_type = json
                    .get("type")
                    .and_then(|value| value.as_str())
                    .unwrap_or("custom")
                    .to_owned();
                if !id.is_empty() {
                    versions.push(InstalledVersion { id, version_type });
                }
            }
        }
        versions.sort_by(|a, b| a.id.cmp(&b.id));
        self.versions = versions;
    }
}

/// Render the instance list page.
pub fn show(ctx: &Context, page: &mut InstancePage) {
    page.scan();
    let palette = theme::palette();
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::same(20))
                .show(ui, |ui| {
                    crate::widgets::card(ui, |ui| {
                        ui.label(
                            RichText::new(crate::i18n::tr("instance.manage"))
                                .size(20.0)
                                .color(palette.on_surface),
                        );
                        ui.add_space(12.0);
                        if page.versions.is_empty() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(24.0);
                                ui.label(
                                    RichText::new(crate::i18n::tr("instance.empty"))
                                        .color(palette.on_surface_variant)
                                        .size(15.0),
                                );
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(crate::i18n::tr("instance.empty.add"))
                                        .color(palette.on_surface_variant)
                                        .size(13.0),
                                );
                                ui.add_space(24.0);
                            });
                        } else {
                            for version in &page.versions {
                                let type_label = match version.version_type.as_str() {
                                    "release" => crate::i18n::tr("instance.game.release"),
                                    "snapshot" => crate::i18n::tr("instance.game.snapshot"),
                                    other => other.to_owned(),
                                };
                                crate::widgets::two_line_list_item(
                                    ui,
                                    ui.id().with(("version", &version.id)),
                                    Some("GRASS"),
                                    &version.id,
                                    &type_label,
                                    false,
                                    true,
                                );
                            }
                            ui.add_space(8.0);
                        }
                    });
                });
        });
}

/// Render the launch page (game overview), with the HMCL title logo
/// (port of `MainPage`'s `titleNode`: `icon-title.png` rotated 180°).
pub fn show_game(ctx: &Context, accounts: &AccountStorage) {
    let palette = theme::palette();
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::same(20))
                .show(ui, |ui| {
                    crate::widgets::card(ui, |ui| {
                        ui.horizontal(|ui| {
                            if let Some(logo) = crate::image::texture(ctx, "img/icon-title.png") {
                                let size = logo.size_vec2();
                                let scale = 24.0 / size.y.max(1.0);
                                let rect = egui::Rect::from_min_size(ui.cursor().min, size * scale);
                                ui.painter().image(
                                    logo.id(),
                                    rect,
                                    egui::Rect::from_min_max(
                                        egui::Pos2::ZERO,
                                        egui::Pos2::new(1.0, 1.0),
                                    ),
                                    egui::Color32::WHITE,
                                );
                                ui.advance_cursor_after_rect(rect);
                            }
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(crate::i18n::tr("instance"))
                                    .size(20.0)
                                    .color(palette.on_surface),
                            );
                        });
                        ui.add_space(20.0);
                        ui.vertical_centered(|ui| {
                            ui.add_space(16.0);
                            let account_name = accounts
                                .selected
                                .as_ref()
                                .and_then(|uuid| {
                                    accounts.accounts.iter().find(|a| a.uuid() == uuid)
                                })
                                .map(|a| a.username().to_owned())
                                .unwrap_or_else(|| crate::i18n::tr("account.missing"));
                            ui.label(
                                RichText::new(account_name)
                                    .size(18.0)
                                    .color(palette.on_surface),
                            );
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(crate::i18n::tr("instance.launch.empty"))
                                    .color(palette.on_surface_variant)
                                    .size(14.0),
                            );
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(crate::i18n::tr(
                                    "instance.empty.launch.goto_download",
                                ))
                                .color(palette.primary)
                                .size(13.0),
                            );
                            ui.add_space(24.0);
                        });
                    });
                });
        });
}

/// Render the settings page (appearance section for now).
pub fn show_settings(ctx: &Context, app: &mut HmclApp) {
    let palette = theme::palette();
    let mut changed = false;
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show(ctx, |ui| {
            egui::Frame::new()
                .inner_margin(egui::Margin::same(20))
                .show(ui, |ui| {
                    crate::widgets::card(ui, |ui| {
                        ui.label(
                            RichText::new(crate::i18n::tr("settings"))
                                .size(20.0)
                                .color(palette.on_surface),
                        );
                        ui.add_space(12.0);
                        egui::ScrollArea::vertical()
                            .max_height(ui.available_height())
                            .show(ui, |ui| {
                                section_title(ui, "settings.launcher.appearance");

                                // Brightness (appearance) selector.
                                let mut appearance = app.appearance;
                                ui.horizontal(|ui| {
                                    ui.label(crate::i18n::tr("settings.launcher.brightness"));
                                    let before = appearance;
                                    ui.radio_value(
                                        &mut appearance,
                                        Appearance::Light,
                                        crate::i18n::tr("settings.launcher.brightness.light"),
                                    );
                                    ui.radio_value(
                                        &mut appearance,
                                        Appearance::Dark,
                                        crate::i18n::tr("settings.launcher.brightness.dark"),
                                    );
                                    if appearance != before {
                                        app.appearance = appearance;
                                        app.apply_theme(ctx);
                                        changed = true;
                                    }
                                });

                                // Accent color selector.
                                ui.add_space(10.0);
                                ui.label(crate::i18n::tr("settings.launcher.theme_color"));
                                ui.horizontal(|ui| {
                                    for color in theme::STANDARD_COLORS {
                                        let (rect, response) = ui.allocate_exact_size(
                                            egui::vec2(28.0, 28.0),
                                            egui::Sense::click(),
                                        );
                                        let selected = app.accent == *color;
                                        ui.painter().circle_filled(
                                            rect.center(),
                                            12.0,
                                            color.color(),
                                        );
                                        if selected {
                                            ui.painter().circle_stroke(
                                                rect.center(),
                                                14.0,
                                                egui::Stroke::new(2.0_f32, palette.on_surface),
                                            );
                                        }
                                        if response.clicked() {
                                            app.accent = *color;
                                            app.apply_theme(ctx);
                                            changed = true;
                                        }
                                    }
                                });

                                // Wallpaper selector.
                                ui.add_space(12.0);
                                section_title(ui, "launcher.background");
                                ui.label(crate::i18n::tr("launcher.background.builtin"));
                                ui.horizontal_wrapped(|ui| {
                                    if wallpaper_button(ui, "none", "NONE", &app.config.wallpaper) {
                                        app.config.wallpaper = "none".to_owned();
                                        changed = true;
                                    }
                                    for (id, _file) in BUILTIN_WALLPAPERS {
                                        if wallpaper_button(ui, id, id, &app.config.wallpaper) {
                                            app.config.wallpaper = (*id).to_owned();
                                            changed = true;
                                        }
                                    }
                                });
                                ui.add_space(10.0);
                                ui.label(crate::i18n::tr(
                                    "settings.launcher.background.settings.opacity",
                                ));
                                let mut opacity = app.config.background_opacity;
                                if ui
                                    .add(
                                        egui::Slider::new(&mut opacity, 0.0..=1.0)
                                            .show_value(false),
                                    )
                                    .changed()
                                {
                                    app.config.background_opacity = opacity;
                                    changed = true;
                                }
                                ui.add_space(8.0);
                            });
                    });
                });
        });
    if changed {
        app.save_config();
    }
}

/// A small wallpaper preview button.
fn wallpaper_button(ui: &mut egui::Ui, id: &str, label: &str, current: &str) -> bool {
    let palette = theme::palette();
    let selected = current == id;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(64.0, 48.0), egui::Sense::click());
    let bg = if selected {
        palette.primary
    } else {
        palette.surface_container_high
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(6), bg);
    if !selected {
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(6),
            egui::Stroke::new(1.0_f32, palette.outline_variant),
            egui::StrokeKind::Inside,
        );
    }
    let fg = if selected {
        palette.on_primary
    } else {
        palette.on_surface_variant
    };
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(10.0),
        fg,
    );
    response.clicked()
}

fn section_title(ui: &mut egui::Ui, key: &str) {
    let palette = theme::palette();
    ui.add_space(8.0);
    ui.label(
        RichText::new(crate::i18n::tr(key).to_uppercase())
            .size(12.0)
            .color(palette.primary),
    );
    ui.add_space(6.0);
}
