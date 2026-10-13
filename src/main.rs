#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod appearance;
mod avatars;
mod catalog;
mod cli;
mod config;
mod game;
mod instance_ops;
mod instance_ui;
mod java_runtime;
mod loaders;
mod metadata;
mod modpack;
mod modrinth;
mod mods;
mod motion;
mod preview;
mod resourcepack_ui;
mod resourcepacks;
mod screenshots;
mod theme;
mod window_chrome;

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

#[derive(Clone, Copy)]
enum VersionAction {
    Launch,
    Settings,
    Favorite,
    Folder,
    Mods,
    Fabric,
    Quilt,
    Repair,
    Duplicate,
    Rename,
}

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
    Appearance,
    Instance,
}
impl Page {
    fn key(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Accounts => "accounts",
            Self::Versions => "versions",
            Self::Mods => "mods",
            Self::Packs => "packs",
            Self::Download => "download",
            Self::Java => "java",
            Self::Settings => "settings",
            Self::Logs => "logs",
            Self::About => "about",
            Self::Appearance => "appearance",
            Self::Instance => "instance",
        }
    }
    fn from_key(key: &str) -> Self {
        match key {
            "accounts" => Self::Accounts,
            "versions" => Self::Versions,
            "mods" => Self::Mods,
            "packs" => Self::Packs,
            "download" => Self::Download,
            "java" => Self::Java,
            "settings" => Self::Settings,
            "logs" => Self::Logs,
            "about" => Self::About,
            "appearance" => Self::Appearance,
            "instance" => Self::Instance,
            _ => Self::Home,
        }
    }
    fn description(self) -> &'static str {
        match self {
            Self::Home => "选择版本，开始探索",
            Self::Accounts => "管理你的游戏身份，在账户之间轻松切换。",
            Self::Versions => "你的所有游戏版本，集中管理与维护。",
            Self::Mods => "整理已安装模组，发现适合当前版本的新内容。",
            Self::Packs => "导入整合包，一次准备好完整的游戏环境。",
            Self::Download => "发现 Minecraft 正式版与快照，安装新的游戏版本。",
            Self::Appearance => "主题、壁纸与动画，让启动器更符合你的习惯。",
            Self::Instance => "管理此实例的游戏设置、组件、模组与本地内容。",
            Self::Java => "为不同年代的 Minecraft 准备合适的运行环境。",
            Self::Settings => "按照你的习惯调整游戏和启动器。",
            Self::Logs => "查看游戏运行信息，更快找到启动问题。",
            Self::About => "关于 Rust HMCL 与这个项目的来源。",
        }
    }
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
            Self::Appearance => "sun",
            Self::Instance => "settings",
        }
    }
    fn title(self) -> &'static str {
        match self {
            Self::Home => "首页",
            Self::Accounts => "账户",
            Self::Versions => "实例列表",
            Self::Mods => "模组管理",
            Self::Packs => "整合包",
            Self::Download => "下载",
            Self::Java => "Java 管理",
            Self::Settings => "设置",
            Self::Logs => "游戏日志",
            Self::About => "关于",
            Self::Appearance => "外观",
            Self::Instance => "实例管理",
        }
    }
}
struct Launcher {
    config: Config,
    page: Page,
    versions: Vec<Version>,
    installed: Vec<String>,
    descriptions: std::collections::BTreeMap<String, catalog::Description>,
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
    wallpaper: Option<appearance::Wallpaper>,
    version_filter: String,
    favorites_only: bool,
    editing_version: Option<String>,
    edit_settings: config::VersionSettings,
    editing_error: String,
    instance_tab: instance_ui::Tab,
    instance_transition: motion::TabTransition,
    instance_return_page: Page,
    runtimes: Vec<java_runtime::Runtime>,
    java_loaded: bool,
    instance_operation: Option<(String, bool, String)>,
    instance_name: String,
    instance_operation_error: String,
    screenshots: screenshots::Browser,
    resourcepacks: resourcepack_ui::Browser,
    window_corners: window_chrome::Corners,
}
impl Launcher {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (config, error) = match Config::load() {
            Ok(config) => (config, String::new()),
            Err(error) => (Config::default(), format!("{error:#}")),
        };
        Self::with_config(&cc.egui_ctx, config, error)
    }
    fn with_config(ctx: &egui::Context, mut config: Config, error: String) -> Self {
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
        ctx.set_fonts(fonts);
        let mut visuals = egui::Visuals::light();
        visuals.panel_fill = Color32::from_rgb(246, 248, 249);
        visuals.selection.bg_fill = GREEN;
        visuals.widgets.active.bg_fill = GREEN;
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(225, 239, 229);
        ctx.set_visuals(visuals);
        ctx.style_mut(|s| {
            s.spacing.item_spacing = egui::vec2(12., 12.);
            s.spacing.button_padding = egui::vec2(14., 9.);
        });
        config.normalize();
        theme::setup(ctx, config.dark_mode);
        ctx.set_zoom_factor(config.ui_scale);
        if config.reduce_motion {
            ctx.style_mut(|style| style.animation_time = 0.);
        }
        let wallpaper = if config.wallpaper.is_empty() {
            None
        } else {
            appearance::Wallpaper::load(std::path::Path::new(&config.wallpaper)).ok()
        };
        let installed = game::installed(std::path::Path::new(&config.game_dir));
        let descriptions = catalog::load(std::path::Path::new(&config.game_dir), &installed);
        let page = Page::from_key(&config.start_page);
        let editing_version = if page == Page::Instance {
            Some(config.selected.clone())
        } else {
            None
        };
        let edit_settings = editing_version
            .as_ref()
            .and_then(|id| config.version_settings.get(id))
            .cloned()
            .unwrap_or_default();
        Self {
            config,
            page,
            versions: vec![],
            installed,
            descriptions,
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
            shown_page: page,
            page_started: -1.,
            version_popup: false,
            wallpaper,
            version_filter: String::new(),
            favorites_only: false,
            editing_version,
            edit_settings,
            editing_error: String::new(),
            instance_tab: instance_ui::Tab::Settings,
            instance_transition: motion::TabTransition::default(),
            instance_return_page: Page::Home,
            runtimes: vec![],
            java_loaded: false,
            instance_operation: None,
            instance_name: String::new(),
            instance_operation_error: String::new(),
            screenshots: screenshots::Browser::default(),
            resourcepacks: resourcepack_ui::Browser::default(),
            window_corners: window_chrome::Corners::default(),
        }
    }
    fn refresh(&mut self) {
        self.installed = game::installed(std::path::Path::new(&self.config.game_dir));
        self.descriptions =
            catalog::load(std::path::Path::new(&self.config.game_dir), &self.installed);
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
                    self.config.hidden_java.retain(|path| path != &p);
                    if !self.config.java_paths.contains(&p) {
                        self.config.java_paths.push(p.clone());
                    }
                    self.java_loaded = false;
                    self.status = format!("Java 环境可用：{p}");
                    self.save();
                }
                Event::Runtimes(runtimes) => {
                    self.runtimes = runtimes;
                    self.java_loaded = true;
                    self.status = format!("发现 {} 个 Java 环境", self.runtimes.len());
                }
                Event::InstanceChanged { old, new, renamed } => {
                    self.config.remap_instance(&old, &new, renamed);
                    if self.editing_version.as_deref() == Some(old.as_str()) && renamed {
                        self.editing_version = Some(new.clone());
                    }
                    self.refresh();
                    self.save();
                    self.status = if renamed {
                        format!("已重命名为 {new}")
                    } else {
                        format!("已复制为 {new}")
                    };
                }
                Event::Installed(id) => {
                    self.config.selected = id;
                    self.refresh();
                    if self.page == Page::Instance
                        && self.instance_tab == instance_ui::Tab::Components
                    {
                        self.editing_version = Some(self.config.selected.clone());
                        self.edit_settings = self
                            .config
                            .version_settings
                            .get(&self.config.selected)
                            .cloned()
                            .unwrap_or_default();
                    }
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
        theme::card(ui, draw);
    }
    fn home(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap();
        ui.allocate_rect(rect, egui::Sense::hover());
        let launch = egui::Rect::from_min_size(
            egui::pos2(rect.right() - 250., rect.bottom() - 77.),
            egui::vec2(230., 57.),
        );
        let popup_motion = ui.ctx().animate_bool_with_time(
            egui::Id::new("version-popup-open"),
            self.version_popup,
            if self.config.reduce_motion { 0. } else { 0.2 },
        );
        let (start, menu) = theme::launch_control(
            ui,
            launch,
            if self.config.selected.is_empty() {
                "安装游戏"
            } else {
                "启动游戏"
            },
            &self.config.selected,
            popup_motion,
            !self.busy,
        );
        if start.clicked() {
            if self.config.selected.is_empty() {
                self.page = Page::Download;
            } else {
                self.launch();
            }
        }
        if menu.clicked() || start.secondary_clicked() || menu.secondary_clicked() {
            self.version_popup = !self.version_popup;
        }
        if (start.hovered() || menu.hovered()) && !self.installed.is_empty() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            let now = ui.input(|i| i.time);
            let last = ui
                .ctx()
                .data(|d| d.get_temp::<f64>(egui::Id::new("version-wheel")))
                .unwrap_or(-1.);
            if scroll.abs() > 2. && now - last > 0.18 {
                let current = self
                    .installed
                    .iter()
                    .position(|v| v == &self.config.selected)
                    .unwrap_or(0);
                let next = if scroll < 0. {
                    (current + 1) % self.installed.len()
                } else {
                    (current + self.installed.len() - 1) % self.installed.len()
                };
                self.config.selected = self.installed[next].clone();
                self.save();
                ui.ctx()
                    .data_mut(|d| d.insert_temp(egui::Id::new("version-wheel"), now));
            }
        }
        if self.version_popup || popup_motion > 0. {
            let height = ((self.installed.len().max(1) as f32 * 52.) + 50.)
                .min(launch.top() - rect.top() - 24.)
                .max(80.);
            let popup = egui::Rect::from_min_size(
                egui::pos2(launch.right() - 280., launch.top() - height - 8.),
                egui::vec2(280., height),
            );
            let area = egui::Area::new(egui::Id::new("version-selector"))
                .order(egui::Order::Foreground)
                .fixed_pos(popup.min + egui::vec2(0., 12. * (1. - popup_motion)))
                .show(ui.ctx(), |ui| {
                    ui.set_opacity(popup_motion);
                    if !self.version_popup {
                        ui.disable();
                    }
                    egui::Frame::popup(ui.style())
                        .inner_margin(8.)
                        .corner_radius(4.)
                        .show(ui, |ui| {
                            ui.set_width(264.);
                            egui::ScrollArea::vertical()
                                .max_height(height - 16.)
                                .show(ui, |ui| {
                                    for version in self.installed.clone() {
                                        if theme::nav(
                                            ui,
                                            "grass",
                                            &version,
                                            Some("Minecraft"),
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
                                        ui.label("尚未安装游戏");
                                        if theme::button(ui, "安装新版本").clicked() {
                                            self.version_popup = false;
                                            self.page = Page::Download;
                                        }
                                    }
                                });
                        });
                });
            if ui.input(|i| {
                i.key_pressed(egui::Key::Escape)
                    || i.pointer.any_pressed()
                        && i.pointer
                            .interact_pos()
                            .is_some_and(|p| !area.response.rect.contains(p) && !launch.contains(p))
            }) {
                self.version_popup = false;
            }
        }
        ui.painter().text(
            rect.left_bottom() + egui::vec2(20., -20.),
            egui::Align2::LEFT_BOTTOM,
            format!("Rust HMCL {}", env!("CARGO_PKG_VERSION")),
            egui::FontId::proportional(11.),
            Color32::from_white_alpha(220),
        );
    }
    fn content(&mut self, ui: &mut egui::Ui) {
        match self.page {
            Page::Instance => self.instance_content(ui),
            Page::Home => self.home(ui),
            Page::Accounts => Self::card(ui, |ui| {
                theme::section(ui, "游戏身份", "选择一个账户，或添加新的离线身份。");
                for player in self.config.accounts.clone() {
                    ui.horizontal(|ui| {
                        if ui
                            .allocate_ui_with_layout(
                                egui::vec2((ui.available_width() - 110.).max(100.), 58.),
                                egui::Layout::top_down(egui::Align::Min),
                                |ui| {
                                    theme::nav(
                                        ui,
                                        "person",
                                        &player,
                                        Some("离线账户"),
                                        self.config.player == player,
                                    )
                                },
                            )
                            .inner
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
                    theme::badge(ui, &format!("{} 个版本", self.installed.len()), false);
                    if theme::button(ui, "刷新本地版本").clicked() {
                        self.refresh();
                    }
                    if theme::button(ui, "安装新版本").clicked() {
                        self.page = Page::Download;
                    }
                });
                ui.horizontal(|ui| {
                    let width = (ui.available_width() - 158.).min(560.);
                    theme::text_field_sized(ui, &mut self.version_filter, width, "搜索游戏版本");
                    theme::toggle(ui, &mut self.favorites_only, "只看收藏");
                });
                if self.installed.is_empty() {
                    theme::empty(
                        ui,
                        "game",
                        "游戏库还是空的",
                        "安装新版本，或在设置中选择已有的游戏目录。",
                    );
                }
                let visible = self
                    .installed
                    .iter()
                    .filter(|id| {
                        id.to_lowercase()
                            .contains(&self.version_filter.to_lowercase())
                            && (!self.favorites_only || self.config.favorites.contains(id))
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if !self.installed.is_empty() && visible.is_empty() {
                    theme::empty(
                        ui,
                        "search",
                        "没有找到匹配版本",
                        "试试其他关键词，或关闭收藏筛选。",
                    );
                }
                for version in visible {
                    ui.push_id(&version, |ui| {
                        let info = self.descriptions.get(&version).cloned().unwrap_or(
                            catalog::Description {
                                icon: "grass",
                                subtitle: "Minecraft".into(),
                            },
                        );
                        let row = theme::game_row(
                            ui,
                            &version,
                            &info.subtitle,
                            info.icon,
                            self.config.selected == version,
                            self.config.favorites.contains(&version),
                        );
                        let mut action = None;
                        if row.select.clicked() {
                            self.config.selected = version.clone();
                            self.save();
                        }
                        if row.settings.clicked() {
                            action = Some(VersionAction::Settings);
                        }
                        if row.launch.clicked() {
                            action = Some(VersionAction::Launch);
                        }
                        row.settings.context_menu(|ui| {
                            action = self.version_menu(ui, &version).or(action);
                        });
                        let popup_id = ui.make_persistent_id("version-actions");
                        if row.more.clicked() {
                            ui.memory_mut(|memory| memory.toggle_popup(popup_id));
                        }
                        egui::popup::popup_above_or_below_widget(
                            ui,
                            popup_id,
                            &row.more,
                            egui::AboveOrBelow::Below,
                            egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                            |ui| {
                                ui.set_min_width(210.);
                                if let Some(chosen) = self.version_menu(ui, &version) {
                                    action = Some(chosen);
                                    ui.memory_mut(|memory| memory.close_popup());
                                }
                            },
                        );
                        if let Some(action) = action {
                            self.apply_version_action(&version, action);
                        }
                    });
                    ui.add_space(2.);
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
                    theme::empty(
                        ui,
                        "extension",
                        "还没有安装模组",
                        "导入本地 .jar，或从下方搜索兼容的 Modrinth 模组。",
                    );
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
                    theme::empty(
                        ui,
                        "download",
                        "发现你的下一个世界",
                        "获取官方版本列表，选择正式版或快照进行安装。",
                    );
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
            Page::Java => {
                ui.horizontal_wrapped(|ui| {
                    if theme::button(ui, "刷新 Java 环境").clicked() {
                        self.scan_java();
                    }
                    if theme::button(ui, "添加 Java").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("选择 Java 可执行文件")
                            .pick_file()
                        {
                            self.start(move |worker| {
                                let runtime = java_runtime::probe(&path)?;
                                worker
                                    .tx
                                    .send(Event::Java(runtime.path.to_string_lossy().into()))?;
                                Ok(())
                            });
                        }
                    }
                    if theme::button(ui, "使用自动选择").clicked() {
                        self.config.java.clear();
                        self.save();
                    }
                });
                ui.label("启动器本身无需 Java；这些环境仅用于运行 Minecraft。");
                if self.config.java.is_empty() {
                    theme::badge(ui, "按游戏需求自动选择", true);
                } else {
                    ui.label(format!("当前指定：{}", self.config.java));
                }
                if self.runtimes.is_empty() {
                    theme::empty(
                        ui,
                        "java",
                        "没有已列出的 Java 环境",
                        "刷新检测本机环境，添加自己的 Java，或下载运行环境。",
                    );
                }
                for runtime in self.runtimes.clone() {
                    ui.push_id(&runtime.path, |ui| {
                        let path = runtime.path.to_string_lossy().into_owned();
                        let (select, reveal, hide) =
                            theme::java_row(ui, &runtime, self.config.java == path);
                        if select.clicked() {
                            self.config.java = path.clone();
                            self.save();
                        }
                        if reveal.clicked() {
                            if let Some(parent) = runtime.path.parent() {
                                let directory = parent
                                    .parent()
                                    .filter(|home| home.join("release").is_file())
                                    .unwrap_or(parent);
                                if let Err(error) = open::that(directory) {
                                    self.error = error.to_string();
                                }
                            }
                        }
                        if hide.clicked() {
                            self.config.hidden_java.push(path.clone());
                            if self.config.java == path {
                                self.config.java.clear();
                            }
                            self.save();
                            self.runtimes
                                .retain(|runtime| runtime.path.to_string_lossy() != path);
                        }
                    });
                    ui.add_space(2.);
                }
                if !self.config.hidden_java.is_empty() {
                    egui::CollapsingHeader::new(format!(
                        "恢复隐藏环境（{}）",
                        self.config.hidden_java.len()
                    ))
                    .show(ui, |ui| {
                        for path in self.config.hidden_java.clone() {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(egui::RichText::new(&path).small());
                                if theme::button(ui, "恢复").clicked() {
                                    self.config.hidden_java.retain(|value| value != &path);
                                    self.save();
                                    self.java_loaded = false;
                                }
                            });
                        }
                    });
                }
                Self::card(ui, |ui| {
                    theme::section(ui, "下载 Java", "下载完成后将刷新本机运行环境列表。");
                    ui.horizontal_wrapped(|ui| {
                        egui::ComboBox::from_id_salt("java-major")
                            .selected_text(format!("Java {}", self.java_major))
                            .show_ui(ui, |ui| {
                                let mut majors = std::collections::BTreeSet::from([8, 17, 21, 25]);
                                majors.extend(self.runtimes.iter().map(|runtime| runtime.major));
                                for major in majors {
                                    ui.selectable_value(
                                        &mut self.java_major,
                                        major,
                                        format!("Java {major}"),
                                    );
                                }
                            });
                        if ui
                            .add(
                                theme::MaterialButton::new("下载 Eclipse Temurin JDK")
                                    .subtitle("官方运行环境"),
                            )
                            .clicked()
                        {
                            let major = self.java_major;
                            self.start(move |worker| worker.download_java(major));
                        }
                    });
                    if theme::toggle(
                        ui,
                        &mut self.config.auto_java,
                        "缺少游戏所需 Java 时自动下载",
                    )
                    .changed()
                    {
                        self.save();
                    }
                });
            }
            Page::Settings => Self::card(ui, |ui| {
                theme::section(ui, "游戏与外观", "设置将在保存后应用到下一次游戏启动。");
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
            Page::Appearance => {
                Self::card(ui, |ui| {
                    theme::section(ui, "主题与显示", "让启动器更符合你的习惯。");
                    if theme::toggle(ui, &mut self.config.dark_mode, "深色主题").changed() {
                        self.apply_appearance(ui.ctx());
                        self.save();
                    }
                    ui.add_space(8.);
                    ui.label("界面缩放");
                    let zoom = ui.add(
                        egui::Slider::new(&mut self.config.ui_scale, 0.8..=1.5)
                            .step_by(0.05)
                            .text("缩放倍数"),
                    );
                    if zoom.drag_stopped() || zoom.changed() && !zoom.dragged() {
                        self.apply_appearance(ui.ctx());
                        self.save();
                    }
                    if theme::toggle(ui, &mut self.config.reduce_motion, "减少动画和动态效果")
                        .changed()
                    {
                        self.apply_appearance(ui.ctx());
                        self.save();
                    }
                    ui.label("启动时打开");
                    egui::ComboBox::from_id_salt("start-page")
                        .selected_text(Page::from_key(&self.config.start_page).title())
                        .show_ui(ui, |ui| {
                            for page in [
                                Page::Home,
                                Page::Versions,
                                Page::Download,
                                Page::Appearance,
                                Page::Accounts,
                                Page::Java,
                            ] {
                                if ui
                                    .selectable_label(
                                        self.config.start_page == page.key(),
                                        page.title(),
                                    )
                                    .clicked()
                                {
                                    self.config.start_page = page.key().into();
                                    self.save();
                                }
                            }
                        });
                });
                ui.add_space(8.);
                Self::card(ui, |ui| {
                    theme::section(
                        ui,
                        "首页壁纸",
                        "选择自己的 PNG 或 JPEG 图片，自动保持比例填满背景。",
                    );
                    let preview = egui::vec2(ui.available_width().min(560.), 180.);
                    let (_, rect) = ui.allocate_space(preview);
                    theme::wallpaper(ui, rect, self.wallpaper.as_ref());
                    ui.label(
                        RichText::new(if self.config.wallpaper.is_empty() {
                            "HMCL 默认壁纸"
                        } else {
                            &self.config.wallpaper
                        })
                        .small()
                        .color(ui.visuals().weak_text_color()),
                    );
                    ui.horizontal(|ui| {
                        if theme::button(ui, "选择壁纸").clicked() {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("壁纸", &["png", "jpg", "jpeg"])
                                .pick_file()
                            {
                                match appearance::Wallpaper::load(&path) {
                                    Ok(wallpaper) => {
                                        if let Some(old) = self.wallpaper.take() {
                                            ui.ctx().forget_image(&old.uri);
                                        }
                                        self.wallpaper = Some(wallpaper);
                                        self.config.wallpaper = path.to_string_lossy().into();
                                        self.save();
                                    }
                                    Err(e) => self.error = e.to_string(),
                                }
                            }
                        }
                        if theme::button(ui, "恢复默认壁纸").clicked() {
                            if let Some(old) = self.wallpaper.take() {
                                ui.ctx().forget_image(&old.uri);
                            }
                            self.config.wallpaper.clear();
                            self.save();
                        }
                    });
                });
            }
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
    fn version_menu(&self, ui: &mut egui::Ui, id: &str) -> Option<VersionAction> {
        let mut action = None;
        for (name, value) in [
            ("启动游戏", VersionAction::Launch),
            ("版本设置", VersionAction::Settings),
            (
                if self.config.favorites.iter().any(|v| v == id) {
                    "取消收藏"
                } else {
                    "收藏"
                },
                VersionAction::Favorite,
            ),
            ("游戏目录", VersionAction::Folder),
            ("管理模组", VersionAction::Mods),
            ("安装 Fabric", VersionAction::Fabric),
            ("安装 Quilt", VersionAction::Quilt),
            ("校验 / 修复", VersionAction::Repair),
            ("重命名", VersionAction::Rename),
            ("复制实例", VersionAction::Duplicate),
        ] {
            if theme::button(ui, name).clicked() {
                action = Some(value);
                ui.close_menu();
            }
        }
        action
    }
    fn apply_version_action(&mut self, id: &str, action: VersionAction) {
        if self.busy
            && matches!(
                action,
                VersionAction::Launch
                    | VersionAction::Fabric
                    | VersionAction::Quilt
                    | VersionAction::Repair
                    | VersionAction::Rename
                    | VersionAction::Duplicate
            )
        {
            return;
        }
        match action {
            VersionAction::Duplicate | VersionAction::Rename => {
                self.instance_operation = Some((
                    id.into(),
                    matches!(action, VersionAction::Rename),
                    self.config.game_dir.clone(),
                ));
                self.instance_name = if matches!(action, VersionAction::Rename) {
                    id.into()
                } else {
                    format!("{id}-copy")
                };
                self.instance_operation_error.clear();
            }
            VersionAction::Launch => {
                self.config.selected = id.into();
                self.launch();
            }
            VersionAction::Settings => {
                if self.page != Page::Instance {
                    self.instance_return_page = self.page;
                }
                self.editing_error.clear();
                self.edit_settings = self
                    .config
                    .version_settings
                    .get(id)
                    .cloned()
                    .unwrap_or_default();
                self.editing_version = Some(id.into());
                self.instance_tab = instance_ui::Tab::Settings;
                self.instance_transition = motion::TabTransition::default();
                self.page = Page::Instance;
            }
            VersionAction::Favorite => {
                if self.config.favorites.iter().any(|v| v == id) {
                    self.config.favorites.retain(|v| v != id);
                } else {
                    self.config.favorites.push(id.into());
                }
                self.save();
            }
            VersionAction::Folder => {
                let config = Config {
                    selected: id.into(),
                    ..self.config.clone()
                };
                self.open_game_dir(&config);
            }
            VersionAction::Mods => {
                self.config.selected = id.into();
                self.save();
                self.refresh_mods();
                self.page = Page::Mods;
            }
            VersionAction::Fabric => self.install_loader(id.into(), loaders::Loader::Fabric),
            VersionAction::Quilt => self.install_loader(id.into(), loaders::Loader::Quilt),
            VersionAction::Repair => {
                let root = std::path::PathBuf::from(&self.config.game_dir);
                let id = id.to_owned();
                self.start(move |w| {
                    let meta = metadata::resolve(&root, &id)?;
                    w.prepare(&root, &id, &meta)
                });
            }
        }
    }
    fn refresh_mods(&mut self) {
        match mods::directory(&self.config).and_then(|dir| mods::list(&dir)) {
            Ok(files) => self.mod_files = files,
            Err(e) => self.error = e.to_string(),
        }
    }
    fn scan_java(&mut self) {
        self.java_loaded = true;
        let paths = self.config.java_paths.clone();
        let hidden = self.config.hidden_java.clone();
        self.start(move |worker| {
            let runtimes = java_runtime::discover(&paths, &hidden, &worker.cancel)?;
            worker.tx.send(Event::Runtimes(runtimes))?;
            Ok(())
        });
    }
    fn instance_operation_dialog(&mut self, ctx: &egui::Context) {
        let Some((old, rename, game_dir)) = self.instance_operation.clone() else {
            return;
        };
        let mut close = false;
        let response = egui::Modal::new(egui::Id::new("instance-operation"))
            .frame(
                egui::Frame::popup(&ctx.style())
                    .inner_margin(24.)
                    .corner_radius(4.),
            )
            .show(ctx, |ui| {
                ui.set_width(440.);
                ui.label(
                    RichText::new(if rename {
                        "重命名实例"
                    } else {
                        "复制实例"
                    })
                    .size(20.)
                    .strong(),
                );
                ui.label(format!("原实例：{old}"));
                ui.label("新名称");
                theme::text_field(ui, &mut self.instance_name);
                ui.label(if rename {
                    "保留游戏文件，其他依赖此版本的实例会继续可用。"
                } else {
                    "复制独立存档、模组和配置；使用共享目录的实例仍共享数据。"
                });
                ui.add_enabled_ui(!self.busy, |ui| {
                    ui.horizontal(|ui| {
                        if theme::button(ui, if rename { "重命名" } else { "复制" }).clicked()
                        {
                            let new = self.instance_name.trim().to_owned();
                            match instance_ops::validate_name(&new) {
                                Ok(()) => {
                                    let root = std::path::PathBuf::from(&game_dir);
                                    let old = old.clone();
                                    self.start(move |worker| {
                                        if rename {
                                            instance_ops::rename(
                                                &root,
                                                &old,
                                                &new,
                                                &worker.cancel,
                                            )?;
                                        } else {
                                            instance_ops::duplicate(
                                                &root,
                                                &old,
                                                &new,
                                                &worker.cancel,
                                            )?;
                                        }
                                        worker.tx.send(Event::InstanceChanged {
                                            old,
                                            new,
                                            renamed: rename,
                                        })?;
                                        Ok(())
                                    });
                                    close = true;
                                }
                                Err(error) => self.instance_operation_error = error.to_string(),
                            }
                        }
                        if theme::button(ui, "取消").clicked() {
                            close = true;
                        }
                    });
                });
                if !self.instance_operation_error.is_empty() {
                    ui.colored_label(
                        if self.config.dark_mode {
                            egui::Color32::from_rgb(255, 180, 171)
                        } else {
                            egui::Color32::from_rgb(160, 40, 35)
                        },
                        &self.instance_operation_error,
                    );
                }
            });
        if response.should_close() || close {
            self.instance_operation = None;
        }
    }
    fn apply_appearance(&self, ctx: &egui::Context) {
        theme::setup(ctx, self.config.dark_mode);
        ctx.set_zoom_factor(self.config.ui_scale);
        if self.config.reduce_motion {
            ctx.style_mut(|style| style.animation_time = 0.);
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
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.window_corners.update(frame);
        if self.page == Page::Java && !self.java_loaded && !self.busy {
            self.scan_java();
        }
        self.draw(ctx);
    }
}
impl Launcher {
    fn draw(&mut self, ctx: &egui::Context) {
        self.poll();
        if self.busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }
        let screen = ctx.screen_rect();
        // Paint first on the same background layer as the panels. A separate
        // background Area can sort above the panel layer and cover its controls.
        let wallpaper_ui = egui::Ui::new(
            ctx.clone(),
            egui::Id::new("hmcl-wallpaper"),
            egui::UiBuilder::new()
                .layer_id(egui::LayerId::background())
                .max_rect(screen),
        );
        theme::wallpaper(&wallpaper_ui, screen, self.wallpaper.as_ref());
        egui::TopBottomPanel::top("title")
            .exact_height(40.)
            .frame(egui::Frame::new().fill(if self.config.dark_mode {
                Color32::from_rgb(49, 61, 113)
            } else {
                Color32::from_rgb(222, 225, 255)
            }))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.;
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2((ui.available_width() - 160.).max(100.), 40.),
                        egui::Sense::click_and_drag(),
                    );
                    if self.page == Page::Home {
                        theme::png(
                            ui,
                            "title-icon",
                            theme::TITLE_ICON,
                            egui::Rect::from_center_size(
                                egui::pos2(rect.left() + 27., rect.center().y),
                                egui::vec2(26., 26.),
                            ),
                        );
                    } else {
                        theme::icon(
                            ui,
                            "back",
                            egui::Rect::from_center_size(
                                egui::pos2(rect.left() + 27., rect.center().y),
                                egui::vec2(24., 24.),
                            ),
                            theme::on_container(ui),
                        );
                    }
                    let title = if self.page == Page::Home {
                        "Hello Minecraft! Launcher · Rust".into()
                    } else if self.page == Page::Instance {
                        format!(
                            "实例管理 - {}",
                            self.editing_version.as_deref().unwrap_or("")
                        )
                    } else {
                        self.page.title().to_owned()
                    };
                    theme::text_line(
                        ui,
                        &title,
                        egui::Rect::from_min_max(
                            egui::pos2(rect.left() + 50., rect.top()),
                            egui::pos2(rect.right() - 8., rect.bottom()),
                        ),
                        14.,
                        theme::on_container(ui),
                    );
                    if response.drag_started() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                    let back = ui
                        .interact(
                            egui::Rect::from_min_size(rect.min, egui::vec2(44., 40.)),
                            ui.id().with("navigate-back"),
                            egui::Sense::click(),
                        )
                        .on_hover_text("返回");
                    if self.page != Page::Home && back.clicked() {
                        self.page = if self.page == Page::Instance {
                            self.instance_return_page
                        } else {
                            Page::Home
                        };
                    }
                    if response.double_clicked() {
                        let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    }
                    if ui
                        .add(
                            theme::MaterialButton::new("")
                                .icon("sun")
                                .fill(Color32::TRANSPARENT)
                                .foreground(theme::on_container(ui))
                                .min_size(egui::vec2(40., 40.)),
                        )
                        .on_hover_text("切换浅色 / 深色主题")
                        .clicked()
                    {
                        self.config.dark_mode = !self.config.dark_mode;
                        self.apply_appearance(ctx);
                        self.save();
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
                                    .foreground(theme::on_container(ui))
                                    .min_size(egui::vec2(40., 40.)),
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
            .exact_width(200.)
            .resizable(false)
            .frame(
                egui::Frame::new()
                    .fill(if self.config.dark_mode {
                        Color32::from_rgba_unmultiplied(30, 32, 44, 224)
                    } else {
                        Color32::from_rgba_unmultiplied(244, 244, 244, 210)
                    })
                    .inner_margin(egui::Margin::symmetric(4, 12)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 0.;
                egui::ScrollArea::vertical()
                    .id_salt("navigation-scroll")
                    .show(ui, |ui| {
                        if self.page == Page::Instance {
                            self.instance_navigation(ui);
                            return;
                        }
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
                        let info = self.descriptions.get(&selected);
                        let title = if selected.is_empty() {
                            "没有游戏实例"
                        } else {
                            "实例管理"
                        };
                        let subtitle = if selected.is_empty() {
                            "进入下载页安装游戏"
                        } else {
                            &selected
                        };
                        if theme::nav(
                            ui,
                            info.map(|v| v.icon).unwrap_or("grass"),
                            title,
                            Some(subtitle),
                            self.page == Page::Instance
                                && self.editing_version.as_deref() == Some(selected.as_str()),
                        )
                        .clicked()
                        {
                            if selected.is_empty() {
                                self.page = Page::Versions;
                            } else {
                                self.apply_version_action(&selected, VersionAction::Settings);
                            }
                        }
                        for (page, icon) in [(Page::Versions, "list"), (Page::Download, "download")]
                        {
                            if theme::nav(ui, icon, page.title(), None, self.page == page).clicked()
                            {
                                self.page = page;
                            }
                        }
                        category(ui, "启动器设置");
                        for (page, icon) in [
                            (Page::Mods, "extension"),
                            (Page::Packs, "package"),
                            (Page::Java, "java"),
                            (Page::Appearance, "sun"),
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
            if self.page == Page::Mods {
                self.refresh_mods();
            }
            self.shown_page = self.page;
            self.page_started = ctx.input(|i| i.time);
            self.version_popup = false;
        }
        let elapsed = ctx.input(|i| i.time) - self.page_started;
        let transition = if self.page_started < 0. || self.config.reduce_motion {
            1.
        } else {
            let progress = (elapsed / 0.32).clamp(0., 1.) as f32;
            1. - (1. - progress).powi(3)
        };
        if transition < 1. {
            ctx.request_repaint();
        }
        let home = self.page == Page::Home;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(if home {
                        Color32::TRANSPARENT
                    } else {
                        ctx.style().visuals.panel_fill.gamma_multiply(0.93)
                    })
                    .inner_margin(if home { 0 } else { 24 }),
            )
            .show(ctx, |ui| {
                ui.set_opacity(transition);
                if !home && self.page != Page::Instance {
                    ui.add_space(22. * (1. - transition));
                    ui.horizontal(|ui| {
                        let (_, rect) = ui.allocate_space(egui::vec2(25., 25.));
                        theme::icon(ui, self.page.icon(), rect, theme::accent(ui));
                        ui.heading(self.page.title());
                    });
                    ui.label(
                        RichText::new(self.page.description())
                            .size(13.)
                            .color(ui.visuals().weak_text_color()),
                    );
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
                            ui.colored_label(
                                if self.config.dark_mode {
                                    Color32::from_rgb(255, 180, 171)
                                } else {
                                    Color32::from_rgb(160, 40, 35)
                                },
                                &self.error,
                            );
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
                        } else if self.page == Page::Logs {
                            self.content(ui);
                        } else {
                            egui::ScrollArea::vertical()
                                .id_salt(self.page.title())
                                .show(ui, |ui| self.content(ui));
                        }
                    },
                );
            });
        let operation_open = self.instance_operation.is_some()
            || self.screenshots.modal_open()
            || self.resourcepacks.modal_open();
        self.instance_operation_dialog(ctx);
        self.resourcepacks.dialogs(ctx, self.busy || self.running);
        if let Some(path) = self.screenshots.dialogs(ctx, self.busy) {
            match appearance::Wallpaper::load(&path) {
                Ok(wallpaper) => {
                    let mut candidate = self.config.clone();
                    candidate.wallpaper = path.to_string_lossy().into();
                    match candidate.save() {
                        Ok(()) => {
                            if let Some(old) = self.wallpaper.take() {
                                ctx.forget_image(&old.uri);
                            }
                            self.wallpaper = Some(wallpaper);
                            self.config = candidate;
                        }
                        Err(error) => self.screenshots.error = error.to_string(),
                    }
                }
                Err(error) => self.screenshots.error = error.to_string(),
            }
        }
        if self.page == Page::Home
            && !self.busy
            && !self.version_popup
            && self.instance_operation.is_none()
            && !self.screenshots.modal_open()
            && !self.resourcepacks.modal_open()
            && !operation_open
            && !ctx.wants_keyboard_input()
            && ctx.input(|i| i.key_pressed(egui::Key::Enter))
        {
            if self.config.selected.is_empty() {
                self.page = Page::Download;
            } else {
                self.launch();
            }
        }
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
