//! Official modern Forge installer, executed headlessly against an isolated copy.
use crate::{
    game::{self, Event, Worker},
    Config,
};
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::Ordering,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const MAVEN: &str = "https://maven.minecraftforge.net/net/minecraftforge/forge";
pub struct Installed {
    pub id: String,
    pub java: PathBuf,
}

pub fn validate_version(version: &str) -> Result<()> {
    crate::loader_catalog::LoaderKind::Forge.validate_version(version)
}
fn stamp() -> Result<String> {
    Ok(format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ))
}
fn read_json_entry(archive: &mut zip::ZipArchive<File>, name: &str) -> Result<Value> {
    let entry = archive
        .by_name(name)
        .with_context(|| format!("Forge 安装器缺少 {name}"))?;
    if entry.size() > 4 * 1024 * 1024 {
        bail!("Forge 安装信息超出大小限制");
    }
    Ok(serde_json::from_reader(entry.take(4 * 1024 * 1024))?)
}
fn verify_profile(game: &str, install: &Value, profile: &Value) -> Result<()> {
    if install["minecraft"].as_str() != Some(game)
        || profile["inheritsFrom"].as_str() != Some(game)
        || install["version"].as_str() != profile["id"].as_str()
    {
        bail!("Forge 安装器与基础游戏版本不匹配");
    }
    if install["spec"].as_u64() != Some(1) || !install["processors"].is_array() {
        bail!("暂不支持此旧版 Forge 安装格式");
    }
    let id = profile["id"]
        .as_str()
        .context("Forge 安装配置缺少实例名称")?;
    crate::instance_ops::validate_name(id)?;
    if profile["mainClass"].as_str() != Some("cpw.mods.bootstraplauncher.BootstrapLauncher") {
        bail!("暂不支持此旧版 Forge 启动格式");
    }
    Ok(())
}
fn artifact_paths(install: &Value, profile: &Value) -> Result<BTreeMap<String, Option<String>>> {
    let mut paths = BTreeMap::new();
    for library in install["libraries"]
        .as_array()
        .into_iter()
        .flatten()
        .chain(profile["libraries"].as_array().into_iter().flatten())
    {
        let artifact = &library["downloads"]["artifact"];
        let path = artifact["path"]
            .as_str()
            .context("Forge 依赖缺少路径")?
            .to_owned();
        game::safe_join(Path::new("libraries"), &path)?;
        let hash = artifact["sha1"]
            .as_str()
            .context("Forge 依赖缺少官方校验值")?
            .to_owned();
        if hash.len() != 40 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
            bail!("Forge 依赖校验值错误");
        }
        if let Some(Some(previous)) = paths.get(&path) {
            if previous != &hash {
                bail!("Forge 依赖校验值存在冲突");
            }
        }
        paths.insert(path, Some(hash));
    }
    let client_processors = install["processors"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|processor| {
            processor["sides"]
                .as_array()
                .map(|sides| sides.iter().any(|side| side == "client"))
                .unwrap_or(true)
        })
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    for (key, value) in install["data"].as_object().into_iter().flatten() {
        if !client_processors.contains(&format!("{{{key}}}")) {
            continue;
        }
        let Some(coordinate) = value["client"]
            .as_str()
            .and_then(|s| s.strip_prefix('['))
            .and_then(|s| s.strip_suffix(']'))
        else {
            continue;
        };
        let path = crate::metadata::maven_path(coordinate)?;
        let hash = install["data"][format!("{key}_SHA")]["client"]
            .as_str()
            .map(|s| s.trim_matches('\'').to_owned());
        if hash
            .as_ref()
            .is_some_and(|hash| hash.len() != 40 || !hash.bytes().all(|c| c.is_ascii_hexdigit()))
        {
            bail!("Forge 产物校验值错误");
        }
        paths.entry(path).or_insert(hash);
    }
    Ok(paths)
}
fn stream_new(reader: &mut File, pending: &mut Vec<u8>, worker: &Worker) -> Result<bool> {
    let mut chunk = [0u8; 8192];
    let mut read_any = false;
    for _ in 0..16 {
        let count = reader.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        read_any = true;
        for byte in &chunk[..count] {
            if *byte == b'\n'
                || (pending.len() >= 16384
                    && std::str::from_utf8(pending)
                        .map(|_| true)
                        .unwrap_or_else(|error| error.error_len().is_some()))
            {
                let line = String::from_utf8_lossy(pending)
                    .trim_end_matches('\r')
                    .to_owned();
                let _ = worker.tx.send(Event::Log(line));
                pending.clear();
            }
            if *byte != b'\n' {
                pending.push(*byte);
            }
        }
    }
    Ok(read_any)
}
fn run_installer(
    worker: &Worker,
    java: &Path,
    installer: &Path,
    stage: &Path,
    game: &Path,
) -> Result<()> {
    worker.check()?;
    fs::create_dir_all(stage.join("home"))?;
    fs::create_dir_all(stage.join("temp"))?;
    let output = stage.join("installer-output.log");
    let writer = fs::OpenOptions::new()
        .write(true)
        .append(true)
        .create_new(true)
        .open(&output)?;
    let mut reader = File::open(&output)?;
    let mut command = Command::new(java);
    command
        .arg("-Djava.awt.headless=true")
        .arg(format!("-Duser.home={}", stage.join("home").display()))
        .arg(format!("-Djava.io.tmpdir={}", stage.join("temp").display()))
        .args([
            "-Dfile.encoding=UTF-8",
            "-Dsun.net.client.defaultConnectTimeout=15000",
            "-Dsun.net.client.defaultReadTimeout=30000",
            "-jar",
        ])
        .arg(installer)
        .arg("--installClient")
        .arg(game)
        .current_dir(stage)
        .stdin(Stdio::null())
        .stdout(Stdio::from(writer.try_clone()?))
        .stderr(Stdio::from(writer));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let mut child = command.spawn().context("无法启动 Forge 官方安装器")?;
    let _ = worker
        .tx
        .send(Event::Log(format!("Forge installer PID: {}", child.id())));
    let deadline = Instant::now() + Duration::from_secs(15 * 60);
    let mut pending = vec![];
    let result = loop {
        let log_size = match reader.metadata() {
            Ok(info) => info.len(),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(error.into());
            }
        };
        if log_size > 64 * 1024 * 1024 {
            let _ = child.kill();
            let _ = child.wait();
            reader.seek(SeekFrom::End(-65536))?;
            pending.clear();
            break Err(anyhow::anyhow!(
                "Forge 安装日志超出大小限制，请查看任务日志"
            ));
        }
        if worker.cancel.load(Ordering::Relaxed) || Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            break Err(anyhow::anyhow!(if worker.cancel.load(Ordering::Relaxed) {
                "Forge 安装已取消"
            } else {
                "Forge 官方安装器执行超时"
            }));
        }
        if let Err(error) = stream_new(&mut reader, &mut pending, worker) {
            let _ = child.kill();
            let _ = child.wait();
            break Err(error);
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "Forge 官方安装器失败 ({status})，请查看任务日志"
                    ))
                }
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                break Err(error.into());
            }
        }
    };
    // Reading a regular file cannot hang on inherited process pipes.
    while stream_new(&mut reader, &mut pending, worker)? {}
    if !pending.is_empty() {
        let _ = worker
            .tx
            .send(Event::Log(String::from_utf8_lossy(&pending).into_owned()));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{atomic::AtomicBool, mpsc, Arc};
    struct Temporary {
        path: PathBuf,
        canonical: PathBuf,
    }
    impl Temporary {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("aster-forge-{}", stamp().unwrap()));
            fs::create_dir(&path).unwrap();
            let canonical = path.canonicalize().unwrap();
            Self { path, canonical }
        }
    }
    impl Drop for Temporary {
        fn drop(&mut self) {
            if self.path.canonicalize().ok().as_ref() == Some(&self.canonical)
                && self.canonical.parent() == std::env::temp_dir().canonicalize().ok().as_deref()
            {
                let _ = fs::remove_dir_all(&self.path);
            }
        }
    }
    fn worker() -> Worker {
        let (tx, _rx) = mpsc::channel();
        Worker::new(tx, Arc::new(AtomicBool::new(false))).unwrap()
    }
    #[test]
    fn untrusted_coordinates_and_incompatible_installers_fail_before_execution() {
        for value in [
            "../escape",
            "47?foo",
            "https://example.com",
            "",
            "...",
            "1/2",
        ] {
            assert!(validate_version(value).is_err());
        }
        assert!(validate_version("47.4.10").is_ok());
        let install = json!({"spec":1,"minecraft":"1.20.1","version":"test","processors":[]});
        let profile = json!({"id":"test","inheritsFrom":"1.20.1","mainClass":"cpw.mods.bootstraplauncher.BootstrapLauncher"});
        verify_profile("1.20.1", &install, &profile).unwrap();
        assert!(verify_profile("1.21", &install, &profile).is_err());
        let mut wrong = profile.clone();
        wrong["mainClass"] = "legacy.LaunchWrapper".into();
        assert!(verify_profile("1.20.1", &install, &wrong).is_err());
    }
    #[test]
    fn only_client_processor_artifacts_are_required() {
        let install = json!({"libraries":[],"data":{"USED":{"client":"[a.b:c:1:client]"},"SERVER_ONLY":{"client":"[a.b:c:1:server]"}},"processors":[{"sides":["client"],"args":["{USED}"]},{"sides":["server"],"args":["{SERVER_ONLY}"]}]});
        let paths = artifact_paths(&install, &json!({"libraries":[]})).unwrap();
        assert_eq!(paths.len(), 1);
        assert!(paths.contains_key("a/b/c/1/c-1-client.jar"));
        assert!(artifact_paths(
            &json!({"libraries":[{"downloads":{"artifact":{"path":"../escape"}}}]}),
            &json!({"libraries":[]})
        )
        .is_err());
    }
    #[test]
    fn verified_library_import_repairs_only_declared_files() {
        let temporary = Temporary::new();
        let stage = temporary.path.join("stage");
        let target = temporary.path.join("libraries");
        fs::create_dir(&stage).unwrap();
        fs::create_dir(&target).unwrap();
        fs::write(stage.join("valid.jar"), b"verified artifact").unwrap();
        fs::write(stage.join("not-declared.txt"), b"not imported").unwrap();
        fs::write(target.join("valid.jar"), b"damaged cache").unwrap();
        let hash = game::sha1_file(&stage.join("valid.jar")).unwrap();
        let paths = BTreeMap::from([("valid.jar".into(), Some(hash.clone()))]);
        let manifest = import_libraries(&worker(), &stage, &target, &paths).unwrap();
        assert_eq!(manifest[0]["sha1"], hash);
        assert_eq!(
            fs::read(target.join("valid.jar")).unwrap(),
            b"verified artifact"
        );
        assert!(!target.join("not-declared.txt").exists());
        let wrong = BTreeMap::from([("valid.jar".into(), Some("0".repeat(40)))]);
        assert!(import_libraries(&worker(), &stage, &target, &wrong).is_err());
        assert_eq!(
            fs::read(target.join("valid.jar")).unwrap(),
            b"verified artifact"
        );
        assert_eq!(fs::read_dir(&target).unwrap().count(), 1);
    }
    #[test]
    fn cancellation_stops_before_starting_an_installer() {
        let temporary = Temporary::new();
        let worker = worker();
        worker.cancel.store(true, Ordering::Relaxed);
        assert!(run_installer(
            &worker,
            Path::new("never-execute"),
            Path::new("absent.jar"),
            &temporary.path,
            &temporary.path
        )
        .is_err());
        assert_eq!(fs::read_dir(&temporary.path).unwrap().count(), 0);
    }
    #[test]
    fn new_installation_does_not_replace_a_modified_base_client() {
        let temporary = Temporary::new();
        let directory = temporary.path.join("versions/source");
        fs::create_dir_all(&directory).unwrap();
        fs::write(temporary.path.join("expected"), b"official original").unwrap();
        let hash = game::sha1_file(&temporary.path.join("expected")).unwrap();
        let profile = json!({"id":"source","rustHmclGameVersion":"1.20.1","javaVersion":{"majorVersion":17},"downloads":{"client":{"url":"https://invalid.invalid/not-requested","sha1":hash}},"libraries":[]});
        fs::write(
            directory.join("source.json"),
            serde_json::to_vec(&profile).unwrap(),
        )
        .unwrap();
        fs::write(directory.join("source.jar"), b"locally modified client").unwrap();
        let config = Config {
            game_dir: temporary.path.to_string_lossy().into_owned(),
            java: "never-execute".into(),
            ..Default::default()
        };
        let error = worker()
            .install_forge(&config, "source", Some("new-forge"), Some("47.4.10"), false)
            .err()
            .unwrap();
        assert!(error.to_string().contains("内容被修改"));
        assert_eq!(
            fs::read(directory.join("source.jar")).unwrap(),
            b"locally modified client"
        );
        assert!(!temporary.path.join("versions/new-forge").exists());
    }
    #[test]
    fn log_streaming_keeps_split_unicode_and_bounds_long_lines() {
        let temporary = Temporary::new();
        let path = temporary.path.join("log");
        fs::write(&path, format!("{}\n", "界".repeat(6000))).unwrap();
        let (tx, rx) = mpsc::channel();
        let mut worker = worker();
        worker.tx = tx;
        let mut reader = File::open(path).unwrap();
        let mut pending = vec![];
        while stream_new(&mut reader, &mut pending, &worker).unwrap() {}
        let output = rx
            .try_iter()
            .filter_map(|event| {
                if let Event::Log(line) = event {
                    Some(line)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert!(output.len() > 1);
        assert_eq!(output.join(""), "界".repeat(6000));
        assert!(output.iter().all(|line| line.len() < 16400));
    }
}
fn import_libraries(
    worker: &Worker,
    stage: &Path,
    target: &Path,
    paths: &BTreeMap<String, Option<String>>,
) -> Result<Vec<Value>> {
    let mut manifest = vec![];
    let stage_root = stage.canonicalize()?;
    for (relative, expected) in paths {
        worker.check()?;
        let source = game::safe_join(stage, relative)?;
        let attributes = fs::symlink_metadata(&source)
            .with_context(|| format!("Forge 安装产物缺失：{relative}"))?;
        if !attributes.is_file()
            || attributes.file_type().is_symlink()
            || !source.canonicalize()?.starts_with(&stage_root)
        {
            bail!("Forge 安装产物超出暂存范围");
        }
        let destination = game::safe_join(target, relative)?;
        let hash = game::sha1_file(&source)?;
        if expected
            .as_ref()
            .is_some_and(|expected| !expected.eq_ignore_ascii_case(&hash))
        {
            bail!("Forge 安装产物校验失败：{relative}");
        }
        if destination.is_file() && game::verify(&destination, &hash)? {
            manifest.push(json!({"path":relative,"sha1":hash}));
            continue;
        }
        fs::create_dir_all(destination.parent().context("依赖缺少目录")?)?;
        let pending = destination.with_file_name(format!(".aster-forge-{}.part", stamp()?));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending)?;
        let result = (|| -> Result<()> {
            std::io::copy(&mut File::open(&source)?, &mut file)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&pending, &destination)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&pending);
        }
        result?;
        manifest.push(json!({"path":relative,"sha1":hash}));
    }
    if manifest.is_empty() {
        bail!("Forge 官方安装器未生成依赖文件");
    }
    Ok(manifest)
}
pub(crate) fn validate_artifacts(root: &Path, meta: &Value) -> Result<()> {
    if let Some(artifacts) = meta["rustHmclForge"]["artifacts"].as_array() {
        for artifact in artifacts {
            let path = game::safe_join(
                &root.join("libraries"),
                artifact["path"].as_str().context("Forge 依赖缺少路径")?,
            )?;
            let hash = artifact["sha1"].as_str().context("Forge 依赖缺少校验值")?;
            if !path.is_file() || !game::verify(&path, hash)? {
                bail!(
                    "Forge 安装产物缺失或损坏，请在游戏库中校验修复：{}",
                    path.display()
                );
            }
        }
    }
    Ok(())
}

