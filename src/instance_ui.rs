//! Page-style instance management, following HMCL GameInstancePage's tabs.
use crate::{config::Config, theme, Launcher, Page, VersionAction};
use eframe::egui::{self, RichText};

#[derive(Clone, Copy, PartialEq, Default)]
pub enum Tab {
    #[default]
    Settings,
    Components,
    Mods,
    Resources,
    Worlds,
    Schematics,
    Screenshots,
}
impl Tab {
    pub(super) fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "settings" => Self::Settings,
            "components" => Self::Components,
            "mods" => Self::Mods,
            "resources" => Self::Resources,
            "worlds" => Self::Worlds,
            "schematics" => Self::Schematics,
            "screenshots" => Self::Screenshots,
            _ => return None,
        })
    }
    fn title(self) -> &'static str {
        match self {
            Self::Settings => "游戏设置",
            Self::Components => "自动安装",
            Self::Mods => "模组管理",
            Self::Resources => "资源包管理",
            Self::Worlds => "世界管理",
            Self::Schematics => "原理图管理",
            Self::Screenshots => "截图管理",
        }
    }
}

impl Launcher {
    pub(super) fn instance_navigation(&mut self, ui: &mut egui::Ui) {
        for (tab, icon) in [
            (Tab::Settings, "settings"),
            (Tab::Components, "component"),
            (Tab::Mods, "extension"),
            (Tab::Resources, "texture"),
            (Tab::Worlds, "world"),
            (Tab::Schematics, "schema"),
        ] {
            if theme::nav(ui, icon, tab.title(), None, self.instance_tab == tab).clicked()
                && self.instance_tab != tab
            {
                self.instance_tab = tab;
                self.instance_tab_started = ui.input(|input| input.time);
                self.editing_error.clear();
            }
        }
        ui.add_space(16.);
        let id = self.editing_version.clone().unwrap_or_default();
        if theme::nav(ui, "launch", "测试游戏", None, false).clicked() {
            self.apply_version_action(&id, VersionAction::Launch);
        }
        if theme::nav(ui, "folder", "浏览", None, false).clicked() {
            self.apply_version_action(&id, VersionAction::Folder);
        }
        let response = theme::nav(ui, "menu", "管理", None, false);
        let popup_id = ui.make_persistent_id("instance-management-menu");
        if response.clicked() {
            ui.memory_mut(|memory| memory.toggle_popup(popup_id));
        }
        egui::popup::popup_below_widget(
            ui,
            popup_id,
            &response,
            egui::popup::PopupCloseBehavior::CloseOnClickOutside,
            |ui| {
                if theme::button(ui, "截图管理").clicked() {
                    self.instance_tab = Tab::Screenshots;
                    self.instance_tab_started = ui.input(|input| input.time);
                    ui.memory_mut(|memory| memory.close_popup());
                }
                if theme::button(ui, "校验 / 修复").clicked() {
                    self.apply_version_action(&id, VersionAction::Repair);
                    ui.memory_mut(|memory| memory.close_popup());
                }
                for (label, action) in [
                    ("重命名", VersionAction::Rename),
                    ("复制实例", VersionAction::Duplicate),
                ] {
                    if theme::button(ui, label).clicked() {
                        self.apply_version_action(&id, action);
                        ui.memory_mut(|memory| memory.close_popup());
                    }
                }
            },
        );
    }
    pub(super) fn instance_content(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self
            .editing_version
            .clone()
            .filter(|id| self.installed.contains(id))
        else {
            theme::empty(
                ui,
                "game",
                "实例不存在",
                "返回实例列表选择一个已安装的游戏。",
            );
            if theme::button(ui, "实例列表").clicked() {
                self.page = Page::Versions;
            }
            return;
        };
        let config = Config {
            selected: id.clone(),
            ..self.config.clone()
        }
        .for_selected();
        let elapsed = ui.input(|input| input.time) - self.instance_tab_started;
        let progress = if self.config.reduce_motion || self.instance_tab_started < 0. {
            1.
        } else {
            (elapsed / 0.4).clamp(0., 1.) as f32
        };
        let eased = 1. - (1. - progress).powi(3);
        if progress < 1. {
            ui.ctx().request_repaint();
        }
        ui.scope(|ui| {
            ui.set_opacity(eased);
            ui.add_space(20. * (1. - eased));
            match self.instance_tab {
                Tab::Settings => self.instance_settings(ui, &id),
                Tab::Components => self.instance_components(ui, &id),
                Tab::Mods => self.instance_mods(ui, &config),
                Tab::Resources => self.instance_files(ui, &config, "resourcepacks", false),
                Tab::Worlds => self.instance_files(ui, &config, "saves", true),
                Tab::Schematics => self.instance_files(ui, &config, "schematics", false),
                Tab::Screenshots => self.instance_files(ui, &config, "screenshots", false),
            }
        });
        if !self.editing_error.is_empty() {
            ui.colored_label(
                if self.config.dark_mode {
                    egui::Color32::from_rgb(255, 180, 171)
                } else {
                    egui::Color32::from_rgb(160, 40, 35)
                },
                &self.editing_error,
            );
        }
    }

