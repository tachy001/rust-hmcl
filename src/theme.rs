//! HMCL-style Material controls, using the original HMCL SVG path assets.
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Response, Sense, Vec2, Widget, WidgetText};
pub const PRIMARY: Color32 = Color32::from_rgb(67, 82, 165);
pub const SECONDARY: Color32 = Color32::from_rgb(87, 92, 127);
pub const BACKGROUND: Color32 = Color32::from_rgb(245, 246, 251);
pub const LOGO: &[u8] = include_bytes!("../assets/hmcl/logo.png");
pub const TITLE_ICON: &[u8] = include_bytes!("../assets/hmcl/title.png");
pub const GRASS: &[u8] = include_bytes!("../assets/hmcl/grass.png");
pub const WALLPAPER: &[u8] = include_bytes!("../assets/hmcl/background.jpg");

pub fn accent(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(171, 184, 255)
    } else {
        PRIMARY
    }
}
pub fn surface(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(35, 39, 54)
    } else {
        Color32::WHITE
    }
}
pub fn border(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(55, 61, 79)
    } else {
        Color32::from_rgb(228, 232, 243)
    }
}
pub fn container(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(49, 61, 113)
    } else {
        Color32::from_rgb(222, 225, 255)
    }
}
pub fn on_container(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(222, 225, 255)
    } else {
        Color32::from_rgb(17, 28, 77)
    }
}
pub fn launch_control(
    ui: &mut egui::Ui,
    rect: Rect,
    title: &str,
    version: &str,
    expanded: f32,
    enabled: bool,
) -> (Response, Response) {
    let main = Rect::from_min_max(rect.min, Pos2::new(rect.right() - 30., rect.bottom()));
    let arrow = Rect::from_min_max(Pos2::new(main.right(), rect.top()), rect.max);
    let start = ui.interact(main, ui.id().with("launch-game"), Sense::click());
    let menu = ui.interact(arrow, ui.id().with("launch-menu"), Sense::click());
    let bg = container(ui);
    let fg = on_container(ui);
    ui.painter().add(
        egui::Shadow {
            offset: [0, 2],
            blur: 10,
            spread: 0,
            color: Color32::from_black_alpha(32),
        }
        .as_shape(rect, egui::CornerRadius::same(4)),
    );
    ui.painter().rect_filled(rect, 4., bg);
    for response in [&start, &menu] {
        let t = ui.ctx().animate_bool_with_time(
            response.id.with("hover"),
            enabled && response.hovered(),
            motion(ui, 0.16),
        );
        if t > 0. {
            ui.painter()
                .rect_filled(response.rect.shrink(1.), 3., fg.gamma_multiply(t * 0.09));
        }
        if enabled {
            ripple(ui, response, fg);
        }
        if response.has_focus() {
            ui.painter().rect_stroke(
                response.rect.shrink(2.),
                3.,
                egui::Stroke::new(1.5_f32, fg),
                egui::StrokeKind::Inside,
            );
        }
    }
    ui.painter().line_segment(
        [
            Pos2::new(main.right(), rect.top() + 12.),
            Pos2::new(main.right(), rect.bottom() - 12.),
        ],
        egui::Stroke::new(1_f32, fg.gamma_multiply(0.35)),
    );
    ui.painter().text(
        main.center() + Vec2::new(0., if version.is_empty() { 0. } else { -9. }),
        egui::Align2::CENTER_CENTER,
        title,
        FontId::proportional(16.),
        fg,
    );
    if !version.is_empty() {
        let text = elided(ui, version, 12., fg, main.width() - 16.);
        ui.painter().galley(
            Pos2::new(main.center().x - text.size().x / 2., main.center().y + 3.),
            text,
            fg,
        );
    }
    egui::Image::from_bytes(
        "bytes://hmcl/arrow_drop_up.svg",
        include_bytes!("../assets/hmcl/icons/arrow_drop_up.svg"),
    )
    .tint(fg)
    .rotate(-expanded * std::f32::consts::PI, Vec2::splat(0.5))
    .paint_at(ui, Rect::from_center_size(arrow.center(), Vec2::splat(26.)));
    start.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, title));
    menu.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, "切换游戏版本")
    });
    (
        start
            .on_hover_text("启动游戏；右键或滚轮切换版本")
            .on_hover_cursor(egui::CursorIcon::PointingHand),
        menu.on_hover_text("切换游戏版本")
            .on_hover_cursor(egui::CursorIcon::PointingHand),
    )
}
pub fn card(ui: &mut egui::Ui, draw: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new()
        .fill(surface(ui))
        .stroke(egui::Stroke::new(1_f32, border(ui)))
        .corner_radius(4.)
        .inner_margin(16.)
        .shadow(egui::Shadow {
            offset: [0, 3],
            blur: 14,
            spread: 0,
            color: Color32::from_black_alpha(if ui.visuals().dark_mode { 18 } else { 8 }),
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            draw(ui);
        });
}
fn elided(
    ui: &egui::Ui,
    text: &str,
    size: f32,
    color: Color32,
    width: f32,
) -> std::sync::Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple(
        text.to_owned(),
        FontId::proportional(size),
        color,
        width.max(1.),
    );
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    ui.painter().layout_job(job)
}
pub fn text_line(ui: &egui::Ui, text: &str, rect: Rect, size: f32, color: Color32) {
    let galley = elided(ui, text, size, color, rect.width());
    ui.painter().with_clip_rect(rect).galley(
        Pos2::new(rect.left(), rect.center().y - galley.size().y / 2.),
        galley,
        color,
    );
}
pub fn badge(ui: &mut egui::Ui, text: &str, positive: bool) {
    let color = if positive {
        if ui.visuals().dark_mode {
            Color32::from_rgb(124, 219, 176)
        } else {
            Color32::from_rgb(35, 126, 86)
        }
    } else {
        accent(ui)
    };
    egui::Frame::new()
        .fill(color.gamma_multiply(0.09))
        .corner_radius(5.)
        .inner_margin(egui::Margin::symmetric(9, 4))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(11.).color(color));
        });
}
pub fn section(ui: &mut egui::Ui, title: &str, description: &str) {
    ui.label(egui::RichText::new(title).size(18.).strong());
    ui.label(
        egui::RichText::new(description)
            .size(13.)
            .color(if ui.visuals().dark_mode {
                ui.visuals().weak_text_color()
            } else {
                SECONDARY
            }),
    );
    ui.add_space(4.);
}
pub fn empty(ui: &mut egui::Ui, name: &str, title: &str, description: &str) {
    card(ui, |ui| {
        ui.add_space(12.);
        ui.vertical_centered(|ui| {
            let (_, rect) = ui.allocate_space(Vec2::splat(52.));
            ui.painter()
                .circle_filled(rect.center(), 26., accent(ui).gamma_multiply(0.09));
            icon(ui, name, rect.shrink(12.), accent(ui));
            ui.add_space(10.);
            ui.label(egui::RichText::new(title).size(17.).strong());
            ui.label(
                egui::RichText::new(description)
                    .size(13.)
                    .color(ui.visuals().weak_text_color()),
            );
        });
        ui.add_space(14.);
    });
}
pub fn wallpaper(ui: &egui::Ui, rect: Rect, custom: Option<&crate::appearance::Wallpaper>) {
    static SIZE: std::sync::OnceLock<(u32, u32)> = std::sync::OnceLock::new();
    let dimensions = custom
        .map(|wallpaper| wallpaper.dimensions)
        .unwrap_or_else(|| {
            *SIZE.get_or_init(|| {
                image::ImageReader::new(std::io::Cursor::new(WALLPAPER))
                    .with_guessed_format()
                    .ok()
                    .and_then(|reader| reader.into_dimensions().ok())
                    .unwrap_or((1920, 1080))
            })
        });
    let uv = crate::appearance::crop(dimensions, rect.size());
    if let Some(wallpaper) = custom {
        egui::Image::from_bytes(wallpaper.uri.clone(), wallpaper.bytes.clone())
            .uv(uv)
            .paint_at(ui, rect);
    } else {
        egui::Image::from_bytes("bytes://hmcl/background.jpg", WALLPAPER)
            .uv(uv)
            .paint_at(ui, rect);
    }
    let top = if ui.visuals().dark_mode {
        Color32::from_rgba_unmultiplied(16, 23, 46, 70)
    } else {
        Color32::from_rgba_unmultiplied(20, 29, 66, 14)
    };
    let bottom =
        Color32::from_rgba_unmultiplied(12, 18, 38, if ui.visuals().dark_mode { 190 } else { 110 });
    let mut mesh = egui::Mesh::default();
    for (pos, color) in [
        (rect.left_top(), top),
        (rect.right_top(), top),
        (rect.right_bottom(), bottom),
        (rect.left_bottom(), bottom),
    ] {
        mesh.vertices.push(egui::epaint::Vertex {
            pos,
            uv: egui::epaint::WHITE_UV,
            color,
        });
    }
    mesh.indices.extend([0, 1, 2, 0, 2, 3]);
    ui.painter().add(egui::Shape::mesh(mesh));
}

