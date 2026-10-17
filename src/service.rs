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
    },
    Quilt {
        id: String,
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
    pub fn mod_files(&self) -> Result<Vec<mods::ModFile>> {
        mods::list(&mods::directory(&self.config()?)?)
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
    pub fn start(&self, job: Job) -> Result<u64> {
        let mut task = self
            .task
            .lock()
            .map_err(|_| anyhow::anyhow!("任务状态不可用"))?;
        if task.running {
            bail!("已有任务正在执行");
        }
        let config = self.config()?;
        match &job {
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
            | Job::Fabric { id }
            | Job::Quilt { id } => {
                game::version_dir(Path::new(&config.game_dir), id)?;
            }
        }
        let id = task.id + 1;
        *task = Task {
            id,
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
                Job::Java { major } => worker.download_java(major)?,
                Job::Fabric { id } => {
                    config.selected =
                        worker.install_loader(root, &id, Loader::Fabric, None, None)?;
                }
                Job::Quilt { id } => {
                    config.selected =
                        worker.install_loader(root, &id, Loader::Quilt, None, None)?;
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
