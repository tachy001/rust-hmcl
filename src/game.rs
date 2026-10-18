use anyhow::{bail, Context, Result};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{BufRead, BufReader, Read, Write},
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::Sender,
        Arc,
    },
    time::{Duration, Instant},
};

const MANIFEST: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Version {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub url: String,
    pub sha1: String,
}
pub enum Event {
    GameStarted,
    Status(String),
    Progress(f32, String),
    Versions(Vec<Version>),
    Java(String),
    Runtimes(Vec<crate::java_runtime::Runtime>),
    InstanceChanged {
        old: String,
        new: String,
        renamed: bool,
    },
    Installed(String),
    Projects(Vec<crate::modrinth::Project>),
    Log(String),
    Finished(Result<(), String>),
}
#[derive(Clone)]
pub struct Worker {
    client: Client,
    pub tx: Sender<Event>,
    pub cancel: Arc<AtomicBool>,
    pub threads: usize,
}

pub fn os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "osx"
    } else {
        "linux"
    }
}
pub fn arch() -> &'static str {
    if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else {
        "x86_64"
    }
}
pub fn safe_join(root: &Path, name: &str) -> Result<PathBuf> {
    // Both separator styles must be checked, even when importing a Windows manifest on Unix.
    let normalized = name.replace('\\', "/");
    if normalized.is_empty()
        || normalized.contains(':')
        || normalized.starts_with('/')
        || normalized.split('/').any(|s| s == ".." || s.is_empty())
        || Path::new(&normalized)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        bail!("不安全的文件路径: {name}");
    }
    Ok(root.join(normalized))
}
pub fn version_dir(root: &Path, id: &str) -> Result<PathBuf> {
    if id.contains('/') || id.contains('\\') {
        bail!("版本名称不能包含路径分隔符");
    }
    safe_join(&root.join("versions"), id)
}
pub fn installed(root: &Path) -> Vec<String> {
    let mut list = fs::read_dir(root.join("versions"))
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().to_string();
            if e.path().join(format!("{id}.json")).is_file() {
                Some(id)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    list.sort();
    list
}
pub fn allowed(rules: Option<&Value>) -> bool {
    let Some(rules) = rules.and_then(Value::as_array) else {
        return true;
    };
    let mut allow = false;
    for rule in rules {
        let mut matches = true;
        if let Some(platform) = rule.get("os") {
            if let Some(name) = platform["name"].as_str() {
                matches &= name == os();
            }
            if let Some(a) = platform["arch"].as_str() {
                matches &= a == arch();
            }
            // Unknown OS-version predicates are not assumed to match.
            if platform.get("version").is_some() {
                matches = false;
            }
        }
        if let Some(features) = rule["features"].as_object() {
            // This launcher supports custom resolution, but no demo or quick-play mode.
            matches &= features
                .iter()
                .all(|(k, v)| v.as_bool() == Some(k == "has_custom_resolution"));
        }
        if matches {
            allow = rule["action"].as_str() == Some("allow");
        }
    }
    allow
}
fn str_field<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    value[name]
        .as_str()
        .with_context(|| format!("版本信息缺少 {name}"))
}
impl Worker {
    pub fn new(tx: Sender<Event>, cancel: Arc<AtomicBool>) -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .user_agent("Rust-HMCL/0.2")
                .connect_timeout(Duration::from_secs(20))
                .timeout(Duration::from_secs(180))
                .build()?,
            tx,
            cancel,
            threads: 8,
        })
    }
    pub fn check(&self) -> Result<()> {
        if self.cancel.load(Ordering::Relaxed) {
            bail!("操作已取消，可重新执行以继续下载");
        }
        Ok(())
    }
    pub fn status(&self, text: impl Into<String>) {
        let _ = self.tx.send(Event::Status(text.into()));
    }
    pub fn json(&self, url: &str) -> Result<Value> {
        self.check()?;
        Ok(self.client.get(url).send()?.error_for_status()?.json()?)
    }
    pub fn manifest(&self) -> Result<()> {
        self.status("正在获取 Minecraft 版本列表…");
        let cache = crate::config::data_dir().join("version-manifest.json");
        let json = match self.json(MANIFEST) {
            Ok(value) => {
                fs::create_dir_all(crate::config::data_dir())?;
                fs::write(&cache, serde_json::to_vec(&value)?)?;
                value
            }
            Err(error) => {
                self.check()?;
                if !cache.exists() {
                    return Err(error);
                }
                self.status("网络不可用，使用缓存的版本列表");
                serde_json::from_slice(&fs::read(cache)?)?
            }
        };
        let list: Vec<Version> = serde_json::from_value(json["versions"].clone())?;
        self.status(format!("已获取 {} 个 Minecraft 版本", list.len()));
        self.tx.send(Event::Versions(list))?;
        Ok(())
    }
    pub fn download(&self, url: &str, destination: &Path, hash: Option<&str>) -> Result<()> {
        self.check()?;
        if destination.is_file() && hash.is_some() && verify(destination, hash.unwrap())? {
            return Ok(());
        }
        let receipt = destination.with_file_name(format!(
            "{}.rust-sha1",
            destination
                .file_name()
                .context("缺少文件名")?
                .to_string_lossy()
        ));
        if hash.is_none() && destination.is_file() && receipt.is_file() {
            if let Ok(expected) = fs::read_to_string(&receipt) {
                if verify(destination, expected.trim())? {
                    return Ok(());
                }
            }
        }
        fs::create_dir_all(destination.parent().context("文件缺少父目录")?)?;
        let temp = destination.with_extension("part");
        let mut last_error = None;
        for attempt in 0..3 {
            self.check()?;
            let result = (|| -> Result<()> {
                let mut response = self.client.get(url).send()?.error_for_status()?;
                let total = response.content_length().unwrap_or(0);
                let started = Instant::now();
                let mut last_update = Instant::now();
                let mut received = 0u64;
                let mut output = File::create(&temp)?;
                let mut buf = [0u8; 65536];
                loop {
                    self.check()?;
                    let n = response.read(&mut buf)?;
                    if n == 0 {
                        break;
                    }
                    output.write_all(&buf[..n])?;
                    received += n as u64;
                    if total > 1024 * 1024 && last_update.elapsed() > Duration::from_millis(350) {
                        let speed =
                            received as f64 / started.elapsed().as_secs_f64().max(0.001) / 1048576.;
                        let _ = self.tx.send(Event::Progress(
                            received as f32 / total as f32,
                            format!(
                                "{} · {:.1}/{:.1} MB · {:.1} MB/s",
                                destination.file_name().unwrap().to_string_lossy(),
                                received as f64 / 1048576.,
                                total as f64 / 1048576.,
                                speed
                            ),
                        ));
                        last_update = Instant::now();
                    }
                }
                output.sync_all()?;
                drop(output);
                if let Some(hash) = hash {
                    if !verify(&temp, hash)? {
                        bail!("下载文件校验失败: {}", destination.display());
                    }
                }
                fs::rename(&temp, destination)?;
                if hash.is_none() {
                    fs::write(&receipt, sha1_file(destination)?)?;
                }
                Ok(())
            })();
            match result {
                Ok(()) => return Ok(()),
                Err(e) => {
                    last_error = Some(e);
                    self.check()?;
                    if attempt < 2 {
                        self.status(format!("下载失败，正在重试 ({}/2)", attempt + 1));
                    }
                }
            }
        }
        Err(last_error.unwrap()).with_context(|| format!("下载失败: {url}"))
    }
    fn artifact(&self, value: &Value, root: &Path) -> Result<PathBuf> {
        let path = safe_join(root, str_field(value, "path")?)?;
        self.download(str_field(value, "url")?, &path, value["sha1"].as_str())?;
        Ok(path)
    }
    pub fn install(&self, root: &Path, version: &Version) -> Result<()> {
        self.status(format!("安装 Minecraft {}", version.id));
        let dir = version_dir(root, &version.id)?;
        fs::create_dir_all(&dir)?;
        // Metadata is published only after all dependencies are installed.
        let pending = dir.join("manifest.pending");
        self.download(&version.url, &pending, Some(&version.sha1))?;
        let meta: Value = serde_json::from_slice(&fs::read(&pending)?)?;
        self.prepare(root, &version.id, &meta)?;
        fs::rename(pending, dir.join(format!("{}.json", version.id)))?;
        let _ = self.tx.send(Event::Installed(version.id.clone()));
        self.status("游戏安装完成");
        Ok(())
    }
    pub fn prepare(&self, root: &Path, id: &str, meta: &Value) -> Result<()> {
        let dir = version_dir(root, id)?;
        let client_id = meta["_clientVersion"].as_str().unwrap_or(id);
        let client_dir = version_dir(root, client_id)?;
        let jar = &meta["downloads"]["client"];
        self.status("下载游戏主文件…");
        self.download(
            str_field(jar, "url")?,
            &client_dir.join(format!("{client_id}.jar")),
            jar["sha1"].as_str(),
        )?;
        let libs = meta["libraries"].as_array().context("缺少依赖库列表")?;
        let native_dir = dir.join("rust-natives");
        fs::create_dir_all(&native_dir)?;
        for (i, lib) in libs.iter().enumerate() {
            self.check()?;
            let _ = self.tx.send(Event::Progress(
                i as f32 / libs.len().max(1) as f32,
                format!("依赖库 {}/{}", i + 1, libs.len()),
            ));
            if !allowed(lib.get("rules")) {
                continue;
            }
            if lib["downloads"]["artifact"].is_object() {
                self.artifact(&lib["downloads"]["artifact"], &root.join("libraries"))?;
            }
            if let Some(key) = lib["natives"][os()].as_str() {
                let key = key.replace(
                    "${arch}",
                    if cfg!(target_pointer_width = "64") {
                        "64"
                    } else {
                        "32"
                    },
                );
                let native = &lib["downloads"]["classifiers"][&key];
                if !native.is_object() {
                    bail!("本平台缺少 native 依赖: {key}");
                }
                let file = self.artifact(native, &root.join("libraries"))?;
                extract_natives(&file, &native_dir)?;
            }
        }
        let index = &meta["assetIndex"];
        let index_id = str_field(index, "id")?;
        let index_file = safe_join(&root.join("assets/indexes"), &format!("{index_id}.json"))?;
        self.status("下载资源索引…");
        self.download(
            str_field(index, "url")?,
            &index_file,
            index["sha1"].as_str(),
        )?;
        let assets: Value = serde_json::from_slice(&fs::read(index_file)?)?;
        let objects = assets["objects"].as_object().context("资源索引格式错误")?;
        let mut unique = std::collections::BTreeSet::new();
        for object in objects.values() {
            let hash = str_field(object, "hash")?;
            if hash.len() != 40 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
                bail!("无效资源哈希");
            }
            unique.insert(hash.to_owned());
        }
        let hashes = unique.into_iter().collect::<Vec<_>>();
        let next = AtomicUsize::new(0);
        let completed = AtomicUsize::new(0);
        let failed = AtomicBool::new(false);
        std::thread::scope(|scope| -> Result<()> {
            let mut handles = vec![];
            for _ in 0..self.threads.clamp(1, 32).min(hashes.len()) {
                handles.push(scope.spawn(|| -> Result<()> {
                    loop {
                        if failed.load(Ordering::Relaxed) {
                            break;
                        }
                        self.check()?;
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(hash) = hashes.get(i) else {
                            break;
                        };
                        let relative = format!("{}/{}", &hash[..2], hash);
                        if let Err(error) = self.download(
                            &format!("https://resources.download.minecraft.net/{relative}"),
                            &root.join("assets/objects").join(&relative),
                            Some(hash),
                        ) {
                            failed.store(true, Ordering::Relaxed);
                            return Err(error);
                        }
                        let count = completed.fetch_add(1, Ordering::Relaxed) + 1;
                        let _ = self.tx.send(Event::Progress(
                            count as f32 / hashes.len().max(1) as f32,
                            format!("游戏资源 {count}/{}", hashes.len()),
                        ));
                    }
                    Ok(())
                }));
            }
            let mut first_error = None;
            for handle in handles {
                match handle.join() {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        if first_error.is_none() {
                            first_error = Some(e);
                        }
                    }
                    Err(_) => {
                        first_error = Some(anyhow::anyhow!("资源下载线程异常"));
                    }
                }
            }
            if let Some(error) = first_error {
                return Err(error);
            }
            Ok(())
        })?;
        for (name, object) in objects {
            self.check()?;
            let hash = str_field(object, "hash")?;
            let file = root.join("assets/objects").join(&hash[..2]).join(hash);
            if assets["virtual"].as_bool() == Some(true) {
                let dest = safe_join(&safe_join(&root.join("assets/virtual"), index_id)?, name)?;
                fs::create_dir_all(dest.parent().unwrap())?;
                fs::copy(&file, dest)?;
            }
            if assets["map_to_resources"].as_bool() == Some(true) {
                let dest = safe_join(&root.join("resources"), name)?;
                fs::create_dir_all(dest.parent().unwrap())?;
                fs::copy(&file, dest)?;
            }
        }
        Ok(())
    }
    pub fn launch(&self, config: &crate::config::Config) -> Result<()> {
        let config = config.for_selected();
        config.validate()?;
        if config.selected.is_empty() {
            bail!("请先选择游戏版本");
        }
        let root =
            java_compatible_path(fs::canonicalize(&config.game_dir).context("游戏目录不存在")?);
        let dir = version_dir(&root, &config.selected)?;
        let meta = crate::metadata::resolve(&root, &config.selected)?;
        let required = meta["javaVersion"]["majorVersion"].as_u64().unwrap_or(8) as u32;
        let java = if config.java.trim().is_empty() {
            match find_java_in(required, &config.java_paths, &config.hidden_java) {
                Some(java) => java,
                None if config.auto_java => {
                    self.download_java(required)?;
                    find_managed_java(required).context("未找到下载的 Java")?
                }
                None => bail!("需要 Java {required}，请在 Java 管理页面下载或设置路径"),
            }
        } else {
            PathBuf::from(&config.java)
        };
        let runtime = crate::java_runtime::probe(&java).context("无法运行所选 Java，请检查路径")?;
        if !runtime.compatible {
            bail!("Java 架构 {} 不适配当前启动器", runtime.architecture);
        }
        let detected = runtime.major;
        if detected < required {
            bail!("所选 Java {detected} 低于游戏需要的 Java {required}");
        }
        if required == 8 && detected > 8 {
            bail!("旧版游戏请使用 Java 8，避免旧依赖与新版 Java 不兼容");
        }
        self.prepare(&root, &config.selected, &meta)?;
        let game_dir = if config.isolated || meta.get("rustHmclPack").is_some() {
            dir.join("game")
        } else {
            root.clone()
        };
        fs::create_dir_all(&game_dir)?;
        if config.isolated
            && meta["assets"].as_str() == Some("legacy")
            && root.join("resources").is_dir()
        {
            for entry in walkdir::WalkDir::new(root.join("resources")) {
                let entry = entry?;
                if entry.file_type().is_file() {
                    let dest = game_dir
                        .join("resources")
                        .join(entry.path().strip_prefix(root.join("resources"))?);
                    fs::create_dir_all(dest.parent().unwrap())?;
                    fs::copy(entry.path(), dest)?;
                }
            }
        }
        let args = launch_args(&config, &root, &game_dir, &dir, &meta)?;
        self.check()?;
        fs::create_dir_all(crate::config::data_dir())?;
        let log_path = crate::config::data_dir().join("game.log");
        let log_file = Arc::new(std::sync::Mutex::new(File::create(&log_path)?));
        let mut command = Command::new(java);
        command
            .args(&args)
            .current_dir(&game_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let mut child = command.spawn().context("启动游戏失败")?;
        self.status(format!("游戏正在运行 · PID {}", child.id()));
        let _ = self.tx.send(Event::GameStarted);
        let tx = self.tx.clone();
        let log = log_file.clone();
        let stdout = child.stdout.take().unwrap();
        let out = std::thread::spawn(move || stream_log(stdout, tx, log));
        let tx = self.tx.clone();
        let log = log_file.clone();
        let stderr = child.stderr.take().unwrap();
        let err = std::thread::spawn(move || stream_log(stderr, tx, log));
        let mut stopped = false;
        let result = loop {
            if let Some(status) = child.try_wait()? {
                break status;
            }
            if self.cancel.load(Ordering::Relaxed) {
                child.kill()?;
                stopped = true;
                break child.wait()?;
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        let _ = out.join();
        let _ = err.join();
        if stopped {
            self.status("游戏已结束");
            return Ok(());
        }
        if !result.success() {
            bail!("游戏异常退出 ({result})，日志: {}", log_path.display());
        }
        self.status("游戏已退出");
        Ok(())
    }
}

fn stream_log(reader: impl Read, tx: Sender<Event>, file: Arc<std::sync::Mutex<File>>) {
    let mut reader = BufReader::new(reader);
    let mut bytes = vec![];
    loop {
        bytes.clear();
        match reader.read_until(b'\n', &mut bytes) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let line = decode_log(&bytes).trim_end_matches(['\r', '\n']).to_owned();
        if let Ok(mut file) = file.lock() {
            let _ = writeln!(file, "{line}");
        }
        let _ = tx.send(Event::Log(line));
    }
}
fn decode_log(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_owned(),
        Err(_) if cfg!(windows) => encoding_rs::GBK.decode(bytes).0.into_owned(),
        Err(_) => String::from_utf8_lossy(bytes).into_owned(),
    }
}
pub(crate) fn java_compatible_path(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{rest}"))
    } else if let Some(rest) = text.strip_prefix(r"\\?\") {
        PathBuf::from(rest)
    } else {
        path
    }
}
fn verify(path: &Path, expected: &str) -> Result<bool> {
    Ok(sha1_file(path)?.eq_ignore_ascii_case(expected))
}
pub(crate) fn sha1_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha1::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}
fn extract_natives(file: &Path, dir: &Path) -> Result<()> {
    let mut archive = zip::ZipArchive::new(File::open(file)?)?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i)?;
        if entry.is_dir() || entry.name().starts_with("META-INF/") {
            continue;
        }
        let path = safe_join(dir, entry.name())?;
        fs::create_dir_all(path.parent().unwrap())?;
        std::io::copy(&mut entry, &mut File::create(path)?)?;
    }
    Ok(())
}
pub fn offline_uuid(name: &str) -> String {
    let mut hash = md5::compute(format!("OfflinePlayer:{name}")).0;
    hash[6] = (hash[6] & 0x0f) | 0x30;
    hash[8] = (hash[8] & 0x3f) | 0x80;
    hash.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn instance_dir(config: &crate::config::Config) -> Result<PathBuf> {
    let effective = config.for_selected();
    let config = &effective;
    let root = PathBuf::from(&config.game_dir);
    let dir = version_dir(&root, &config.selected)?;
    let pack = fs::read(dir.join(format!("{}.json", config.selected)))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .is_some_and(|v| v.get("rustHmclPack").is_some());
    Ok(if config.isolated || pack {
        dir.join("game")
    } else {
        root
    })
}
fn expand(text: &str, vars: &BTreeMap<&str, String>) -> Result<String> {
    let mut text = text.to_owned();
    for (key, value) in vars {
        text = text.replace(&format!("${{{key}}}"), value);
    }
    if text.contains("${") {
        bail!("暂不支持的启动参数: {text}");
    }
    Ok(text)
}
fn argument_list(value: &Value, vars: &BTreeMap<&str, String>) -> Result<Vec<String>> {
    let mut result = vec![];
    for arg in value.as_array().context("缺少启动参数")? {
        if let Some(s) = arg.as_str() {
            result.push(expand(s, vars)?);
        } else if allowed(arg.get("rules")) {
            if let Some(s) = arg["value"].as_str() {
                result.push(expand(s, vars)?);
            } else if let Some(items) = arg["value"].as_array() {
                for item in items {
                    result.push(expand(item.as_str().context("无效启动参数")?, vars)?);
                }
            }
        }
    }
    Ok(result)
}
fn launch_args(
    config: &crate::config::Config,
    root: &Path,
    game: &Path,
    dir: &Path,
    meta: &Value,
) -> Result<Vec<String>> {
    let libraries = root.join("libraries");
    let mut classpath = vec![];
    for lib in meta["libraries"].as_array().context("缺少依赖库")? {
        if allowed(lib.get("rules")) {
            if let Some(path) = lib["downloads"]["artifact"]["path"].as_str() {
                classpath.push(safe_join(&libraries, path)?);
            }
        }
    }
    let client_id = meta["_clientVersion"].as_str().unwrap_or(&config.selected);
    classpath.push(version_dir(root, client_id)?.join(format!("{client_id}.jar")));
    let cp = std::env::join_paths(classpath)?
        .to_string_lossy()
        .to_string();
    let index = meta["assetIndex"]["id"].as_str().unwrap_or("legacy");
    let vars = BTreeMap::from([
        ("auth_player_name", config.player.clone()),
        ("auth_uuid", offline_uuid(&config.player)),
        ("auth_access_token", "0".into()),
        ("auth_session", "0".into()),
        ("user_type", "legacy".into()),
        ("user_properties", "{}".into()),
        ("version_name", config.selected.clone()),
        (
            "version_type",
            meta["type"].as_str().unwrap_or("release").into(),
        ),
        ("game_directory", game.to_string_lossy().into()),
        ("assets_root", root.join("assets").to_string_lossy().into()),
        ("assets_index_name", index.into()),
        (
            "game_assets",
            root.join("assets/virtual")
                .join(index)
                .to_string_lossy()
                .into(),
        ),
        (
            "natives_directory",
            dir.join("rust-natives").to_string_lossy().into(),
        ),
        ("launcher_name", "Rust-HMCL".into()),
        ("launcher_version", env!("CARGO_PKG_VERSION").into()),
        ("classpath", cp.clone()),
        (
            "classpath_separator",
            if cfg!(windows) { ";" } else { ":" }.into(),
        ),
        ("library_directory", libraries.to_string_lossy().into()),
        ("resolution_width", config.width.to_string()),
        ("resolution_height", config.height.to_string()),
        ("clientid", "".into()),
        ("auth_xuid", "".into()),
    ]);
    let mut args = vec![format!("-Xmx{}M", config.memory)];
    args.extend(shell_words::split(&config.extra_jvm)?);
    if meta["arguments"]["jvm"].is_array() {
        args.extend(argument_list(&meta["arguments"]["jvm"], &vars)?);
    } else {
        args.push(format!(
            "-Djava.library.path={}",
            dir.join("rust-natives").display()
        ));
        if cfg!(target_os = "macos") {
            args.push("-XstartOnFirstThread".into());
        }
        args.extend(["-cp".into(), cp]);
    }
    args.push(str_field(meta, "mainClass")?.into());
    let game_args_start = args.len();
    if meta["arguments"]["game"].is_array() {
        args.extend(argument_list(&meta["arguments"]["game"], &vars)?);
    } else {
        for arg in shell_words::split(str_field(meta, "minecraftArguments")?)? {
            args.push(expand(&arg, &vars)?);
        }
        args.extend([
            "--width".into(),
            config.width.to_string(),
            "--height".into(),
            config.height.to_string(),
        ]);
    }
    if config.fullscreen
        && !args[game_args_start..]
            .iter()
            .any(|arg| arg == "--fullscreen")
    {
        args.push("--fullscreen".into());
    }
    Ok(args)
}
/// Read the same argument plan used by launch, without Java, downloads or writes.
pub fn launch_plan(config: &crate::config::Config) -> Result<Vec<String>> {
    let mut effective = config.for_selected();
    effective.validate()?;
    effective.game_dir =
        java_compatible_path(fs::canonicalize(&effective.game_dir).context("游戏目录不存在")?)
            .to_string_lossy()
            .into_owned();
    let root = Path::new(&effective.game_dir);
    let dir = version_dir(root, &effective.selected)?;
    let metadata = crate::metadata::resolve(root, &effective.selected)?;
    launch_args(
        &effective,
        root,
        &instance_dir(&effective)?,
        &dir,
        &metadata,
    )
}
pub fn java_major(path: &Path) -> Result<u32> {
    Ok(crate::java_runtime::probe(path)?.major)
}
fn find_java_in(required: u32, registered: &[String], hidden: &[String]) -> Option<PathBuf> {
    crate::java_runtime::candidates(registered)
        .into_iter()
        .find_map(|path| {
            crate::java_runtime::probe(&path)
                .ok()
                .filter(|runtime| {
                    runtime.major == required
                        && runtime.compatible
                        && !crate::java_runtime::is_hidden(&runtime.path, hidden)
                })
                .map(|runtime| runtime.path)
        })
}
fn find_managed_java(required: u32) -> Option<PathBuf> {
    walkdir::WalkDir::new(
        crate::config::data_dir()
            .join("java")
            .join(required.to_string()),
    )
    .max_depth(7)
    .into_iter()
    .filter_map(Result::ok)
    .filter(|entry| {
        entry.file_type().is_file() && entry.file_name() == crate::java_runtime::executable()
    })
    .find_map(|entry| {
        crate::java_runtime::probe(entry.path())
            .ok()
            .filter(|runtime| runtime.major == required && runtime.compatible)
            .map(|runtime| runtime.path)
    })
}
fn runtime_in(directory: &Path, major: u32) -> Option<crate::java_runtime::Runtime> {
    walkdir::WalkDir::new(directory)
        .max_depth(8)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_file() && entry.file_name() == crate::java_runtime::executable()
        })
        .find_map(|entry| {
            crate::java_runtime::probe(entry.path())
                .ok()
                .filter(|runtime| runtime.major == major && runtime.compatible)
        })
}
impl Worker {
    pub fn download_java(&self, major: u32) -> Result<()> {
        self.install_java(major).map(|_| ())
    }
    pub fn install_java(&self, major: u32) -> Result<PathBuf> {
        use sha2::Sha256;
        self.check()?;
        if !(8..=99).contains(&major) {
            bail!("Java 主版本应在 8–99 之间");
        }
        let platform = if os() == "osx" { "mac" } else { os() };
        let cpu = match arch() {
            "x86_64" => "x64",
            value => value,
        };
        self.status(format!("获取 Java {major} 运行环境信息…"));
        let endpoint = |image| {
            format!("https://api.adoptium.net/v3/assets/latest/{major}/hotspot?architecture={cpu}&image_type={image}&os={platform}")
        };
        let mut list = self.json(&endpoint("jre"))?;
        if list.as_array().context("Java 下载信息格式错误")?.is_empty() {
            self.status("当前平台使用 JDK 发行包…");
            list = self.json(&endpoint("jdk"))?;
        }
        let package = &list
            .as_array()
            .and_then(|items| items.first())
            .context("本平台暂时没有可用的 Java 构建")?["binary"]["package"];
        let name = str_field(package, "name")?;
        let checksum = str_field(package, "checksum")?.to_ascii_lowercase();
        if checksum.len() != 64 || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            bail!("Java 下载校验信息无效");
        }
        let link = str_field(package, "link")?;
        if reqwest::Url::parse(link)?.scheme() != "https" {
            bail!("Java 下载地址必须使用 HTTPS");
        }
        let parent = crate::config::data_dir()
            .join("java")
            .join(major.to_string());
        if let Ok(entries) = fs::read_dir(&parent) {
            for entry in entries.filter_map(Result::ok) {
                let label = entry.file_name().to_string_lossy().into_owned();
                if (label == checksum || label.starts_with(&format!("{checksum}-")))
                    && entry.file_type()?.is_dir()
                {
                    if let Some(runtime) = runtime_in(&entry.path(), major) {
                        self.tx
                            .send(Event::Java(runtime.path.to_string_lossy().into()))?;
                        self.status(format!("Java {major} 已可用"));
                        return Ok(runtime.path);
                    }
                }
            }
        }
        let cache = crate::config::data_dir().join("downloads");
        let archive = safe_join(&cache, name)?;
        self.status(format!("正在下载 Java {major}，请稍候…"));
        self.download(link, &archive, None)?;
        let mut file = File::open(&archive)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 65536];
        loop {
            self.check()?;
            let count = file.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            hasher.update(&buffer[..count]);
        }
        if format!("{:x}", hasher.finalize()) != checksum {
            bail!("Java 下载校验失败，请重试");
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let stage = cache.join(format!("java-stage-{major}-{}-{stamp}", std::process::id()));
        fs::create_dir(&stage)?;
        let installed = (|| -> Result<PathBuf> {
            self.status(format!("解压 Java {major}…"));
            if name.ends_with(".zip") {
                let mut archive = zip::ZipArchive::new(File::open(&archive)?)?;
                for i in 0..archive.len() {
                    self.check()?;
                    let mut entry = archive.by_index(i)?;
                    if entry
                        .unix_mode()
                        .is_some_and(|mode| mode & 0o170000 == 0o120000)
                    {
                        bail!("Java ZIP 包包含不支持的符号链接");
                    }
                    let path = safe_join(&stage, entry.name().trim_end_matches('/'))?;
                    if entry.is_dir() {
                        fs::create_dir_all(path)?;
                    } else {
                        fs::create_dir_all(path.parent().context("缺少解压目录")?)?;
                        std::io::copy(&mut entry, &mut File::create(path)?)?;
                    }
                }
            } else if name.ends_with(".tar.gz") {
                let decoder = flate2::read::GzDecoder::new(File::open(&archive)?);
                for entry in tar::Archive::new(decoder).entries()? {
                    self.check()?;
                    if !entry?.unpack_in(&stage)? {
                        bail!("Java 压缩包包含不安全路径");
                    }
                }
            } else {
                bail!("不支持的 Java 压缩格式");
            }
            self.check()?;
            let runtime = runtime_in(&stage, major).context("解压后未找到匹配版本的 Java")?;
            let stage_root = java_compatible_path(stage.canonicalize()?);
            let relative = runtime
                .path
                .strip_prefix(&stage_root)
                .context("Java 可执行文件超出暂存目录")?
                .to_path_buf();
            fs::create_dir_all(&parent)?;
            let normal_target = parent.join(&checksum);
            let target = if fs::symlink_metadata(&normal_target).is_ok() {
                parent.join(format!("{checksum}-{stamp}"))
            } else {
                normal_target
            };
            self.check()?;
            crate::file_ops::commit_new(&stage, &target)?;
            let runtime = crate::java_runtime::probe(&target.join(relative))?;
            anyhow::ensure!(runtime.major == major, "发布后的 Java 版本不匹配");
            Ok(runtime.path)
        })();
        if stage.exists() {
            if let Err(error) = crate::file_ops::remove_owned_directory(&stage, &cache) {
                let _ = self
                    .tx
                    .send(Event::Log(format!("Java 暂存目录清理未完成：{error:#}")));
            }
        }
        let java = installed?;
        self.tx.send(Event::Java(java.to_string_lossy().into()))?;
        self.status(format!("Java {major} 已安装"));
        Ok(java)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fullscreen_flag_reaches_modern_and_legacy_game_arguments_once() {
        let root = Path::new("launch-plan-fixture");
        let dir = root.join("versions/custom");
        let game = root.join("game");
        let mut config = crate::Config {
            selected: "custom".into(),
            fullscreen: true,
            ..Default::default()
        };
        let modern = json!({"mainClass":"net.minecraft.client.main.Main","libraries":[],"arguments":{"jvm":["-cp","${classpath}"],"game":["--username","${auth_player_name}",{"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],"value":["--width","${resolution_width}","--height","${resolution_height}"]}]}});
        let legacy = json!({"mainClass":"net.minecraft.client.main.Main","libraries":[],"minecraftArguments":"--username ${auth_player_name}"});
        for meta in [&modern, &legacy] {
            let args = launch_args(&config, root, &game, &dir, meta).unwrap();
            let main = args
                .iter()
                .position(|v| v == "net.minecraft.client.main.Main")
                .unwrap();
            assert_eq!(args.iter().filter(|v| *v == "--fullscreen").count(), 1);
            assert!(args[main + 1..].contains(&"--fullscreen".into()));
            assert!(args.windows(2).any(|v| v == ["--width", "854"]));
        }
        let declared = json!({"mainClass":"net.minecraft.client.main.Main","libraries":[],"arguments":{"jvm":["-cp","${classpath}"],"game":["--fullscreen"]}});
        assert_eq!(
            launch_args(&config, root, &game, &dir, &declared)
                .unwrap()
                .iter()
                .filter(|v| *v == "--fullscreen")
                .count(),
            1
        );
        config.fullscreen = false;
        assert!(!launch_args(&config, root, &game, &dir, &modern)
            .unwrap()
            .iter()
            .any(|v| v == "--fullscreen"));
        config.version_settings.insert(
            "custom".into(),
            crate::config::VersionSettings {
                fullscreen: Some(true),
                ..Default::default()
            },
        );
        assert!(
            launch_args(&config.for_selected(), root, &game, &dir, &modern)
                .unwrap()
                .iter()
                .any(|v| v == "--fullscreen")
        );
    }
    use serde_json::json;
    #[test]
    fn windows_verbatim_paths_are_removed_for_java() {
        assert_eq!(
            java_compatible_path(PathBuf::from(r"\\?\C:\games\minecraft")),
            PathBuf::from(r"C:\games\minecraft")
        );
        assert_eq!(
            java_compatible_path(PathBuf::from(r"\\?\UNC\server\games")),
            PathBuf::from(r"\\server\games")
        );
    }
    #[test]
    fn invalid_utf8_does_not_drop_log_lines() {
        let decoded = decode_log(b"Java error: \xff\n");
        assert!(decoded.contains("Java error:"));
    }
    #[test]
    fn paths_reject_traversal() {
        for path in ["../x", "a/../../x", "C:/x", "/x", "..\\x", "a//x"] {
            assert!(safe_join(Path::new("root"), path).is_err(), "{path}");
        }
        assert!(safe_join(Path::new("root"), "a/b.jar").is_ok());
    }
    #[test]
    fn rules_order_and_features() {
        assert!(allowed(None));
        assert!(!allowed(Some(&json!([]))));
        assert!(allowed(Some(
            &json!([{"action":"allow","os":{"name":os()}}])
        )));
        assert!(!allowed(Some(
            &json!([{"action":"allow"},{"action":"disallow","os":{"name":os()}}])
        )));
        assert!(!allowed(Some(
            &json!([{"action":"allow","features":{"is_demo_user":true}}])
        )));
        assert!(allowed(Some(
            &json!([{"action":"allow","features":{"has_custom_resolution":true}}])
        )));
    }
    #[test]
    fn known_offline_uuid() {
        assert_eq!(offline_uuid("Notch"), "b50ad385829d3141a2167e7d7539ba7f");
    }
    #[test]
    fn structured_args_preserve_spaces() {
        let vars = BTreeMap::from([("game_directory", "/home/user/game directory".into())]);
        let args = argument_list(
            &json!(["--gameDir","${game_directory}",
            {"rules":[{"action":"allow","features":{"is_demo_user":true}}],"value":"--demo"}]),
            &vars,
        )
        .unwrap();
        assert_eq!(args, vec!["--gameDir", "/home/user/game directory"]);
        assert!(expand("${unknown}", &vars).is_err());
    }
    #[test]
    fn modern_launch_plan() {
        let config = crate::config::Config {
            selected: "test".into(),
            ..Default::default()
        };
        let meta = json!({"mainClass":"net.minecraft.client.main.Main","type":"release","assetIndex":{"id":"test"},
            "libraries":[],"arguments":{"jvm":["-cp","${classpath}"],"game":["--username","${auth_player_name}","--gameDir","${game_directory}"]}});
        let args = launch_args(
            &config,
            Path::new("root"),
            Path::new("space dir"),
            Path::new("version"),
            &meta,
        )
        .unwrap();
        assert!(args.contains(&"space dir".into()));
        assert!(args.contains(&"net.minecraft.client.main.Main".into()));
        assert!(args.contains(&"-Xmx2048M".into()));
    }
    #[test]
    fn per_instance_settings_reach_launch_arguments_and_mod_directory() {
        let mut config = crate::config::Config {
            game_dir: "fixture-root".into(),
            selected: "custom".into(),
            ..Default::default()
        };
        config.version_settings.insert(
            "custom".into(),
            crate::config::VersionSettings {
                width: Some(1280),
                height: Some(720),
                memory: Some(4096),
                isolated: Some(false),
                ..Default::default()
            },
        );
        let effective = config.for_selected();
        assert_eq!(
            instance_dir(&config).unwrap(),
            PathBuf::from("fixture-root")
        );
        assert_eq!(
            crate::mods::directory(&config).unwrap(),
            PathBuf::from("fixture-root").join("mods")
        );
        let meta = json!({"mainClass":"net.minecraft.client.main.Main","libraries":[],"arguments":{"jvm":["-cp","${classpath}"],"game":[{"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],"value":["--width","${resolution_width}","--height","${resolution_height}"]}]}});
        let args = launch_args(
            &effective,
            Path::new("fixture-root"),
            &instance_dir(&config).unwrap(),
            Path::new("version"),
            &meta,
        )
        .unwrap();
        assert!(args.contains(&"-Xmx4096M".into()));
        assert!(args
            .windows(4)
            .any(|v| v == ["--width", "1280", "--height", "720"]));
        assert!(config.isolated);
        assert_eq!(config.memory, 2048);
    }
    #[test]
    fn download_repairs_corrupt_file_and_reuses_verified_cache() {
        use std::net::TcpListener;
        let root = std::env::temp_dir().join(format!("rust-hmcl-test-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let file = root.join("download.bin");
        fs::write(&file, b"corrupt").unwrap();
        let server = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/file", server.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut socket, _) = server.accept().unwrap();
            let mut request = [0u8; 2048];
            assert!(socket.read(&mut request).unwrap() > 0);
            socket
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello",
                )
                .unwrap();
        });
        let (tx, _) = std::sync::mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker = Worker::new(tx, cancel.clone()).unwrap();
        let hash = format!("{:x}", Sha1::digest(b"hello"));
        worker.download(&url, &file, Some(&hash)).unwrap();
        handle.join().unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"hello");
        // Server is gone: a valid cache must avoid a second network request.
        worker.download(&url, &file, Some(&hash)).unwrap();
        cancel.store(true, Ordering::Relaxed);
        assert!(worker.download(&url, &file, Some(&hash)).is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "requires access to Mojang official servers"]
    fn official_manifest_and_metadata() {
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = Worker::new(tx, Arc::new(AtomicBool::new(false))).unwrap();
        worker.manifest().unwrap();
        let list = rx
            .try_iter()
            .find_map(|e| match e {
                Event::Versions(v) => Some(v),
                _ => None,
            })
            .unwrap();
        let version = list.iter().find(|v| v.kind == "release").unwrap();
        let root =
            std::env::temp_dir().join(format!("rust-hmcl-network-test-{}", std::process::id()));
        let file = root.join("metadata.json");
        worker
            .download(&version.url, &file, Some(&version.sha1))
            .unwrap();
        let meta: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
        assert_eq!(meta["id"].as_str(), Some(version.id.as_str()));
        assert!(meta["downloads"]["client"]["url"]
            .as_str()
            .unwrap()
            .starts_with("https://"));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    #[ignore = "requires access to Adoptium official servers"]
    fn official_java_package_metadata() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let worker = Worker::new(tx, Arc::new(AtomicBool::new(false))).unwrap();
        let platform = if os() == "osx" { "mac" } else { os() };
        let cpu = if arch() == "x86_64" { "x64" } else { arch() };
        let metadata = worker.json(&format!("https://api.adoptium.net/v3/assets/latest/21/hotspot?architecture={cpu}&image_type=jdk&os={platform}")).unwrap();
        let package = &metadata[0]["binary"]["package"];
        assert!(str_field(package, "link").unwrap().starts_with("https://"));
        assert_eq!(str_field(package, "checksum").unwrap().len(), 64);
        assert!(safe_join(Path::new("cache"), str_field(package, "name").unwrap()).is_ok());
    }
}
