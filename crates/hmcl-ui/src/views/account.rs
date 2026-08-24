//! The account management page and login dialogs.
//!
//! Port of HMCL's `ui.account.AccountListPage` and login panes.

use std::time::{Duration, Instant};

use egui::{Align2, Context, Id, Pos2, Rect, RichText, Ui};

use hmcl_core::auth::microsoft::{DeviceCodeResponse, DeviceTokenResponse, MicrosoftAuthenticator};
use hmcl_core::auth::{offline_uuid, Account, AccountStorage};

use crate::async_runtime::{spawn, AsyncTask};
use crate::theme;
use crate::widgets::dialog::{Dialog, DialogResult};
use crate::widgets::toast::{hint, ToastKind, Toasts};

/// The login dialog currently shown (if any).
pub enum LoginDialog {
    MethodSelect,
    Offline {
        name: String,
        error: Option<String>,
    },
    Microsoft(Box<MsLoginState>),
}

/// State machine of the Microsoft device-code login.
pub enum MsLoginState {
    Requesting(AsyncTask<DeviceCodeResponse>),
    WaitingUser {
        device: DeviceCodeResponse,
        next_poll_at: Instant,
        poll: Option<AsyncTask<Option<DeviceTokenResponse>>>,
        error: Option<String>,
    },
    Authenticating(AsyncTask<Account>),
}

/// Persistent state of the account page.
#[derive(Default)]
pub struct AccountPage {
    pub dialog: Option<LoginDialog>,
}

impl AccountPage {
    /// Render the page and handle its interactions.
    pub fn show(&mut self, ctx: &Context, accounts: &mut AccountStorage, toasts: &mut Toasts) {
        let palette = theme::palette();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ctx, |ui| {
                ui.add_space(20.0);
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    crate::widgets::card(ui, |ui| {
                        ui.set_width(480.0);
                        page_header(ui, "account");
                        ui.add_space(8.0);
                        if accounts.accounts.is_empty() {
                            ui.vertical_centered(|ui| {
                                ui.add_space(32.0);
                                ui.label(
                                    RichText::new(crate::i18n::tr("account.empty"))
                                        .color(palette.on_surface_variant),
                                );
                                ui.add_space(8.0);
                                if crate::widgets::filled_button(
                                    ui,
                                    ui.id().with("add_account"),
                                    &crate::i18n::tr("account.missing.add"),
                                    Some("ADD"),
                                )
                                .clicked()
                                {
                                    self.dialog = Some(LoginDialog::MethodSelect);
                                }
                                ui.add_space(24.0);
                            });
                        } else {
                            let mut remove_index = None;
                            let mut copied = false;
                            for (index, account) in accounts.accounts.iter().enumerate() {
                                let selected = accounts.selected.as_deref() == Some(account.uuid());
                                let response = crate::widgets::two_line_list_item(
                                    ui,
                                    ui.id().with(("account", account.uuid())),
                                    Some("PERSON"),
                                    account.username(),
                                    &format!("UUID: {}", account.uuid_dashed()),
                                    selected,
                                    true,
                                );
                                if response.clicked() {
                                    accounts.selected = Some(account.uuid().to_owned());
                                }
                                response.context_menu(|ui| {
                                    if ui.button(crate::i18n::tr("account.copy_uuid")).clicked() {
                                        ui.ctx().copy_text(account.uuid_dashed());
                                        copied = true;
                                        ui.close();
                                    }
                                    if ui.button(crate::i18n::tr("button.remove")).clicked() {
                                        remove_index = Some(index);
                                        ui.close();
                                    }
                                });
                            }
                            if copied {
                                toasts.info(crate::i18n::tr("message.copied"));
                            }
                            if let Some(index) = remove_index {
                                let account = accounts.accounts[index].clone();
                                accounts.accounts.remove(index);
                                if accounts.selected.as_deref() == Some(account.uuid()) {
                                    accounts.selected = None;
                                }
                                if let Err(e) = accounts.save(&crate::data_dir().join("accounts.json")) {
                                    toasts.error(format!("{e}"));
                                }
                            }
                            ui.add_space(8.0);
                        }
                    });
                });

