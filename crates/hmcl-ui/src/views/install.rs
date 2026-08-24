//! The download/version list page.
//!
//! Port of HMCL's `ui.download.DownloadPage`: fetches the Mojang version
//! manifest and lists installable versions.

use egui::{Context, RichText, Ui};

use hmcl_core::download::install::InstallTask;
use hmcl_core::download::spawn_install;
use hmcl_core::download::version_list::{RemoteVersion, VersionManifest, VersionType};

use crate::async_runtime::{AsyncTask, spawn};
use crate::theme;
use crate::widgets::toast::{ToastKind, Toasts, hint};

/// Persistent state of the download page.
#[derive(Default)]
pub struct DownloadPage {
    task: Option<AsyncTask<VersionManifest>>,
    manifest: Option<VersionManifest>,
    error: Option<String>,
    search: String,
    tab: usize,
    initialized: bool,
    install: Option<InstallTask>,
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
            && let Some(result) = task.poll()
        {
            self.task = None;
            match result {
                Ok(manifest) => self.manifest = Some(manifest),
                Err(e) => self.error = Some(e),
            }
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .inner_margin(egui::Margin::same(20))
                    .show(ui, |ui| {
                        crate::widgets::card(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(crate::i18n::tr("download"))
                                        .size(20.0)
                                        .color(palette.on_surface),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if crate::widgets::outlined_button(
                                            ui,
                                            ui.id().with("refresh"),
                                            &crate::i18n::tr("button.refresh"),
                                            Some("REFRESH"),
                                        )
                                        .clicked()
                                        {
                                            self.refresh();
                                        }
                                    },
                                );
                            });

                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                crate::widgets::rounded_text_edit_singleline(
                                    ui,
                                    &mut self.search,
                                    &crate::i18n::tr("search"),
                                    (ui.available_width() - 8.0).min(340.0),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let labels = vec![
                                            crate::i18n::tr("instance.game.releases"),
                                            crate::i18n::tr("instance.game.snapshots"),
                                            crate::i18n::tr("instance.game.old"),
                                        ];
                                        let _ = crate::widgets::tab_bar(
                                            ui,
                                            ui.id().with("version_tabs"),
                                            &labels,
                                            &mut self.tab,
                                        );
                                    },
                                );
                            });

                            ui.add_space(8.0);
                            let mut install_request: Option<RemoteVersion> = None;
                            egui::ScrollArea::vertical()
                                .max_height(ui.available_height())
                                .show(ui, |ui| match (&self.manifest, &self.error) {
                                    (Some(manifest), _) => {
                                        install_request = self.version_list(ui, manifest);
                                    }
                                    (None, Some(error)) => {
                                        hint(
                                            ui,
                                            ToastKind::Error,
                                            &crate::i18n::tr("download.failed"),
                                        );
                                        ui.label(error);
                                    }
                                    _ => {
                                        ui.horizontal(|ui| {
                                            crate::widgets::spinner(ui, 20.0);
                                            ui.label(crate::i18n::tr("download.content"));
                                        });
                                    }
                                });
                            if let Some(version) = install_request {
                                let game_dir =
                                    hmcl_core::download::default_game_dir(&crate::data_dir());
                                self.install = Some(spawn_install(version, game_dir));
                            }
                        });
                    });
            });

        // Install progress dialog.
        if let Some(task) = self.install.take() {
            let result = crate::widgets::Dialog::new(
                egui::Id::new("install_progress"),
                crate::i18n::tr("install.new_game"),
            )
            .positive_text(None)
            .show(ctx, |ui| {
                ui.set_width(360.0);
                if let Some(status) = task.poll_status() {
                    ui.label(status.message);
                    ui.add_space(6.0);
                    crate::widgets::progress_bar(ui, status.fraction, 8.0);
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!(
                            "{:.1} MB / {:.1} MB",
                            status.done_bytes as f64 / 1048576.0,
                            status.total_bytes as f64 / 1048576.0
                        ))
                        .size(12.0)
                        .color(theme::palette().on_surface_variant),
                    );
                } else {
                    ui.horizontal(|ui| {
                        crate::widgets::spinner(ui, 20.0);
                        ui.label(crate::i18n::tr("install.new_game.installation"));
                    });
                }
                ui.add_space(4.0);
            });
            if result.is_some() {
                if let Some(outcome) = task.poll_result() {
                    match outcome {
                        Ok(()) => toasts.info(crate::i18n::tr("install.success")),
                        Err(e) => toasts.error(e.to_string()),
                    }
                }
            } else {
                // Still open: re-arm.
                self.install = Some(task);
            }
        }
    }

    fn refresh(&mut self) {
        self.error = None;
        self.task = Some(spawn(async move {
            hmcl_core::download::fetch_version_manifest()
                .await
                .map_err(|e| format!("{e:#}"))
        }));
    }

    /// Render the version rows, returning the version to install when the
    /// user clicks an install button.
    fn version_list(&self, ui: &mut Ui, manifest: &VersionManifest) -> Option<RemoteVersion> {
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
                    _ => matches!(version_type, VersionType::OldBeta | VersionType::OldAlpha),
                };
                matches_tab && (search.is_empty() || version.id.to_lowercase().contains(&search))
            })
            .collect();

        let mut install_request: Option<RemoteVersion> = None;
        egui::Grid::new(ui.id().with("version_grid"))
            .num_columns(3)
            .spacing(egui::vec2(24.0, 2.0))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(crate::i18n::tr("world.game_version"))
                        .color(palette.on_surface_variant),
                );
                ui.label(
                    RichText::new(crate::i18n::tr("instance.game.release"))
                        .color(palette.on_surface_variant),
                );
                ui.end_row();
                for version in show_versions {
                    // Row: grass icon + version id, type label, install icon.
                    ui.horizontal(|ui| {
                        if let Some(icon_texture) =
                            crate::image::texture(ctx_of(ui), "img/grass.png")
                        {
                            let rect =
                                egui::Rect::from_min_size(ui.cursor().min, egui::vec2(20.0, 20.0));
                            ui.painter().image(
                                icon_texture.id(),
                                rect,
                                egui::Rect::from_min_max(
                                    egui::Pos2::ZERO,
                                    egui::Pos2::new(1.0, 1.0),
                                ),
                                egui::Color32::WHITE,
                            );
                            ui.advance_cursor_after_rect(rect);
                        }
                        ui.add_space(6.0);
                        ui.label(
                            RichText::new(&version.id)
                                .size(15.0)
                                .color(palette.on_surface),
                        );
                    });
                    let version_type = VersionType::of(&version.version_type);
                    let label = match version_type {
                        VersionType::Release => crate::i18n::tr("instance.game.release"),
                        VersionType::Snapshot => crate::i18n::tr("instance.game.snapshot"),
                        _ => version.version_type.clone(),
                    };
                    ui.label(RichText::new(label).color(palette.on_surface_variant));
                    if crate::widgets::icon_button(
                        ui,
                        ui.id().with(("install", &version.id)),
                        "DOWNLOAD",
                        30.0,
                    )
                    .on_hover_text(crate::i18n::tr("button.install"))
                    .clicked()
                    {
                        install_request = Some(version.clone());
                    }
                    ui.end_row();
                }
            });
        install_request
    }
}

/// The egui context of a `Ui`, for texture lookups inside rows.
fn ctx_of(ui: &egui::Ui) -> &egui::Context {
    ui.ctx()
}
