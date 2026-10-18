//! Typed local imports, scoped to one existing game instance.
use crate::{game, mods, resourcepacks, Config};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Content {
    Mods,
    Resources,
}

pub fn validate(
    config: &Config,
    id: &str,
    content: Content,
    paths: &[String],
) -> Result<(Config, Vec<PathBuf>)> {
    if paths.is_empty() || paths.len() > 256 {
        bail!("每次请选择 1–256 个文件或文件夹");
    }
    game::version_dir(Path::new(&config.game_dir), id)?;
    crate::metadata::resolve(Path::new(&config.game_dir), id)?;
    let mut scoped = config.clone();
    scoped.selected = id.into();
    let destination = match content {
        Content::Mods => mods::directory(&scoped)?,
        Content::Resources => resourcepacks::ContextInfo::from_config(&scoped)?.directory,
    };
    let mut names = HashSet::new();
    let mut sources = vec![];
    for path in paths {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            bail!("请选择本机文件的完整路径");
        }
        let metadata = std::fs::symlink_metadata(&path)
            .with_context(|| format!("无法读取 {}", path.display()))?;
        if metadata.file_type().is_symlink() {
            bail!("暂不支持导入链接文件或文件夹");
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("文件名不是有效文本")?;
        let key = if cfg!(windows) {
            name.to_lowercase()
        } else {
            name.to_owned()
        };
        if !names.insert(key) {
            bail!("所选内容中有同名文件：{name}");
        }
        let target = game::safe_join(&destination, name)?;
        if std::fs::symlink_metadata(&target).is_ok() {
            bail!("{name} 已存在，不会覆盖");
        }
        match content {
            Content::Mods
                if !metadata.is_file()
                    || !path.extension().is_some_and(|extension| extension == "jar") =>
            {
                bail!("请选择 .jar 模组文件")
            }
            Content::Resources
                if !(metadata.is_dir()
                    || metadata.is_file()
                        && path
                            .extension()
                            .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))) =>
            {
                bail!("请选择 ZIP 资源包或资源包文件夹")
            }
            _ => {}
        }
        sources.push(path);
    }
    Ok((scoped, sources))
}

pub fn run(
    config: &Config,
    id: &str,
    content: Content,
    paths: &[String],
    worker: &game::Worker,
) -> Result<()> {
    let (scoped, sources) = validate(config, id, content, paths)?;
    let total = sources.len();
    let mut completed = 0;
    let result = (|| -> Result<()> {
        match content {
            Content::Mods => {
                let dir = mods::directory(&scoped)?;
                for source in &sources {
                    check_cancel(&worker.cancel)?;
                    worker.status(format!(
                        "导入模组 {}/{}：{}",
                        completed + 1,
                        total,
                        source.file_name().unwrap().to_string_lossy()
                    ));
                    mods::import_with_cancel(&dir, std::slice::from_ref(source), &worker.cancel)?;
                    completed += 1;
                }
            }
            Content::Resources => {
                let context = resourcepacks::ContextInfo::from_config(&scoped)?;
                for source in &sources {
                    check_cancel(&worker.cancel)?;
                    worker.status(format!(
                        "导入资源包 {}/{}：{}",
                        completed + 1,
                        total,
                        source.file_name().unwrap().to_string_lossy()
                    ));
                    resourcepacks::import(&context, source)?;
                    completed += 1;
                }
            }
        }
        Ok(())
    })();
    let summary = format!("已导入 {completed}/{total} 项；源文件未移动");
    let _ = worker.tx.send(game::Event::Log(summary.clone()));
    result.with_context(|| summary.clone())?;
    worker.status(summary);
    Ok(())
}
pub(crate) fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::SeqCst) {
        bail!("导入已取消");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validation_is_read_only_scoped_and_cancel_does_not_copy() {
        let root =
            std::env::temp_dir().join(format!("aster-import-boundary-{}", std::process::id()));
        let version = root.join("versions/sample");
        std::fs::create_dir_all(&version).unwrap();
        std::fs::write(
            version.join("sample.json"),
            br#"{"id":"sample","libraries":[]}"#,
        )
        .unwrap();
        let source = root.join("source.jar");
        std::fs::write(&source, b"source must remain untouched").unwrap();
        let config = Config {
            game_dir: root.to_string_lossy().into_owned(),
            selected: "another-instance".into(),
            isolated: true,
            ..Default::default()
        };
        let paths = vec![source.to_str().unwrap().to_owned()];
        let (scoped, _) = validate(&config, "sample", Content::Mods, &paths).unwrap();
        assert_eq!(scoped.selected, "sample");
        assert_eq!(config.selected, "another-instance");
        assert!(validate(&config, "../escape", Content::Mods, &paths).is_err());
        assert!(validate(&config, "sample", Content::Mods, &[]).is_err());
        assert!(validate(&config, "sample", Content::Mods, &["relative.jar".into()]).is_err());
        assert!(validate(
            &config,
            "sample",
            Content::Mods,
            &[paths[0].clone(), paths[0].clone()]
        )
        .is_err());
        assert!(validate(&config, "sample", Content::Resources, &paths).is_err());
        let (tx, _rx) = std::sync::mpsc::channel();
        let worker = game::Worker::new(tx, std::sync::Arc::new(AtomicBool::new(true))).unwrap();
        assert!(run(&config, "sample", Content::Mods, &paths, &worker).is_err());
        assert!(!version.join("game").exists());
        assert_eq!(
            std::fs::read(&source).unwrap(),
            b"source must remain untouched"
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