                // Floating action button, bottom-right.
                if !accounts.accounts.is_empty() {
                    let fab_size = 56.0;
                    let fab_rect = Rect::from_min_size(
                        Pos2::new(
                            ui.max_rect().max.x - fab_size - 28.0,
                            ui.max_rect().max.y - fab_size - 28.0,
                        ),
                        egui::vec2(fab_size, fab_size),
                    );
                    let response = ui.interact(fab_rect, ui.id().with("fab_add"), egui::Sense::click());
                    ui.painter().circle_filled(
                        fab_rect.center(),
                        fab_size / 2.0,
                        if response.hovered() {
                            palette.primary_container
                        } else {
                            palette.primary
                        },
                    );
                    crate::widgets::icon::icon_in_rect(
                        ui.painter(),
                        fab_rect,
                        "ADD",
                        palette.on_primary,
                    );
                    if response.clicked() {
                        self.dialog = Some(LoginDialog::MethodSelect);
                    }
                }
            });

        if let Some(dialog) = self.dialog.take() {
            self.show_login_dialog(ctx, dialog, accounts, toasts);
        }
    }

    /// Render the active login dialog, re-arming it while still open.
    fn show_login_dialog(
        &mut self,
        ctx: &Context,
        dialog: LoginDialog,
        accounts: &mut AccountStorage,
        toasts: &mut Toasts,
    ) {
        match dialog {
            LoginDialog::MethodSelect => {
                let mut next: Option<LoginDialog> = None;
                let result = Dialog::new(Id::new("login_method"), crate::i18n::tr("account.login"))
                    .positive_text(None)
                    .show(ctx, |ui| {
                        ui.add_space(4.0);
                        if method_button(ui, "PERSON", &crate::i18n::tr("account.methods.offline")) {
                            next = Some(LoginDialog::Offline {
                                name: String::new(),
                                error: None,
                            });
                        }
                        ui.add_space(6.0);
                        if method_button(ui, "MICROSOFT", &crate::i18n::tr("account.methods.microsoft"))
                        {
                            next = Some(LoginDialog::Microsoft(Box::new(
                                MsLoginState::Requesting(request_device_code()),
                            )));
                        }
                        ui.add_space(4.0);
                    });
                self.dialog = if result.is_none() {
                    Some(LoginDialog::MethodSelect)
                } else {
                    next
                };
            }
            LoginDialog::Offline { mut name, error } => {
                let mut next: Option<LoginDialog> = None;
                let mut error = error;
                let result = Dialog::new(Id::new("login_offline"), crate::i18n::tr("account.methods.offline"))
                    .positive_text(Some(crate::i18n::tr("account.login")))
                    .positive_enabled(!name.trim().is_empty())
                    .show(ctx, |ui| {
                        ui.add_space(4.0);
                        ui.label(crate::i18n::tr("account.username"));
                        crate::widgets::rounded_text_edit_singleline(
                            ui,
                            &mut name,
                            "",
                            ui.available_width(),
                        );
                        if let Some(message) = &error {
                            hint(ui, ToastKind::Error, message);
                        }
                        ui.add_space(4.0);
                    });
                match result {
                    None => {
                        // Still open: keep the dialog with updated input.
                        next = Some(LoginDialog::Offline { name, error });
                    }
                    Some(DialogResult::Accept) => {
                        let trimmed = name.trim().to_owned();
                        if trimmed.is_empty() {
                            error = Some(crate::i18n::tr("account.methods.offline.name.invalid"));
                            next = Some(LoginDialog::Offline { name, error });
                        } else {
                            let uuid = offline_uuid(&trimmed);
                            let account = Account::Offline {
                                uuid: uuid.clone(),
                                username: trimmed,
                            };
                            accounts.accounts.push(account);
                            accounts.selected = Some(uuid);
                            save_accounts(accounts, toasts);
                            toasts.info(crate::i18n::tr("account.login.refresh"));
                        }
                    }
                    Some(_) => {}
                }
                self.dialog = next;
            }
            LoginDialog::Microsoft(mut state) => {
                let mut transition: Option<MsLoginState> = None;
                let mut flow_done = false;
                let result = Dialog::new(
                    Id::new("login_microsoft"),
                    crate::i18n::tr("account.methods.microsoft"),
                )
                .positive_text(None)
                .show(ctx, |ui| match &mut *state {
                    MsLoginState::Requesting(task) => {
                        if let Some(result) = task.poll() {
                            match result {
                                Ok(device) => {
                                    transition = Some(MsLoginState::WaitingUser {
                                        next_poll_at: Instant::now()
                                            + Duration::from_secs(device.interval.max(1) as u64),
                                        device,
                                        poll: None,
                                        error: None,
                                    });
                                }
                                Err(e) => {
                                    hint(ui, ToastKind::Error, &e);
                                }
                            }
                        } else {
                            crate::widgets::spinner(ui, 24.0);
                        }
                    }
                    MsLoginState::WaitingUser {
                        device,
                        next_poll_at,
                        poll,
                        error,
                    } => {
                        show_device_code(ui, device, toasts);
                        if Instant::now() >= *next_poll_at && poll.is_none() {
                            let device_code = device.device_code.clone();
                            *poll = Some(spawn_poll(device_code));
                        }
                        if let Some(task) = poll
                            && let Some(result) = task.poll() {
                                match result {
                                    Ok(Some(token)) => {
                                        let authenticator = MicrosoftAuthenticator::new();
                                        let access = token.access_token.clone();
                                        let refresh = token.refresh_token.clone();
                                        transition =
                                            Some(MsLoginState::Authenticating(spawn(
                                                async move {
                                                    authenticator
                                                        .authenticate_with_access_token(
                                                            &access,
                                                            &refresh,
                                                        )
                                                        .await
                                                        .map_err(|e| e.to_string())
                                                },
                                            )));
                                    }
                                    Ok(None) => {
                                        *next_poll_at = Instant::now()
                                            + Duration::from_secs(
                                                device.interval.max(1) as u64,
                                            );
                                        *poll = None;
                                    }
                                    Err(e) => {
                                        *error = Some(e);
                                    }
                                }
                            }
                        if let Some(message) = error {
                            hint(ui, ToastKind::Error, message);
                        }
                    }
                    MsLoginState::Authenticating(task) => {
                        if let Some(result) = task.poll() {
                            match result {
                                Ok(account) => {
                                    let uuid = account.uuid().to_owned();
                                    accounts.accounts.push(account);
                                    accounts.selected = Some(uuid);
                                    save_accounts(accounts, toasts);
                                    toasts.info(crate::i18n::tr("account.login.refresh"));
                                    flow_done = true;
                                }
                                Err(e) => {
                                    hint(ui, ToastKind::Error, &e);
                                    flow_done = true;
                                }
                            }
                        } else {
                            ui.horizontal(|ui| {
                                crate::widgets::spinner(ui, 20.0);
                                ui.label(crate::i18n::tr("account.methods.microsoft.profile"));
                            });
                        }
                    }
                });
                if let Some(next) = transition {
                    *state = next;
                }
                if result.is_none() && !flow_done {
                    self.dialog = Some(LoginDialog::Microsoft(state));
                }
            }
        }
    }
}

