//! Local resource pack metadata and Minecraft options, following ResourcePackManager.
use crate::file_ops::commit_new;
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Format(pub u32, pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compatibility {
    Compatible,
    TooOld,
    TooNew,
    Invalid,
    MissingPack,
    MissingGame,
}
impl Compatibility {
    pub fn warning(self) -> Option<&'static str> {
        match self {
            Self::Compatible => None,
            Self::TooOld => Some("资源包适用于较旧的游戏版本"),
            Self::TooNew => Some("资源包适用于较新的游戏版本"),
            Self::Invalid => Some("资源包格式信息无效"),
            Self::MissingPack => Some("缺少或无法读取 pack.mcmeta"),
            Self::MissingGame => Some("无法读取游戏资源包格式版本"),
        }
    }
}
#[derive(Clone)]
pub struct ContextInfo {
    pub directory: PathBuf,
    pub format: Option<Format>,
    pub modern_options: bool,
}
impl ContextInfo {
    pub fn from_config(config: &crate::Config) -> Result<Self> {
        let profile = crate::metadata::resolve(Path::new(&config.game_dir), &config.selected)?;
        let base = profile["_clientVersion"]
            .as_str()
            .context("缺少基础游戏版本")?;
        let jar = crate::game::version_dir(Path::new(&config.game_dir), base)?
            .join(format!("{base}.jar"));
        let version = game_generation(base);
        if version.0 == Some(1) && version.1 == 0 {
            bail!("此游戏版本早于资源包支持版本（1.6.1 / 13w24a）");
        }
        let format = if version.0 == Some(1) {
            Some(Format(version.1, 0))
        } else {
            read_zip(&jar, "version.json", 1024 * 1024)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .and_then(|json| game_format(&json))
        };
        Ok(Self {
            directory: crate::game::instance_dir(config)?.join("resourcepacks"),
            format,
            modern_options: version.2,
        })
    }
}
// Old releases do not carry version.json; snapshots use HMCL's own thresholds.
fn game_generation(version: &str) -> (Option<u32>, u32, bool) {
    if version.len() == 6 && version.as_bytes()[2] == b'w' {
        let old = ["13w24a", "15w31a", "16w32a", "17w48a", "18w47b"];
        if version < "18w47b" {
            let format = old
                .iter()
                .filter(|threshold| version >= **threshold)
                .count() as u32;
            return (Some(1), format, version >= "17w43a");
        }
        return (None, 0, true);
    }
    let parts = version
        .split('.')
        .map(str::parse::<u32>)
        .collect::<std::result::Result<Vec<_>, _>>();
    if let Ok(parts) = parts {
        if parts.len() >= 2 && parts[0] == 1 {
            let minor = parts[1];
            if minor < 14 {
                return (
                    Some(1),
                    match minor {
                        0..=5 => 0,
                        6..=8 => 1,
                        9..=10 => 2,
                        11..=12 => 3,
                        13 => 4,
                        _ => 0,
                    },
                    minor >= 13,
                );
            }
        }
    }
    (None, 0, true)
}
fn game_format(json: &Value) -> Option<Format> {
    let info = &json["pack_version"];
    if let Some(major) = info["resource_major"].as_u64().filter(|v| *v > 64) {
        return Some(Format(
            u32::try_from(major).ok()?,
            u32::try_from(info["resource_minor"].as_u64().unwrap_or(0)).ok()?,
        ));
    }
    let major = info["resource"].as_u64().or_else(|| info.as_u64())?;
    Some(Format(u32::try_from(major).ok()?, 0))
}
fn format(value: &Value, max: bool) -> Option<Format> {
    let (major, minor) = if let Some(major) = value.as_u64() {
        (major, if max { u32::MAX as u64 } else { 0 })
    } else {
        let array = value.as_array()?;
        if !(1..=2).contains(&array.len()) {
            return None;
        }
        (
            array[0].as_u64()?,
            array.get(1).map(Value::as_u64).unwrap_or(Some(if max {
                u32::MAX as u64
            } else {
                0
            }))?,
        )
    };
    Some(Format(
        u32::try_from(major).ok()?,
        u32::try_from(minor).ok()?,
    ))
}
fn range(pack: &Value, required: Format) -> Option<(Format, Format)> {
    let pack_format = pack["pack_format"]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .filter(|v| *v > 0);
    let supported = match &pack["supported_formats"] {
        Value::Null => None,
        Value::Number(_) => {
            let v = format(&pack["supported_formats"], false)?.0;
            Some((v, v))
        }
        Value::Array(array) if array.len() == 2 => Some((
            u32::try_from(array[0].as_u64()?).ok()?,
            u32::try_from(array[1].as_u64()?).ok()?,
        )),
        Value::Object(object) => Some((
            u32::try_from(object.get("min_inclusive")?.as_u64()?).ok()?,
            u32::try_from(object.get("max_inclusive")?.as_u64()?).ok()?,
        )),
        _ => return None,
    };
    if required.0 <= 15 {
        let v = pack_format?;
        return Some((Format(v, 0), Format(v, 0)));
    }
    if required.0 <= 64 {
        let v = pack_format?;
        let (min, max) = supported.unwrap_or((v, v));
        return (min <= max).then_some((Format(min, 0), Format(max, 0)));
    }
    let min = format(&pack["min_format"], false);
    let max = format(&pack["max_format"], true);
    if let (Some(min), Some(max)) = (min, max) {
        if min > max {
            return None;
        }
        if min.0 > 64 {
            if supported.is_some() {
                return None;
            }
            if pack_format.is_some_and(|v| v < min.0 || v > max.0 || v < 15) {
                return None;
            }
        } else {
            let (old_min, old_max) = supported?;
            let v = pack_format?;
            if old_min != min.0
                || (old_max != max.0 && old_max != 64)
                || v < min.0
                || v > max.0
                || v < 15
            {
                return None;
            }
        }
        Some((min, max))
    } else {
        let v = pack_format?;
        let (min, max) = supported.unwrap_or((v, v));
        if min > max || max > 64 || v < min || v > max || (supported.is_some() && v < 15) {
            return None;
        }
        Some((Format(min, 0), Format(max, 0)))
    }
}
fn compatibility(meta: Option<&Value>, required: Option<Format>) -> Compatibility {
    let Some(pack) = meta
        .and_then(|meta| meta.get("pack"))
        .filter(|pack| pack.is_object())
    else {
        return Compatibility::MissingPack;
    };
    let Some(required) = required.filter(|v| v.0 > 0) else {
        return Compatibility::MissingGame;
    };
    let Some((min, max)) = range(pack, required) else {
        return Compatibility::Invalid;
    };
    if required < min {
        Compatibility::TooNew
    } else if required > max {
        Compatibility::TooOld
    } else {
        Compatibility::Compatible
    }
}
fn text(value: &Value, depth: usize) -> String {
    if depth > 128 {
        return String::new();
    }
    match value {
        Value::String(s) => description_text(s),
        Value::Number(_) | Value::Bool(_) => value.to_string(),
        Value::Array(values) => values.iter().map(|v| text(v, depth + 1)).collect(),
        Value::Object(_) => format!(
            "{}{}",
            match &value["text"] {
                Value::String(_) | Value::Number(_) | Value::Bool(_) =>
                    text(&value["text"], depth + 1),
                _ => String::new(),
            },
            if value["extra"].is_array() {
                text(&value["extra"], depth + 1)
            } else {
                String::new()
            }
        ),
        _ => String::new(),
    }
}
// StringUtils.parseColorEscapes / parseMinecraftColorCodes: only lowercase
// vanilla codes are stripped; unknown codes and a trailing section sign remain.
fn strip_color_codes(text: &str) -> String {
    let mut chars = text.chars().peekable();
    let mut result = String::with_capacity(text.len());
    while let Some(ch) = chars.next() {
        if ch == '§'
            && chars
                .peek()
                .is_some_and(|code| matches!(code, '0'..='9' | 'a'..='f' | 'k'..='o' | 'r'))
        {
            chars.next();
        } else {
            result.push(ch);
        }
    }
    result
}
fn description_text(text: &str) -> String {
    if text.trim().is_empty() {
        String::new()
    } else {
        strip_color_codes(text)
    }
}
fn single_line(text: &str) -> String {
    text.split(['\r', '\n'])
        .map(|line| line.trim_matches(|ch: char| ch <= '\u{0020}'))
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" | ")
}
fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let meta = fs::symlink_metadata(path)?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        bail!("不是普通文件：{}", path.display());
    }
    let mut bytes = vec![];
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        bail!("文件大小超出限制");
    }
    Ok(bytes)
}
fn read_zip(path: &Path, entry: &str, limit: u64) -> Result<Vec<u8>> {
    let mut zip = zip::ZipArchive::new(fs::File::open(path)?)?;
    let mut file = zip.by_name(entry)?;
    if file.size() > limit {
        bail!("资源包元数据大小超出限制");
    }
    let mut bytes = vec![];
    file.by_ref().take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        bail!("资源包元数据大小超出限制");
    }
    Ok(bytes)
}
#[derive(Clone)]
pub struct Pack {
    pub path: PathBuf,
    pub name: String,
    pub title: String,
    pub description: String,
    pub summary: String,
    pub compatibility: Compatibility,
    pub enabled: bool,
    pub icon: Option<image::RgbaImage>,
}
fn inspect(path: &Path, context: &ContextInfo, options: &Options) -> Result<Pack> {
    let attr = fs::symlink_metadata(path)?;
    if attr.file_type().is_symlink() {
        bail!("暂不支持链接资源包");
    }
    let read = |entry, limit| {
        if attr.is_dir() {
            let child = path.join(entry);
            if fs::canonicalize(&child)?.parent() != Some(fs::canonicalize(path)?.as_path()) {
                bail!("资源包元数据位于包目录外");
            }
            read_limited(&child, limit)
        } else {
            read_zip(path, entry, limit)
        }
    };
    let meta = read("pack.mcmeta", 1024 * 1024)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok());
    let compatibility = compatibility(meta.as_ref(), context.format);
    let description = meta
        .as_ref()
        .map(|meta| text(&meta["pack"]["description"], 0))
        .unwrap_or_default();
    let icon = read("pack.png", 4 * 1024 * 1024).ok().and_then(|bytes| {
        let mut reader =
            image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(2048);
        limits.max_image_height = Some(2048);
        limits.max_alloc = Some(32 * 1024 * 1024);
        reader.limits(limits);
        let rgba = reader.decode().ok()?.thumbnail(64, 64).to_rgba8();
        Some(rgba)
    });
    let name = path
        .file_name()
        .context("缺少资源包文件名")?
        .to_string_lossy()
        .into_owned();
    let enabled = contains(&options.enabled, &name)
        && contains(&options.incompatible, &name) == (compatibility != Compatibility::Compatible);
    Ok(Pack {
        path: path.into(),
        title: strip_color_codes(if attr.is_file() {
            name.rsplit_once('.')
                .map(|(title, _)| title)
                .unwrap_or(&name)
        } else {
            &name
        }),
        name,
        summary: single_line(&description),
        description,
        compatibility,
        enabled,
        icon,
    })
}
pub fn list(context: &ContextInfo) -> Result<Vec<Pack>> {
    if !context.directory.exists() {
        return Ok(vec![]);
    }
    let options = Options::load(context.directory.parent().context("缺少游戏目录")?)?;
    let mut packs = vec![];
    for entry in fs::read_dir(&context.directory)? {
        let path = entry?.path();
        if path.file_name().is_some_and(|name| {
            name.to_string_lossy()
                .starts_with(".rust-hmcl-resourcepack-folder-")
        }) {
            continue;
        }
        let attr = fs::symlink_metadata(&path)?;
        if attr.file_type().is_symlink() {
            continue;
        }
        if (attr.is_dir() && path.join("pack.mcmeta").is_file())
            || (attr.is_file()
                && path
                    .extension()
                    .is_some_and(|v| v.eq_ignore_ascii_case("zip")))
        {
            packs.push(inspect(&path, context, &options)?);
        }
    }
    packs.sort_by_key(|pack| pack.name.to_lowercase());
    Ok(packs)
}
fn contains(list: &[String], name: &str) -> bool {
    list.iter()
        .any(|id| id == name || id == &format!("file/{name}"))
}
fn parse_list(bytes: &[u8]) -> Result<Vec<String>> {
    let text = if let Ok(text) = std::str::from_utf8(bytes) {
        std::borrow::Cow::Borrowed(text)
    } else {
        let (text, _, errors) = encoding_rs::GB18030.decode(bytes);
        if errors {
            bail!("资源包设置编码无法读取");
        }
        text
    };
    serde_json::from_str(&text).context("资源包列表设置不是有效的 JSON 数组，未改动文件")
}
fn ascii_json(value: &[String]) -> Result<Vec<u8>> {
    let mut output = String::new();
    for ch in serde_json::to_string(value)?.chars() {
        if ch.is_ascii() {
            output.push(ch);
        } else {
            let mut units = [0u16; 2];
            for unit in ch.encode_utf16(&mut units) {
                output.push_str(&format!("\\u{unit:04x}"));
            }
        }
    }
    Ok(output.into_bytes())
}
struct Options {
    path: PathBuf,
    original: Option<Vec<u8>>,
    enabled: Vec<String>,
    incompatible: Vec<String>,
}
impl Options {
    fn load(directory: &Path) -> Result<Self> {
        let path = directory.join("options.txt");
        let original = match fs::symlink_metadata(&path) {
            Ok(_) => Some(read_limited(&path, 4 * 1024 * 1024)?),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let mut enabled = None;
        let mut incompatible = None;
        for line in original
            .as_deref()
            .unwrap_or_default()
            .split(|v| *v == b'\n')
        {
            let line = line.strip_suffix(b"\r").unwrap_or(line);
            for (key, target) in [
                (b"resourcePacks:".as_slice(), &mut enabled),
                (b"incompatibleResourcePacks:".as_slice(), &mut incompatible),
            ] {
                if let Some(value) = line.strip_prefix(key) {
                    if target.is_some() {
                        bail!("资源包设置项重复，未改动文件");
                    }
                    *target = Some(parse_list(value)?);
                }
            }
        }
        Ok(Self {
            path,
            original,
            enabled: enabled.unwrap_or_default(),
            incompatible: incompatible.unwrap_or_default(),
        })
    }
    fn bytes(&self) -> Result<Vec<u8>> {
        let original = self.original.as_deref().unwrap_or_default();
        let newline = if original.windows(2).any(|v| v == b"\r\n") {
            b"\r\n".as_slice()
        } else {
            b"\n".as_slice()
        };
        let replacements = [
            (b"resourcePacks:".as_slice(), ascii_json(&self.enabled)?),
            (
                b"incompatibleResourcePacks:".as_slice(),
                ascii_json(&self.incompatible)?,
            ),
        ];
        let mut found = [false; 2];
        let mut output = vec![];
        for segment in original.split_inclusive(|v| *v == b'\n') {
            let mut replaced = false;
            for (i, (key, value)) in replacements.iter().enumerate() {
                if segment.starts_with(key) {
                    output.extend_from_slice(key);
                    output.extend_from_slice(value);
                    if segment.ends_with(b"\r\n") {
                        output.extend_from_slice(b"\r\n");
                    } else if segment.ends_with(b"\n") {
                        output.push(b'\n');
                    }
                    found[i] = true;
                    replaced = true;
                    break;
                }
            }
            if !replaced {
                output.extend_from_slice(segment);
            }
        }
        for (i, (key, value)) in replacements.iter().enumerate() {
            if !found[i] {
                if !output.is_empty() && !output.ends_with(b"\n") {
                    output.extend_from_slice(newline);
                }
                output.extend_from_slice(key);
                output.extend_from_slice(value);
                output.extend_from_slice(newline);
            }
        }
        Ok(output)
    }
    fn save(&self) -> Result<()> {
        let bytes = self.bytes()?;
        if self.original.is_some() && fs::metadata(&self.path)?.permissions().readonly() {
            bail!("游戏设置文件为只读，未改动文件");
        }
        fs::create_dir_all(self.path.parent().context("缺少游戏目录")?)?;
        let temp = temporary(self.path.parent().unwrap(), "options");
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        let result = (|| -> Result<()> {
            output.write_all(&bytes)?;
            output.sync_all()?;
            drop(output);
            let current = match fs::symlink_metadata(&self.path) {
                Ok(_) => Some(read_limited(&self.path, 4 * 1024 * 1024)?),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(error.into()),
            };
            if current != self.original {
                bail!("游戏设置在操作期间发生改变，请刷新后重试");
            }
            if self.original.is_some() {
                fs::set_permissions(&temp, fs::metadata(&self.path)?.permissions())?;
                fs::rename(&temp, &self.path)?;
            } else {
                commit_new(&temp, &self.path)?;
            }
            Ok(())
        })();
        if temp.exists() {
            let _ = fs::remove_file(temp);
        }
        result
    }
}
fn temporary(directory: &Path, kind: &str) -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    directory.join(format!(
        ".rust-hmcl-{kind}-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}
pub fn set_enabled(context: &ContextInfo, names: &[String], enable: bool) -> Result<()> {
    let mut options = Options::load(context.directory.parent().context("缺少游戏目录")?)?;
    let mut selected = vec![];
    for name in names {
        if Path::new(name).file_name().and_then(|v| v.to_str()) != Some(name.as_str())
            || name.contains(['/', '\\', ':'])
        {
            bail!("资源包名称无效");
        }
        selected.push(inspect(&context.directory.join(name), context, &options)?);
    }
    for pack in selected {
        let id = if context.modern_options {
            format!("file/{}", pack.name)
        } else {
            pack.name.clone()
        };
        if enable {
            if !contains(&options.enabled, &pack.name) {
                options.enabled.push(id.clone());
            }
            if pack.compatibility != Compatibility::Compatible {
                if !contains(&options.incompatible, &pack.name) {
                    options.incompatible.push(id);
                }
            } else {
                options
                    .incompatible
                    .retain(|v| v != &pack.name && v != &format!("file/{}", pack.name));
            }
        } else {
            for values in [&mut options.enabled, &mut options.incompatible] {
                values.retain(|v| v != &pack.name && v != &format!("file/{}", pack.name));
            }
        }
    }
    options.save()
}
pub fn import(context: &ContextInfo, source: &Path) -> Result<()> {
    let attributes = fs::symlink_metadata(source)?;
    if attributes.file_type().is_symlink() {
        bail!("暂不支持链接资源包");
    }
    if attributes.is_dir() {
        return import_directory(context, source);
    }
    if !source
        .extension()
        .is_some_and(|v| v.eq_ignore_ascii_case("zip"))
    {
        bail!("请选择 ZIP 资源包");
    }
    if !fs::symlink_metadata(source)?.is_file() {
        bail!("资源包不是普通文件");
    }
    fs::create_dir_all(&context.directory)?;
    let name = source.file_name().context("缺少文件名")?;
    let destination = context.directory.join(name);
    if fs::symlink_metadata(&destination).is_ok() {
        bail!("同名资源包已存在，不会覆盖");
    }
    let temp = temporary(&context.directory, "resourcepack");
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)?;
    let result = (|| -> Result<()> {
        std::io::copy(&mut fs::File::open(source)?, &mut output)?;
        output.sync_all()?;
        drop(output);
        // HMCL permits valid ZIPs with missing metadata, then shows a warning.
        zip::ZipArchive::new(fs::File::open(&temp)?).context("资源包不是可读取的 ZIP")?;
        commit_new(&temp, &destination).context("无法提交资源包，目标未被覆盖")?;
        Ok(())
    })();
    if temp.exists() {
        let _ = fs::remove_file(temp);
    }
    result
}

fn future_path(path: &Path) -> Result<PathBuf> {
    let mut ancestor = std::path::absolute(path)?;
    let mut tail = vec![];
    while !ancestor.exists() {
        tail.push(
            ancestor
                .file_name()
                .context("目标路径没有已存在的父目录")?
                .to_os_string(),
        );
        if !ancestor.pop() {
            bail!("目标路径无效");
        }
    }
    let mut result = fs::canonicalize(ancestor)?;
    for name in tail.into_iter().rev() {
        result.push(name);
    }
    Ok(result)
}
fn cleanup_directory(stage: &Path, parent: &Path) -> Result<()> {
    let stage = fs::canonicalize(stage)?;
    let parent = fs::canonicalize(parent)?;
    if stage.parent() != Some(parent.as_path())
        || !stage.file_name().is_some_and(|name| {
            name.to_string_lossy()
                .starts_with(".rust-hmcl-resourcepack-folder-")
        })
    {
        bail!("未清理不属于导入任务的目录");
    }
    #[cfg(target_os = "windows")]
    #[allow(clippy::permissions_set_readonly_false)] // Windows attribute, never Unix modes.
    for entry in walkdir::WalkDir::new(&stage).follow_links(false) {
        let entry = entry?;
        if entry.file_type().is_file() {
            let mut permissions = fs::symlink_metadata(entry.path())?.permissions();
            if permissions.readonly() {
                permissions.set_readonly(false);
                fs::set_permissions(entry.path(), permissions)?;
            }
        }
    }
    fs::remove_dir_all(stage)?;
    Ok(())
}
fn import_directory(context: &ContextInfo, source: &Path) -> Result<()> {
    read_limited(&source.join("pack.mcmeta"), 1024 * 1024)
        .context("文件夹根目录必须包含普通 pack.mcmeta 文件")?;
    let source = fs::canonicalize(source)?;
    if future_path(&context.directory)?.starts_with(&source) {
        bail!("导入目标位于资源包内部，无法递归复制");
    }
    let destination = context
        .directory
        .join(source.file_name().context("缺少文件夹名称")?);
    if fs::symlink_metadata(&destination).is_ok() {
        bail!("同名资源包已存在，不会覆盖");
    }
    fs::create_dir_all(&context.directory)?;
    let stage = temporary(&context.directory, "resourcepack-folder");
    fs::create_dir(&stage)?;
    let result = (|| -> Result<()> {
        for entry in walkdir::WalkDir::new(&source)
            .follow_links(false)
            .into_iter()
            .skip(1)
        {
            let entry = entry?;
            let attributes = fs::symlink_metadata(entry.path())?;
            if attributes.file_type().is_symlink()
                || !entry.path().canonicalize()?.starts_with(&source)
            {
                bail!("资源包包含链接文件，原文件夹保持不变");
            }
            let target = stage.join(entry.path().strip_prefix(&source)?);
            if attributes.is_dir() {
                fs::create_dir(&target)?;
            } else if attributes.is_file() {
                let mut input = fs::File::open(entry.path())?;
                let mut output = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&target)?;
                std::io::copy(&mut input, &mut output)?;
                output.sync_all()?;
                drop(output);
                fs::set_permissions(&target, attributes.permissions())?;
            } else {
                bail!("资源包包含特殊文件，不能导入");
            }
        }
        commit_new(&stage, &destination)?;
        Ok(())
    })();
    if result.is_err() && stage.exists() {
        cleanup_directory(&stage, &context.directory)
            .context("导入未完成；临时目录清理失败，保留供检查")?;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static SEQUENCE: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "rust-hmcl-resourcepacks-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn context(&self, major: u32, modern: bool) -> ContextInfo {
            ContextInfo {
                directory: self.0.join("game/resourcepacks"),
                format: Some(Format(major, 0)),
                modern_options: modern,
            }
        }
        fn zip(&self, name: &str, meta: Value) -> PathBuf {
            let path = self.0.join(name);
            let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
            writer
                .start_file("pack.mcmeta", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer
                .write_all(&serde_json::to_vec(&meta).unwrap())
                .unwrap();
            writer
                .start_file(
                    "../not-extracted.txt",
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
            writer.write_all(b"archive content only").unwrap();
            writer.finish().unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let root = fs::canonicalize(&self.0).unwrap();
            assert_eq!(
                root.parent(),
                Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
            );
            assert!(root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("rust-hmcl-resourcepacks-"));
            fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn directory_import_preserves_nested_and_empty_content_and_enable_identity() {
        let fixture = Fixture::new();
        let context = fixture.context(15, true);
        let source = fixture.0.join("folder-resource");
        fs::create_dir_all(source.join("assets/minecraft/textures")).unwrap();
        fs::create_dir(source.join("empty")).unwrap();
        fs::write(
            source.join("pack.mcmeta"),
            br#"{"pack":{"pack_format":15,"description":"folder fixture"}}"#,
        )
        .unwrap();
        fs::write(
            source.join("assets/minecraft/textures/sample.png"),
            b"byte exact asset fixture",
        )
        .unwrap();
        import(&context, &source).unwrap();
        let destination = context.directory.join("folder-resource");
        assert!(destination.join("empty").is_dir());
        for name in ["pack.mcmeta", "assets/minecraft/textures/sample.png"] {
            assert_eq!(
                fs::read(source.join(name)).unwrap(),
                fs::read(destination.join(name)).unwrap()
            );
        }
        assert!(import(&context, &source).is_err());
        assert_eq!(fs::read_dir(&context.directory).unwrap().count(), 1);
        set_enabled(&context, &["folder-resource".into()], true).unwrap();
        assert!(list(&context).unwrap()[0].enabled);
        assert!(Options::load(context.directory.parent().unwrap())
            .unwrap()
            .enabled
            .contains(&"file/folder-resource".into()));
    }
    #[test]
    fn invalid_recursive_and_existing_directory_imports_preserve_their_targets() {
        let fixture = Fixture::new();
        let context = fixture.context(15, true);
        let source = fixture.0.join("folder-resource");
        fs::create_dir(&source).unwrap();
        assert!(import(&context, &source).is_err());
        assert!(!context.directory.exists());
        fs::write(
            source.join("pack.mcmeta"),
            b"invalid JSON is still a folder pack with a warning",
        )
        .unwrap();
        let recursive = ContextInfo {
            directory: source.join("resourcepacks"),
            ..context.clone()
        };
        assert!(import(&recursive, &source).is_err());
        assert!(!recursive.directory.exists());
        fs::create_dir_all(context.directory.join("folder-resource")).unwrap();
        assert!(import(&context, &source).is_err());
        assert_eq!(
            fs::read_dir(context.directory.join("folder-resource"))
                .unwrap()
                .count(),
            0
        );
        let staged = fixture.0.join("staged-file");
        let existing = fixture.0.join("existing-file");
        fs::write(&staged, b"new").unwrap();
        fs::write(&existing, b"original").unwrap();
        assert!(commit_new(&staged, &existing).is_err());
        assert_eq!(fs::read(&existing).unwrap(), b"original");
        assert_eq!(fs::read(&staged).unwrap(), b"new");
        let staged_dir = fixture.0.join("staged-directory");
        let existing_dir = fixture.0.join("existing-directory");
        fs::create_dir(&staged_dir).unwrap();
        fs::write(staged_dir.join("file"), b"new").unwrap();
        fs::create_dir(&existing_dir).unwrap();
        assert!(commit_new(&staged_dir, &existing_dir).is_err());
        assert!(staged_dir.join("file").exists());
        assert_eq!(fs::read_dir(existing_dir).unwrap().count(), 0);
    }
    #[test]
    fn compatibility_matches_hmcl_legacy_supported_and_new_minor_ranges() {
        assert_eq!(game_generation("1.12.2"), (Some(1), 3, false));
        assert_eq!(game_generation("17w43a"), (Some(1), 3, true));
        assert_eq!(
            game_format(&json!({"pack_version":{"resource":15}})),
            Some(Format(15, 0))
        );
        assert_eq!(game_format(&json!({"pack_version":4})), Some(Format(4, 0)));
        assert_eq!(
            game_format(
                &json!({"pack_version":{"resource":64,"resource_major":72,"resource_minor":1}})
            ),
            Some(Format(72, 1))
        );
        let old = json!({"pack":{"pack_format":15,"supported_formats":[15,32]}});
        assert_eq!(
            compatibility(Some(&old), Some(Format(14, 0))),
            Compatibility::TooNew
        );
        assert_eq!(
            compatibility(Some(&old), Some(Format(32, 0))),
            Compatibility::Compatible
        );
        assert_eq!(
            compatibility(Some(&old), Some(Format(33, 0))),
            Compatibility::TooOld
        );
        let new = json!({"pack":{"min_format":[72,1],"max_format":73}});
        assert_eq!(
            compatibility(Some(&new), Some(Format(72, 0))),
            Compatibility::TooNew
        );
        assert_eq!(
            compatibility(Some(&new), Some(Format(73, 5))),
            Compatibility::Compatible
        );
        assert_eq!(
            compatibility(Some(&new), Some(Format(74, 0))),
            Compatibility::TooOld
        );
        let invalid = json!({"pack":{"min_format":73,"max_format":72}});
        assert_eq!(
            compatibility(Some(&invalid), Some(Format(73, 0))),
            Compatibility::Invalid
        );
        assert_eq!(
            compatibility(None, Some(Format(73, 0))),
            Compatibility::MissingPack
        );
        assert_eq!(compatibility(Some(&old), None), Compatibility::MissingGame);
    }
    #[test]
    fn descriptions_match_hmcl_components_codes_and_single_line_text() {
        let description = json!([
            {"text":"§a你好§r\r\n  第二行  ", "extra":[{"text":7}, true, null]},
            "\n  \n§l末尾§z保留§A大写§",
            {"text":{}, "extra":{"text":"ignored"}},
            {"translate":"unsupported", "extra":[{"text":"附加"}]}
        ]);
        let result = text(&description, 0);
        assert_eq!(result, "你好\r\n  第二行  7true\n  \n末尾§z保留§A大写§附加");
        assert_eq!(
            single_line(&result),
            "你好 | 第二行  7true | 末尾§z保留§A大写§附加"
        );
        assert_eq!(text(&json!(" \r\n\t"), 0), "");
        let fixture = Fixture::new();
        let context = fixture.context(15, true);
        let source = fixture.zip(
            "§b颜色.zip",
            json!({"pack":{"pack_format":15,"description":description}}),
        );
        import(&context, &source).unwrap();
        let pack = list(&context).unwrap().remove(0);
        assert_eq!(pack.title, "颜色");
        assert_eq!(pack.name, "§b颜色.zip");
        assert_eq!(pack.description, result);
        assert_eq!(pack.summary, single_line(&result));
        assert_eq!(fs::read(&source).unwrap(), fs::read(&pack.path).unwrap());
    }
    #[test]
    fn zip_import_retains_bytes_never_extracts_and_refuses_overwrite() {
        let fixture = Fixture::new();
        let context = fixture.context(15, true);
        let source=fixture.zip("资源包.zip",json!({"pack":{"pack_format":15,"description":{"text":"你好","extra":[{"text":" 世界"}]}}}));
        import(&context, &source).unwrap();
        assert_eq!(
            fs::read(&source).unwrap(),
            fs::read(context.directory.join("资源包.zip")).unwrap()
        );
        assert!(!context
            .directory
            .parent()
            .unwrap()
            .join("not-extracted.txt")
            .exists());
        let pack = list(&context).unwrap().remove(0);
        assert_eq!(pack.description, "你好 世界");
        assert_eq!(pack.compatibility, Compatibility::Compatible);
        assert!(!pack.enabled);
        assert!(import(&context, &source).is_err());
        let no_meta = fixture.zip("missing.zip", json!({}));
        import(&context, &no_meta).unwrap();
        assert!(list(&context)
            .unwrap()
            .iter()
            .any(|pack| pack.compatibility == Compatibility::MissingPack));
        let corrupt = fixture.0.join("corrupt.zip");
        fs::write(&corrupt, b"not a zip").unwrap();
        assert!(import(&context, &corrupt).is_err());
        assert!(!context.directory.join("corrupt.zip").exists());
    }
    #[test]
    fn enabling_preserves_unknown_bytes_line_endings_order_and_incompatible_ids() {
        let fixture = Fixture::new();
        let context = fixture.context(15, true);
        let source = fixture.zip(
            "中文😀.zip",
            json!({"pack":{"pack_format":14,"description":"older"}}),
        );
        import(&context, &source).unwrap();
        let options = context.directory.parent().unwrap().join("options.txt");
        let original=b"fov:0.5\r\nunknown:\xff\r\nresourcePacks:[\"vanilla\",\"file/existing.zip\"]\r\nincompatibleResourcePacks:[]\r\nlast:keep";
        fs::write(&options, original).unwrap();
        set_enabled(&context, &["中文😀.zip".into()], true).unwrap();
        let loaded = Options::load(options.parent().unwrap()).unwrap();
        assert_eq!(
            loaded.enabled,
            vec!["vanilla", "file/existing.zip", "file/中文😀.zip"]
        );
        assert_eq!(loaded.incompatible, vec!["file/中文😀.zip"]);
        let bytes = fs::read(&options).unwrap();
        assert!(bytes.starts_with(b"fov:0.5\r\nunknown:\xff\r\n"));
        assert!(bytes.ends_with(b"last:keep"));
        assert!(bytes.windows(12).any(|v| v == b"\\ud83d\\ude00"));
        assert!(list(&context).unwrap()[0].enabled);
        set_enabled(&context, &["中文😀.zip".into()], false).unwrap();
        assert_eq!(
            Options::load(options.parent().unwrap()).unwrap().enabled,
            vec!["vanilla", "file/existing.zip"]
        );
        assert!(!list(&context).unwrap()[0].enabled);
        assert_eq!(
            fs::read(source).unwrap(),
            fs::read(context.directory.join("中文😀.zip")).unwrap()
        );
    }
    #[test]
    fn folder_packs_and_legacy_options_use_unprefixed_names() {
        let fixture = Fixture::new();
        let context = fixture.context(3, false);
        let folder = context.directory.join("folder-pack");
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("pack.mcmeta"),
            br#"{"pack":{"pack_format":3,"description":"folder"}}"#,
        )
        .unwrap();
        set_enabled(&context, &["folder-pack".into()], true).unwrap();
        let options = Options::load(context.directory.parent().unwrap()).unwrap();
        assert_eq!(options.enabled, vec!["folder-pack"]);
        assert!(list(&context).unwrap()[0].enabled);
        assert!(set_enabled(&context, &["../outside".into()], true).is_err());
    }
    #[test]
    fn malformed_duplicate_and_concurrent_options_are_not_overwritten() {
        let fixture = Fixture::new();
        let game = fixture.0.join("game");
        fs::create_dir(&game).unwrap();
        let path = game.join("options.txt");
        for original in [
            b"resourcePacks:broken".as_slice(),
            b"resourcePacks:[]\nresourcePacks:[]".as_slice(),
        ] {
            fs::write(&path, original).unwrap();
            assert!(Options::load(&game).is_err());
            assert_eq!(fs::read(&path).unwrap(), original);
        }
        fs::write(&path, b"resourcePacks:[]\nfov:1\n").unwrap();
        let options = Options::load(&game).unwrap();
        fs::write(&path, b"resourcePacks:[]\nfov:2\n").unwrap();
        assert!(options.save().is_err());
        assert_eq!(fs::read(&path).unwrap(), b"resourcePacks:[]\nfov:2\n");
        assert_eq!(fs::read_dir(&game).unwrap().count(), 1);
    }
}
