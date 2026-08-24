//! The download/version list page.
//!
//! Port of HMCL's `ui.download.DownloadPage`: fetches the Mojang version
//! manifest and lists installable versions.

use egui::{Context, RichText, Ui};

use hmcl_core::download::version_list::{RemoteVersion, VersionManifest, VersionType};

use crate::async_runtime::{spawn, AsyncTask};
use crate::theme;
use crate::widgets::toast::{hint, ToastKind, Toasts};

/// Persistent state of the download page.
#[derive(Default)]
pub struct DownloadPage {
    task: Option<AsyncTask<VersionManifest>>,
    manifest: Option<VersionManifest>,
    error: Option<String>,
    search: String,
    tab: usize,
    initialized: bool,
}

impl DownloadPage {
    /// Render the page.
    pub fn show(&mut self, ctx: &Context, toasts: &mut Toasts) {
        let palette = theme::palette();
        if !self.initialized {
            self.initialized = true;
            self.refresh();
        }

        // Drain the fetch result.
        if let Some(task) = &self.task
            && let Some(result) = task.poll() {
                self.task = None;
                match result {
                    Ok(manifest) => self.manifest = Some(manifest),
                    Err(e) => self.error = Some(e),
                }
            }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(palette.surface))
            .show(ctx, |ui| {
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.label(
                        RichText::new(crate::i18n::tr("download"))
                            .size(22.0)
                            .color(palette.on_surface),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(16.0);
                        if ui.button(crate::i18n::tr("button.refresh")).clicked() {
                            self.refresh();
                        }
                    });
                    ui.add_space(24.0);
                });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.add_sized(
                        egui::vec2((ui.available_width() - 24.0).min(360.0), 32.0),
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text(crate::i18n::tr("search")),
                    );
                    ui.add_space(8.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(16.0);
                        let labels = vec![
                            crate::i18n::tr("instance.game.releases"),
                            crate::i18n::tr("instance.game.snapshots"),
                            crate::i18n::tr("instance.game.old"),
                        ];
                        let _ = crate::widgets::tab_bar(ui, ui.id().with("version_tabs"), &labels, &mut self.tab);
                    });
                });

                ui.add_space(8.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.horizontal(|ui| ui.add_space(24.0));
                    match (&self.manifest, &self.error) {
                        (Some(manifest), _) => self.version_list(ui, manifest, toasts),
                        (None, Some(error)) => {
                            hint(ui, ToastKind::Error, &crate::i18n::tr("download.failed"));
                            ui.label(error);
                        }
                        _ => {
                            ui.horizontal(|ui| {
                                crate::widgets::spinner(ui, 20.0);
                                ui.label(crate::i18n::tr("download.content"));
                            });
                        }
                    }
                });
            });
    }

    fn refresh(&mut self) {
        self.error = None;
        self.task = Some(spawn(async move {
            hmcl_core::download::fetch_version_manifest()
                .await
                .map_err(|e| format!("{e:#}"))
        }));
    }

    fn version_list(&self, ui: &mut Ui, manifest: &VersionManifest, toasts: &mut Toasts) {
        let palette = theme::palette();
        let search = self.search.trim().to_lowercase();
        let show_versions: Vec<&RemoteVersion> = manifest
            .versions
            .iter()
            .filter(|version| {
                let version_type = VersionType::of(&version.version_type);
                let matches_tab = match self.tab {
                    0 => version_type == VersionType::Release,
                    1 => version_type == VersionType::Snapshot,
                    _ => matches!(
                        version_type,
                        VersionType::OldBeta | VersionType::OldAlpha
                    ),
                };
                matches_tab && (search.is_empty() || version.id.to_lowercase().contains(&search))
            })
            .collect();

        egui::Grid::new(ui.id().with("version_grid"))
            .num_columns(3)
            .spacing(egui::vec2(24.0, 4.0))
            .show(ui, |ui| {
                ui.label(RichText::new(crate::i18n::tr("world.game_version")).color(palette.on_surface_variant));
                ui.label(RichText::new(crate::i18n::tr("instance.game.release")).color(palette.on_surface_variant));
                ui.end_row();
                for version in show_versions {
                    ui.label(
                        RichText::new(&version.id)
                            .size(15.0)
                            .color(palette.on_surface),
                    );
                    let version_type = VersionType::of(&version.version_type);
                    let label = match version_type {
                        VersionType::Release => crate::i18n::tr("instance.game.release"),
                        VersionType::Snapshot => crate::i18n::tr("instance.game.snapshot"),
                        _ => version.version_type.clone(),
                    };
                    ui.label(RichText::new(label).color(palette.on_surface_variant));
                    if ui
                        .small_button(crate::i18n::tr("button.install"))
                        .clicked()
                    {
                        toasts.info(crate::i18n::tr("install.new_game.installation"));
                    }
                    ui.end_row();
                }
            });
    }
}
