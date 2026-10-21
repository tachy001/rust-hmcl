//! Explicit hidden verification mode. Uses normal IPC and an isolated existing configuration.
use anyhow::{Context, Result};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};
use tauri::Manager;

#[derive(Clone)]
pub struct PlatformProbe {
    directory: PathBuf,
}

impl PlatformProbe {
    pub fn from_args() -> Result<Option<Self>> {
        if !std::env::args().any(|arg| arg == "--verify-platform") {
            return Ok(None);
        }
        anyhow::ensure!(
            !std::env::args().any(|arg| arg == "--verify-hidden-ui"),
            "Choose one verification mode"
        );
        let directory = PathBuf::from(std::env::var("RUST_HMCL_DATA_DIR")?).canonicalize()?;
        let config: serde_json::Value =
            serde_json::from_slice(&fs::read(directory.join("config.json"))?)?;
        anyhow::ensure!(
            config["player"] == "PlatformProof" && config["auto_java"] == false,
            "Requires a fresh isolated platform configuration"
        );
        let game = PathBuf::from(
            config["game_dir"]
                .as_str()
                .context("Missing game directory")?,
        );
        anyhow::ensure!(
            game.file_name() == Some(std::ffi::OsStr::new("minecraft"))
                && game
                    .parent()
                    .context("Missing fixture parent")?
                    .canonicalize()?
                    == directory
                && !game.exists(),
            "Verification game directory must stay in its fixture"
        );
        Ok(Some(Self { directory }))
    }
    pub fn configure(&self, context: &mut tauri::Context<tauri::Wry>) {
        for window in &mut context.config_mut().app.windows {
            window.visible = false;
            window.focus = false;
            window.incognito = true;
            window.data_directory = Some(self.directory.join("webview-profile"));
        }
    }
    pub fn setup(&self, app: &tauri::App) -> Result<()> {
        let window = app
            .get_webview_window("main")
            .context("Missing main window")?;
        let visible = window.is_visible()?;
        anyhow::ensure!(!visible, "Verification window must remain hidden");
        fs::write(
            self.directory.join("native-window.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "visible":visible, "platform":rust_hmcl::platform::Platform::current().info()
            }))?,
        )?;
        let directory = self.directory.clone();
        let app = app.handle().clone();
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(90);
            loop {
                if let Ok(bytes) = fs::read(directory.join("config.json")) {
                    if let Ok(config) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                        match config["player"].as_str() {
                            Some("IPCReady") => {
                                app.exit(0);
                                return;
                            }
                            Some("IPCFailed") => {
                                app.exit(3);
                                return;
                            }
                            _ => {}
                        }
                    }
                }
                if Instant::now() > deadline {
                    app.exit(4);
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        });
        Ok(())
    }
    pub fn page_loaded(
        &self,
        webview: &tauri::Webview,
        payload: &tauri::webview::PageLoadPayload<'_>,
    ) -> Result<()> {
        if webview.label() != "main" || payload.event() != tauri::webview::PageLoadEvent::Finished {
            return Ok(());
        }
        webview.eval(r#"
          (async () => {
            if (window.__asterPlatformProbe) return;
            window.__asterPlatformProbe = true;
            const invoke = (cmd, args) => window.__TAURI_INTERNALS__.invoke(cmd, args);
            let initial;
            try {
              initial = await invoke('snapshot');
              if (initial.config.player !== 'PlatformProof') throw Error('Fixture was not isolated');
              const deadline = Date.now() + 20000;
              while (!document.querySelector('.launcher')) {
                if (Date.now() > deadline) throw Error('Vue application did not mount');
                await new Promise(resolve => setTimeout(resolve, 50));
              }
              const platform = await invoke('platform_info');
              if (!['windows', 'macos', 'linux'].includes(platform.system)) throw Error('Unknown platform');
              if ((await invoke('task')).running) throw Error('Unexpected background task');
              const image = new Image(); image.src = '/aster/forge.svg'; await image.decode();
              if (!image.naturalWidth) throw Error('Bundled asset failed');
              const menu = new MouseEvent('contextmenu', { bubbles: true, cancelable: true });
              document.dispatchEvent(menu);
              if (!menu.defaultPrevented) throw Error('Browser menu was not suppressed');
              await invoke('update_config', {config: {...initial.config, player: 'IPCSaved'}});
              const saved = await invoke('snapshot');
              if (saved.config.player !== 'IPCSaved') throw Error('IPC write/read round trip failed');
              await invoke('update_config', {config: {...saved.config, player: 'IPCReady'}});
            } catch (error) {
              if (initial) await invoke('update_config', {config: {...initial.config, player: 'IPCFailed', extra_jvm: '-Daster.probe.error=' + encodeURIComponent(String(error))}});
              console.error('Platform verification:', error);
            }
          })();
        "#)?;
        Ok(())
    }
}
