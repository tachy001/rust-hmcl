//! The launcher application: window frame, sidebar navigation and pages.
//!
//! Port of HMCL's `RootPage`/`MainWindowPane` structure: a custom title bar,
//! a categorized left sidebar and a central page area.

use egui::{Color32, Context, Frame, Pos2, Rect, RichText, Ui, Vec2};

use hmcl_core::auth::AccountStorage;

use crate::config::LauncherConfig;
use crate::theme::{self, AccentColor, Appearance};
use crate::views;
use crate::widgets::icon;
use crate::widgets::toast::Toasts;

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
    pub toasts: Toasts,
    pub accounts: AccountStorage,
    pub config: LauncherConfig,
    account_page: views::account::AccountPage,
    download_page: views::install::DownloadPage,
    instance_page: views::instance::InstancePage,
}

impl HmclApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_fonts(&cc.egui_ctx);
        let config_path = crate::data_dir().join("config.json");
        let first_run = !config_path.exists();
        let config = LauncherConfig::load(&config_path).unwrap_or_else(|e| {
            tracing::warn!("failed to load config: {e}");
            LauncherConfig::default()
        });
        let appearance = if first_run {
            detect_appearance(&cc.egui_ctx)
        } else {
            config.appearance()
        };
        let accent = config.accent();
        cc.egui_ctx.set_style(theme_style(appearance, accent));
        theme::set_state(theme::ThemeState { appearance, accent });

        let accounts = AccountStorage::load(&crate::data_dir().join("accounts.json"))
            .unwrap_or_else(|e| {
                tracing::warn!("failed to load accounts: {e}");
                AccountStorage::default()
            });

        Self {
            appearance,
            accent,
            nav: NavPage::Account,
            toasts: Toasts::default(),
            accounts,
            config,
            account_page: views::account::AccountPage::default(),
            download_page: views::install::DownloadPage::default(),
            instance_page: views::instance::InstancePage::default(),
        }
    }

    pub fn apply_theme(&mut self, ctx: &Context) {
        ctx.set_style(theme_style(self.appearance, self.accent));
        theme::set_state(theme::ThemeState {
            appearance: self.appearance,
            accent: self.accent,
        });
        let _ = self.config.save(&crate::data_dir().join("config.json"));
    }

    /// Persist the current appearance settings.
    pub(crate) fn save_config(&self) {
        let mut config = self.config.clone();
        config.appearance = match self.appearance {
            Appearance::Dark => "dark".to_owned(),
            Appearance::Light => "light".to_owned(),
        };
        config.accent = self.accent.name().to_owned();
        if let Err(e) = config.save(&crate::data_dir().join("config.json")) {
            tracing::warn!("failed to save config: {e}");
        }
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
            fonts.font_data.insert(
                "noto_sans_sc".to_owned(),
                egui::FontData::from_owned(bytes).into(),
            );
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
            tracing::warn!(
                "failed to load bundled CJK font {}: {e}",
                font_path.display()
            );
        }
    }
    ctx.set_fonts(fonts);
}

/// Detect the initial light/dark preference, used on first run.
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
        paint_background(ctx, self);
        title_bar(ctx, self);
        sidebar(ctx, self);
        self.central_panel(ctx);
        self.toasts.show(ctx);
    }
}

/// Paint the wallpaper layer behind all panels (port of
/// `MainWindowPane`'s background node: bottom-center, contain-sized).
fn paint_background(ctx: &Context, app: &HmclApp) {
    let painter = ctx.layer_painter(egui::LayerId::background());
    let screen = ctx.screen_rect();
    let palette = theme::palette();

    // Base fill under the wallpaper.
    painter.rect_filled(screen, 0.0, palette.surface_container);

    let opacity = app.config.background_opacity.clamp(0.0, 1.0);
    if opacity >= 0.001 {
        if let Some(texture) = crate::image::wallpaper(ctx, &app.config.wallpaper) {
            let image_size = texture.size_vec2();
            let scale = (screen.width() / image_size.x).min(screen.height() / image_size.y);
            let size = image_size * scale;
            let rect = Rect::from_min_size(
                Pos2::new(screen.center().x - size.x / 2.0, screen.max.y - size.y),
                size,
            );
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            painter.image(
                texture.id(),
                rect,
                uv,
                Color32::WHITE.gamma_multiply(opacity),
            );
        }
        // Dark scrim so light text stays readable over bright wallpapers.
        if app.appearance.is_dark() {
            painter.rect_filled(screen, 0.0, Color32::from_black_alpha(64));
        }
    }
}

