use crate::{
    config::Config,
    game::{Event, Version, Worker},
    loaders::Loader,
};
use anyhow::{bail, Context, Result};
use std::sync::{atomic::AtomicBool, mpsc, Arc};
pub fn requested() -> bool {
    std::env::args().len() > 1
}
pub fn run() -> Result<()> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let command = args.first().context("缺少命令")?;
    if command == "--scan-java" {
        let config = Config::load()?;
        for runtime in crate::java_runtime::discover(
            &config.java_paths,
            &config.hidden_java,
            &AtomicBool::new(false),
        )? {
            println!(
                "Java {} · {} · {}\n{}",
                runtime.version,
                runtime.vendor,
                runtime.architecture,
                runtime.path.display()
            );
        }
        return Ok(());
    }
    if command == "--render-ui" {
        return crate::preview::run(&args[1..]);
    }
    if command == "--help" {
        println!("Local resource packs (current instance):\n  --resourcepack-list\n  --resourcepack-import <ZIP path>\n  --resourcepack-enable <file name>\n  --resourcepack-disable <file name>\nUI preview: --render-ui <PNG> instance light 1080 700 resources [info]");
        println!("Rust HMCL\n  --install <Minecraft version>\n  --fabric <installed version>\n  --quilt <installed version>\n  --java <major>\n  --launch <installed version>\n  --mod <Modrinth project ID or slug>\n  --pack <mrpack path> <new instance name>\n  --list\nEnvironment: RUST_HMCL_DATA_DIR isolates configuration, Java and game files.");
        return Ok(());
    }
    let mut config = Config::load()?;
    config.normalize();
    if command.starts_with("--resourcepack-") {
        let context = crate::resourcepacks::ContextInfo::from_config(&config)?;
        match command.as_str() {
            "--resourcepack-list" => {
                for pack in crate::resourcepacks::list(&context)? {
                    println!(
                        "{} · {} · {}",
                        pack.name,
                        if pack.enabled {
                            "已启用"
                        } else {
                            "未启用"
                        },
                        pack.compatibility.warning().unwrap_or("兼容")
                    );
                }
            }
            "--resourcepack-import" => crate::resourcepacks::import(
                &context,
                std::path::Path::new(args.get(1).context("需要 ZIP 文件路径")?),
            )?,
            "--resourcepack-enable" | "--resourcepack-disable" => {
                crate::resourcepacks::set_enabled(
                    &context,
                    &[args.get(1).context("需要资源包文件名")?.clone()],
                    command == "--resourcepack-enable",
                )?
            }
            _ => bail!("未知资源包命令"),
        }
        return Ok(());
    }
    if command == "--duplicate" || command == "--rename" {
        let old = args.get(1).context("需要原实例名称")?;
        let new = args.get(2).context("需要新实例名称")?;
        let root = std::path::Path::new(&config.game_dir);
        let rename = command == "--rename";
        if rename {
            crate::instance_ops::rename(root, old, new, &AtomicBool::new(false))?;
        } else {
            crate::instance_ops::duplicate(root, old, new, &AtomicBool::new(false))?;
        }
        config.remap_instance(old, new, rename);
        config.save()?;
        println!("实例操作完成：{old} -> {new}");
        return Ok(());
    }
    if command == "--list" {
        for version in crate::game::installed(std::path::Path::new(&config.game_dir)) {
            println!("{version}");
        }
        return Ok(());
    }
    let argument = args
        .get(1)
        .context("该命令需要版本参数，使用 --help 查看")?
        .clone();
    let (tx, rx) = mpsc::channel();
    let mut w = Worker::new(tx, Arc::new(AtomicBool::new(false)))?;
    w.threads = config.download_threads;
    let root = std::path::PathBuf::from(&config.game_dir);
    let logging = std::thread::spawn(move || {
        let mut last = std::time::Instant::now();
        for event in rx {
            match event {
                Event::Status(s) | Event::Log(s) => println!("{s}"),
                Event::Progress(p, s) if last.elapsed().as_secs() >= 2 => {
                    println!("{:.0}% {s}", p * 100.);
                    last = std::time::Instant::now();
                }
                _ => {}
            }
        }
    });
    let result = (|| -> Result<()> {
        match command.as_str() {
            "--install" => {
                let manifest =
                    w.json("https://piston-meta.mojang.com/mc/game/version_manifest_v2.json")?;
                let versions: Vec<Version> = serde_json::from_value(manifest["versions"].clone())?;
                w.install(
                    &root,
                    versions
                        .iter()
                        .find(|v| v.id == argument)
                        .context("未知 Minecraft 版本")?,
                )?;
                config.selected = argument;
            }
            "--fabric" | "--quilt" => {
                config.selected = w.install_loader(
                    &root,
                    &argument,
                    if command == "--fabric" {
                        Loader::Fabric
                    } else {
                        Loader::Quilt
                    },
                    None,
                    None,
                )?;
            }
            "--java" => w.download_java(argument.parse()?)?,
            "--mod" => w.install_mod(&config, &argument)?,
            "--pack" => {
                let id = args.get(2).context("整合包命令需要指定新实例名称")?;
                w.import_pack(&root, std::path::Path::new(&argument), id, true)?;
                config.selected = id.clone();
            }
            "--launch" => {
                config.selected = argument;
                w.launch(&config)?;
            }
            _ => bail!("未知命令，使用 --help 查看"),
        }
        config.save()?;
        Ok(())
    })();
    drop(w);
    let _ = logging.join();
    result
}
