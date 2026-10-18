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
    #[serde(default, rename = "versionId")]
    pub version: String,
    #[serde(default)]
    pub summary: String,
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
        for value in self.dependencies.values() {
            crate::instance_ops::validate_name(value).context("整合包依赖版本不合法")?;
        }
        let mut names = std::collections::HashSet::new();
        for file in &self.files {
            if file.env.get("client").is_some_and(|value| {
                !["required", "optional", "unsupported"].contains(&value.as_str())
            }) {
                bail!("未知的整合包客户端文件标记");
            }
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
    let mut expanded = 0u64;
    let mut names = std::collections::HashSet::new();
    for i in 0..zip.len() {
        let entry = zip.by_index(i)?;
        let Some((prefix, relative)) =
            ["overrides/", "client-overrides/"]
                .into_iter()
                .find_map(|prefix| {
                    entry
                        .name()
                        .strip_prefix(prefix)
                        .map(|relative| (prefix, relative))
                })
        else {
            continue;
        };
        let relative = relative.trim_end_matches('/');
        if relative.is_empty() {
            continue;
        }
        crate::game::safe_join(Path::new("game"), relative)?;
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            bail!("整合包 overrides 不能包含符号链接");
        }
        if !names.insert(format!(
            "{prefix}{}",
            relative.replace('\\', "/").to_lowercase()
        )) {
            bail!("整合包 overrides 中存在重复路径");
        }
        expanded = expanded
            .checked_add(entry.size())
            .context("整合包体积溢出")?;
        if expanded > 2 * 1024 * 1024 * 1024 {
            bail!("整合包 overrides 超出 2 GB 限制");
        }
    }
    Ok(index)
}

