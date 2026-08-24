//! The launcher application: window frame, sidebar navigation and pages.
//!
//! Port of HMCL's `RootPage`/`MainWindowPane` structure: a custom title bar,
//! a categorized left sidebar and a central page area.

use egui::{Color32, Context, Frame, Pos2, Rect, RichText, Ui, Vec2};

use crate::theme::{self, AccentColor, Appearance};
use crate::widgets::icon;

/// Sidebar navigation entries, mirroring `RootPage`'s sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NavPage {
    #[default]
    Account,
    Game,
    Instances,
    Download,
    Settings,
}

impl NavPage {
    fn label_key(&self) -> &'static str {
        match self {
            NavPage::Account => "account",
            NavPage::Game => "instance",
            NavPage::Instances => "instance.manage",
            NavPage::Download => "download",
            NavPage::Settings => "settings",
        }
    }

    fn icon_name(&self) -> &'static str {
        match self {
            NavPage::Account => "PERSON",
            NavPage::Game => "GAMEPAD",
            NavPage::Instances => "FORMAT_LIST_BULLETED",
            NavPage::Download => "DOWNLOAD",
            NavPage::Settings => "SETTINGS",
        }
    }
}

/// The root application state.
pub struct HmclApp {
    pub appearance: Appearance,
    pub accent: AccentColor,
    pub nav: NavPage,
}

impl HmclApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(&cc.egui_ctx);
        let appearance = detect_appearance(&cc.egui_ctx);
        let accent = AccentColor::Blue;
        cc.egui_ctx.set_style(theme_style(appearance, accent));
        Self {
            appearance,
            accent,
            nav: NavPage::Account,
        }
    }

    fn apply_theme(&mut self, ctx: &Context) {
        ctx.set_style(theme_style(self.appearance, self.accent));
    }
}

fn theme_style(appearance: Appearance, accent: AccentColor) -> egui::Style {
    let mut style = egui::Style::default();
    theme::apply_style(&mut style, appearance, accent);
    style
}

/// Load fonts: the default egui fonts plus the bundled CJK font.
fn setup_fonts(ctx: &Context) {
    let mut fonts = egui::FontDefinitions::default();
    let font_path = crate::assets_dir().join("fonts/NotoSansSC.ttf");
    match std::fs::read(&font_path) {
        Ok(bytes) => {
            fonts
                .font_data
                .insert("noto_sans_sc".to_owned(), egui::FontData::from_owned(bytes).into());
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                fonts
                    .families
                    .entry(family)
                    .or_default()
                    .push("noto_sans_sc".to_owned());
            }
            tracing::info!("loaded bundled CJK font from {}", font_path.display());
        }
        Err(e) => {
            tracing::warn!("failed to load bundled CJK font {}: {e}", font_path.display());
        }
    }
    ctx.set_fonts(fonts);
}

/// Detect the initial light/dark preference (system dark mode, then egui).
fn detect_appearance(ctx: &Context) -> Appearance {
    match dark_light::detect() {
        Ok(dark_light::Mode::Dark) => Appearance::Dark,
        Ok(dark_light::Mode::Light) => Appearance::Light,
        _ => {
            if ctx.style().visuals.dark_mode {
                Appearance::Dark
            } else {
                Appearance::Light
            }
        }
    }
}

impl eframe::App for HmclApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.apply_theme(ctx);
        title_bar(ctx, self);
        sidebar(ctx, self);
        central_panel(ctx, self);
    }
}

/// The custom window title bar (port of `MainWindowPane`).
fn title_bar(ctx: &Context, app: &HmclApp) {
    let palette = theme::MonetPalette::resolve(app.appearance, app.accent);
    let height = 36.0;
    egui::TopBottomPanel::top("title_bar")
        .exact_height(height)
        .frame(Frame::NONE.fill(palette.surface_container_low))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Drag region covering the whole bar minus the buttons.
                let buttons_width = 3.0 * 44.0;
                let drag_rect = Rect::from_min_max(
                    ui.max_rect().min,
                    Pos2::new(ui.max_rect().max.x - buttons_width, ui.max_rect().max.y),
                );
                let response = ui.interact(drag_rect, ui.id().with("title_drag"), egui::Sense::drag());
                if response.drag_started() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if response.double_clicked() {
                    let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                }

                let _ = icon(ui, "FORT", 18.0, palette.primary);
                ui.add_space(6.0);
                ui.label(
                    RichText::new(crate::i18n::tr("launcher"))
                        .color(palette.on_surface)
                        .size(13.0),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(4.0);
                    window_button(ui, "CLOSE", palette, |ctx| {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    });
                    window_button(ui, "MINIMIZE_CENTER", palette, |ctx| {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    });
                    let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                    let icon_name = if maximized { "CHECKROOM" } else { "OUTPUT" };
                    window_button(ui, icon_name, palette, |ctx| {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                    });
                });
            });
        });
}

