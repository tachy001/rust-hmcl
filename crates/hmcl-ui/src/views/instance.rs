//! The instance management, launch and settings pages.
//!
//! Ports of HMCL's `ui.instances.GameListPage` (placeholder), the launch
//! view of `MainPage` and `main.SettingsPage` (appearance section).

use egui::{Context, RichText, Ui};

use hmcl_core::auth::AccountStorage;

use crate::app::HmclApp;
use crate::theme::{self, Appearance};

/// Render the instance list page.
pub fn show(ctx: &Context) {
    let palette = theme::palette();
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(palette.surface))
        .show(ctx, |ui| {
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                ui.label(
                    RichText::new(crate::i18n::tr("instance.manage"))
                        .size(22.0)
                        .color(palette.on_surface),
                );
            });
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
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
            });
        });
}

/// Render the launch page (game overview).
pub fn show_game(ctx: &Context, accounts: &AccountStorage) {
    let palette = theme::palette();
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(palette.surface))
        .show(ctx, |ui| {
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                ui.label(
                    RichText::new(crate::i18n::tr("instance"))
                        .size(22.0)
                        .color(palette.on_surface),
                );
            });
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                let account_name = accounts
                    .selected
                    .as_ref()
                    .and_then(|uuid| accounts.accounts.iter().find(|a| a.uuid() == uuid))
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
                    RichText::new(crate::i18n::tr("instance.empty.launch.goto_download"))
                        .color(palette.primary)
                        .size(13.0),
                );
            });
        });
}

/// Render the settings page (appearance section for now).
pub fn show_settings(ctx: &Context, app: &mut HmclApp) {
    let palette = theme::palette();
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(palette.surface))
        .show(ctx, |ui| {
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                ui.add_space(24.0);
                ui.label(
                    RichText::new(crate::i18n::tr("settings"))
                        .size(22.0)
                        .color(palette.on_surface),
                );
            });
            ui.add_space(16.0);
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.horizontal(|ui| ui.add_space(24.0));
                ui.vertical(|ui| {
                    ui.set_width((ui.available_width() - 24.0).min(480.0));
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
                        }
                    });

                    // Accent color selector.
                    ui.add_space(8.0);
                    ui.label(crate::i18n::tr("settings.launcher.theme_color"));
                    ui.horizontal(|ui| {
                        for color in theme::STANDARD_COLORS {
                            let (rect, response) =
                                ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::click());
                            let selected = app.accent == *color;
                            ui.painter().circle_filled(rect.center(), 12.0, color.color());
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
                            }
                        }
                    });
                });
            });
        });
}

fn section_title(ui: &mut Ui, key: &str) {
    let palette = theme::palette();
    ui.add_space(8.0);
    ui.label(
        RichText::new(crate::i18n::tr(key).to_uppercase())
            .size(12.0)
            .color(palette.primary),
    );
    ui.add_space(6.0);
}