    fn instance_settings(&mut self, ui: &mut egui::Ui, id: &str) {
        theme::card(ui, |ui| {
            theme::section(
                ui,
                "游戏设置",
                "未单独设置的选项继承全局设置，保存后用于下一次启动。",
            );
            let mut own = self.edit_settings.java.is_some();
            if theme::toggle(ui, &mut own, "单独指定 Java").changed() {
                self.edit_settings.java = own.then(|| self.config.java.clone());
            }
            if let Some(java) = &mut self.edit_settings.java {
                theme::text_field_sized(
                    ui,
                    java,
                    ui.available_width().min(560.),
                    "留空时自动选择适合此实例的 Java",
                );
                if theme::button(ui, "浏览 Java").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        *java = path.to_string_lossy().into();
                    }
                }
            } else {
                ui.label(
                    RichText::new("使用全局 Java 设置")
                        .small()
                        .color(ui.visuals().weak_text_color()),
                );
            }
            let mut own = self.edit_settings.memory.is_some();
            if theme::toggle(ui, &mut own, "单独设置内存").changed() {
                self.edit_settings.memory = own.then_some(self.config.memory);
            }
            if let Some(memory) = &mut self.edit_settings.memory {
                ui.add(
                    egui::Slider::new(memory, 512..=65536)
                        .step_by(256.)
                        .text("MB"),
                );
            } else {
                ui.label(format!("继承全局内存：{} MB", self.config.memory));
            }
            let mut own = self.edit_settings.isolated.is_some();
            if theme::toggle(ui, &mut own, "单独设置版本隔离").changed() {
                self.edit_settings.isolated = own.then_some(self.config.isolated);
            }
            if let Some(isolated) = &mut self.edit_settings.isolated {
                theme::toggle(ui, isolated, "使用此实例独立的游戏目录");
            }
            ui.label(
                RichText::new("切换隔离不会搬移已有存档；整合包始终使用独立目录。")
                    .small()
                    .color(ui.visuals().weak_text_color()),
            );
            let mut own = self.edit_settings.width.is_some() || self.edit_settings.height.is_some();
            if theme::toggle(ui, &mut own, "单独设置游戏窗口尺寸").changed() {
                self.edit_settings.width = own.then_some(self.config.width);
                self.edit_settings.height = own.then_some(self.config.height);
            }
            if own {
                ui.horizontal_wrapped(|ui| {
                    ui.label("宽度");
                    ui.add(
                        egui::DragValue::new(
                            self.edit_settings.width.get_or_insert(self.config.width),
                        )
                        .range(320..=7680),
                    );
                    ui.label("高度");
                    ui.add(
                        egui::DragValue::new(
                            self.edit_settings.height.get_or_insert(self.config.height),
                        )
                        .range(240..=4320),
                    );
                });
            }
            let mut own = self.edit_settings.auto_java.is_some();
            if theme::toggle(ui, &mut own, "单独设置自动下载 Java").changed() {
                self.edit_settings.auto_java = own.then_some(self.config.auto_java);
            }
            if let Some(auto) = &mut self.edit_settings.auto_java {
                theme::toggle(ui, auto, "缺少适合游戏的 Java 时自动下载");
            }
            let mut own = self.edit_settings.extra_jvm.is_some();
            if theme::toggle(ui, &mut own, "单独设置 JVM 参数").changed() {
                self.edit_settings.extra_jvm = own.then(|| self.config.extra_jvm.clone());
            }
            if let Some(args) = &mut self.edit_settings.extra_jvm {
                theme::text_field_sized(
                    ui,
                    args,
                    ui.available_width().min(560.),
                    "例如 -XX:+UseG1GC",
                );
            }
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(theme::MaterialButton::new("保存设置").fill(theme::PRIMARY))
                    .clicked()
                {
                    let mut candidate = self.config.clone();
                    candidate
                        .version_settings
                        .insert(id.into(), self.edit_settings.clone());
                    self.save_instance_candidate(candidate);
                }
                if theme::button(ui, "恢复全局设置").clicked() {
                    let mut candidate = self.config.clone();
                    candidate.version_settings.remove(id);
                    if self.save_instance_candidate(candidate) {
                        self.edit_settings = Default::default();
                    }
                }
            });
        });
    }
    fn save_instance_candidate(&mut self, mut candidate: Config) -> bool {
        candidate.normalize();
        match candidate.save() {
            Ok(()) => {
                self.config = candidate;
                self.editing_error.clear();
                self.status = "实例设置已保存".into();
                true
            }
            Err(error) => {
                self.editing_error = format!("{error:#}");
                false
            }
        }
    }
    fn instance_components(&mut self, ui: &mut egui::Ui, id: &str) {
        theme::card(ui, |ui| {
            theme::section(ui, "游戏组件", "为此实例安装加载器，或校验现有依赖。");
            if let Some(info) = self.descriptions.get(id) {
                ui.label(&info.subtitle);
            }
            theme::text_field_sized(
                ui,
                &mut self.loader_version,
                ui.available_width().min(400.),
                "加载器版本；留空安装最新稳定版",
            );
            ui.horizontal_wrapped(|ui| {
                for (label, action) in [
                    ("安装 Fabric", VersionAction::Fabric),
                    ("安装 Quilt", VersionAction::Quilt),
                    ("校验 / 修复", VersionAction::Repair),
                ] {
                    if theme::button(ui, label).clicked() {
                        self.apply_version_action(id, action);
                    }
                }
            });
            ui.label("Forge / NeoForge 安装尚未接入。");
        });
    }
    fn instance_mods(&mut self, ui: &mut egui::Ui, config: &Config) {
        let directory = match crate::mods::directory(config) {
            Ok(path) => path,
            Err(error) => {
                self.editing_error = error.to_string();
                return;
            }
        };
        if theme::button(ui, "在线搜索模组").clicked() {
            self.apply_version_action(&config.selected, VersionAction::Mods);
        }
        if theme::button(ui, "导入 .jar 模组").clicked() {
            if let Some(files) = rfd::FileDialog::new()
                .add_filter("Minecraft 模组", &["jar"])
                .pick_files()
            {
                if let Err(error) = crate::mods::import(&directory, &files) {
                    self.editing_error = error.to_string();
                }
            }
        }
        match crate::mods::list(&directory) {
            Ok(files) => {
                if files.is_empty() {
                    theme::empty(
                        ui,
                        "extension",
                        "没有模组",
                        "导入模组文件或在线搜索适合此实例的模组。",
                    );
                }
                for file in files {
                    ui.push_id(&file.name, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            let mut enabled = file.enabled;
                            if theme::toggle(ui, &mut enabled, &file.name).changed() {
                                if let Err(error) = crate::mods::toggle(&directory, &file.name) {
                                    self.editing_error = error.to_string();
                                }
                            }
                            ui.label(
                                RichText::new(format!("{} KB", file.size / 1024))
                                    .small()
                                    .color(ui.visuals().weak_text_color()),
                            );
                        });
                    });
                }
            }
            Err(error) => self.editing_error = error.to_string(),
        }
    }
    fn instance_files(
        &mut self,
        ui: &mut egui::Ui,
        config: &Config,
        sub: &str,
        directories_only: bool,
    ) {
        let directory = match crate::game::instance_dir(config) {
            Ok(path) => path.join(sub),
            Err(error) => {
                self.editing_error = error.to_string();
                return;
            }
        };
        ui.label(
            RichText::new(directory.to_string_lossy())
                .small()
                .color(ui.visuals().weak_text_color()),
        );
        if theme::button(ui, "打开目录").clicked() {
            let result = std::fs::create_dir_all(&directory)
                .map_err(anyhow::Error::from)
                .and_then(|()| open::that(&directory).map_err(anyhow::Error::from));
            if let Err(error) = result {
                self.editing_error = error.to_string();
            }
        }
        let mut files = vec![];
        if directory.exists() {
            match std::fs::read_dir(&directory) {
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            Ok(entry) => {
                                if !directories_only || entry.path().is_dir() {
                                    files.push(entry.path());
                                }
                            }
                            Err(error) => self.editing_error = error.to_string(),
                        }
                    }
                }
                Err(error) => self.editing_error = error.to_string(),
            }
        }
        files.sort();
        if files.is_empty() {
            theme::empty(ui, "folder", "没有内容", "游戏产生的本地内容会显示在这里。");
        }
        for file in files {
            ui.horizontal(|ui| {
                ui.add(
                    egui::Label::new(file.file_name().unwrap_or_default().to_string_lossy())
                        .truncate(),
                );
                if theme::button(ui, "打开").clicked() {
                    if let Err(error) = open::that(&file) {
                        self.editing_error = error.to_string();
                    }
                }
            });
        }
    }
}