impl Worker {
    pub fn install_forge(
        &self,
        config: &Config,
        selected: &str,
        alias: Option<&str>,
        requested: Option<&str>,
        repair: bool,
    ) -> Result<Installed> {
        self.check()?;
        let root = Path::new(&config.game_dir);
        let base = crate::metadata::resolve(root, selected)?;
        let client = base["_clientVersion"].as_str().context("缺少基础客户端")?;
        let minecraft = base["_gameVersion"].as_str().context("缺少基础游戏版本")?;
        // Release coordinates must not inject path/query components into the official Maven URL.
        validate_version(minecraft)?;
        let parent = crate::metadata::resolve(root, client)?;
        let major = parent["javaVersion"]["majorVersion"].as_u64().unwrap_or(8) as u32;
        if major < 17 {
            bail!("Forge 自动安装目前需要使用 Java 17 及以上的现代游戏版本");
        }
        let source_client = game::version_dir(root, client)?.join(format!("{client}.jar"));
        let client_hash = parent["downloads"]["client"]["sha1"]
            .as_str()
            .context("基础游戏缺少客户端校验值")?;
        if !repair && (!source_client.is_file() || !game::verify(&source_client, client_hash)?) {
            bail!("基础游戏客户端缺失或内容被修改，请先单独校验原版，或选择其他基础实例");
        }
        let version = if let Some(version) = requested {
            validate_version(version)?;
            version.to_owned()
        } else {
            self.status("获取 Forge 官方推荐版本…");
            crate::loader_catalog::LoaderCatalog::new(self)
                .recommended(minecraft, crate::loader_catalog::LoaderKind::Forge)?
        };
        validate_version(&version)?;
        let coordinate = format!("{minecraft}-{version}");
        let name = format!("forge-{coordinate}-installer.jar");
        let url = format!("{MAVEN}/{coordinate}/{name}");
        self.status("校验 Forge 官方安装器…");
        let checksum = self.text(&format!("{url}.sha1"))?.trim().to_owned();
        if checksum.len() != 40 || !checksum.bytes().all(|c| c.is_ascii_hexdigit()) {
            bail!("Forge 官方校验值格式错误");
        }
        let cache = crate::config::data_dir().join("downloads/forge");
        fs::create_dir_all(&cache)?;
        let installer = game::java_compatible_path(cache.canonicalize()?).join(&name);
        self.download(&url, &installer, Some(&checksum))?;
        let mut archive = zip::ZipArchive::new(File::open(&installer)?)?;
        let install = read_json_entry(&mut archive, "install_profile.json")?;
        let mut profile = read_json_entry(&mut archive, "version.json")?;
        verify_profile(minecraft, &install, &profile)?;
        let paths = artifact_paths(&install, &profile)?;
        let official_id = profile["id"]
            .as_str()
            .context("Forge 缺少实例名称")?
            .to_owned();
        let id = alias.unwrap_or(&official_id).to_owned();
        crate::instance_ops::validate_name(&id)?;
        let destination = game::version_dir(root, &id)?;
        let profile_path = destination.join(format!("{id}.json"));
        let existing = if repair {
            Some(fs::read(&profile_path).context("找不到要修复的 Forge 实例")?)
        } else {
            None
        };
        if !repair && fs::symlink_metadata(&profile_path).is_ok() {
            bail!("同名实例已存在，请使用其他名称或校验已有实例");
        }
        let mut effective = config.clone();
        effective.selected = selected.into();
        let runtime = self.resolve_java(&effective.for_selected(), major)?;
        if repair {
            self.prepare(root, client, &parent)?;
        } else {
            self.prepare_without_client(root, client, &parent)?;
        }
        let stage_parent = crate::config::data_dir().join("downloads");
        fs::create_dir_all(&stage_parent)?;
        let stage = stage_parent.join(format!("forge-stage-{}", stamp()?));
        fs::create_dir(&stage)?;
        let stage = game::java_compatible_path(stage.canonicalize()?);
        let result = (|| -> Result<()> {
            let staged_game = stage.join("minecraft");
            let staged_base = game::version_dir(&staged_game, minecraft)?;
            fs::create_dir_all(&staged_base)?;
            if !source_client.is_file() || !game::verify(&source_client, client_hash)? {
                bail!("基础游戏客户端缺失或损坏，请先校验原版游戏");
            }
            fs::copy(&source_client, staged_base.join(format!("{minecraft}.jar")))?;
            let mut vanilla = parent.clone();
            vanilla["id"] = minecraft.into();
            fs::write(
                staged_base.join(format!("{minecraft}.json")),
                serde_json::to_vec_pretty(&vanilla)?,
            )?;
            fs::write(
                staged_game.join("launcher_profiles.json"),
                b"{\"profiles\":{}}",
            )?;
            self.status("准备 Forge 安装副本…");
            for (relative, hash) in &paths {
                self.check()?;
                let from = game::safe_join(&root.join("libraries"), relative)?;
                if from.is_file()
                    && hash
                        .as_ref()
                        .map(|hash| game::verify(&from, hash))
                        .transpose()?
                        .unwrap_or(true)
                {
                    let to = game::safe_join(&staged_game.join("libraries"), relative)?;
                    fs::create_dir_all(to.parent().context("Forge 依赖缺少目录")?)?;
                    fs::copy(from, to)?;
                }
            }
            let _ = self
                .tx
                .send(Event::Progress(0., "Forge 官方安装器处理中…".into()));
            run_installer(self, &runtime.path, &installer, &stage, &staged_game)?;
            self.check()?;
            let installed_profile =
                game::version_dir(&staged_game, &official_id)?.join(format!("{official_id}.json"));
            let produced: Value = serde_json::from_slice(
                &fs::read(&installed_profile).context("Forge 安装器未生成实例配置")?,
            )?;
            if produced != profile {
                bail!("Forge 安装器生成的配置与官方包不一致");
            }
            // Do not import synthetic launcher accounts/profiles, only library artifacts.
            let artifacts = import_libraries(
                self,
                &staged_game.join("libraries"),
                &root.join("libraries"),
                &paths,
            )?;
            profile["id"] = id.clone().into();
            profile["inheritsFrom"] = client.into();
            profile["rustHmclForge"] =
                json!({"version":version,"installer_sha1":checksum,"artifacts":artifacts});
            crate::metadata::normalize_libraries(&mut profile)?;
            let merged = crate::metadata::merge(&parent, &profile)?;
            self.prepare_without_client(root, &id, &merged)?;
            if !game::verify(&source_client, client_hash)? {
                bail!("基础客户端在安装期间发生变化，请重新检查后安装");
            }
            self.check()?;
            if let Some(original) = &existing {
                if fs::read(&profile_path)? != *original {
                    bail!("修复期间实例配置发生变化，请重试");
                }
                validate_artifacts(root, &base)?;
            } else {
                fs::create_dir_all(&destination)?;
                crate::loaders::publish_profile(&destination, &id, &profile)?;
            }
            Ok(())
        })();
        if let Err(error) = crate::file_ops::remove_owned_directory(&stage, &stage_parent) {
            let _ = self
                .tx
                .send(Event::Log(format!("Forge 暂存清理未完成：{error:#}")));
        }
        result?;
        if !repair {
            let _ = self.tx.send(Event::Installed(id.clone()));
        }
        self.status(if repair {
            "Forge 校验修复完成"
        } else {
            "Forge 安装完成"
        });
        Ok(Installed {
            id,
            java: runtime.path,
        })
    }
}
