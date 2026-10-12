//! HMCL-style Material controls, using the original HMCL SVG path assets.
use eframe::egui::{self, Color32, FontId, Pos2, Rect, Response, Sense, Vec2, Widget, WidgetText};
pub const PRIMARY: Color32 = Color32::from_rgb(67, 82, 165);
pub const SECONDARY: Color32 = Color32::from_rgb(87, 92, 127);
pub const BACKGROUND: Color32 = Color32::from_rgb(247, 247, 252);
pub const LOGO: &[u8] = include_bytes!("../assets/hmcl/logo.png");
pub const GRASS: &[u8] = include_bytes!("../assets/hmcl/grass.png");
pub const WALLPAPER: &[u8] = include_bytes!("../assets/hmcl/background.jpg");

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
    visuals.selection.stroke = egui::Stroke::new(1_f32, Color32::WHITE);
    for w in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        w.corner_radius = egui::CornerRadius::same(5);
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
        style.spacing.item_spacing = Vec2::new(12., 12.);
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
            .insert(egui::TextStyle::Heading, FontId::proportional(22.));
        style
            .text_styles
            .insert(egui::TextStyle::Small, FontId::proportional(12.));
    });
}
pub fn icon(ui: &egui::Ui, name: &str, rect: Rect, color: Color32) {
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
        "home" => include_bytes!("../assets/hmcl/icons/home.svg"),
        "person" => include_bytes!("../assets/hmcl/icons/person.svg"),
        "game" => include_bytes!("../assets/hmcl/icons/stadia_controller.svg"),
        "extension" => include_bytes!("../assets/hmcl/icons/extension.svg"),
        "package" => include_bytes!("../assets/hmcl/icons/package.svg"),
        "download" => include_bytes!("../assets/hmcl/icons/download.svg"),
        "java" => include_bytes!("../assets/hmcl/icons/local_cafe.svg"),
        "settings" => include_bytes!("../assets/hmcl/icons/settings.svg"),
        "logs" => include_bytes!("../assets/hmcl/icons/output.svg"),
        "info" => include_bytes!("../assets/hmcl/icons/info.svg"),
        "folder" => include_bytes!("../assets/hmcl/icons/folder_open.svg"),
        "refresh" => include_bytes!("../assets/hmcl/icons/refresh.svg"),
        "add" => include_bytes!("../assets/hmcl/icons/add.svg"),
        "search" => include_bytes!("../assets/hmcl/icons/search.svg"),
        "copy" => include_bytes!("../assets/hmcl/icons/content_copy.svg"),
        "close" => include_bytes!("../assets/hmcl/icons/close.svg"),
        "launch" => include_bytes!("../assets/hmcl/icons/rocket_launch.svg"),
        "check" => include_bytes!("../assets/hmcl/icons/check.svg"),
        "arrow" => include_bytes!("../assets/hmcl/icons/arrow_drop_up.svg"),
        "sun" => include_bytes!("../assets/hmcl/icons/wb_sunny.svg"),
        "maximize" => include_bytes!("../assets/hmcl/icons/screenshot_monitor.svg"),
        "minimize" => include_bytes!("../assets/hmcl/icons/minimize_center.svg"),
        _ => include_bytes!("../assets/hmcl/icons/tune.svg"),
    };
    egui::Image::from_bytes(format!("bytes://hmcl/{name}.svg"), bytes)
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
        }
    }
    pub fn fill(mut self, color: Color32) -> Self {
        self.fill = Some(color);
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
        let foreground = if self.fill.is_some() {
            Color32::WHITE
        } else {
            ui.visuals().text_color()
        };
        let galley = ui
            .painter()
            .layout_no_wrap(self.text, FontId::proportional(14.), foreground);
        let subtitle = self.subtitle.map(|s| {
            ui.painter().layout_no_wrap(
                s,
                FontId::proportional(11.),
                foreground.gamma_multiply(0.85),
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
        let hover =
            ui.ctx()
                .animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.16);
        let press = ui.ctx().animate_bool_with_time(
            response.id.with("press"),
            response.is_pointer_button_down_on(),
            0.08,
        );
        let background = self.fill.unwrap_or(if ui.visuals().dark_mode {
            Color32::from_rgb(42, 45, 61)
        } else {
            Color32::from_rgb(239, 240, 249)
        });
        let background = blend(
            background,
            if self.fill.is_some() {
                Color32::from_rgb(86, 101, 183)
            } else {
                Color32::from_rgb(218, 222, 243)
            },
            hover,
        );
        let rect = rect.shrink(press * 0.9);
        ui.painter().rect_filled(rect, 5., background);
        ripple(ui, &response, foreground);
        let x = rect.center().x - (galley.size().x + icon_space) / 2.;
        if let Some(icon_name) = self.icon {
            icon(
                ui,
                icon_name,
                Rect::from_center_size(Pos2::new(x + 10., rect.center().y), Vec2::splat(20.)),
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
    if text.contains("Fabric") {
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
pub fn text_field(ui: &mut egui::Ui, value: &mut String) -> Response {
    let width = ui.available_width().min(560.);
    let background = if ui.visuals().dark_mode {
        Color32::from_rgb(43, 46, 63)
    } else {
        Color32::from_rgb(247, 248, 253)
    };
    ui.add(
        egui::TextEdit::singleline(value)
            .desired_width(width)
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
    let height = if subtitle.is_some() { 58. } else { 45. };
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), height), Sense::click());
    let t = ui
        .ctx()
        .animate_bool_with_time(response.id.with("selected"), selected, 0.18);
    let hover =
        ui.ctx()
            .animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.16);
    let background = if ui.visuals().dark_mode {
        Color32::from_rgb(61, 64, 98)
    } else {
        Color32::from_rgb(221, 225, 247)
    };
    if t + hover > 0. {
        ui.painter().rect_filled(
            rect.shrink2(Vec2::new(7., 2.)),
            6.,
            background.gamma_multiply((t * 0.85 + hover * 0.35).min(1.)),
        );
    }
    let fg = blend(ui.visuals().text_color(), PRIMARY, t);
    icon(
        ui,
        key,
        Rect::from_center_size(
            Pos2::new(rect.left() + 28., rect.center().y),
            Vec2::splat(24.),
        ),
        fg,
    );
    let y = if subtitle.is_some() {
        rect.center().y - 9.
    } else {
        rect.center().y
    };
    ui.painter().text(
        Pos2::new(rect.left() + 54., y),
        egui::Align2::LEFT_CENTER,
        name,
        FontId::proportional(14.),
        fg,
    );
    if let Some(subtitle) = subtitle {
        let text = ui.painter().layout(
            subtitle.to_owned(),
            FontId::proportional(11.),
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
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
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
    let t = ui.ctx().animate_bool_with_time(response.id, *value, 0.17);
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
