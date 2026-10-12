#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod cli;
mod config;
mod game;
mod loaders;
mod metadata;
mod modpack;
mod modrinth;
mod mods;
mod theme;

use config::Config;
use eframe::egui::{self, Color32, RichText};
use game::{Event, Version, Worker};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
    Arc,
};

const GREEN: Color32 = theme::PRIMARY;
const MUTED: Color32 = Color32::from_rgb(112, 123, 130);

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Home,
    Accounts,
    Versions,
    Mods,
    Packs,
    Download,
    Java,
    Settings,
    Logs,
    About,
}
impl Page {
    fn icon(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Accounts => "person",
            Self::Versions => "game",
            Self::Mods => "extension",
            Self::Packs => "package",
            Self::Download => "download",
            Self::Java => "java",
            Self::Settings => "settings",
            Self::Logs => "logs",
            Self::About => "info",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Home => "首页",
            Self::Accounts => "账户",
            Self::Versions => "版本管理",
            Self::Mods => "模组管理",
            Self::Packs => "整合包",
            Self::Download => "下载游戏",
            Self::Java => "Java 管理",
            Self::Settings => "设置",
            Self::Logs => "游戏日志",
            Self::About => "关于",
        }
    }
}
struct Launcher {
    config: Config,
    page: Page,
    versions: Vec<Version>,
    installed: Vec<String>,
    filter: String,
    snapshots: bool,
    rx: Option<Receiver<Event>>,
    cancel: Arc<AtomicBool>,
    busy: bool,
    running: bool,
    status: String,
    error: String,
    progress: f32,
    logs: Vec<String>,
    java_major: u32,
    font_warning: bool,
    new_player: String,
    mod_files: Vec<mods::ModFile>,
    log_filter: String,
    log_follow: bool,
    loader_version: String,
    mod_query: String,
    projects: Vec<modrinth::Project>,
    pack_path: String,
    pack_id: String,
    pack_optional: bool,
    pack_summary: String,
    shown_page: Page,
    page_started: f64,
    version_popup: bool,
}
impl Launcher {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        // Use platform CJK fonts without bundling proprietary font files.
        let candidates = [
            "C:/Windows/Fonts/msyh.ttc",
            "/System/Library/Fonts/PingFang.ttc",
            "/System/Library/Fonts/STHeiti Light.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        ];
        let mut found = false;
        for path in candidates {
            if let Ok(data) = std::fs::read(path) {
                fonts
                    .font_data
                    .insert("cjk".into(), egui::FontData::from_owned(data).into());
                for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                    fonts
                        .families
                        .entry(family)
                        .or_default()
                        .insert(0, "cjk".into());
                }
                found = true;
                break;
            }
        }
        cc.egui_ctx.set_fonts(fonts);
        let mut visuals = egui::Visuals::light();
        visuals.panel_fill = Color32::from_rgb(246, 248, 249);
        visuals.selection.bg_fill = GREEN;
        visuals.widgets.active.bg_fill = GREEN;
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(225, 239, 229);
        cc.egui_ctx.set_visuals(visuals);
        cc.egui_ctx.style_mut(|s| {
            s.spacing.item_spacing = egui::vec2(12., 12.);
            s.spacing.button_padding = egui::vec2(14., 9.);
        });
        let (mut config, error) = match Config::load() {
            Ok(c) => (c, String::new()),
            Err(e) => (Config::default(), format!("{e:#}")),
        };
        config.normalize();
        theme::setup(&cc.egui_ctx, config.dark_mode);
        let installed = game::installed(std::path::Path::new(&config.game_dir));
        Self {
            config,
            page: Page::Home,
            versions: vec![],
            installed,
            filter: String::new(),
            snapshots: false,
            rx: None,
            cancel: Arc::new(AtomicBool::new(false)),
            busy: false,
            running: false,
            status: "准备就绪".into(),
            error,
            progress: 0.,
            logs: vec![],
            java_major: 21,
            font_warning: !found,
            new_player: String::new(),
            mod_files: vec![],
            log_filter: String::new(),
            log_follow: true,
            loader_version: String::new(),
            mod_query: String::new(),
            projects: vec![],
            pack_path: String::new(),
            pack_id: "my-modpack".into(),
            pack_optional: true,
            pack_summary: String::new(),
            shown_page: Page::Home,
            page_started: -1.,
            version_popup: false,
        }
    }
    fn refresh(&mut self) {
        self.installed = game::installed(std::path::Path::new(&self.config.game_dir));
        if !self.installed.contains(&self.config.selected) {
            self.config.selected = self.installed.first().cloned().unwrap_or_default();
        }
    }
    fn save(&mut self) {
        self.config.normalize();
        match self.config.save() {
            Ok(()) => {
                self.status = "设置已保存".into();
                self.error.clear();
            }
            Err(e) => self.error = format!("{e:#}"),
        }
    }
    fn start(&mut self, work: impl FnOnce(Worker) -> anyhow::Result<()> + Send + 'static) {
        if self.busy {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.cancel = Arc::new(AtomicBool::new(false));
        self.rx = Some(rx);
        self.busy = true;
        self.error.clear();
        self.progress = 0.;
        let cancel = self.cancel.clone();
        let threads = self.config.download_threads;
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                Worker::new(tx.clone(), cancel)
                    .and_then(|mut w| {
                        w.threads = threads;
                        work(w)
                    })
                    .map_err(|e| format!("{e:#}"))
            }))
            .unwrap_or_else(|_| Err("后台任务发生异常，请检查版本配置和日志后重试".into()));
            let _ = tx.send(Event::Finished(result));
        });
    }
    fn launch(&mut self) {
        if let Err(e) = self.config.validate() {
            self.error = e.to_string();
            return;
        }
        self.save();
        if !self.error.is_empty() {
            return;
        }
        self.running = false;
        let config = self.config.clone();
        self.start(move |w| w.launch(&config));
    }
    fn poll(&mut self) {
        let events = self
            .rx
            .as_ref()
            .map(|rx| rx.try_iter().take(500).collect::<Vec<_>>())
            .unwrap_or_default();
        for event in events {
            match event {
                Event::Status(s) => self.status = s,
                Event::Progress(p, s) => {
                    self.progress = p;
                    self.status = s;
                }
                Event::Versions(v) => self.versions = v,
                Event::Projects(projects) => self.projects = projects,
                Event::Java(p) => {
                    self.status = format!("Java 安装完成：{p}");
                    self.save();
                }
                Event::Installed(id) => {
                    self.config.selected = id;
                    self.refresh();
                    if self.page == Page::Mods {
                        self.refresh_mods();
                    }
                    self.save();
                }
                Event::Log(line) => {
                    self.logs.push(line);
                    if self.logs.len() > 2000 {
                        self.logs.drain(..500);
                    }
                }
                Event::GameStarted => self.running = true,
                Event::Finished(result) => {
                    self.busy = false;
                    self.running = false;
                    self.progress = 0.;
                    self.refresh();
                    if self.page == Page::Mods {
                        self.refresh_mods();
                    }
                    match result {
                        Ok(()) => {}
                        Err(e) => {
                            self.status = "操作未完成".into();
                            self.error = e;
                        }
                    }
                }
            }
        }
    }
    fn card(ui: &mut egui::Ui, draw: impl FnOnce(&mut egui::Ui)) {
        egui::Frame::new()
            .fill(if ui.visuals().dark_mode {
                Color32::from_rgb(36, 40, 43)
            } else {
                Color32::WHITE
            })
            .inner_margin(20.)
            .corner_radius(8.)
            .stroke(egui::Stroke::new(1_f32, Color32::from_rgb(226, 231, 234)))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                draw(ui);
            });
    }
    fn home(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        ui.allocate_rect(rect, egui::Sense::hover());
        egui::Image::from_bytes("bytes://hmcl/background.jpg", theme::WALLPAPER).paint_at(ui, rect);
        ui.painter().rect_filled(
            rect,
            0.,
            Color32::from_black_alpha(if self.config.dark_mode { 105 } else { 22 }),
        );
        let notice =
            egui::Rect::from_min_size(rect.min + egui::vec2(24., 24.), egui::vec2(390., 98.));
        ui.painter().rect_filled(
            notice.translate(egui::vec2(0., 3.)),
            7.,
            Color32::from_black_alpha(18),
        );
        ui.painter()
            .rect_filled(notice, 7., Color32::from_white_alpha(238));
        theme::png(
            ui,
            "logo",
            theme::LOGO,
            egui::Rect::from_min_size(notice.min + egui::vec2(18., 23.), egui::vec2(48., 48.)),
        );
        ui.painter().text(
            notice.min + egui::vec2(80., 29.),
            egui::Align2::LEFT_CENTER,
            "Hello Minecraft! Launcher",
            egui::FontId::proportional(18.),
            theme::PRIMARY,
        );
        ui.painter().text(
            notice.min + egui::vec2(80., 57.),
            egui::Align2::LEFT_CENTER,
            "探索世界，从这里开始",
            egui::FontId::proportional(13.),
            theme::SECONDARY,
        );
        let tools = egui::Rect::from_min_size(
            egui::pos2(rect.left() + 24., rect.bottom() - 102.),
            egui::vec2(240., 46.),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(tools), |ui| {
            ui.horizontal(|ui| {
                if theme::button(ui, "版本管理").clicked() {
                    self.page = Page::Versions;
                }
                if theme::button(ui, "下载游戏").clicked() {
                    self.page = Page::Download;
                }
            });
        });
        let launch = egui::Rect::from_min_size(
            egui::pos2(rect.right() - 320., rect.bottom() - 110.),
            egui::vec2(296., 62.),
        );
        ui.painter().rect_filled(
            launch.translate(egui::vec2(0., 4.)),
            5.,
            Color32::from_black_alpha(35),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(launch), |ui| {
            ui.spacing_mut().item_spacing.x = 2.;
            ui.horizontal(|ui| {
                let subtitle = if self.config.selected.is_empty() {
                    "尚未安装游戏"
                } else {
                    &self.config.selected
                };
                let response = ui.add_enabled(
                    !self.busy,
                    theme::MaterialButton::new(if self.config.selected.is_empty() {
                        "安装游戏"
                    } else {
                        "启动游戏"
                    })
                    .subtitle(subtitle)
                    .icon("launch")
                    .fill(GREEN)
                    .min_size(egui::vec2(249., 62.)),
                );
                if response.clicked() {
                    if self.config.selected.is_empty() {
                        self.page = Page::Download;
                    } else {
                        self.launch();
                    }
                }
                if ui
                    .add(
                        theme::MaterialButton::new("")
                            .icon("arrow")
                            .fill(GREEN)
                            .min_size(egui::vec2(42., 62.)),
                    )
                    .clicked()
                {
                    self.version_popup = !self.version_popup;
                }
            });
        });
        if self.version_popup {
            let height = (self.installed.len().clamp(1, 7) as f32 * 42.) + 48.;
            let pos = egui::pos2(launch.left(), launch.top() - height - 10.);
            egui::Area::new(egui::Id::new("version-selector"))
                .order(egui::Order::Foreground)
                .fixed_pos(pos)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style())
                        .inner_margin(12.)
                        .show(ui, |ui| {
                            ui.set_width(270.);
                            ui.label(RichText::new("切换游戏版本").strong());
                            egui::ScrollArea::vertical()
                                .max_height(280.)
                                .show(ui, |ui| {
                                    for version in self.installed.clone() {
                                        if theme::nav(
                                            ui,
                                            "game",
                                            &version,
                                            None,
                                            self.config.selected == version,
                                        )
                                        .clicked()
                                        {
                                            self.config.selected = version;
                                            self.version_popup = false;
                                            self.save();
                                        }
                                    }
                                    if self.installed.is_empty() {
                                        ui.label("请先下载安装游戏");
                                    }
                                });
                        });
                });
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.version_popup = false;
            }
        }
        ui.painter().text(
            egui::pos2(rect.left() + 24., rect.bottom() - 23.),
            egui::Align2::LEFT_CENTER,
            "Hello Minecraft! Launcher · Rust 0.3.0",
            egui::FontId::proportional(12.),
            Color32::WHITE,
        );
        ui.painter().text(
            egui::pos2(rect.right() - 24., rect.bottom() - 23.),
            egui::Align2::RIGHT_CENTER,
            "Copyright © huangyuhui and contributors",
            egui::FontId::proportional(11.),
            Color32::from_white_alpha(225),
        );
    }
    fn content(&mut self, ui: &mut egui::Ui) {
        match self.page {
            Page::Home => self.home(ui),
            Page::Accounts => Self::card(ui, |ui| {
                ui.heading("账户管理");
                for player in self.config.accounts.clone() {
                    ui.horizontal(|ui| {
                        if ui
                            .selectable_label(self.config.player == player, &player)
                            .clicked()
                        {
                            self.config.player = player.clone();
                            self.save();
                        }
                        if self.config.player != player && theme::button(ui, "移除").clicked() {
                            self.config.accounts.retain(|p| p != &player);
                            self.save();
                        }
                    });
                }
                ui.separator();
                ui.label("用户名（1–16 位英文字母、数字或下划线）");
                theme::text_field(ui, &mut self.new_player);
                ui.label("离线账户不能进入要求正版认证的服务器。微软登录尚未接入。");
                if theme::button(ui, "添加离线账户").clicked() {
                    match config::validate_player(&self.new_player) {
                        Ok(()) => {
                            self.config.player = self.new_player.clone();
                            self.new_player.clear();
                            self.save();
                        }
                        Err(e) => self.error = e.to_string(),
                    }
                }
            }),
            Page::Versions => {
                ui.horizontal(|ui| {
                    if theme::button(ui, "刷新本地版本").clicked() {
                        self.refresh();
                    }
                    if theme::button(ui, "安装新版本").clicked() {
                        self.page = Page::Download;
                    }
                });
                if self.installed.is_empty() {
                    ui.label("没有发现游戏，请先安装或在设置中选择已有 .minecraft 目录。");
                }
                for version in self.installed.clone() {
                    Self::card(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (_, rect) = ui.allocate_space(egui::vec2(40., 40.));
                            theme::png(ui, "grass", theme::GRASS, rect);
                            ui.label(RichText::new(&version).size(20.));
                            if ui
                                .selectable_label(self.config.selected == version, "选择")
                                .clicked()
                            {
                                self.config.selected = version.clone();
                                self.save();
                            }
                        });
                        ui.horizontal_wrapped(|ui| {
                            if theme::button(ui, "游戏目录").clicked() {
                                let config = Config {
                                    selected: version.clone(),
                                    ..self.config.clone()
                                };
                                self.open_game_dir(&config);
                            }
                            if theme::button(ui, "管理模组").clicked() {
                                self.config.selected = version.clone();
                                self.refresh_mods();
                                self.page = Page::Mods;
                            }
                            if theme::button(ui, "安装 Fabric").clicked() {
                                self.install_loader(version.clone(), loaders::Loader::Fabric);
                            }
                            if theme::button(ui, "安装 Quilt").clicked() {
                                self.install_loader(version.clone(), loaders::Loader::Quilt);
                            }
                            if theme::button(ui, "校验 / 修复").clicked() {
                                let root = std::path::PathBuf::from(&self.config.game_dir);
                                let id = version.clone();
                                self.start(move |w| {
                                    let meta = metadata::resolve(&root, &id)?;
                                    w.prepare(&root, &id, &meta)
                                });
                            }
                        });
                    });
                }
                ui.label("Fabric / Quilt 默认安装最新稳定加载器（若没有稳定版则使用最新版本）。");
                ui.horizontal(|ui| {
                    ui.label("指定加载器版本（可留空）");
                    theme::text_field(ui, &mut self.loader_version);
                });
            }
            Page::Mods => {
                ui.label(format!("当前游戏：{}", self.config.selected));
                ui.label("模组需要相应加载器；本页面只管理文件，不自动解决模组依赖。");
                ui.horizontal(|ui| {
                    if theme::button(ui, "刷新").clicked() {
                        self.refresh_mods();
                    }
                    if theme::button(ui, "打开模组目录").clicked() {
                        match mods::directory(&self.config).and_then(|dir| {
                            std::fs::create_dir_all(&dir)?;
                            open::that(dir)?;
                            Ok(())
                        }) {
                            Ok(()) => {}
                            Err(e) => self.error = e.to_string(),
                        }
                    }
                    if theme::button(ui, "导入 .jar 模组").clicked() {
                        if let Some(files) = rfd::FileDialog::new()
                            .add_filter("Minecraft 模组", &["jar"])
                            .pick_files()
                        {
                            match mods::directory(&self.config)
                                .and_then(|dir| mods::import(&dir, &files))
                            {
                                Ok(n) => {
                                    self.status = format!("已导入 {n} 个模组");
                                    self.refresh_mods();
                                }
                                Err(e) => self.error = e.to_string(),
                            }
                        }
                    }
                });
                if self.mod_files.is_empty() {
                    ui.label("尚未发现模组。可以导入本地 .jar 文件。");
                }
                for file in self.mod_files.clone() {
                    ui.horizontal(|ui| {
                        let mut enabled = file.enabled;
                        if theme::toggle(ui, &mut enabled, "").changed() {
                            match mods::directory(&self.config)
                                .and_then(|dir| mods::toggle(&dir, &file.name))
                            {
                                Ok(()) => self.refresh_mods(),
                                Err(e) => self.error = e.to_string(),
                            }
                        }
                        ui.label(&file.name);
                        ui.label(
                            RichText::new(format!("{:.1} MB", file.size as f64 / 1048576.))
                                .color(MUTED),
                        );
                    });
                }
                ui.add_space(16.);
                ui.separator();
                ui.heading("从 Modrinth 下载模组");
                ui.label("按当前 Minecraft 和 Fabric / Quilt 环境筛选；安装时同时下载必需依赖。");
                ui.horizontal(|ui| {
                    theme::text_field(ui, &mut self.mod_query);
                    if theme::button(ui, "搜索兼容模组").clicked() {
                        let config = self.config.clone();
                        let query = self.mod_query.clone();
                        self.start(move |w| w.search_mods(&config, &query));
                    }
                });
                for project in self.projects.clone() {
                    Self::card(ui, |ui| {
                        ui.label(RichText::new(&project.title).size(18.).strong());
                        ui.label(&project.description);
                        ui.horizontal(|ui| {
                            ui.label(format!("下载次数：{}", project.downloads));
                            ui.hyperlink_to(
                                "项目详情",
                                format!("https://modrinth.com/mod/{}", project.slug),
                            );
                            if theme::button(ui, "安装及必需依赖").clicked() {
                                let config = self.config.clone();
                                let id = project.project_id.clone();
                                self.start(move |w| w.install_mod(&config, &id));
                            }
                        });
                    });
                }
            }
            Page::Packs => {
                Self::card(ui, |ui| {
                    ui.heading("导入 Modrinth 整合包");
                    ui.label("支持 .mrpack 格式的原版、Fabric 和 Quilt 整合包；每个整合包使用独立游戏目录。");
                    if theme::button(ui, "选择 .mrpack 文件").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Modrinth 整合包", &["mrpack"])
                            .pick_file()
                        {
                            match modpack::inspect(&path) {
                                Ok(index) => {
                                    self.pack_summary = format!(
                                        "{} · {} 个文件 · Minecraft {}",
                                        index.name,
                                        index.files.len(),
                                        index.dependencies.get("minecraft").unwrap()
                                    );
                                    self.pack_path = path.to_string_lossy().into();
                                }
                                Err(e) => self.error = e.to_string(),
                            }
                        }
                    }
                    ui.label(&self.pack_path);
                    ui.label(&self.pack_summary);
                    ui.label("新实例名称（不可与已有版本重名）");
                    theme::text_field(ui, &mut self.pack_id);
                    theme::toggle(
                        ui,
                        &mut self.pack_optional,
                        "同时安装整合包中的可选客户端文件",
                    );
                    if ui
                        .add_enabled(
                            !self.pack_path.is_empty(),
                            theme::MaterialButton::new("导入整合包").fill(GREEN),
                        )
                        .clicked()
                    {
                        let root = std::path::PathBuf::from(&self.config.game_dir);
                        let path = std::path::PathBuf::from(&self.pack_path);
                        let id = self.pack_id.trim().to_owned();
                        let optional = self.pack_optional;
                        self.start(move |w| w.import_pack(&root, &path, &id, optional));
                    }
                    ui.label("下载中断后可使用相同文件和实例名称继续导入。Forge / NeoForge 整合包暂不支持。");
                })
            }
            Page::Download => {
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!self.busy, theme::MaterialButton::new("获取版本列表"))
                        .clicked()
                    {
                        self.start(|w| w.manifest());
                    }
                    theme::toggle(ui, &mut self.snapshots, "显示快照 / 历史测试版");
                });
                ui.horizontal(|ui| {
                    ui.label("搜索");
                    theme::text_field(ui, &mut self.filter);
                });
                if self.versions.is_empty() {
                    ui.label("点击“获取版本列表”，从 Minecraft 官方下载源获取可用版本。");
                }
                let list = self
                    .versions
                    .iter()
                    .filter(|v| {
                        (self.snapshots || v.kind == "release") && v.id.contains(&self.filter)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                for version in list {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&version.id).strong());
                        ui.label(RichText::new(&version.kind).color(MUTED));
                        if ui
                            .add_enabled(
                                !self.busy,
                                theme::MaterialButton::new(
                                    if self.installed.contains(&version.id) {
                                        "校验 / 修复"
                                    } else {
                                        "安装"
                                    },
                                ),
                            )
                            .clicked()
                        {
                            let root = std::path::PathBuf::from(&self.config.game_dir);
                            self.config.selected = version.id.clone();
                            self.start(move |w| w.install(&root, &version));
                        }
                    });
                    ui.separator();
                }
            }
            Page::Java => Self::card(ui, |ui| {
                ui.heading("Java 运行环境");
                ui.label(
                    "Java 仅用于游戏。留空时按游戏需求检测已下载 Java、JAVA_HOME 和系统 Java。",
                );
                ui.label("Java 可执行文件路径");
                theme::text_field(ui, &mut self.config.java);
                if theme::button(ui, "浏览 Java 可执行文件").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        self.config.java = path.to_string_lossy().into();
                        self.save();
                    }
                }
                ui.horizontal(|ui| {
                    if theme::button(ui, "使用自动检测").clicked() {
                        self.config.java.clear();
                        self.save();
                    }
                    if theme::button(ui, "保存路径").clicked() {
                        self.save();
                    }
                });
                ui.separator();
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("java-major")
                        .selected_text(format!("Java {}", self.java_major))
                        .show_ui(ui, |ui| {
                            for major in [8, 17, 21, 25] {
                                ui.selectable_value(
                                    &mut self.java_major,
                                    major,
                                    format!("Java {major}"),
                                );
                            }
                        });
                    if ui
                        .add_enabled(
                            !self.busy,
                            theme::MaterialButton::new("下载 Eclipse Temurin JDK"),
                        )
                        .clicked()
                    {
                        let major = self.java_major;
                        self.start(move |w| w.download_java(major));
                    }
                });
                theme::toggle(
                    ui,
                    &mut self.config.auto_java,
                    "启动时缺少合适 Java 则自动下载（仅在路径留空时）",
                );
                ui.label("下载完成后自动检测可用环境。自动模式适合在不同年代的游戏间切换。");
                ui.label(format!("当前平台：{} / {}", game::os(), game::arch()));
            }),
            Page::Settings => Self::card(ui, |ui| {
                ui.heading("游戏设置");
                ui.label("游戏目录（可填入已有 .minecraft 的完整路径）");
                theme::text_field(ui, &mut self.config.game_dir);
                ui.horizontal(|ui| {
                    if theme::button(ui, "选择游戏目录").clicked() {
                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                            self.config.game_dir = path.to_string_lossy().into();
                            self.save();
                            self.refresh();
                        }
                    }
                    egui::ComboBox::from_id_salt("directories")
                        .selected_text("切换已记住的目录")
                        .show_ui(ui, |ui| {
                            for dir in self.config.directories.clone() {
                                if ui
                                    .selectable_label(self.config.game_dir == dir, &dir)
                                    .clicked()
                                {
                                    self.config.game_dir = dir;
                                    self.refresh();
                                    self.save();
                                }
                            }
                        });
                });
                theme::toggle(
                    ui,
                    &mut self.config.isolated,
                    "版本隔离：每个版本使用独立的存档、模组和配置目录",
                );
                ui.add(
                    egui::Slider::new(&mut self.config.memory, 512..=16384)
                        .text("内存 MB")
                        .step_by(256.),
                );
                ui.add(
                    egui::Slider::new(&mut self.config.download_threads, 1..=32)
                        .text("资源下载并发"),
                );
                ui.label("额外 JVM 参数（支持带引号的参数）");
                theme::text_field(ui, &mut self.config.extra_jvm);
                theme::toggle(ui, &mut self.config.auto_java, "自动下载游戏所需 Java");
                if theme::toggle(ui, &mut self.config.dark_mode, "深色主题").changed() {
                    theme::setup(ui.ctx(), self.config.dark_mode);
                    self.save();
                }
                ui.horizontal(|ui| {
                    ui.label("窗口宽度");
                    ui.add(egui::DragValue::new(&mut self.config.width).range(320..=7680));
                    ui.label("高度");
                    ui.add(egui::DragValue::new(&mut self.config.height).range(240..=4320));
                });
                if theme::button(ui, "保存设置并刷新版本").clicked() {
                    self.save();
                    if self.error.is_empty() {
                        self.refresh();
                    }
                }
                ui.label(RichText::new("使用已有游戏目录前，建议先备份存档。启用隔离后，已有根目录存档不会自动迁移。").color(MUTED));
            }),
            Page::Logs => {
                ui.label(format!(
                    "完整日志：{}",
                    config::data_dir().join("game.log").display()
                ));
                ui.horizontal(|ui| {
                    if theme::button(ui, "清空显示").clicked() {
                        self.logs.clear();
                    }
                    if theme::button(ui, "复制日志").clicked() {
                        ui.ctx().copy_text(self.logs.join("\n"));
                    }
                    if theme::button(ui, "打开日志文件").clicked() {
                        if let Err(e) = open::that(config::data_dir().join("game.log")) {
                            self.error = e.to_string();
                        }
                    }
                    theme::toggle(ui, &mut self.log_follow, "跟随最新日志");
                });
                ui.horizontal(|ui| {
                    ui.label("筛选");
                    theme::text_field(ui, &mut self.log_filter);
                });
                egui::ScrollArea::vertical()
                    .id_salt("log-lines")
                    .stick_to_bottom(self.log_follow)
                    .max_height(ui.available_height())
                    .show(ui, |ui| {
                        for line in self
                            .logs
                            .iter()
                            .filter(|s| s.to_lowercase().contains(&self.log_filter.to_lowercase()))
                        {
                            let color = if line.contains("ERROR") {
                                Color32::from_rgb(220, 75, 65)
                            } else if line.contains("WARN") {
                                Color32::from_rgb(185, 125, 30)
                            } else {
                                ui.visuals().text_color()
                            };
                            ui.label(RichText::new(line).monospace().color(color));
                        }
                    });
            }
            Page::About => Self::card(ui, |ui| {
                ui.heading(format!("Rust HMCL · {}", env!("CARGO_PKG_VERSION")));
                ui.label("独立的 Rust 原生桌面实现，界面参考 Hello Minecraft! Launcher。");
                ui.label("Hello Minecraft! Launcher");
                ui.label("Copyright (C) huangyuhui and contributors.");
                ui.label("本项目采用 GPLv3 或更新版本；并保留原 HMCL LICENSE 中的附加条款。");
                ui.hyperlink_to("HMCL 源项目", "https://github.com/HMCL-dev/HMCL");
                ui.hyperlink_to("Eclipse Temurin", "https://adoptium.net");
            }),
        }
    }
    fn refresh_mods(&mut self) {
        match mods::directory(&self.config).and_then(|dir| mods::list(&dir)) {
            Ok(files) => self.mod_files = files,
            Err(e) => self.error = e.to_string(),
        }
    }
    fn open_game_dir(&mut self, config: &Config) {
        let result = (|| -> anyhow::Result<()> {
            let dir = game::instance_dir(config)?;
            std::fs::create_dir_all(&dir)?;
            open::that(dir)?;
            Ok(())
        })();
        if let Err(e) = result {
            self.error = e.to_string();
        }
    }
    fn install_loader(&mut self, id: String, loader: loaders::Loader) {
        let root = std::path::PathBuf::from(&self.config.game_dir);
        let version = self.loader_version.trim().to_owned();
        self.start(move |w| {
            w.install_loader(
                &root,
                &id,
                loader,
                if version.is_empty() {
                    None
                } else {
                    Some(&version)
                },
                None,
            )
            .map(|_| ())
        });
    }
}
impl eframe::App for Launcher {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if !self.running {
            self.cancel.store(true, Ordering::Relaxed);
        }
        self.config.normalize();
        let _ = self.config.save();
    }
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll();
        if self.busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }
        egui::TopBottomPanel::top("title")
            .exact_height(48.)
            .frame(egui::Frame::new().fill(theme::PRIMARY))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.;
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2((ui.available_width() - 148.).max(100.), 48.),
                        egui::Sense::click_and_drag(),
                    );
                    theme::png(
                        ui,
                        "logo",
                        theme::LOGO,
                        egui::Rect::from_center_size(
                            egui::pos2(rect.left() + 27., rect.center().y),
                            egui::vec2(26., 26.),
                        ),
                    );
                    ui.painter().text(
                        egui::pos2(rect.left() + 50., rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        "Hello Minecraft! Launcher · Rust",
                        egui::FontId::proportional(14.),
                        Color32::WHITE,
                    );
                    if response.drag_started() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    if response.double_clicked() {
                        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    }
                    for (name, hint) in [
                        ("minimize", "最小化"),
                        ("maximize", "最大化 / 还原"),
                        ("close", "关闭"),
                    ] {
                        if ui
                            .add(
                                theme::MaterialButton::new("")
                                    .icon(name)
                                    .fill(Color32::TRANSPARENT)
                                    .min_size(egui::vec2(48., 48.)),
                            )
                            .on_hover_text(hint)
                            .clicked()
                        {
                            match name {
                                "minimize" => {
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true))
                                }
                                "maximize" => {
                                    let maximized =
                                        ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(
                                        !maximized,
                                    ));
                                }
                                _ => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                            }
                        }
                    }
                });
            });
        egui::TopBottomPanel::bottom("status")
            .exact_height(if self.busy { 64. } else { 30. })
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(egui::Margin::symmetric(16, 5)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if self.busy {
                        ui.spinner();
                    } else {
                        let (_, rect) = ui.allocate_space(egui::vec2(16., 16.));
                        theme::icon(
                            ui,
                            if self.error.is_empty() {
                                "check"
                            } else {
                                "info"
                            },
                            rect,
                            theme::PRIMARY,
                        );
                    }
                    ui.label(RichText::new(&self.status).size(12.));
                    if self.busy
                        && theme::button(
                            ui,
                            if self.running {
                                "结束游戏"
                            } else {
                                "取消"
                            },
                        )
                        .clicked()
                    {
                        self.cancel.store(true, Ordering::Relaxed);
                        self.status = "正在停止当前操作…".into();
                    }
                });
                if self.busy && self.progress > 0. {
                    ui.add(
                        egui::ProgressBar::new(self.progress)
                            .desired_height(3.)
                            .fill(theme::PRIMARY),
                    );
                }
            });
        egui::SidePanel::left("navigation")
            .exact_width(228.)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(if self.config.dark_mode {
                        Color32::from_rgb(30, 32, 44)
                    } else {
                        Color32::from_rgb(244, 244, 250)
                    })
                    .inner_margin(egui::Margin::symmetric(4, 12)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 0.;
                egui::ScrollArea::vertical()
                    .id_salt("navigation-scroll")
                    .show(ui, |ui| {
                        let category = |ui: &mut egui::Ui, text: &str| {
                            ui.add_space(10.);
                            ui.horizontal(|ui| {
                                ui.add_space(18.);
                                ui.label(RichText::new(text).size(11.).color(MUTED));
                            });
                            ui.add_space(2.);
                        };
                        category(ui, "账户");
                        if theme::nav(
                            ui,
                            "person",
                            &self.config.player,
                            Some("离线账户"),
                            self.page == Page::Accounts,
                        )
                        .clicked()
                        {
                            self.page = Page::Accounts;
                        }
                        category(ui, "游戏");
                        let selected = self.config.selected.clone();
                        for (page, icon) in [
                            (Page::Home, "home"),
                            (Page::Versions, "game"),
                            (Page::Download, "download"),
                            (Page::Mods, "extension"),
                            (Page::Packs, "package"),
                        ] {
                            let subtitle = if page == Page::Home {
                                Some(if selected.is_empty() {
                                    "尚未选择游戏"
                                } else {
                                    &selected
                                })
                            } else {
                                None
                            };
                            if theme::nav(ui, icon, page.title(), subtitle, self.page == page)
                                .clicked()
                            {
                                self.page = page;
                                if page == Page::Mods {
                                    self.refresh_mods();
                                }
                            }
                        }
                        category(ui, "启动器设置");
                        for (page, icon) in [
                            (Page::Java, "java"),
                            (Page::Settings, "settings"),
                            (Page::Logs, "logs"),
                            (Page::About, "info"),
                        ] {
                            if theme::nav(ui, icon, page.title(), None, self.page == page).clicked()
                            {
                                self.page = page;
                            }
                        }
                    });
            });
        if self.shown_page != self.page {
            self.shown_page = self.page;
            self.page_started = ctx.input(|i| i.time);
            self.version_popup = false;
        }
        let elapsed = ctx.input(|i| i.time) - self.page_started;
        let transition = if self.page_started < 0. {
            1.
        } else {
            (elapsed / 0.22).clamp(0., 1.) as f32
        };
        if transition < 1. {
            ctx.request_repaint();
        }
        let home = self.page == Page::Home;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(ctx.style().visuals.panel_fill)
                    .inner_margin(if home { 0 } else { 24 }),
            )
            .show(ctx, |ui| {
                ui.set_opacity(transition);
                if !home {
                    ui.add_space(10. * (1. - transition));
                    ui.horizontal(|ui| {
                        let (_, rect) = ui.allocate_space(egui::vec2(25., 25.));
                        theme::icon(ui, self.page.icon(), rect, theme::PRIMARY);
                        ui.heading(self.page.title());
                    });
                    ui.add_space(12.);
                }
                if self.font_warning {
                    ui.label("CJK font missing: install Noto Sans CJK.");
                }
                if !self.error.is_empty() {
                    egui::Frame::new()
                        .fill(if self.config.dark_mode {
                            Color32::from_rgb(82, 43, 48)
                        } else {
                            Color32::from_rgb(255, 235, 234)
                        })
                        .inner_margin(14.)
                        .corner_radius(6.)
                        .show(ui, |ui| {
                            ui.colored_label(Color32::from_rgb(195, 74, 65), &self.error);
                            if theme::button(ui, "关闭提示").clicked() {
                                self.error.clear();
                            }
                        });
                }
                ui.add_enabled_ui(
                    !self.busy || self.page == Page::Logs || self.page == Page::About,
                    |ui| {
                        if home {
                            self.home(ui);
                        } else {
                            egui::ScrollArea::vertical()
                                .id_salt(self.page.title())
                                .show(ui, |ui| self.content(ui));
                        }
                    },
                );
            });
    }
}
fn main() -> eframe::Result {
    if cli::requested() {
        if let Err(e) = cli::run() {
            eprintln!("{e:#}");
            std::process::exit(1);
        }
        return Ok(());
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_decorations(false)
            .with_inner_size([1080., 700.])
            .with_icon({
                let image = image::load_from_memory(theme::LOGO)
                    .expect("embedded HMCL icon")
                    .into_rgba8();
                egui::IconData {
                    width: image.width(),
                    height: image.height(),
                    rgba: image.into_raw(),
                }
            })
            .with_min_inner_size([860., 560.]),
        ..Default::default()
    };
    eframe::run_native(
        "Rust HMCL",
        options,
        Box::new(|cc| Ok(Box::new(Launcher::new(cc)))),
    )
}