fn window_button(ui: &mut Ui, icon_name: &str, palette: theme::MonetPalette, action: impl FnOnce(&Context)) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(44.0), egui::Sense::click());
    let color = if response.hovered() {
        palette.primary_container
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 0.0, color);
    let _ = icon(ui, icon_name, 16.0, palette.on_surface_variant);
    if response.clicked() {
        action(ui.ctx());
    }
}

/// The categorized left sidebar (port of `RootPage`'s `AdvancedListBox`).
fn sidebar(ctx: &Context, app: &mut HmclApp) {
    let palette = theme::MonetPalette::resolve(app.appearance, app.accent);
    egui::SidePanel::left("sidebar")
        .exact_width(200.0)
        .frame(Frame::NONE.fill(palette.surface_container_low))
        .show(ctx, |ui| {
            ui.add_space(10.0);
            nav_category(ui, "account");
            nav_item(ui, app, NavPage::Account, true);
            ui.add_space(14.0);
            nav_category(ui, "instance");
            nav_item(ui, app, NavPage::Game, true);
            nav_item(ui, app, NavPage::Instances, false);
            nav_item(ui, app, NavPage::Download, false);
            ui.add_space(14.0);
            nav_category(ui, "settings.launcher.general");
            nav_item(ui, app, NavPage::Settings, false);
        });
}

fn nav_category(ui: &mut Ui, key: &str) {
    let palette = theme::MonetPalette::resolve(Appearance::Light, AccentColor::Blue);
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(16.0);
        ui.label(
            RichText::new(crate::i18n::tr(key).to_uppercase())
                .size(11.0)
                .color(palette.on_surface_variant),
        );
    });
}

fn nav_item(ui: &mut Ui, app: &mut HmclApp, page: NavPage, highlight: bool) {
    let palette = theme::MonetPalette::resolve(app.appearance, app.accent);
    let selected = app.nav == page;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), egui::Sense::click());

    if selected {
        let indicator = Rect::from_min_max(
            Pos2::new(rect.min.x, rect.min.y),
            Pos2::new(rect.min.x + 3.0, rect.max.y),
        );
        ui.painter().rect_filled(indicator, 0.0, palette.primary);
        ui.painter().rect_filled(rect, 0.0, palette.primary_container);
    } else if response.hovered() {
        ui.painter().rect_filled(rect, 0.0, palette.surface_container_high);
    }

    let icon_color = if selected { palette.on_primary_container } else { palette.on_surface_variant };
    let text_color = if selected { palette.on_primary_container } else { palette.on_surface };

    let icon_rect = Rect::from_min_size(rect.min + Vec2::new(16.0, 9.0), Vec2::splat(18.0));
    ui.painter().extend(
        crate::widgets::icon_shapes(page.icon_name(), icon_rect.min, 18.0, icon_color).unwrap_or_default(),
    );
    let text_pos = Pos2::new(rect.min.x + 44.0, rect.center().y);
    ui.painter().text(
        text_pos,
        egui::Align2::LEFT_CENTER,
        crate::i18n::tr(page.label_key()),
        egui::FontId::proportional(14.0),
        text_color,
    );

    let _ = highlight;
    if response.clicked() {
        app.nav = page;
    }
}

/// The central page area. Currently a placeholder for the ported pages.
fn central_panel(ctx: &Context, app: &HmclApp) {
    let palette = theme::MonetPalette::resolve(app.appearance, app.accent);
    egui::CentralPanel::default()
        .frame(Frame::NONE.fill(palette.surface))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() / 2.0 - 40.0);
                let _ = icon(ui, app.nav.icon_name(), 64.0, palette.primary);
                ui.add_space(12.0);
                ui.label(
                    RichText::new(crate::i18n::tr(app.nav.label_key()))
                        .color(palette.on_surface_variant)
                        .size(16.0),
                );
            });
        });
}