#[derive(serde::Serialize)]
pub struct Preview {
    pub name: String,
    pub version: String,
    pub summary: String,
    pub minecraft: String,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub files: usize,
    pub required: usize,
    pub optional: usize,
    pub server_only: usize,
    pub source_sha1: String,
    pub suggested_name: String,
}
pub fn preview(root: &Path, path: &Path) -> Result<Preview> {
    if !path.is_absolute()
        || path
            .extension()
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("mrpack"))
    {
        bail!("请选择本机 .mrpack 文件");
    }
    let attributes = fs::symlink_metadata(path)?;
    if !attributes.is_file() || attributes.file_type().is_symlink() {
        bail!("请选择普通 .mrpack 文件，不支持链接");
    }
    let index = inspect(path)?;
    let mut base = String::new();
    for character in index.name.trim().chars() {
        if base.encode_utf16().count() + character.len_utf16() > 85 {
            break;
        }
        base.push(
            if character.is_control() || "<>:\"/\\|?*".contains(character) {
                '-'
            } else {
                character
            },
        );
    }
    base = base.trim_matches(['.', ' ']).into();
    if crate::instance_ops::validate_name(&base).is_err() {
        base = "My Modpack".into();
    }
    let mut suggested_name = base.clone();
    for suffix in 1..=1000 {
        if suggested_name != index.dependencies["minecraft"]
            && fs::symlink_metadata(crate::game::version_dir(root, &suggested_name)?).is_err()
        {
            break;
        }
        suggested_name = format!("{base}-{suffix}");
    }
    let loader_key = ["fabric-loader", "quilt-loader"]
        .into_iter()
        .find(|key| index.dependencies.contains_key(*key));
    let optional = index
        .files
        .iter()
        .filter(|file| {
            file.env
                .get("client")
                .is_some_and(|value| value == "optional")
        })
        .count();
    let server_only = index
        .files
        .iter()
        .filter(|file| {
            file.env
                .get("client")
                .is_some_and(|value| value == "unsupported")
        })
        .count();
    Ok(Preview {
        source_sha1: crate::game::sha1_file(path)?,
        suggested_name,
        minecraft: index.dependencies["minecraft"].clone(),
        loader: loader_key.map(|key| {
            if key == "fabric-loader" {
                "Fabric".into()
            } else {
                "Quilt".into()
            }
        }),
        loader_version: loader_key.map(|key| index.dependencies[key].clone()),
        files: index.files.len(),
        required: index.files.len() - optional - server_only,
        optional,
        server_only,
        name: index.name,
        version: index.version,
        summary: index.summary,
    })
}
pub fn validate_target(root: &Path, path: &Path, id: &str, expected_sha1: &str) -> Result<()> {
    crate::instance_ops::validate_name(id)?;
    let preview = preview(root, path)?;
    if preview.source_sha1 != expected_sha1 {
        bail!("整合包文件已改变，请重新选择并预览");
    }
    if id == preview.minecraft {
        bail!("整合包实例名称不能与基础 Minecraft 版本相同");
    }
    let dir = crate::game::version_dir(root, id)?;
    if let Ok(attributes) = fs::symlink_metadata(&dir) {
        if !attributes.is_dir()
            || attributes.file_type().is_symlink()
            || dir.join(format!("{id}.json")).exists()
            || fs::read_to_string(dir.join(".rust-pack-source"))
                .ok()
                .as_deref()
                != Some(expected_sha1)
        {
            bail!("实例目录 {id} 已存在，请使用新名称；不会覆盖已有文件");
        }
    }
    Ok(())
}
impl Worker {
    pub fn import_pack(&self, root: &Path, path: &Path, id: &str, optional: bool) -> Result<()> {
        self.import_pack_checked(root, &std::path::absolute(path)?, id, optional, None)
    }
    pub fn import_pack_checked(
        &self,
        root: &Path,
        path: &Path,
        id: &str,
        optional: bool,
        expected_sha1: Option<&str>,
    ) -> Result<()> {
        self.check()?;
        let index = inspect(path)?;
        let dir = crate::game::version_dir(root, id)?;
        let source_hash = crate::game::sha1_file(path)?;
        validate_target(root, path, id, expected_sha1.unwrap_or(&source_hash))?;
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
        if crate::game::sha1_file(path)? != source_hash {
            bail!("整合包文件在安装期间改变，请重新预览");
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
        crate::file_ops::commit_new(&pending, &dir.join(format!("{id}.json")))?;
        let cleanup = (|| -> Result<()> {
            let attributes = fs::symlink_metadata(&staging)?;
            if attributes.file_type().is_symlink() || !attributes.is_dir() {
                bail!("暂存路径不是普通文件夹");
            }
            let owned = fs::canonicalize(&dir)?.join("pack-staging");
            let resolved = fs::canonicalize(&staging)?;
            if owned != resolved {
                bail!("暂存路径超出当前实例");
            }
            fs::remove_dir_all(resolved)?;
            Ok(())
        })();
        if let Err(error) = cleanup {
            let _ = self.tx.send(Event::Log(format!(
                "整合包已安装，暂存目录清理未完成：{error:#}"
            )));
        }
        let _ = self.tx.send(Event::Installed(id.into()));
        self.status(format!("整合包 {} 导入完成", index.name));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn write_preview_fixture(path: &Path, override_name: Option<&str>) {
        let mut archive = zip::ZipWriter::new(File::create(path).unwrap());
        archive
            .start_file(
                "modrinth.index.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(br#"{"formatVersion":1,"game":"minecraft","name":"CON","dependencies":{"minecraft":"1.20.1"},"files":[]}"#).unwrap();
        if let Some(name) = override_name {
            archive
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            archive.write_all(b"override").unwrap();
        }
        archive.finish().unwrap();
    }
    #[test]
    fn preview_and_resume_bind_the_source_and_never_replace_existing_instances() {
        let root = std::env::temp_dir().join(format!("aster-pack-preview-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("fixture.mrpack");
        write_preview_fixture(&path, None);
        let data = preview(&root, &path).unwrap();
        assert_eq!(data.suggested_name, "My Modpack");
        assert_eq!(data.required, 0);
        assert!(!root.join("versions").exists());
        assert!(validate_target(&root, &path, "../escape", &data.source_sha1).is_err());
        assert!(validate_target(&root, &path, "1.20.1", &data.source_sha1).is_err());
        let dir = root.join("versions/resume");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("user.txt"), b"preserve").unwrap();
        assert!(validate_target(&root, &path, "resume", &data.source_sha1).is_err());
        fs::write(dir.join(".rust-pack-source"), &data.source_sha1).unwrap();
        assert!(validate_target(&root, &path, "resume", &data.source_sha1).is_ok());
        fs::write(dir.join("resume.json"), b"{}").unwrap();
        assert!(validate_target(&root, &path, "resume", &data.source_sha1).is_err());
        assert!(validate_target(&root, &path, "fresh", &"0".repeat(40)).is_err());
        assert_eq!(fs::read(dir.join("user.txt")).unwrap(), b"preserve");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unsafe_overrides_fail_before_any_installation_directory_is_created() {
        let root =
            std::env::temp_dir().join(format!("aster-pack-overrides-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let path = root.join("fixture.mrpack");
        write_preview_fixture(&path, Some("overrides/../../escape"));
        assert!(inspect(&path).is_err());
        let (tx, _rx) = std::sync::mpsc::channel();
        let worker = Worker::new(
            tx,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .unwrap();
        assert!(worker.import_pack(&root, &path, "fresh", false).is_err());
        assert!(!root.join("versions").exists());
        fs::remove_dir_all(root).unwrap();
    }
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
