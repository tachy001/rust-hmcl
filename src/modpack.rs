use crate::{
    game::{Event, Worker},
    loaders::Loader,
};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::Read,
    path::Path,
};

#[derive(Clone, Deserialize)]
pub struct PackIndex {
    #[serde(rename = "formatVersion")]
    pub format: u32,
    pub game: String,
    pub name: String,
    pub dependencies: BTreeMap<String, String>,
    pub files: Vec<PackFile>,
}
#[derive(Clone, Deserialize)]
pub struct PackFile {
    pub path: String,
    pub hashes: BTreeMap<String, String>,
    pub downloads: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}
impl PackIndex {
    pub fn validate(&self) -> Result<()> {
        if self.format != 1 || self.game != "minecraft" {
            bail!("只支持 formatVersion=1 的 Minecraft .mrpack");
        }
        if !self.dependencies.contains_key("minecraft") {
            bail!("整合包没有指定 Minecraft 版本");
        }
        if self.dependencies.contains_key("fabric-loader")
            && self.dependencies.contains_key("quilt-loader")
        {
            bail!("整合包不能同时声明 Fabric 和 Quilt");
        }
        for key in self.dependencies.keys() {
            if !["minecraft", "fabric-loader", "quilt-loader"].contains(&key.as_str()) {
                bail!("暂不支持整合包依赖 {key}；当前支持原版、Fabric 和 Quilt");
            }
        }
        let mut names = std::collections::HashSet::new();
        for file in &self.files {
            let safe = crate::game::safe_join(Path::new("game"), &file.path)?;
            if !names.insert(safe.to_string_lossy().to_lowercase()) {
                bail!("整合包中存在重复文件路径: {}", file.path);
            }
            let sha = file
                .hashes
                .get("sha1")
                .context("整合包文件缺少 SHA-1 校验值")?;
            if sha.len() != 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
                bail!("无效整合包文件哈希");
            }
            if file.downloads.is_empty() {
                bail!("整合包文件缺少下载地址");
            }
            for url in &file.downloads {
                validate_url(url)?;
            }
        }
        Ok(())
    }
}
pub(crate) fn validate_url(url: &str) -> Result<()> {
    let url = reqwest::Url::parse(url)?;
    if url.scheme() != "https"
        || !matches!(
            url.host_str(),
            Some("cdn.modrinth.com" | "github.com" | "raw.githubusercontent.com" | "gitlab.com")
        )
    {
        bail!("整合包下载地址不在支持的来源范围内");
    }
    Ok(())
}
pub fn inspect(path: &Path) -> Result<PackIndex> {
    let mut zip = zip::ZipArchive::new(File::open(path)?)?;
    let entry = zip
        .by_name("modrinth.index.json")
        .context("未找到 modrinth.index.json")?;
    if entry.size() > 8 * 1024 * 1024 {
        bail!("整合包索引过大");
    }
    let mut contents = String::new();
    entry.take(8 * 1024 * 1024).read_to_string(&mut contents)?;
    let index: PackIndex = serde_json::from_str(&contents)?;
    index.validate()?;
    Ok(index)
}
impl Worker {
    pub fn import_pack(&self, root: &Path, path: &Path, id: &str, optional: bool) -> Result<()> {
        let index = inspect(path)?;
        let dir = crate::game::version_dir(root, id)?;
        let source_hash = crate::game::sha1_file(path)?;
        let marker = dir.join(".rust-pack-source");
        if dir.exists()
            && (dir.join(format!("{id}.json")).exists()
                || fs::read_to_string(&marker).ok().as_deref() != Some(&source_hash))
        {
            bail!("实例目录 {id} 已存在，请使用新名称；不会覆盖已有文件");
        }
        fs::create_dir_all(&dir)?;
        fs::write(&marker, &source_hash)?;
        let game = index.dependencies.get("minecraft").unwrap();
        self.ensure_base(root, game)?;
        let parent = crate::metadata::resolve(root, game)?;
        let mut profile = if let Some(version) = index.dependencies.get("fabric-loader") {
            self.loader_profile(game, Loader::Fabric, Some(version))?
        } else if let Some(version) = index.dependencies.get("quilt-loader") {
            self.loader_profile(game, Loader::Quilt, Some(version))?
        } else {
            json!({"id":id,"inheritsFrom":game})
        };
        profile["id"] = json!(id);
        profile["rustHmclPack"] = json!({"name":index.name});
        self.prepare(root, id, &crate::metadata::merge(&parent, &profile)?)?;
        let staging = dir.join("pack-staging");
        fs::create_dir_all(&staging)?;
        for (i, file) in index.files.iter().enumerate() {
            self.check()?;
            if file
                .env
                .get("client")
                .is_some_and(|s| s == "unsupported" || (!optional && s == "optional"))
            {
                continue;
            }
            let destination = crate::game::safe_join(&staging, &file.path)?;
            let mut result = Err(anyhow::anyhow!("没有下载地址"));
            for url in &file.downloads {
                result = self.download(
                    url,
                    &destination,
                    file.hashes.get("sha1").map(String::as_str),
                );
                if result.is_ok() {
                    break;
                }
                self.check()?;
            }
            result.with_context(|| format!("整合包文件下载失败: {}", file.path))?;
            let _ = self.tx.send(Event::Progress(
                (i + 1) as f32 / index.files.len().max(1) as f32,
                format!("整合包文件 {}/{}", i + 1, index.files.len()),
            ));
        }
        let mut zip = zip::ZipArchive::new(File::open(path)?)?;
        let mut extracted = 0u64;
        // Apply client overrides last, as required by the mrpack format.
        for prefix in ["overrides/", "client-overrides/"] {
            for i in 0..zip.len() {
                self.check()?;
                let mut entry = zip.by_index(i)?;
                let Some(relative) = entry.name().strip_prefix(prefix).map(str::to_owned) else {
                    continue;
                };
                if relative.is_empty() || entry.is_dir() {
                    continue;
                }
                if entry
                    .unix_mode()
                    .is_some_and(|mode| mode & 0o170000 == 0o120000)
                {
                    bail!("整合包 overrides 不能包含符号链接");
                }
                extracted = extracted
                    .checked_add(entry.size())
                    .context("整合包体积溢出")?;
                if extracted > 2 * 1024 * 1024 * 1024 {
                    bail!("整合包 overrides 超出 2 GB 限制");
                }
                let destination = crate::game::safe_join(&staging, &relative)?;
                fs::create_dir_all(destination.parent().unwrap())?;
                std::io::copy(&mut entry, &mut File::create(destination)?)?;
            }
        }
        self.check()?;
        for entry in walkdir::WalkDir::new(&staging) {
            self.check()?;
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let target = dir.join("game").join(entry.path().strip_prefix(&staging)?);
            fs::create_dir_all(target.parent().unwrap())?;
            fs::copy(entry.path(), target)?;
        }
        let pending = dir.join("pack.pending");
        fs::write(&pending, serde_json::to_vec_pretty(&profile)?)?;
        fs::rename(pending, dir.join(format!("{id}.json")))?;
        let _ = self.tx.send(Event::Installed(id.into()));
        self.status(format!("整合包 {} 导入完成", index.name));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn index() -> PackIndex {
        serde_json::from_value(json!({"formatVersion":1,"game":"minecraft","name":"Test","dependencies":{"minecraft":"1.20.1","fabric-loader":"0.16.0"},"files":[]})).unwrap()
    }
    #[test]
    fn unsupported_loader_is_rejected_before_install() {
        let mut i = index();
        i.dependencies.insert("forge".into(), "47.0".into());
        assert!(i.validate().is_err());
    }
    #[test]
    fn reject_traversal_and_untrusted_downloads() {
        let mut i = index();
        i.files.push(PackFile {
            path: "../escape".into(),
            hashes: BTreeMap::from([("sha1".into(), "a".repeat(40))]),
            downloads: vec!["https://cdn.modrinth.com/a.jar".into()],
            env: BTreeMap::new(),
        });
        assert!(i.validate().is_err());
        i.files[0].path = "mods/a.jar".into();
        assert!(i.validate().is_ok());
        i.files[0].downloads[0] = "http://127.0.0.1/a.jar".into();
        assert!(i.validate().is_err());
    }
    #[test]
    fn inspect_real_zip() {
        let root = std::env::temp_dir().join(format!("rust-hmcl-pack-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let file = root.join("test.mrpack");
        let mut zip = zip::ZipWriter::new(File::create(&file).unwrap());
        zip.start_file(
            "modrinth.index.json",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(br#"{"formatVersion":1,"game":"minecraft","name":"Fixture","dependencies":{"minecraft":"1.20.1"},"files":[]}"#).unwrap();
        zip.finish().unwrap();
        assert_eq!(inspect(&file).unwrap().name, "Fixture");
        fs::remove_dir_all(root).unwrap();
    }
}
