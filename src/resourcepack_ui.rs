use crate::{
    config::Config,
    resourcepacks::{self, Compatibility, ContextInfo, Pack},
    theme,
};
use eframe::egui::{self, RichText};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::mpsc::{self, Receiver},
};
type Scan = Result<(ContextInfo, Vec<Pack>, String), String>;
struct Row {
    pack: Pack,
    icon: Option<egui::TextureHandle>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn batch_import_retains_successful_rows_and_reports_failed_files() {
        use std::{fs, io::Write};
        let root = std::env::temp_dir().join(format!(
            "rust-hmcl-pack-batch-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let game = root.join("minecraft");
        let version = game.join("versions/1.20.1");
        fs::create_dir_all(&version).unwrap();
        fs::write(
            version.join("1.20.1.json"),
            br#"{"id":"1.20.1","libraries":[]}"#,
        )
        .unwrap();
        let valid = root.join("valid.zip");
        let mut zip = zip::ZipWriter::new(fs::File::create(&valid).unwrap());
        zip.start_file("pack.mcmeta", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(br#"{"pack":{"pack_format":15,"description":"fixture"}}"#)
            .unwrap();
        zip.finish().unwrap();
        let bad = root.join("bad.zip");
        fs::write(&bad, b"not a zip").unwrap();
        let config = Config {
            game_dir: game.to_string_lossy().into_owned(),
            selected: "1.20.1".into(),
            isolated: true,
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let mut browser = Browser::default();
        browser.prepare_preview(&config, &ctx).unwrap();
        let folder = root.join("folder-resource");
        fs::create_dir(&folder).unwrap();
        fs::write(
            folder.join("pack.mcmeta"),
            br#"{"pack":{"pack_format":15,"description":"folder fixture"}}"#,
        )
        .unwrap();
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800., 500.),
                )),
                dropped_files: vec![
                    egui::DroppedFile {
                        path: Some(valid.clone()),
                        ..Default::default()
                    },
                    egui::DroppedFile {
                        path: Some(bad),
                        ..Default::default()
                    },
                    egui::DroppedFile {
                        path: Some(folder),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| browser.page(ui, &config, false));
            },
        );
        let result = browser
            .rx
            .take()
            .unwrap()
            .recv_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        browser.install(result, &ctx);
        assert_eq!(browser.rows.len(), 2);
        assert!(browser.rows.iter().any(|row| row.pack.name == "valid.zip"));
        assert!(browser
            .rows
            .iter()
            .any(|row| row.pack.name == "folder-resource"));
        assert!(browser.context.is_some());
        assert!(browser.error.contains("bad.zip"));
        assert_eq!(
            fs::read(valid).unwrap(),
            fs::read(version.join("game/resourcepacks/valid.zip")).unwrap()
        );
        assert!(!version.join("game/resourcepacks/bad.zip").exists());
        drop(browser);
        let canonical = fs::canonicalize(&root).unwrap();
        assert_eq!(
            canonical.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        assert!(canonical
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("rust-hmcl-pack-batch-"));
        fs::remove_dir_all(canonical).unwrap();
    }
    #[test]
    fn narrow_pack_lists_keep_controls_inside_content() {
        for width in [300., 550.] {
            let ctx = egui::Context::default();
            crate::theme::setup(&ctx, true);
            let config = Config::default();
            let mut browser = Browser {
                key: Some(key(&config)),
                ..Default::default()
            };
            for warning in [Compatibility::Compatible, Compatibility::TooOld] {
                browser.rows.push(Row {
                    pack: Pack {
                        path: PathBuf::from("sample.zip"),
                        name: format!("sample-{warning:?}.zip"),
                        title: "A long resource pack title to exercise truncation".into(),
                        description:
                            "Long description which must stay before the toggle and details button"
                                .into(),
                        compatibility: warning,
                        enabled: false,
                        icon: None,
                    },
                    icon: None,
                });
            }
            let _ = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(width, 500.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(12., 12.);
                        let right = ui.max_rect().right();
                        browser.page(ui, &config, false);
                        assert!(
                            ui.min_rect().right() <= right + 0.1,
                            "content exceeded available width: {} > {right}",
                            ui.min_rect().right()
                        );
                    });
                },
            );
        }
    }
}
#[derive(Default)]
pub struct Browser {
    key: Option<(String, String, bool)>,
    context: Option<ContextInfo>,
    rows: Vec<Row>,
    rx: Option<Receiver<Scan>>,
    selected: BTreeSet<String>,
    filter: String,
    searching: bool,
    info: Option<Pack>,
    pending: Option<(ContextInfo, Vec<String>)>,
    error: String,
}
fn key(config: &Config) -> (String, String, bool) {
    (
        config.game_dir.clone(),
        config.selected.clone(),
        config.isolated,
    )
}
fn scan(config: &Config) -> Scan {
    (|| -> anyhow::Result<_> {
        let context = ContextInfo::from_config(config)?;
        let packs = resourcepacks::list(&context)?;
        Ok((context, packs, String::new()))
    })()
    .map_err(|error| error.to_string())
}
impl Browser {
    fn install(&mut self, result: Scan, ctx: &egui::Context) {
        match result {
            Ok((context, packs, warning)) => {
                self.context = Some(context);
                self.rows = packs
                    .into_iter()
                    .map(|mut pack| {
                        let icon = pack.icon.take().map(|image| {
                            ctx.load_texture(
                                format!("resourcepack:{}", pack.path.display()),
                                image,
                                egui::TextureOptions::LINEAR,
                            )
                        });
                        Row { pack, icon }
                    })
                    .collect();
                self.selected
                    .retain(|name| self.rows.iter().any(|row| &row.pack.name == name));
                self.error = warning;
            }
            Err(error) => {
                self.context = None;
                self.rows.clear();
                self.selected.clear();
                self.error = error;
            }
        }
    }
    pub(super) fn prepare_preview(
        &mut self,
        config: &Config,
        ctx: &egui::Context,
    ) -> anyhow::Result<()> {
        self.key = Some(key(config));
        self.install(scan(config), ctx);
        if !self.error.is_empty() {
            anyhow::bail!("{}", self.error);
        }
        Ok(())
    }
    pub(super) fn prepare_dialog_preview(&mut self, mode: &str) -> anyhow::Result<()> {
        if mode == "warning" {
            let names: Vec<_> = self
                .rows
                .iter()
                .filter(|row| row.pack.compatibility != Compatibility::Compatible)
                .map(|row| row.pack.name.clone())
                .collect();
            if names.is_empty() {
                anyhow::bail!("没有不兼容的资源包可供预览");
            }
            self.pending = Some((
                self.context
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("缺少资源包上下文"))?,
                names,
            ));
            return Ok(());
        }
        if mode != "info" {
            anyhow::bail!("资源包弹窗预览模式应为 info 或 warning");
        }
        self.info = Some(
            self.rows
                .first()
                .ok_or_else(|| anyhow::anyhow!("没有资源包可供预览"))?
                .pack
                .clone(),
        );
        Ok(())
    }
    fn refresh(&mut self, config: &Config, ctx: &egui::Context, sources: Option<Vec<PathBuf>>) {
        self.key = Some(key(config));
        self.error.clear();
        let config = config.clone();
        let ctx = ctx.clone();
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        std::thread::spawn(move || {
            let result = (|| -> Scan {
                if let Some(sources) = sources {
                    let context = ContextInfo::from_config(&config).map_err(|e| e.to_string())?;
                    let mut failures = vec![];
                    for source in sources {
                        if let Err(error) = resourcepacks::import(&context, &source) {
                            failures.push(format!("{}：{error}", source.display()));
                        }
                    }
                    let packs = resourcepacks::list(&context).map_err(|e| e.to_string())?;
                    return Ok((context, packs, failures.join("\n")));
                }
                scan(&config)
            })();
            let _ = tx.send(result);
            ctx.request_repaint();
        });
    }
    fn change(&mut self, config: &Config, names: Vec<String>, enabled: bool, ctx: &egui::Context) {
        let Some(context) = &self.context else {
            return;
        };
        if enabled
            && self.rows.iter().any(|row| {
                names.contains(&row.pack.name)
                    && row.pack.compatibility != Compatibility::Compatible
            })
        {
            self.pending = Some((context.clone(), names));
            return;
        }
        match resourcepacks::set_enabled(context, &names, enabled) {
            Ok(()) => self.refresh(config, ctx, None),
            Err(error) => self.error = error.to_string(),
        }
    }
    pub fn page(&mut self, ui: &mut egui::Ui, config: &Config, blocked: bool) {
        if self.key.as_ref() != Some(&key(config)) {
            self.rows.clear();
            self.context = None;
            self.selected.clear();
            self.info = None;
            self.pending = None;
            self.refresh(config, ui.ctx(), None);
        }
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(result) => {
                    self.rx = None;
                    self.install(result, ui.ctx());
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.rx = None;
                    self.error = "资源包读取任务中断".into();
                }
                Err(mpsc::TryRecvError::Empty) => ui
                    .ctx()
                    .request_repaint_after(std::time::Duration::from_millis(50)),
            }
        }
        if blocked {
            ui.label("游戏运行或任务执行期间，资源包设置暂不可修改。");
        }
        let dropped = ui
            .ctx()
            .input_mut(|input| std::mem::take(&mut input.raw.dropped_files));
        if !dropped.is_empty() {
            if self.rx.is_none() && !blocked && ui.is_enabled() {
                let sources: Vec<_> = dropped.into_iter().filter_map(|file| file.path).collect();
                if sources.is_empty() {
                    self.error = "拖放文件缺少本地路径".into();
                } else {
                    self.refresh(config, ui.ctx(), Some(sources));
                }
            } else {
                self.error = "请等待当前任务结束后再导入资源包".into();
            }
        }
        let ready = self.rx.is_none() && !blocked && ui.is_enabled();
        let mut change = None;
        ui.horizontal_wrapped(|ui| {
            if self.selected.is_empty() {
                if ui
                    .add_enabled(self.rx.is_none(), theme::MaterialButton::new("刷新"))
                    .clicked()
                {
                    self.refresh(config, ui.ctx(), None);
                }
                let add =
                    ui.add_enabled(ready, theme::MaterialButton::new("添加资源包").icon("add"));
                add.context_menu(|ui| {
                    if ui
                        .add_enabled(
                            ready,
                            theme::MaterialButton::new("添加资源包文件夹").icon("folder"),
                        )
                        .clicked()
                    {
                        if let Some(source) = rfd::FileDialog::new().pick_folder() {
                            self.refresh(config, ui.ctx(), Some(vec![source]));
                        }
                        ui.close_menu();
                    }
                });
                if add.clicked() {
                    if let Some(source) = rfd::FileDialog::new()
                        .add_filter("ZIP 资源包", &["zip"])
                        .pick_files()
                    {
                        self.refresh(config, ui.ctx(), Some(source));
                    }
                }
                if ui
                    .add_enabled(
                        self.context.is_some(),
                        theme::MaterialButton::new("打开目录"),
                    )
                    .clicked()
                {
                    if let Some(context) = &self.context {
                        if let Err(error) = std::fs::create_dir_all(&context.directory)
                            .map_err(anyhow::Error::from)
                            .and_then(|()| {
                                open::that(&context.directory).map_err(anyhow::Error::from)
                            })
                        {
                            self.error = error.to_string();
                        }
                    }
                }
                if theme::button(ui, "搜索").clicked() {
                    self.searching = !self.searching;
                    if !self.searching {
                        self.filter.clear();
                    }
                }
            } else {
                ui.label(format!("已选择 {} 个", self.selected.len()));
                if ui
                    .add_enabled(ready, theme::MaterialButton::new("启用").icon("check"))
                    .clicked()
                {
                    change = Some((self.selected.iter().cloned().collect(), true));
                }
                if ui
                    .add_enabled(ready, theme::MaterialButton::new("禁用").icon("close"))
                    .clicked()
                {
                    change = Some((self.selected.iter().cloned().collect(), false));
                }
                if theme::button(ui, "取消选择").clicked() {
                    self.selected.clear();
                }
            }
            if theme::button(ui, "全选").clicked() {
                self.selected = self
                    .rows
                    .iter()
                    .filter(|row| self.matches(&row.pack))
                    .map(|row| row.pack.name.clone())
                    .collect();
            }
            if self.rx.is_some() {
                ui.spinner();
            }
        });
        ui.label(
            egui::RichText::new("可拖入 ZIP 或资源包文件夹；右键“添加资源包”可选择文件夹。")
                .small()
                .color(ui.visuals().weak_text_color()),
        );
        if self.searching {
            theme::text_field_sized(
                ui,
                &mut self.filter,
                ui.available_width(),
                "搜索资源包名称或描述",
            );
        }
        if self.rows.is_empty() && self.rx.is_none() && self.error.is_empty() {
            theme::empty(
                ui,
                "texture",
                "没有资源包",
                "添加 ZIP 资源包，或将资源包文件夹放入此实例的 resourcepacks 目录。",
            );
        }
        for row in &self.rows {
            if !self.matches(&row.pack) {
                continue;
            }
            ui.push_id(&row.pack.name, |ui| {
                egui::Frame::new()
                    .fill(if self.selected.contains(&row.pack.name) {
                        theme::container(ui)
                    } else {
                        theme::surface(ui)
                    })
                    .corner_radius(4.)
                    .inner_margin(8.)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let mut selected = self.selected.contains(&row.pack.name);
                            if ui.checkbox(&mut selected, "").changed() {
                                if selected {
                                    self.selected.insert(row.pack.name.clone());
                                } else {
                                    self.selected.remove(&row.pack.name);
                                }
                            }
                            let (rect, response) =
                                ui.allocate_exact_size(egui::vec2(40., 40.), egui::Sense::click());
                            if let Some(icon) = &row.icon {
                                egui::Image::new(icon).paint_at(ui, rect);
                            } else {
                                theme::icon(ui, "texture", rect, theme::accent(ui));
                            }
                            if response.clicked() {
                                self.info = Some(row.pack.clone());
                            }
                            ui.vertical(|ui| {
                                // Toggle is 58px, info is 38px, plus two 12px gaps.
                                ui.set_width((ui.available_width() - 130.).max(20.));
                                if ui
                                    .add(
                                        egui::Label::new(&row.pack.title)
                                            .truncate()
                                            .sense(egui::Sense::click()),
                                    )
                                    .double_clicked()
                                {
                                    self.info = Some(row.pack.clone());
                                }
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&row.pack.description)
                                            .small()
                                            .color(ui.visuals().weak_text_color()),
                                    )
                                    .truncate(),
                                );
                                if let Some(warning) = row.pack.compatibility.warning() {
                                    ui.add(
                                        egui::Label::new(RichText::new(warning).small().color(
                                            if ui.visuals().dark_mode {
                                                egui::Color32::from_rgb(255, 202, 135)
                                            } else {
                                                egui::Color32::from_rgb(142, 82, 8)
                                            },
                                        ))
                                        .truncate(),
                                    );
                                }
                            });
                            let mut enabled = row.pack.enabled;
                            if ui
                                .add_enabled_ui(ready, |ui| theme::toggle(ui, &mut enabled, ""))
                                .inner
                                .changed()
                            {
                                change = Some((vec![row.pack.name.clone()], enabled));
                            }
                            if ui
                                .add(theme::MaterialButton::new("").icon("info"))
                                .on_hover_text("资源包详情")
                                .clicked()
                            {
                                self.info = Some(row.pack.clone());
                            }
                        });
                    });
            });
        }
        if let Some((names, enabled)) = change {
            self.change(config, names, enabled, ui.ctx());
        }
        if !self.error.is_empty() {
            ui.label(&self.error);
        }
    }
    fn matches(&self, pack: &Pack) -> bool {
        let filter = self.filter.to_lowercase();
        pack.name.to_lowercase().contains(&filter)
            || pack.description.to_lowercase().contains(&filter)
    }
    pub fn modal_open(&self) -> bool {
        self.info.is_some() || self.pending.is_some()
    }
    pub fn dialogs(&mut self, ctx: &egui::Context, blocked: bool) {
        if let Some(pack) = &self.info {
            let mut close = false;
            let modal = egui::Modal::new(egui::Id::new("resourcepack-info"))
                .frame(
                    egui::Frame::popup(&ctx.style())
                        .inner_margin(24.)
                        .corner_radius(4.),
                )
                .show(ctx, |ui| {
                    ui.set_width((ctx.screen_rect().width() * 0.65).min(620.));
                    ui.horizontal(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(40., 40.), egui::Sense::hover());
                        if let Some(icon) = self
                            .rows
                            .iter()
                            .find(|row| row.pack.path == pack.path)
                            .and_then(|row| row.icon.as_ref())
                        {
                            egui::Image::new(icon).paint_at(ui, rect);
                        } else {
                            theme::icon(ui, "texture", rect, theme::accent(ui));
                        }
                        ui.vertical(|ui| {
                            ui.add(
                                egui::Label::new(RichText::new(&pack.title).size(20.).strong())
                                    .truncate(),
                            );
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&pack.name)
                                        .small()
                                        .color(ui.visuals().weak_text_color()),
                                )
                                .truncate(),
                            );
                        });
                    });
                    if let Some(warning) = pack.compatibility.warning() {
                        ui.label(warning);
                    }
                    egui::ScrollArea::vertical()
                        .max_height(ctx.screen_rect().height() * 0.5)
                        .show(ui, |ui| {
                            ui.label(&pack.description);
                        });
                    close = theme::button(ui, "关闭").clicked();
                });
            if close || modal.should_close() {
                self.info = None;
            }
        }
        if let Some((context, names)) = self.pending.clone() {
            let mut confirm = false;
            let mut cancel = false;
            let modal = egui::Modal::new(egui::Id::new("resourcepack-compatibility"))
                .frame(
                    egui::Frame::popup(&ctx.style())
                        .inner_margin(24.)
                        .corner_radius(4.),
                )
                .show(ctx, |ui| {
                    ui.set_width(440.);
                    ui.label(RichText::new("资源包兼容性提示").size(20.).strong());
                    ui.label("选中的资源包可能不兼容当前游戏，仍要启用吗？");
                    for name in names.iter().take(5) {
                        ui.add(egui::Label::new(name).truncate());
                    }
                    ui.horizontal(|ui| {
                        confirm = ui
                            .add_enabled(!blocked, theme::MaterialButton::new("仍然启用"))
                            .clicked();
                        cancel = theme::button(ui, "取消").clicked();
                    });
                    if !self.error.is_empty() {
                        ui.label(&self.error);
                    }
                });
            if confirm {
                match resourcepacks::set_enabled(&context, &names, true) {
                    Ok(()) => {
                        self.pending = None;
                        self.key = None;
                    }
                    Err(error) => self.error = error.to_string(),
                }
            } else if cancel || modal.should_close() {
                self.pending = None;
            }
        }
    }
}
