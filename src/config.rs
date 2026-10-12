use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub game_dir: String,
    pub player: String,
    pub java: String,
    pub memory: u32,
    pub selected: String,
    pub isolated: bool,
    pub width: u32,
    pub height: u32,
    pub accounts: Vec<String>,
    pub directories: Vec<String>,
    pub download_threads: usize,
    pub auto_java: bool,
    pub dark_mode: bool,
    pub extra_jvm: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            game_dir: data_dir().join("minecraft").to_string_lossy().into(),
            player: "Player".into(),
            java: String::new(),
            memory: 2048,
            selected: String::new(),
            isolated: true,
            width: 854,
            height: 480,
            accounts: vec![],
            directories: vec![],
            download_threads: 8,
            auto_java: true,
            dark_mode: false,
            extra_jvm: String::new(),
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
