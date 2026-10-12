use crate::{
    game::{Event, Version, Worker},
    metadata,
};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::{fs, path::Path};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Loader {
    Fabric,
    Quilt,
}
impl Loader {
    pub fn name(self) -> &'static str {
        match self {
            Self::Fabric => "Fabric",
            Self::Quilt => "Quilt",
        }
    }
    fn api(self) -> &'static str {
        match self {
            Self::Fabric => "https://meta.fabricmc.net/v2",
            Self::Quilt => "https://meta.quiltmc.org/v3",
        }
    }
}
impl Worker {
    pub fn ensure_base(&self, root: &Path, id: &str) -> Result<()> {
        let dir = crate::game::version_dir(root, id)?;
        if dir.join(format!("{id}.json")).is_file() {
            return Ok(());
        }
        let manifest =
            self.json("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")?;
        let versions: Vec<Version> = serde_json::from_value(manifest["versions"].clone())?;
        let version = versions
            .iter()
            .find(|v| v.id == id)
            .context("官方版本列表中找不到整合包需要的 Minecraft")?;
        self.install(root, version)
    }
    pub fn loader_profile(
        &self,
        game: &str,
        loader: Loader,
        version: Option<&str>,
    ) -> Result<Value> {
        crate::game::version_dir(Path::new("."), game)?;
        let selected = if let Some(version) = version {
            version.to_owned()
        } else {
            self.status(format!("获取 {} 加载器版本…", loader.name()));
            let versions = self.json(&format!("{}/versions/loader/{game}", loader.api()))?;
            let list = versions.as_array().context("加载器版本列表格式错误")?;
            let item = list
                .iter()
                .find(|v| v["loader"]["stable"].as_bool() == Some(true))
                .or_else(|| list.first())
                .context("该 Minecraft 版本没有可用加载器")?;
            item["loader"]["version"]
                .as_str()
                .context("缺少加载器版本")?
                .to_owned()
        };
        crate::game::version_dir(Path::new("."), &selected)?;
        let mut profile = self.json(&format!(
            "{}/versions/loader/{game}/{selected}/profile/json",
            loader.api()
        ))?;
        if profile["inheritsFrom"].as_str() != Some(game) {
            bail!("加载器返回的基础版本不匹配");
        }
        metadata::normalize_libraries(&mut profile)?;
        Ok(profile)
    }
    pub fn install_loader(
        &self,
        root: &Path,
        selected: &str,
        loader: Loader,
        version: Option<&str>,
        alias: Option<&str>,
    ) -> Result<String> {
        self.ensure_base(root, selected)?;
        let base = metadata::resolve(root, selected)?;
        let game = base["_clientVersion"].as_str().unwrap_or(selected);
        let parent = metadata::resolve(root, game)?;
        let mut profile = self.loader_profile(game, loader, version)?;
        if let Some(alias) = alias {
            profile["id"] = Value::String(alias.into());
        }
        let id = profile["id"]
            .as_str()
            .context("缺少加载器实例名称")?
            .to_owned();
        let dir = crate::game::version_dir(root, &id)?;
        self.status(format!("安装 {} · {game}", loader.name()));
        let merged = metadata::merge(&parent, &profile)?;
        self.prepare(root, &id, &merged)?;
        fs::create_dir_all(&dir)?;
        let pending = dir.join("loader.pending");
        fs::write(&pending, serde_json::to_vec_pretty(&profile)?)?;
        fs::rename(pending, dir.join(format!("{id}.json")))?;
        let _ = self.tx.send(Event::Installed(id.clone()));
        self.status(format!("{} 安装完成", loader.name()));
        Ok(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::AtomicBool, mpsc, Arc};
    #[test]
    #[ignore = "requires official Fabric and Quilt endpoints"]
    fn official_loader_profiles() {
        let (tx, _rx) = mpsc::channel();
        let w = Worker::new(tx, Arc::new(AtomicBool::new(false))).unwrap();
        for loader in [Loader::Fabric, Loader::Quilt] {
            let profile = w.loader_profile("1.20.1", loader, None).unwrap();
            assert_eq!(profile["inheritsFrom"], "1.20.1");
            assert!(profile["mainClass"]
                .as_str()
                .unwrap()
                .contains("KnotClient"));
            assert!(profile["libraries"]
                .as_array()
                .unwrap()
                .iter()
                .all(|lib| lib["downloads"]["artifact"]["path"].is_string()));
        }
    }
}
