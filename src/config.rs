use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::PathBuf};

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct VersionSettings {
    pub memory: Option<u32>,
    pub java: Option<String>,
    pub extra_jvm: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fullscreen: Option<bool>,
    pub isolated: Option<bool>,
    pub auto_java: Option<bool>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub game_dir: String,
    pub player: String,
    pub java: String,
    pub java_paths: Vec<String>,
    pub hidden_java: Vec<String>,
    pub memory: u32,
    pub selected: String,
    pub isolated: bool,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub accounts: Vec<String>,
    pub directories: Vec<String>,
    pub download_threads: usize,
    pub auto_java: bool,
    pub dark_mode: bool,
    pub extra_jvm: String,
    pub wallpaper: String,
    pub ui_scale: f32,
    pub reduce_motion: bool,
    pub favorites: Vec<String>,
    pub version_settings: BTreeMap<String, VersionSettings>,
    pub start_page: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            game_dir: data_dir().join("minecraft").to_string_lossy().into(),
            player: "Player".into(),
            java: String::new(),
            java_paths: vec![],
            hidden_java: vec![],
            memory: 2048,
            selected: String::new(),
            isolated: true,
            width: 854,
            height: 480,
            fullscreen: false,
            accounts: vec![],
            directories: vec![],
            download_threads: 8,
            auto_java: true,
            dark_mode: true,
            extra_jvm: String::new(),
            wallpaper: String::new(),
            ui_scale: 1.,
            reduce_motion: false,
            favorites: vec![],
            version_settings: BTreeMap::new(),
            start_page: "home".into(),
        }
    }
}

pub fn data_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("RUST_HMCL_DATA_DIR") {
        return PathBuf::from(path);
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("rust-hmcl")
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = data_dir().join("config.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let mut config: Self = serde_json::from_slice(&fs::read(&path)?)
            .context("配置文件损坏，请备份后修复 config.json")?;
        config.normalize();
        Ok(config)
    }
    pub fn save(&self) -> Result<()> {
        self.validate()?;
        fs::create_dir_all(data_dir())?;
        // Write fully before replacing, so a failed serialization cannot truncate settings.
        let path = data_dir().join("config.json");
        let temporary = path.with_extension("tmp");
        fs::write(&temporary, serde_json::to_vec_pretty(self)?)?;
        if path.exists() {
            fs::copy(&path, path.with_extension("json.bak"))?;
        }
        fs::rename(temporary, path).context("无法保存配置")?;
        Ok(())
    }
    pub fn validate(&self) -> Result<()> {
        if self.game_dir.trim().is_empty() {
            bail!("游戏目录不能为空");
        }
        if !(512..=65536).contains(&self.memory) {
            bail!("内存应在 512–65536 MB 之间");
        }
        if !(320..=7680).contains(&self.width) || !(240..=4320).contains(&self.height) {
            bail!("窗口尺寸超出允许范围");
        }
        validate_player(&self.player)?;
        for account in &self.accounts {
            validate_player(account)?;
        }
        if !(1..=32).contains(&self.download_threads) {
            bail!("下载并发应在 1–32 之间");
        }
        shell_words::split(&self.extra_jvm).context("额外 JVM 参数的引号未闭合")?;
        if !self.ui_scale.is_finite() || !(0.8..=1.5).contains(&self.ui_scale) {
            bail!("界面缩放应在 80%–150% 之间");
        }
        for settings in self.version_settings.values() {
            if settings.memory.is_some_and(|m| !(512..=65536).contains(&m)) {
                bail!("版本内存超出允许范围");
            }
            if let Some(args) = &settings.extra_jvm {
                shell_words::split(args).context("版本 JVM 参数的引号未闭合")?;
            }
            if settings.width.is_some_and(|v| !(320..=7680).contains(&v))
                || settings.height.is_some_and(|v| !(240..=4320).contains(&v))
            {
                bail!("版本窗口尺寸超出允许范围");
            }
        }
        Ok(())
    }
    pub fn normalize(&mut self) {
        if !self.accounts.contains(&self.player) {
            self.accounts.push(self.player.clone());
        }
        if !self.directories.contains(&self.game_dir) {
            self.directories.push(self.game_dir.clone());
        }
        self.accounts.sort();
        self.accounts.dedup();
        self.directories.sort();
        self.directories.dedup();
        self.download_threads = self.download_threads.clamp(1, 32);
        if !self.ui_scale.is_finite() {
            self.ui_scale = 1.;
        }
        self.ui_scale = self.ui_scale.clamp(0.8, 1.5);
        self.favorites.sort();
        self.favorites.dedup();
        self.java_paths.retain(|path| !path.trim().is_empty());
        self.java_paths.sort();
        self.java_paths.dedup();
        self.hidden_java.sort();
        self.hidden_java.dedup();
    }
    pub fn for_selected(&self) -> Self {
        let mut config = self.clone();
        if let Some(settings) = self.version_settings.get(&self.selected) {
            if let Some(memory) = settings.memory {
                config.memory = memory;
            }
            if let Some(java) = &settings.java {
                config.java = java.clone();
            }
            if let Some(args) = &settings.extra_jvm {
                config.extra_jvm = args.clone();
            }
            if let Some(width) = settings.width {
                config.width = width;
            }
            if let Some(height) = settings.height {
                config.height = height;
            }
            if let Some(fullscreen) = settings.fullscreen {
                config.fullscreen = fullscreen;
            }
            if let Some(isolated) = settings.isolated {
                config.isolated = isolated;
            }
            if let Some(auto_java) = settings.auto_java {
                config.auto_java = auto_java;
            }
        }
        config
    }
    pub fn remap_instance(&mut self, old: &str, new: &str, rename: bool) {
        if let Some(settings) = self.version_settings.get(old).cloned() {
            self.version_settings.insert(new.into(), settings);
        }
        if self.favorites.iter().any(|id| id == old) {
            self.favorites.push(new.into());
        }
        if rename {
            self.version_settings.remove(old);
            self.favorites.retain(|id| id != old);
        }
        self.selected = new.into();
        self.normalize();
    }
}