pub fn setup(ctx: &egui::Context, dark: bool) {
    egui_extras::install_image_loaders(ctx);
    let mut visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = if dark {
        Color32::from_rgb(26, 28, 38)
    } else {
        BACKGROUND
    };
    visuals.selection.bg_fill = PRIMARY;
    visuals.override_text_color = Some(if dark {
        Color32::from_rgb(229, 225, 234)
    } else {
        Color32::from_rgb(29, 27, 32)
    });
    visuals.selection.stroke = egui::Stroke::new(1_f32, Color32::WHITE);
    for w in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        w.corner_radius = egui::CornerRadius::same(8);
        w.bg_stroke = egui::Stroke::new(
            1_f32,
            if dark {
                Color32::from_rgb(65, 68, 87)
            } else {
                Color32::from_rgb(216, 216, 229)
            },
        );
    }
    visuals.widgets.active.bg_fill = PRIMARY;
    visuals.widgets.hovered.bg_fill = if dark {
        Color32::from_rgb(53, 57, 80)
    } else {
        Color32::from_rgb(233, 235, 249)
    };
    visuals.extreme_bg_color = if dark {
        Color32::from_rgb(33, 35, 47)
    } else {
        Color32::WHITE
    };
    ctx.set_visuals(visuals);
    ctx.style_mut(|style| {
        style.animation_time = 0.18;
        style.spacing.item_spacing = Vec2::new(12., 14.);
        style.spacing.button_padding = Vec2::new(16., 10.);
        style.spacing.interact_size = Vec2::new(42., 36.);
        style.spacing.combo_height = 300.;
        style.spacing.slider_width = 260.;
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(14.));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(14.));
        style
            .text_styles
            .insert(egui::TextStyle::Heading, FontId::proportional(26.));
        style
            .text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(12.));
    });
}
pub fn icon(ui: &egui::Ui, name: &str, rect: Rect, color: Color32) {
    if name == "grass" {
        png(ui, "grass", GRASS, rect);
        return;
    }
    if name == "fabric" {
        png(
            ui,
            "fabric",
            include_bytes!("../assets/hmcl/fabric.png"),
            rect,
        );
        return;
    }
    if name == "quilt" {
        png(
            ui,
            "quilt",
            include_bytes!("../assets/hmcl/quilt.png"),
            rect,
        );
        return;
    }
    let bytes: &'static [u8] = match name {
        "forge" => include_bytes!("../assets/aster/forge.svg"),
        "home" => include_bytes!("../assets/hmcl/icons/home.svg"),
        "person" => include_bytes!("../assets/hmcl/icons/person.svg"),
        "game" => include_bytes!("../assets/hmcl/icons/stadia_controller.svg"),
        "extension" => include_bytes!("../assets/hmcl/icons/extension.svg"),
        "package" => include_bytes!("../assets/hmcl/icons/package.svg"),
        "download" => include_bytes!("../assets/hmcl/icons/download.svg"),
        "java" => include_bytes!("../assets/hmcl/icons/local_cafe.svg"),
        "settings" => include_bytes!("../assets/hmcl/icons/settings.svg"),
        "list" => include_bytes!("../assets/hmcl/icons/format_list_bulleted.svg"),
        "more" => include_bytes!("../assets/hmcl/icons/more_vert.svg"),
        "favorite" => include_bytes!("../assets/hmcl/icons/keep.svg"),
        "back" => include_bytes!("../assets/hmcl/icons/arrow_back.svg"),
        "component" => include_bytes!("../assets/hmcl/icons/deployed_code.svg"),
        "texture" => include_bytes!("../assets/hmcl/icons/texture.svg"),
        "world" => include_bytes!("../assets/hmcl/icons/public.svg"),
        "schema" => include_bytes!("../assets/hmcl/icons/schema.svg"),
        "menu" => include_bytes!("../assets/hmcl/icons/menu.svg"),
        "edit" => include_bytes!("../assets/hmcl/icons/edit.svg"),
        "duplicate" => include_bytes!("../assets/hmcl/icons/folder_copy.svg"),
        "logs" => include_bytes!("../assets/hmcl/icons/output.svg"),
        "info" => include_bytes!("../assets/hmcl/icons/info.svg"),
        "folder" => include_bytes!("../assets/hmcl/icons/folder_open.svg"),
        "refresh" => include_bytes!("../assets/hmcl/icons/refresh.svg"),
        "add" => include_bytes!("../assets/hmcl/icons/add.svg"),
        "search" => include_bytes!("../assets/hmcl/icons/search.svg"),
        "copy" => include_bytes!("../assets/hmcl/icons/content_copy.svg"),
        "delete" => include_bytes!("../assets/hmcl/icons/delete_forever.svg"),
        "close" => include_bytes!("../assets/hmcl/icons/close.svg"),
        "launch" => include_bytes!("../assets/hmcl/icons/rocket_launch.svg"),
        "check" => include_bytes!("../assets/hmcl/icons/check.svg"),
        "arrow" => include_bytes!("../assets/hmcl/icons/arrow_drop_up.svg"),
        "sun" => include_bytes!("../assets/hmcl/icons/wb_sunny.svg"),
        "maximize" => include_bytes!("../assets/hmcl/icons/screenshot_monitor.svg"),
        "minimize" => include_bytes!("../assets/hmcl/icons/minimize_center.svg"),
        _ => include_bytes!("../assets/hmcl/icons/tune.svg"),
    };
    let rotation = if name == "arrow" {
        ui.ctx()
            .data(|data| data.get_temp::<f32>(egui::Id::new("version-popup-rotation")))
            .unwrap_or(0.)
            * std::f32::consts::PI
    } else {
        0.
    };
    egui::Image::from_bytes(format!("bytes://hmcl/{name}.svg"), bytes)
        .rotate(rotation, egui::Vec2::splat(0.5))
        .tint(color)
        .paint_at(ui, rect);
}
pub fn png(ui: &egui::Ui, name: &str, bytes: &'static [u8], rect: Rect) {
    egui::Image::from_bytes(format!("bytes://hmcl/{name}.png"), bytes).paint_at(ui, rect);
}
fn blend(a: Color32, b: Color32, t: f32) -> Color32 {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t.clamp(0., 1.)) as u8;
    Color32::from_rgba_unmultiplied(
        f(a.r(), b.r()),
        f(a.g(), b.g()),
        f(a.b(), b.b()),
        f(a.a(), b.a()),
    )
}
fn ripple(ui: &egui::Ui, response: &Response, color: Color32) {
    if ui.style().animation_time == 0. {
        return;
    }
    let key = response.id.with("hmcl-ripple");
    let now = ui.input(|i| i.time);
    if response.clicked() {
        let point = response
            .interact_pointer_pos()
            .unwrap_or(response.rect.center());
        ui.ctx().data_mut(|d| d.insert_temp(key, (now, point)));
    }
    if let Some((started, origin)) = ui.ctx().data(|d| d.get_temp::<(f64, Pos2)>(key)) {
        let t = ((now - started) / 0.42) as f32;
        if t < 1. {
            let painter = ui.painter().with_clip_rect(response.rect.shrink(1.));
            let radius =
                response.rect.width().max(response.rect.height()) * (1. - (1. - t).powi(3));
            painter.circle_filled(origin, radius, color.gamma_multiply((1. - t) * 0.15));
            ui.ctx().request_repaint();
        }
    }
}
pub struct MaterialButton {
    text: String,
    fill: Option<Color32>,
    minimum: Vec2,
    icon: Option<&'static str>,
    subtitle: Option<String>,
    foreground: Option<Color32>,
}
impl MaterialButton {
    pub fn new(text: impl Into<WidgetText>) -> Self {
        let text = text.into().text().to_owned();
        let icon = infer_icon(&text);
        Self {
            text,
            fill: None,
            minimum: Vec2::ZERO,
            icon,
            subtitle: None,
            foreground: None,
        }
    }
    pub fn fill(mut self, color: Color32) -> Self {
        self.fill = Some(color);
        self
    }
    pub fn foreground(mut self, color: Color32) -> Self {
        self.foreground = Some(color);
        self
    }
    pub fn min_size(mut self, size: Vec2) -> Self {
        self.minimum = size;
        self
    }
    pub fn icon(mut self, icon: &'static str) -> Self {
        self.icon = Some(icon);
        self
    }
    pub fn subtitle(mut self, text: impl Into<String>) -> Self {
        self.subtitle = Some(text.into());
        self
    }
}
impl Widget for MaterialButton {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let label = self.text.clone();
        let foreground = if !ui.is_visually_enabled() {
            ui.visuals().weak_text_color()
        } else if let Some(color) = self.foreground {
            color
        } else if self.fill.is_some() {
            Color32::WHITE
        } else {
            ui.visuals().text_color()
        };
        let galley = ui
            .painter()
            .layout_no_wrap(self.text, FontId::proportional(14.), foreground);
        let subtitle = self.subtitle.map(|s| {
            elided(
                ui,
                &s,
                11.,
                foreground.gamma_multiply(0.85),
                (self.minimum.x - 50.).max(120.),
            )
        });
        let icon_space = if self.icon.is_some() { 28. } else { 0. };
        let size = Vec2::new(
            if galley.size().x == 0. {
                self.minimum.x.max(38.)
            } else {
                (galley.size().x + 32. + icon_space).max(self.minimum.x)
            },
            (galley.size().y + 18.)
                .max(if subtitle.is_some() { 58. } else { 38. })
                .max(self.minimum.y),
        );
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &label)
        });
        let hover = ui.ctx().animate_bool_with_time(
            response.id.with("hover"),
            response.hovered(),
            motion(ui, 0.16),
        );
        let press = ui.ctx().animate_bool_with_time(
            response.id.with("press"),
            response.is_pointer_button_down_on(),
            motion(ui, 0.08),
        );
        let background = self.fill.unwrap_or(if ui.visuals().dark_mode {
            Color32::from_rgb(42, 45, 61)
        } else {
            Color32::from_rgb(239, 240, 249)
        });
        let background = blend(
            background,
            if self.fill.is_some_and(|fill| fill.a() == 0) {
                foreground.gamma_multiply(0.1)
            } else if self.fill.is_some() {
                Color32::from_rgb(86, 101, 183)
            } else {
                if ui.visuals().dark_mode {
                    Color32::from_rgb(57, 64, 89)
                } else {
                    Color32::from_rgb(225, 229, 249)
                }
            },
            hover,
        );
        let rect = rect.shrink(press * 0.9);
        let background = if ui.is_visually_enabled() {
            background
        } else {
            blend(background, surface(ui), 0.6)
        };
        if self.fill.is_some_and(|fill| fill.a() > 0) && rect.height() > 48. {
            ui.painter().add(
                egui::Shadow {
                    offset: [0, 3],
                    blur: 12,
                    spread: 0,
                    color: Color32::from_black_alpha(24),
                }
                .as_shape(rect, egui::CornerRadius::same(10)),
            );
        }
        ui.painter().rect_filled(rect, 4., background);
        if response.has_focus() {
            ui.painter().rect_stroke(
                rect,
                4.,
                egui::Stroke::new(2_f32, accent(ui)),
                egui::StrokeKind::Inside,
            );
        }
        ripple(ui, &response, foreground);
        let x = rect.center().x - (galley.size().x + icon_space) / 2.;
        if let Some(icon_name) = self.icon {
            icon(
                ui,
                icon_name,
                Rect::from_center_size(
                    if galley.size().x == 0. {
                        rect.center()
                    } else {
                        Pos2::new(x + 10., rect.center().y)
                    },
                    Vec2::splat(20.),
                ),
                foreground,
            );
        }
        let y = rect.center().y - galley.size().y / 2. - if subtitle.is_some() { 8. } else { 0. };
        ui.painter()
            .galley(Pos2::new(x + icon_space, y), galley, foreground);
        if let Some(subtitle) = subtitle {
            ui.painter().galley(
                Pos2::new(
                    rect.center().x - subtitle.size().x / 2. + icon_space / 2.,
                    rect.center().y + 6.,
                ),
                subtitle,
                foreground,
            );
        }
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }
}
fn infer_icon(text: &str) -> Option<&'static str> {
    if text.contains("重命名") {
        Some("edit")
    } else if text.contains("复制实例") {
        Some("duplicate")
    } else if text.contains("收藏") {
        Some("favorite")
    } else if text.contains("设置") {
        Some("settings")
    } else if text.contains("Fabric") {
        Some("fabric")
    } else if text.contains("Quilt") {
        Some("quilt")
    } else if text.contains("启动") {
        Some("launch")
    } else if text.contains("搜索") {
        Some("search")
    } else if text.contains("刷新") || text.contains("修复") {
        Some("refresh")
    } else if text.contains("下载") || text.contains("安装") || text.contains("获取") {
        Some("download")
    } else if text.contains("目录") || text.contains("浏览") || text.contains("打开") {
        Some("folder")
    } else if text.contains("添加") || text.contains("导入") {
        Some("add")
    } else if text.contains("保存") {
        Some("check")
    } else if text.contains("复制") {
        Some("copy")
    } else if text.contains("取消")
        || text.contains("关闭")
        || text.contains("移除")
        || text.contains("结束")
    {
        Some("close")
    } else {
        None
    }
}
pub fn button(ui: &mut egui::Ui, text: impl Into<WidgetText>) -> Response {
    ui.add(MaterialButton::new(text))
}
pub struct GameRow {
    pub select: Response,
    pub settings: Response,
    pub launch: Response,
    pub more: Response,
}
pub fn game_row(
    ui: &mut egui::Ui,
    id: &str,
    subtitle: &str,
    icon_name: &str,
    selected: bool,
    favorite: bool,
) -> GameRow {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 64.), Sense::hover());
    let select_rect = Rect::from_center_size(
        Pos2::new(rect.left() + 18., rect.center().y),
        Vec2::splat(32.),
    );
    let select = ui.interact(select_rect, ui.id().with((id, "select")), Sense::click());
    let more_rect = Rect::from_center_size(
        Pos2::new(rect.right() - 22., rect.center().y),
        Vec2::splat(36.),
    );
    let launch_rect = more_rect.translate(Vec2::new(-40., 0.));
    let more = ui.interact(more_rect, ui.id().with((id, "more")), Sense::click());
    let launch = ui.interact(launch_rect, ui.id().with((id, "launch")), Sense::click());
    let body = Rect::from_min_max(
        Pos2::new(rect.left() + 38., rect.top()),
        Pos2::new(launch_rect.left() - 4., rect.bottom()),
    );
    let settings = ui.interact(body, ui.id().with((id, "settings")), Sense::click());
    let hover = ui.ctx().animate_bool_with_time(
        settings.id.with("hover"),
        settings.hovered(),
        motion(ui, 0.16),
    );
    ui.painter().rect_filled(
        rect,
        4.,
        blend(
            surface(ui),
            container(ui),
            if selected { 0.32 } else { hover * 0.18 },
        ),
    );
    let fg = accent(ui);
    ui.painter().circle_stroke(
        select_rect.center(),
        8.,
        egui::Stroke::new(
            2_f32,
            if selected {
                fg
            } else {
                ui.visuals().weak_text_color()
            },
        ),
    );
    let selection =
        ui.ctx()
            .animate_bool_with_time(select.id.with("selected"), selected, motion(ui, 0.18));
    if selection > 0. {
        ui.painter()
            .circle_filled(select_rect.center(), 4. * selection, fg);
    }
    icon(
        ui,
        icon_name,
        Rect::from_center_size(
            Pos2::new(body.left() + 20., body.center().y),
            Vec2::splat(32.),
        ),
        fg,
    );
    let x = body.left() + 44.;
    let width = (body.right() - x - if favorite { 26. } else { 0. }).max(1.);
    let title = elided(ui, id, 14., ui.visuals().text_color(), width);
    ui.painter().galley(
        Pos2::new(x, rect.center().y - 19.),
        title,
        ui.visuals().text_color(),
    );
    let sub = elided(ui, subtitle, 11., ui.visuals().weak_text_color(), width);
    ui.painter().galley(
        Pos2::new(x, rect.center().y + 4.),
        sub,
        ui.visuals().weak_text_color(),
    );
    if favorite {
        icon(
            ui,
            "favorite",
            Rect::from_center_size(
                Pos2::new(body.right() - 14., body.center().y),
                Vec2::splat(18.),
            ),
            fg,
        );
    }
    for (response, name, label) in [(&launch, "launch", "启动游戏"), (&more, "more", "游戏管理")]
    {
        let t = ui.ctx().animate_bool_with_time(
            response.id.with("hover"),
            response.hovered(),
            motion(ui, 0.16),
        );
        ui.painter()
            .rect_filled(response.rect, 4., fg.gamma_multiply(0.1 * t));
        icon(ui, name, response.rect.shrink(8.), fg);
        ripple(ui, response, fg);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
        });
    }
    select.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::RadioButton, ui.is_enabled(), selected, id)
    });
    settings.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::Button,
            ui.is_enabled(),
            format!("版本设置 {id}"),
        )
    });
    ripple(ui, &settings, fg);
    GameRow {
        select: select.on_hover_text("选择此游戏版本"),
        settings: settings
            .on_hover_text(id)
            .on_hover_cursor(egui::CursorIcon::PointingHand),
        launch: launch
            .on_hover_text("启动游戏")
            .on_hover_cursor(egui::CursorIcon::PointingHand),
        more: more
            .on_hover_text("游戏管理")
            .on_hover_cursor(egui::CursorIcon::PointingHand),
    }
}
pub fn text_field(ui: &mut egui::Ui, value: &mut String) -> Response {
    let width = ui.available_width().min(560.);
    text_field_sized(ui, value, width, "")
}
pub fn java_row(
    ui: &mut egui::Ui,
    runtime: &crate::java_runtime::Runtime,
    selected: bool,
) -> (Response, Response, Response) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.), Sense::hover());
    let hide_rect = Rect::from_center_size(
        Pos2::new(rect.right() - 22., rect.center().y),
        Vec2::splat(36.),
    );
    let folder_rect = hide_rect.translate(Vec2::new(-40., 0.));
    let body = Rect::from_min_max(rect.min, Pos2::new(folder_rect.left() - 4., rect.bottom()));
    let select = ui.interact(body, ui.id().with("select-java"), Sense::click());
    let folder = ui.interact(folder_rect, ui.id().with("reveal-java"), Sense::click());
    let hide = ui.interact(hide_rect, ui.id().with("hide-java"), Sense::click());
    ui.painter()
        .rect_filled(rect, 4., if selected { container(ui) } else { surface(ui) });
    let number = Rect::from_center_size(
        Pos2::new(rect.left() + 24., rect.center().y),
        Vec2::splat(32.),
    );
    ui.painter().rect_filled(number, 2., container(ui));
    ui.painter().text(
        number.center(),
        egui::Align2::CENTER_CENTER,
        runtime.major.to_string(),
        FontId::proportional(16.),
        on_container(ui),
    );
    let title = format!(
        "{} {} {} · {}",
        runtime.vendor,
        if runtime.is_jdk { "JDK" } else { "JRE" },
        runtime.version,
        runtime.architecture
    );
    text_line(
        ui,
        &title,
        Rect::from_min_max(
            Pos2::new(rect.left() + 48., rect.top() + 8.),
            Pos2::new(body.right(), rect.center().y),
        ),
        14.,
        ui.visuals().text_color(),
    );
    text_line(
        ui,
        &runtime.path.to_string_lossy(),
        Rect::from_min_max(
            Pos2::new(rect.left() + 48., rect.center().y),
            Pos2::new(body.right(), rect.bottom() - 8.),
        ),
        11.,
        ui.visuals().weak_text_color(),
    );
    for (response, name) in [(&folder, "folder"), (&hide, "close")] {
        let t = ui.ctx().animate_bool_with_time(
            response.id.with("hover"),
            response.hovered(),
            motion(ui, 0.16),
        );
        ui.painter()
            .rect_filled(response.rect, 4., accent(ui).gamma_multiply(t * 0.1));
        icon(ui, name, response.rect.shrink(8.), accent(ui));
        ripple(ui, response, accent(ui));
    }
    ripple(ui, &select, accent(ui));
    (
        select
            .on_hover_text(format!("使用此环境\n{}", runtime.path.display()))
            .on_hover_cursor(egui::CursorIcon::PointingHand),
        folder.on_hover_text("打开 Java 目录"),
        hide.on_hover_text("隐藏此环境，保留文件"),
    )
}
pub fn text_field_sized(ui: &mut egui::Ui, value: &mut String, width: f32, hint: &str) -> Response {
    let background = if ui.visuals().dark_mode {
        Color32::from_rgb(43, 46, 63)
    } else {
        Color32::from_rgb(247, 248, 253)
    };
    ui.add(
        egui::TextEdit::singleline(value)
            .desired_width(width.max(80.))
            .hint_text(hint)
            .margin(Vec2::new(12., 10.))
            .font(FontId::proportional(14.))
            .background_color(background),
    )
}
pub fn nav(
    ui: &mut egui::Ui,
    key: &str,
    name: &str,
    subtitle: Option<&str>,
    selected: bool,
) -> Response {
    let height = if subtitle.is_some() { 56. } else { 42. };
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
    let t =
        ui.ctx()
            .animate_bool_with_time(response.id.with("selected"), selected, motion(ui, 0.18));
    let hover = ui.ctx().animate_bool_with_time(
        response.id.with("hover"),
        response.hovered(),
        motion(ui, 0.16),
    );
    let background = if ui.visuals().dark_mode {
        Color32::from_rgb(61, 64, 98)
    } else {
        Color32::from_rgb(221, 225, 247)
    };
    if t + hover > 0. {
        ui.painter().rect_filled(
            rect,
            0.,
            background.gamma_multiply((t * 0.85 + hover * 0.35).min(1.)),
        );
    }
    let fg = blend(ui.visuals().text_color(), accent(ui), t);
    if key == "person" && subtitle.is_some() {
        let face = Rect::from_center_size(
            Pos2::new(rect.left() + 28., rect.center().y),
            Vec2::splat(32.),
        );
        let (skin, bytes) = crate::avatars::skin(name);
        for (uv, target) in [
            (
                Rect::from_min_max(
                    Pos2::new(8. / 64., 8. / 64.),
                    Pos2::new(16. / 64., 16. / 64.),
                ),
                face.shrink(2.),
            ),
            (
                Rect::from_min_max(
                    Pos2::new(40. / 64., 8. / 64.),
                    Pos2::new(48. / 64., 16. / 64.),
                ),
                face,
            ),
        ] {
            egui::Image::from_bytes(format!("bytes://hmcl/skin/{skin}.png"), bytes)
                .texture_options(egui::TextureOptions::NEAREST)
                .uv(uv)
                .paint_at(ui, target);
        }
    } else {
        icon(
            ui,
            key,
            Rect::from_center_size(
                Pos2::new(rect.left() + 28., rect.center().y),
                Vec2::splat(24.),
            ),
            fg,
        );
    }
    let y = if subtitle.is_some() {
        rect.center().y - 9.
    } else {
        rect.center().y
    };
    let title = elided(ui, name, 13., fg, rect.width() - 70.);
    ui.painter().galley(
        Pos2::new(rect.left() + 54., y - title.size().y / 2.),
        title,
        fg,
    );
    if let Some(subtitle) = subtitle {
        let text = elided(
            ui,
            subtitle,
            10.,
            ui.visuals().weak_text_color(),
            rect.width() - 70.,
        );
        ui.painter().galley(
            Pos2::new(rect.left() + 54., rect.center().y + 4.),
            text,
            ui.visuals().weak_text_color(),
        );
    }
    ripple(ui, &response, PRIMARY);
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            ui.is_enabled(),
            selected,
            name,
        )
    });
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.),
            0.,
            egui::Stroke::new(1_f32, accent(ui)),
            egui::StrokeKind::Inside,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