/// Render the header of a page with a large title.
fn page_header(ui: &mut Ui, title_key: &str) {
    let palette = theme::palette();
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(crate::i18n::tr(title_key))
                .size(20.0)
                .color(palette.on_surface),
        );
    });
}

/// A large method-selection button for the login dialog.
fn method_button(ui: &mut Ui, icon_name: &str, label: &str) -> bool {
    let palette = theme::palette();
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), 52.0),
        egui::Sense::click(),
    );
    let bg = if response.hovered() {
        palette.surface_container_highest
    } else {
        palette.surface_container
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(10), bg);
    let icon_rect = egui::Rect::from_min_size(
        egui::pos2(rect.min.x + 14.0, rect.center().y - 12.0),
        egui::vec2(24.0, 24.0),
    );
    crate::widgets::icon::icon_in_rect(ui.painter(), icon_rect, icon_name, palette.primary);
    ui.painter().text(
        egui::pos2(rect.min.x + 48.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(14.0),
        palette.on_surface,
    );
    response.clicked()
}

/// Render the device code display block.
fn show_device_code(ui: &mut Ui, device: &DeviceCodeResponse, toasts: &mut Toasts) {
    let palette = theme::palette();
    ui.add_space(4.0);
    ui.label(crate::i18n::tr("account.methods.microsoft.hint"));
    ui.add_space(10.0);
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(&device.user_code)
                .size(24.0)
                .color(palette.primary),
        );
        if ui
            .small_button(crate::i18n::tr("button.copy"))
            .clicked()
        {
            ui.ctx().copy_text(device.user_code.clone());
            toasts.info(crate::i18n::tr("message.copied"));
        }
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(crate::i18n::tr("account.methods.microsoft.methods.device.hint"));
        if ui
            .link(device.verification_uri.clone())
            .clicked()
        {
            let _ = webbrowser::open(&device.verification_uri);
        }
    });
    ui.add_space(4.0);
}

/// Spawn the device code request.
fn request_device_code() -> AsyncTask<DeviceCodeResponse> {
    spawn(async move {
        let authenticator = MicrosoftAuthenticator::new();
        authenticator
            .request_device_code()
            .await
            .map_err(|e| e.to_string())
    })
}

/// Spawn a single token poll.
fn spawn_poll(device_code: String) -> AsyncTask<Option<DeviceTokenResponse>> {
    spawn(async move {
        let authenticator = MicrosoftAuthenticator::new();
        authenticator
            .poll_token(&device_code)
            .await
            .map_err(|e| e.to_string())
    })
}

/// Persist the account storage, surfacing errors as toasts.
pub fn save_accounts(accounts: &AccountStorage, toasts: &mut Toasts) {
    let path = crate::data_dir().join("accounts.json");
    if let Err(e) = accounts.save(&path) {
        toasts.error(format!("{e}"));
    }
}



