use crate::{
    game::{Event, Version, Worker},
    metadata,
};
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::{fs, io::Write, path::Path};

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
            crate::loader_catalog::validate_component(version)?;
            version.to_owned()
        } else {
            self.status(format!("获取 {} 加载器版本…", loader.name()));
            let kind = match loader {
                Loader::Fabric => crate::loader_catalog::LoaderKind::Fabric,
                Loader::Quilt => crate::loader_catalog::LoaderKind::Quilt,
            };
            crate::loader_catalog::LoaderCatalog::new(self).recommended(game, kind)?
        };
        crate::loader_catalog::validate_component(game)?;
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
        let client = base["_clientVersion"].as_str().unwrap_or(selected);
        let game = base["_gameVersion"].as_str().unwrap_or(client);
        let parent = metadata::resolve(root, client)?;
        let mut profile = self.loader_profile(game, loader, version)?;
        profile["inheritsFrom"] = client.into();
        if let Some(alias) = alias {
            profile["id"] = Value::String(alias.into());
        }
        let id = profile["id"]
            .as_str()
            .context("缺少加载器实例名称")?
            .to_owned();
        let dir = crate::game::version_dir(root, &id)?;
        if fs::symlink_metadata(dir.join(format!("{id}.json"))).is_ok() {
            bail!("同名实例已存在，请使用其他名称，或在游戏库校验已有实例");
        }
        self.status(format!("安装 {} · {game}", loader.name()));
        let merged = metadata::merge(&parent, &profile)?;
        self.prepare(root, &id, &merged)?;
        fs::create_dir_all(&dir)?;
        self.check()?;
        publish_profile(&dir, &id, &profile)?;
        let _ = self.tx.send(Event::Installed(id.clone()));
        self.status(format!("{} 安装完成", loader.name()));
        Ok(id)
    }
}

pub(crate) fn publish_profile(dir: &Path, id: &str, profile: &Value) -> Result<()> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let pending = dir.join(format!("loader-{}-{stamp}.pending", std::process::id()));
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)?;
    let result = (|| -> Result<()> {
        let mut file = file;
        file.write_all(&serde_json::to_vec_pretty(profile)?)?;
        file.sync_all()?;
        drop(file);
        crate::file_ops::commit_new(&pending, &dir.join(format!("{id}.json")))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&pending);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::AtomicBool, mpsc, Arc};
    #[test]
    fn concurrent_profile_publication_retains_one_complete_winner() {
        let dir = std::env::temp_dir().join(format!(
            "aster-loader-publish-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&dir).unwrap();
        let canonical = dir.canonicalize().unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let handles: Vec<_> = (0..2)
            .map(|number| {
                let dir = dir.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    publish_profile(
                        &dir,
                        "same",
                        &serde_json::json!({"id":"same","winner":number}),
                    )
                })
            })
            .collect();
        assert_eq!(
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap().is_ok())
                .filter(|success| *success)
                .count(),
            1
        );
        let before = fs::read(dir.join("same.json")).unwrap();
        let profile: Value = serde_json::from_slice(&before).unwrap();
        assert!(profile["winner"] == 0 || profile["winner"] == 1);
        assert!(publish_profile(&dir, "same", &serde_json::json!({"replace":true})).is_err());
        assert_eq!(fs::read(dir.join("same.json")).unwrap(), before);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        if dir.canonicalize().unwrap() == canonical
            && canonical.parent() == std::env::temp_dir().canonicalize().ok().as_deref()
        {
            fs::remove_dir_all(&canonical).unwrap();
        }
    }
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