/// The custom window title bar (port of `MainWindowPane`'s jfx-decorator).
fn title_bar(ctx: &Context, _app: &HmclApp) {
    let palette = theme::palette();
    let height = 40.0;
    let fill = palette.surface_container_low.gamma_multiply(0.82);
    egui::TopBottomPanel::top("title_bar")
        .exact_height(height)
        .frame(Frame::NONE.fill(fill))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Drag region covering the whole bar minus the buttons.
                let buttons_width = 3.0 * 40.0;
                let drag_rect = Rect::from_min_max(
                    ui.max_rect().min,
                    Pos2::new(ui.max_rect().max.x - buttons_width, ui.max_rect().max.y),
                );
                let response =
                    ui.interact(drag_rect, ui.id().with("title_drag"), egui::Sense::drag());
                if response.drag_started() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if response.double_clicked() {
                    let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
                }

                ui.add_space(10.0);
                let _ = icon(ui, "FORT", 20.0, palette.primary);
                ui.add_space(6.0);
                ui.label(
                    RichText::new(crate::i18n::tr("launcher"))
                        .color(palette.on_surface)
                        .size(14.0),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(4.0);
                    window_button(ui, "CLOSE", palette, |ctx| {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    });
                    window_button(ui, "MINIMIZE_CENTER", palette, |ctx| {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    });
                    window_button(ui, "HELP", palette, |_ctx| {
                        let _ = webbrowser::open("https://hmcl.huangyuhui.net/");
                    });
                });
            });
        });
}

fn window_button(
    ui: &mut Ui,
    icon_name: &str,
    palette: theme::MonetPalette,
    action: impl FnOnce(&Context),
) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(40.0), egui::Sense::click());
    let color = if response.hovered() {
        palette.primary_container
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, 0.0, color);
    let _ = icon(ui, icon_name, 18.0, palette.on_surface_variant);
    if response.clicked() {
        action(ui.ctx());
    }
}

/// The categorized left sidebar (port of `RootPage`'s `AdvancedListBox`).
fn sidebar(ctx: &Context, app: &mut HmclApp) {
    let palette = theme::palette();
    let fill = palette.surface_container_low.gamma_multiply(0.82);
    egui::SidePanel::left("sidebar")
        .exact_width(200.0)
        .frame(Frame::NONE.fill(fill))
        .show(ctx, |ui| {
            ui.add_space(10.0);
            nav_category(ui, "account");
            nav_item(ui, app, NavPage::Account);
            ui.add_space(14.0);
            nav_category(ui, "instance");
            nav_item(ui, app, NavPage::Game);
            nav_item(ui, app, NavPage::Instances);
            nav_item(ui, app, NavPage::Download);
            ui.add_space(14.0);
            nav_category(ui, "settings.launcher.general");
            nav_item(ui, app, NavPage::Settings);
        });
}

fn nav_category(ui: &mut Ui, key: &str) {
    let palette = theme::palette();
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

fn nav_item(ui: &mut Ui, app: &mut HmclApp, page: NavPage) {
    let palette = theme::palette();
    let selected = app.nav == page;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), egui::Sense::click());

    let item_rect = rect.shrink2(Vec2::new(10.0, 2.0));
    if selected {
        ui.painter().rect_filled(
            item_rect,
            egui::CornerRadius::same(8),
            palette.primary_container,
        );
    } else if response.hovered() {
        ui.painter().rect_filled(
            item_rect,
            egui::CornerRadius::same(8),
            palette.surface_container_high.gamma_multiply(0.6),
        );
    }

    let icon_color = if selected {
        palette.on_primary_container
    } else {
        palette.on_surface_variant
    };
    let text_color = if selected {
        palette.on_primary_container
    } else {
        palette.on_surface
    };

    let icon_rect = Rect::from_min_size(rect.min + Vec2::new(22.0, 8.0), Vec2::splat(20.0));
    ui.painter().extend(
        crate::widgets::icon_shapes(page.icon_name(), icon_rect.min, 20.0, icon_color)
            .unwrap_or_default(),
    );
    let text_pos = Pos2::new(rect.min.x + 52.0, rect.center().y);
    ui.painter().text(
        text_pos,
        egui::Align2::LEFT_CENTER,
        crate::i18n::tr(page.label_key()),
        egui::FontId::proportional(14.0),
        text_color,
    );

    if response.clicked() {
        app.nav = page;
    }
}

impl HmclApp {
    /// The central page area, dispatching by the active navigation page.
    fn central_panel(&mut self, ctx: &Context) {
        match self.nav {
            NavPage::Account => {
                self.account_page
                    .show(ctx, &mut self.accounts, &mut self.toasts);
            }
            NavPage::Download => {
                self.download_page.show(ctx, &mut self.toasts);
            }
            NavPage::Instances => views::instance::show(ctx, &mut self.instance_page),
            NavPage::Game => views::instance::show_game(ctx, &self.accounts),
            NavPage::Settings => views::instance::show_settings(ctx, self),
        }
    }
}