pub fn validate_player(player: &str) -> Result<()> {
    if player.is_empty()
        || player.len() > 16
        || !player
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_')
    {
        bail!("离线用户名需为 1–16 位英文字母、数字或下划线");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fullscreen_inherits_and_explicit_windowed_override_survives_migration() {
        let mut config: Config = serde_json::from_str(
            r#"{"selected":"custom","version_settings":{"custom":{"memory":4096}}}"#,
        )
        .unwrap();
        assert!(!config.fullscreen);
        assert_eq!(config.version_settings["custom"].fullscreen, None);
        config.fullscreen = true;
        assert!(config.for_selected().fullscreen);
        config
            .version_settings
            .get_mut("custom")
            .unwrap()
            .fullscreen = Some(false);
        let restored: Config =
            serde_json::from_slice(&serde_json::to_vec(&config).unwrap()).unwrap();
        assert!(restored.fullscreen);
        assert!(!restored.for_selected().fullscreen);
        config.remap_instance("custom", "copy", false);
        assert_eq!(config.version_settings["copy"].fullscreen, Some(false));
        config.version_settings.get_mut("copy").unwrap().fullscreen = Some(true);
        assert!(config.for_selected().fullscreen);
        assert_eq!(config.version_settings["custom"].fullscreen, Some(false));
        config.version_settings.get_mut("copy").unwrap().fullscreen = None;
        assert!(config.for_selected().fullscreen);
    }
    #[test]
    fn per_version_settings_do_not_change_global_defaults() {
        let mut config = Config {
            selected: "fabric".into(),
            ..Default::default()
        };
        config.version_settings.insert(
            "fabric".into(),
            VersionSettings {
                memory: Some(4096),
                java: Some("java17".into()),
                extra_jvm: None,
                ..Default::default()
            },
        );
        assert_eq!(config.for_selected().memory, 4096);
        assert_eq!(config.memory, 2048);
        assert_eq!(config.for_selected().java, "java17");
        config.selected = "vanilla".into();
        assert_eq!(config.for_selected().memory, 2048);
    }
    #[test]
    fn per_instance_window_and_directory_overrides_round_trip() {
        let mut config = Config {
            selected: "custom".into(),
            ..Default::default()
        };
        config.version_settings.insert(
            "custom".into(),
            VersionSettings {
                width: Some(1280),
                height: Some(720),
                isolated: Some(false),
                auto_java: Some(false),
                ..Default::default()
            },
        );
        let saved = serde_json::to_vec(&config).unwrap();
        let restored: Config = serde_json::from_slice(&saved).unwrap();
        let effective = restored.for_selected();
        assert_eq!((effective.width, effective.height), (1280, 720));
        assert!(!effective.isolated);
        assert!(!effective.auto_java);
        assert_eq!((restored.width, restored.height), (854, 480));
        assert!(restored.isolated);
        assert!(restored.auto_java);
        let mut invalid = restored;
        invalid.version_settings.get_mut("custom").unwrap().width = Some(0);
        assert!(invalid.validate().is_err());
        let old: VersionSettings = serde_json::from_str(r#"{"memory":4096}"#).unwrap();
        assert!(old.width.is_none() && old.isolated.is_none());
    }
    #[test]
    fn migrate_old_settings() {
        let mut c: Config =
            serde_json::from_str(r#"{"player":"Steve","game_dir":"games"}"#).unwrap();
        c.normalize();
        assert_eq!(c.accounts, vec!["Steve"]);
        assert_eq!(c.directories, vec!["games"]);
        assert_eq!(c.download_threads, 8);
        assert!(c.auto_java);
    }
    #[test]
    fn cloning_and_renaming_keep_instance_preferences_independent() {
        let mut config = Config::default();
        config.favorites.push("original".into());
        config.version_settings.insert(
            "original".into(),
            VersionSettings {
                memory: Some(4096),
                ..Default::default()
            },
        );
        config.remap_instance("original", "copy", false);
        assert_eq!(config.selected, "copy");
        assert!(config.version_settings.contains_key("original"));
        config.version_settings.get_mut("copy").unwrap().memory = Some(8192);
        assert_eq!(config.version_settings["original"].memory, Some(4096));
        config.remap_instance("copy", "renamed", true);
        assert!(!config.version_settings.contains_key("copy"));
        assert_eq!(config.version_settings["renamed"].memory, Some(8192));
        assert!(config.favorites.contains(&"renamed".into()));
        assert!(!config.favorites.contains(&"copy".into()));
    }
    #[test]
    fn reject_invalid_players_and_arguments() {
        assert!(validate_player("玩家").is_err());
        assert!(validate_player("Steve_123").is_ok());
        let c = Config {
            extra_jvm: "-Dfoo=\"unclosed".into(),
            ..Default::default()
        };
        assert!(c.validate().is_err());
    }
}
