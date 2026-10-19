//! UI-independent command boundary. Only typed launcher actions are exposed.
use crate::{
    catalog,
    config::Config,
    game::{self, Event, Worker},
    java_runtime,
    loaders::Loader,
    mods, resourcepacks,
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
};

#[derive(Clone, Serialize)]
pub struct Instance {
    pub id: String,
    pub subtitle: String,
    pub icon: String,
    pub favorite: bool,
}
#[derive(Serialize)]
pub struct Snapshot {
    pub config: Config,
    pub instances: Vec<Instance>,
    pub avatars: std::collections::BTreeMap<String, String>,
}
#[derive(Clone, Default, Serialize)]
pub struct Task {
    pub id: u64,
    pub launch_id: Option<String>,
    pub clues: Vec<crate::diagnostics::Clue>,
    pub running: bool,
    pub game_running: bool,
    pub progress: f32,
    pub status: String,
    pub error: Option<String>,
    pub logs: Vec<String>,
}
#[derive(Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Job {
    Launch {
        id: String,
    },
    Install {
        id: String,
    },
    Repair {
        id: String,
    },
    Java {
        major: u32,
    },
    Fabric {
        id: String,
        #[serde(default)]
        name: Option<String>,
    },
    Quilt {
        id: String,
        #[serde(default)]
        name: Option<String>,
    },
    InstallMod {
        id: String,
        project: String,
    },
    ImportFiles {
        id: String,
        content: crate::imports::Content,
        paths: Vec<String>,
    },
    ImportPack {
        path: String,
        id: String,
        optional: bool,
        source_sha1: String,
    },
    Duplicate {
        id: String,
        name: String,
        #[serde(default)]
        copy_saves: bool,
    },
    Rename {
        id: String,
        name: String,
    },
}
#[derive(Serialize)]
pub struct ResourcePack {
    pub name: String,
    pub title: String,
    pub summary: String,
    pub description: String,
    pub enabled: bool,
    pub compatible: bool,
    pub warning: Option<String>,
}
#[derive(Serialize)]
pub struct ModEnvironment {
    pub game: String,
    pub loader: String,
}
#[derive(Serialize)]
pub struct RegisteredJava {
    pub runtime: java_runtime::Runtime,
    pub snapshot: Snapshot,
}
#[derive(Clone)]
pub struct Service {
    config: Arc<Mutex<Config>>,
    task: Arc<Mutex<Task>>,
    cancel: Arc<AtomicBool>,
}
impl Service {
    pub fn load() -> Result<Self> {
        Ok(Self::new(Config::load()?))
    }
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(Mutex::new(config)),
            task: Arc::new(Mutex::new(Task::default())),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }
    fn config(&self) -> Result<Config> {
        Ok(self
            .config
            .lock()
            .map_err(|_| anyhow::anyhow!("配置状态不可用"))?
            .clone())
    }
    pub fn snapshot(&self) -> Result<Snapshot> {
        let config = self.config()?;
        let root = Path::new(&config.game_dir);
        let ids = game::installed(root);
        let descriptions = catalog::load(root, &ids);
        let instances = ids
            .into_iter()
            .map(|id| {
                let description = &descriptions[&id];
                Instance {
                    favorite: config.favorites.contains(&id),
                    id,
                    subtitle: description.subtitle.clone(),
                    icon: description.icon.into(),
                }
            })
            .collect();
        let avatars = config
            .accounts
            .iter()
            .chain(std::iter::once(&config.player))
            .map(|player| (player.clone(), crate::avatars::skin(player).0.to_owned()))
            .collect();
        Ok(Snapshot {
            config,
            instances,
            avatars,
        })
    }
    pub fn task(&self) -> Result<Task> {
        Ok(self
            .task
            .lock()
            .map_err(|_| anyhow::anyhow!("任务状态不可用"))?
            .clone())
    }
    pub fn update_config(&self, mut config: Config) -> Result<Snapshot> {
        let task = self
            .task
            .lock()
            .map_err(|_| anyhow::anyhow!("任务状态不可用"))?;
        if task.running {
            bail!("请等待当前任务结束后再修改设置");
        }
        config.validate()?;
        if !config.selected.is_empty() {
            game::version_dir(Path::new(&config.game_dir), &config.selected)?;
        }
        config.normalize();
        config.save()?;
        *self
            .config
            .lock()
            .map_err(|_| anyhow::anyhow!("配置状态不可用"))? = config;
        drop(task);
        self.snapshot()
    }
    pub fn versions(&self) -> Result<Vec<game::Version>> {
        let (tx, rx) = mpsc::channel();
        let worker = Worker::new(tx, Arc::new(AtomicBool::new(false)))?;
        worker.manifest()?;
        drop(worker);
        rx.into_iter()
            .find_map(|event| {
                if let Event::Versions(versions) = event {
                    Some(versions)
                } else {
                    None
                }
            })
            .context("版本列表为空")
    }
    pub fn runtimes(&self) -> Result<Vec<java_runtime::Runtime>> {
        let config = self.config()?;
        java_runtime::discover(
            &config.java_paths,
            &config.hidden_java,
            &AtomicBool::new(false),
        )
    }
    pub fn register_java(&self, path: &str) -> Result<RegisteredJava> {
        let task = self
            .task
            .lock()
            .map_err(|_| anyhow::anyhow!("任务状态不可用"))?;
        if task.running {
            bail!("请等待当前任务结束后再添加 Java");
        }
        let runtime = java_runtime::probe_selected(path)?;
        if !runtime.compatible {
            bail!(
                "Java 架构 {} 不适配当前启动器，请选择匹配的运行环境",
                runtime.architecture
            );
        }
        let mut config = self.config()?;
        let path = runtime
            .path
            .to_str()
            .context("Java 路径不是有效文本")?
            .to_owned();
        if !java_runtime::is_hidden(&runtime.path, &config.java_paths) {
            config.java_paths.push(path);
        }
        config
            .hidden_java
            .retain(|path| !java_runtime::is_hidden(&runtime.path, std::slice::from_ref(path)));
        config.normalize();
        config.validate()?;
        config.save()?;
        *self
            .config
            .lock()
            .map_err(|_| anyhow::anyhow!("配置状态不可用"))? = config;
        drop(task);
        Ok(RegisteredJava {
            runtime,
            snapshot: self.snapshot()?,
        })
    }
    pub fn mod_files(&self) -> Result<Vec<mods::ModFile>> {
        mods::list(&mods::directory(&self.config()?)?)
    }
    pub fn inspect_modpack(&self, path: &str) -> Result<crate::modpack::Preview> {
        crate::modpack::preview(Path::new(&self.config()?.game_dir), Path::new(path))
    }
    fn mod_config(&self, id: &str) -> Result<Config> {
        let mut config = self.config()?;
        game::version_dir(Path::new(&config.game_dir), id)?;
        config.selected = id.into();
        Ok(config)
    }
    pub fn mod_environment(&self, id: &str) -> Result<ModEnvironment> {
        let (game, loader) = crate::modrinth::environment(&self.mod_config(id)?)?;
        Ok(ModEnvironment { game, loader })
    }
    pub fn search_mods(&self, id: &str, query: &str) -> Result<Vec<crate::modrinth::Project>> {
        let query = query.trim();
        if query.is_empty() || query.chars().count() > 200 {
            bail!("请输入 1–200 个字符的模组名称");
        }
        let config = self.mod_config(id)?;
        // Validate locally before constructing a network worker.
        crate::modrinth::environment(&config)?;
        let (tx, rx) = mpsc::channel();
        let worker = Worker::new(tx, Arc::new(AtomicBool::new(false)))?;
        worker.search_mods(&config, query)?;
        drop(worker);
        rx.into_iter()
            .find_map(|event| match event {
                Event::Projects(projects) => Some(projects),
                _ => None,
            })
            .context("模组搜索未返回结果")
    }
    pub fn toggle_mod(&self, name: &str) -> Result<Vec<mods::ModFile>> {
        let task = self
            .task
            .lock()
            .map_err(|_| anyhow::anyhow!("任务状态不可用"))?;
        if task.running {
            bail!("游戏运行或任务执行期间不能修改模组");
        }
        mods::toggle(&mods::directory(&self.config()?)?, name)?;
        drop(task);
        self.mod_files()
    }
    pub fn resource_packs(&self) -> Result<Vec<ResourcePack>> {
        let context = resourcepacks::ContextInfo::from_config(&self.config()?)?;
        Ok(resourcepacks::list(&context)?
            .into_iter()
            .map(|pack| ResourcePack {
                name: pack.name,
                title: pack.title,
                summary: pack.summary,
                description: pack.description,
                enabled: pack.enabled,
                compatible: pack.compatibility == resourcepacks::Compatibility::Compatible,
                warning: pack.compatibility.warning().map(str::to_owned),
            })
            .collect())
    }
    pub fn enable_resources(
        &self,
        names: &[String],
        enabled: bool,
        confirmed: bool,
    ) -> Result<Vec<ResourcePack>> {
        let task = self
            .task
            .lock()
            .map_err(|_| anyhow::anyhow!("任务状态不可用"))?;
        if task.running {
            bail!("游戏运行或任务执行期间不能修改资源包");
        }
        let context = resourcepacks::ContextInfo::from_config(&self.config()?)?;
        if enabled
            && !confirmed
            && resourcepacks::list(&context)?.iter().any(|pack| {
                names.contains(&pack.name)
                    && pack.compatibility != resourcepacks::Compatibility::Compatible
            })
        {
            bail!("资源包可能不兼容，请先确认");
        }
        resourcepacks::set_enabled(&context, names, enabled)?;
        drop(task);
        self.resource_packs()
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
    pub fn task_report(&self, task_id: u64) -> Result<String> {
        let task = self.task()?;
        if task.id == 0 || task.id != task_id {
            bail!("任务已变化，请重新打开当前日志后导出");
        }
        let mut report = format!(
            "星屿 Aster · 任务报告\n任务编号：{}\n状态：{}\n实例：{}\n\n",
            task.id,
            task.status,
            task.launch_id.as_deref().unwrap_or("非游戏启动任务")
        );
        if let Some(error) = &task.error {
            report.push_str(&format!("错误：{error}\n\n"));
        }
        for clue in &task.clues {
            report.push_str(&format!(
                "日志线索：{}\n依据：{}\n建议：{}\n\n",
                clue.title, clue.evidence, clue.suggestion
            ));
        }
        report.push_str("最近保留的输出（最多 500 行，不等同于完整游戏日志）：\n");
        for line in &task.logs {
            report.push_str(line);
            report.push('\n');
        }
        Ok(report)
    }
    pub fn start(&self, job: Job) -> Result<u64> {
        let mut task = self
            .task
            .lock()
            .map_err(|_| anyhow::anyhow!("任务状态不可用"))?;
        if task.running {
            bail!("已有任务正在执行");
        }
        let config = self.config()?;
        if let Job::Fabric {
            id,
            name: Some(name),
        }
        | Job::Quilt {
            id,
            name: Some(name),
        } = &job
        {
            crate::instance_ops::validate_name(name)?;
            if name == id
                || std::fs::symlink_metadata(
                    game::version_dir(Path::new(&config.game_dir), name)?
                        .join(format!("{name}.json")),
                )
                .is_ok()
            {
                bail!("同名实例已存在，请使用其他名称");
            }
        }
        match &job {
            Job::ImportPack {
                path,
                id,
                source_sha1,
                ..
            } => {
                crate::modpack::validate_target(
                    Path::new(&config.game_dir),
                    Path::new(path),
                    id,
                    source_sha1,
                )?;
            }
            Job::ImportFiles { id, content, paths } => {
                crate::imports::validate(&config, id, *content, paths)?;
            }
            Job::InstallMod { id, project } => {
                crate::modrinth::validate_project(project)?;
                self.mod_environment(id)?;
            }
            Job::Java { major } => {
                if !(8..=99).contains(major) {
                    bail!("Java 主版本应在 8–99 之间");
                }
            }
            Job::Duplicate { id, name, .. } | Job::Rename { id, name } => {
                game::version_dir(Path::new(&config.game_dir), id)?;
                crate::instance_ops::validate_name(name)?;
                if id == name {
                    bail!("新名称与原名称相同");
                }
                let destination = game::version_dir(Path::new(&config.game_dir), name)?;
                if std::fs::symlink_metadata(destination).is_ok() {
                    bail!("同名实例已存在，不能覆盖");
                }
            }
            Job::Launch { id }
            | Job::Install { id }
            | Job::Repair { id }
            | Job::Fabric { id, .. }
            | Job::Quilt { id, .. } => {
                game::version_dir(Path::new(&config.game_dir), id)?;
            }
        }
        let id = task.id + 1;
        *task = Task {
            id,
            launch_id: match &job {
                Job::Launch { id } => Some(id.clone()),
                _ => None,
            },
            running: true,
            status: "正在准备任务…".into(),
            ..Default::default()
        };
        self.cancel.store(false, Ordering::SeqCst);
        let service = self.clone();
        std::thread::spawn(move || service.run(job, config));
        Ok(id)
    }
    fn run(&self, job: Job, mut config: Config) {
        let (tx, rx) = mpsc::channel();
        let monitor = self.clone();
        let events = std::thread::spawn(move || {
            for event in rx {
                let Ok(mut task) = monitor.task.lock() else {
                    return;
                };
                match event {
                    Event::Status(status) => task.status = status,
                    Event::Progress(progress, status) => {
                        task.progress = progress.clamp(0., 1.);
                        task.status = status;
                    }
                    Event::GameStarted => task.game_running = true,
                    Event::Log(line) => {
                        task.logs.push(line);
                        if task.logs.len() > 500 {
                            task.logs.remove(0);
                        }
                    }
                    _ => {}
                }
            }
        });
        let result = (|| -> Result<()> {
            let mut worker = Worker::new(tx, self.cancel.clone())?;
            worker.threads = config.download_threads;
            let root = Path::new(&config.game_dir);
            match job {
                Job::ImportPack {
                    path,
                    id,
                    optional,
                    source_sha1,
                } => {
                    worker.import_pack_checked(
                        root,
                        Path::new(&path),
                        &id,
                        optional,
                        Some(&source_sha1),
                    )?;
                    config
                        .version_settings
                        .entry(id.clone())
                        .or_default()
                        .isolated = Some(true);
                    config.selected = id;
                }
                Job::ImportFiles { id, content, paths } => {
                    crate::imports::run(&config, &id, content, &paths, &worker)?;
                }
                Job::Launch { id } => {
                    config.selected = id;
                    worker.launch(&config)?;
                }
                Job::Install { id } => {
                    let manifest = worker
                        .json("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")?;
                    let versions: Vec<game::Version> =
                        serde_json::from_value(manifest["versions"].clone())?;
                    worker.install(
                        root,
                        versions
                            .iter()
                            .find(|version| version.id == id)
                            .context("未知 Minecraft 版本")?,
                    )?;
                    config.selected = id;
                }
                Job::Repair { id } => {
                    let metadata = crate::metadata::resolve(root, &id)?;
                    worker.prepare(root, &id, &metadata)?;
                }
                Job::Java { major } => {
                    let path = worker.install_java(major)?;
                    if !java_runtime::is_hidden(&path, &config.java_paths) {
                        config.java_paths.push(path.to_string_lossy().into_owned());
                    }
                    config.hidden_java.retain(|value| {
                        !java_runtime::is_hidden(&path, std::slice::from_ref(value))
                    });
                }
                Job::Fabric { id, name } => {
                    config.selected =
                        worker.install_loader(root, &id, Loader::Fabric, None, name.as_deref())?;
                }
                Job::Quilt { id, name } => {
                    config.selected =
                        worker.install_loader(root, &id, Loader::Quilt, None, name.as_deref())?;
                }
                Job::InstallMod { id, project } => {
                    config.selected = id;
                    worker.install_mod(&config, &project)?;
                }
                Job::Duplicate {
                    id,
                    name,
                    copy_saves,
                } => {
                    crate::instance_ops::duplicate_config(
                        &config,
                        &id,
                        &name,
                        copy_saves,
                        &self.cancel,
                    )?;
                    config.remap_instance(&id, &name, false);
                    config.version_settings.entry(name).or_default().isolated = Some(true);
                }
                Job::Rename { id, name } => {
                    crate::instance_ops::rename(root, &id, &name, &self.cancel)?;
                    config.remap_instance(&id, &name, true);
                }
            }
            config.save()?;
            *self
                .config
                .lock()
                .map_err(|_| anyhow::anyhow!("配置状态不可用"))? = config;
            Ok(())
        })();
        let _ = events.join();
        if let Ok(mut task) = self.task.lock() {
            task.running = false;
            task.game_running = false;
            task.error = result.err().map(|error| format!("{error:#}"));
            if task.launch_id.is_some() {
                task.clues = task
                    .error
                    .as_deref()
                    .map(|error| crate::diagnostics::launch_clues(error, &task.logs))
                    .unwrap_or_default();
            }
            task.status = if self.cancel.load(Ordering::SeqCst) {
                "任务已停止"
            } else if task.error.is_some() {
                "任务失败"
            } else {
                task.progress = 1.;
                "任务完成"
            }
            .into();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn loader_alias_rejections_are_read_only_and_legacy_requests_still_parse() {
        let service = Service::new(Config::default());
        let before = serde_json::to_value(service.config().unwrap()).unwrap();
        let legacy: Job = serde_json::from_str(r#"{"kind":"fabric","id":"source"}"#).unwrap();
        assert!(matches!(legacy, Job::Fabric { name: None, .. }));
        for name in ["source", "../escape", ""] {
            assert!(service
                .start(Job::Quilt {
                    id: "source".into(),
                    name: Some(name.into())
                })
                .is_err());
        }
        assert_eq!(service.task().unwrap().id, 0);
        assert_eq!(
            serde_json::to_value(service.config().unwrap()).unwrap(),
            before
        );
    }
    #[test]
    fn failed_launch_keeps_attempted_instance_without_executing_java() {
        let game_dir = std::env::temp_dir().join(format!(
            "aster-missing-game-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        assert!(!game_dir.exists());
        let service = Service::new(Config {
            game_dir: game_dir.to_string_lossy().into_owned(),
            selected: "Original".into(),
            ..Default::default()
        });
        let task_id = service
            .start(Job::Launch {
                id: "Attempted".into(),
            })
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            let task = service.task().unwrap();
            if !task.running {
                assert_eq!(task.id, task_id);
                assert_eq!(task.launch_id.as_deref(), Some("Attempted"));
                assert!(task.error.as_deref().unwrap().contains("游戏目录不存在"));
                assert!(task.clues.is_empty());
                assert!(!task.game_running);
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(service.config().unwrap().selected, "Original");
        assert!(!game_dir.exists());
    }
    #[test]
    fn reports_preserve_task_context_and_reject_stale_requests() {
        let service = Service::new(Config::default());
        assert!(service.task_report(0).is_err());
        let mut task = service.task.lock().unwrap();
        task.id = 7;
        task.launch_id = Some("Test Instance".into());
        task.error =
            Some("Could not find or load main class net.minecraft.client.main.Main".into());
        task.logs = vec![
            "[main/INFO] retained output".into(),
            "<script>plain text</script>".into(),
        ];
        task.clues = crate::diagnostics::launch_clues(task.error.as_deref().unwrap(), &task.logs);
        drop(task);
        let report = service.task_report(7).unwrap();
        assert!(report.contains("实例：Test Instance"));
        assert!(report.contains("游戏主类未能加载"));
        assert!(report.contains("<script>plain text</script>"));
        assert!(report.contains("最多 500 行"));
        assert!(service.task_report(6).is_err());
        assert!(service.task_report(8).is_err());
    }
    #[test]
    fn failed_java_registration_does_not_mutate_configuration() {
        let service = Service::new(Config::default());
        let before = serde_json::to_value(service.snapshot().unwrap().config).unwrap();
        assert!(service.register_java("relative/java").is_err());
        let path = std::env::temp_dir().join("not-a-java-program.txt");
        assert!(service.register_java(path.to_str().unwrap()).is_err());
        assert_eq!(
            serde_json::to_value(service.snapshot().unwrap().config).unwrap(),
            before
        );
        service.task.lock().unwrap().running = true;
        assert!(service.register_java("java").is_err());
    }
    #[test]
    fn mod_discovery_is_scoped_to_explicit_instance_and_rejects_before_network() {
        let root = std::env::temp_dir().join(format!("aster-mod-service-{}", std::process::id()));
        let version = root.join("versions/fabric-sample");
        std::fs::create_dir_all(&version).unwrap();
        std::fs::write(version.join("fabric-sample.json"), r#"{"id":"fabric-sample","inheritsFrom":"1.20.1","libraries":[{"name":"net.fabricmc:fabric-loader:0.16.0"}]}"#).unwrap();
        let vanilla = root.join("versions/1.20.1");
        std::fs::create_dir_all(&vanilla).unwrap();
        std::fs::write(
            vanilla.join("1.20.1.json"),
            r#"{"id":"1.20.1","libraries":[]}"#,
        )
        .unwrap();
        let service = Service::new(Config {
            game_dir: root.to_string_lossy().into_owned(),
            selected: "1.20.1".into(),
            ..Default::default()
        });
        assert!(service.mod_environment("1.20.1").is_err());
        assert!(service.search_mods("1.20.1", "sodium").is_err());
        assert!(service.mod_environment("../escape").is_err());
        let environment = service.mod_environment("fabric-sample").unwrap();
        assert_eq!(environment.game, "1.20.1");
        assert_eq!(environment.loader, "fabric");
        assert_eq!(service.snapshot().unwrap().config.selected, "1.20.1");
        assert!(service.search_mods("fabric-sample", " ").is_err());
        assert!(service
            .search_mods("fabric-sample", &"a".repeat(201))
            .is_err());
        assert!(service
            .start(Job::InstallMod {
                id: "fabric-sample".into(),
                project: "../invalid".into()
            })
            .is_err());
        assert!(service
            .start(Job::InstallMod {
                id: "1.20.1".into(),
                project: "sodium".into()
            })
            .is_err());
        assert!(!service.task().unwrap().running);
        assert!(!root.join("mods").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bridge_rejects_traversal_and_concurrent_mutation_without_starting_a_worker() {
        let service = Service::new(Config::default());
        assert!(service
            .start(Job::Launch {
                id: "../escape".into()
            })
            .is_err());
        assert!(!service.task().unwrap().running);
        service.task.lock().unwrap().running = true;
        assert!(service.start(Job::Java { major: 17 }).is_err());
        assert!(service.update_config(Config::default()).is_err());
        service.cancel();
        assert!(service.cancel.load(Ordering::SeqCst));
    }
    #[test]
    fn bridge_read_only_snapshot_does_not_create_a_game_directory() {
        let path = std::env::temp_dir().join(format!("rust-hmcl-read-only-{}", std::process::id()));
        assert!(!path.exists());
        let service = Service::new(Config {
            game_dir: path.to_string_lossy().into_owned(),
            ..Default::default()
        });
        assert!(service.snapshot().unwrap().instances.is_empty());
        assert!(!path.exists());
        assert!(serde_json::from_str::<Job>(r#"{"kind":"shell","command":"anything"}"#).is_err());
    }
}