pub fn checkbox(ui: &mut egui::Ui, value: &mut bool, label: &str) -> Response {
    ui.scope(|ui| {
        let color = accent(ui);
        let outline = border(ui);
        let checked = *value;
        let visuals = ui.visuals_mut();
        for widget in [
            &mut visuals.widgets.inactive,
            &mut visuals.widgets.hovered,
            &mut visuals.widgets.active,
            &mut visuals.widgets.noninteractive,
        ] {
            widget.corner_radius = egui::CornerRadius::same(2);
            widget.bg_fill = if checked { color } else { Color32::TRANSPARENT };
            widget.bg_stroke = egui::Stroke::new(2_f32, if checked { color } else { outline });
            if checked {
                widget.fg_stroke = egui::Stroke::new(2_f32, Color32::WHITE);
            }
        }
        ui.checkbox(value, label)
    })
    .inner
}
pub fn toggle(ui: &mut egui::Ui, value: &mut bool, label: &str) -> Response {
    let text = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(14.),
        ui.visuals().text_color(),
    );
    let (rect, mut response) =
        ui.allocate_exact_size(Vec2::new(text.size().x + 58., 30.), Sense::click());
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), *value, label)
    });
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id, *value, motion(ui, 0.17));
    let track = Rect::from_center_size(
        Pos2::new(rect.left() + 20., rect.center().y),
        Vec2::new(38., 22.),
    );
    ui.painter().rect_filled(
        track,
        11.,
        blend(Color32::from_rgb(170, 174, 190), PRIMARY, t),
    );
    ui.painter().circle_filled(
        Pos2::new(track.left() + 11. + 16. * t, track.center().y),
        8.,
        Color32::WHITE,
    );
    ui.painter().galley(
        Pos2::new(rect.left() + 52., rect.center().y - text.size().y / 2.),
        text,
        ui.visuals().text_color(),
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}
fn motion(ui: &egui::Ui, duration: f32) -> f32 {
    if ui.style().animation_time == 0. {
        0.
    } else {
        duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draw_row(ctx: &egui::Context, width: f32, events: Vec<egui::Event>) -> GameRow {
        let mut row = None;
        let _ = ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 200.))),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    row = Some(game_row(
                        ui,
                        "fabric-loader-0.19.5-1.20.1-with-a-very-long-instance-name",
                        "Minecraft 1.20.1 · Fabric 0.19.5",
                        "fabric",
                        true,
                        true,
                    ));
                });
            },
        );
        row.unwrap()
    }

    #[test]
    fn game_list_actions_stay_separate_at_narrow_widths() {
        for width in [320., 400., 600.] {
            let ctx = egui::Context::default();
            setup(&ctx, false);
            let _ = draw_row(&ctx, width, vec![]);
            let row = draw_row(&ctx, width, vec![]);
            assert!(row.settings.rect.is_positive());
            assert!(row.select.rect.right() <= row.settings.rect.left());
            assert!(row.settings.rect.right() <= row.launch.rect.left());
            assert!(row.launch.rect.right() <= row.more.rect.left());
            assert!(row.more.rect.right() <= width);
            let p = row.launch.rect.center();
            let clicked = draw_row(
                &ctx,
                width,
                vec![
                    egui::Event::PointerMoved(p),
                    egui::Event::PointerButton {
                        pos: p,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                    egui::Event::PointerButton {
                        pos: p,
                        button: egui::PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
            );
            assert!(clicked.launch.clicked());
            assert!(!clicked.settings.clicked());
            assert!(!clicked.select.clicked());
            assert!(!clicked.more.clicked());
        }
    }
}
