use anyhow::{bail, Context, Result};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

#[derive(Clone, serde::Serialize)]
pub struct ModFile {
    pub name: String,
    pub enabled: bool,
    pub size: u64,
}
pub fn directory(config: &crate::config::Config) -> Result<PathBuf> {
    if config.selected.is_empty() {
        bail!("请先选择游戏版本");
    }
    Ok(crate::game::instance_dir(config)?.join("mods"))
}
pub fn list(dir: &Path) -> Result<Vec<ModFile>> {
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut result = vec![];
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.file_type()?.is_file()
            && (name.ends_with(".jar") || name.ends_with(".jar.disabled"))
        {
            result.push(ModFile {
                enabled: name.ends_with(".jar"),
                name,
                size: entry.metadata()?.len(),
            });
        }
    }
    result.sort_by_key(|a| a.name.to_lowercase());
    Ok(result)
}
pub fn toggle(dir: &Path, name: &str) -> Result<()> {
    if !name.ends_with(".jar") && !name.ends_with(".jar.disabled") {
        bail!("不是模组文件");
    }
    let source = crate::game::safe_join(dir, name)?;
    let target = if name.ends_with(".disabled") {
        name.trim_end_matches(".disabled").to_owned()
    } else {
        format!("{name}.disabled")
    };
    let dest = crate::game::safe_join(dir, &target)?;
    if dest.exists() {
        bail!("目标文件已存在，请先检查模组目录");
    }
    fs::rename(source, dest)?;
    Ok(())
}
pub fn import(dir: &Path, files: &[PathBuf]) -> Result<usize> {
    import_with_cancel(dir, files, &AtomicBool::new(false))
}
pub fn import_with_cancel(dir: &Path, files: &[PathBuf], cancel: &AtomicBool) -> Result<usize> {
    let mut destinations = vec![];
    let mut names = std::collections::HashSet::new();
    for source in files {
        let metadata = fs::symlink_metadata(source)?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            bail!("请选择普通模组文件，不支持链接或文件夹");
        }
        if source.extension().and_then(|s| s.to_str()) != Some("jar") {
            bail!("请选择 .jar 模组文件");
        }
        let name = source.file_name().context("缺少文件名")?.to_string_lossy();
        let dest = crate::game::safe_join(dir, &name)?;
        let key = if cfg!(windows) {
            name.to_lowercase()
        } else {
            name.to_string()
        };
        if !names.insert(key) {
            bail!("所选模组包含同名文件：{name}");
        }
        if fs::symlink_metadata(&dest).is_ok() {
            bail!("模组 {name} 已存在，不会覆盖");
        }
        destinations.push(dest);
    }
    crate::imports::check_cancel(cancel)?;
    if files.is_empty() {
        return Ok(0);
    }
    fs::create_dir_all(dir)?;
    for (source, destination) in files.iter().zip(destinations) {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let stage = dir.join(format!(
            ".aster-import-{}-{}.part",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&stage)?;
        let result = (|| -> Result<()> {
            let mut input = fs::File::open(source)?;
            let mut buffer = [0u8; 65536];
            loop {
                crate::imports::check_cancel(cancel)?;
                let count = input.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                output.write_all(&buffer[..count])?;
            }
            output.sync_all()?;
            drop(output);
            crate::imports::check_cancel(cancel)?;
            crate::file_ops::commit_new(&stage, &destination)?;
            Ok(())
        })();
        if stage.exists() {
            let _ = fs::remove_file(&stage);
        }
        result?;
    }
    Ok(files.len())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn imports_validate_batch_before_copy_and_preserve_existing_files() {
        let root = std::env::temp_dir().join(format!("aster-mod-import-{}", std::process::id()));
        fs::create_dir_all(root.join("sources")).unwrap();
        let source = root.join("sources/a.jar");
        let invalid = root.join("sources/wrong.txt");
        fs::write(&source, b"source mod bytes").unwrap();
        fs::write(&invalid, b"wrong type").unwrap();
        let dest = root.join("mods");
        assert!(import(&dest, &[source.clone(), invalid]).is_err());
        assert!(!dest.exists());
        assert!(
            import_with_cancel(&dest, std::slice::from_ref(&source), &AtomicBool::new(true))
                .is_err()
        );
        assert!(!dest.exists());
        assert_eq!(import(&dest, std::slice::from_ref(&source)).unwrap(), 1);
        assert_eq!(fs::read(dest.join("a.jar")).unwrap(), b"source mod bytes");
        fs::write(&source, b"replacement must not win").unwrap();
        assert!(import(&dest, &[source]).is_err());
        assert_eq!(fs::read(dest.join("a.jar")).unwrap(), b"source mod bytes");
        assert_eq!(fs::read_dir(&dest).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn simultaneous_imports_never_replace_the_winning_file() {
        let root = std::env::temp_dir().join(format!("aster-mod-race-{}", std::process::id()));
        for directory in ["one", "two", "mods"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        fs::write(root.join("one/a.jar"), b"first complete file").unwrap();
        fs::write(root.join("two/a.jar"), b"second complete file").unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let threads = ["one", "two"].map(|name| {
            let barrier = barrier.clone();
            let root = root.clone();
            std::thread::spawn(move || {
                barrier.wait();
                import(&root.join("mods"), &[root.join(name).join("a.jar")]).is_ok()
            })
        });
        assert_eq!(
            threads
                .into_iter()
                .map(|thread| usize::from(thread.join().unwrap()))
                .sum::<usize>(),
            1
        );
        let bytes = fs::read(root.join("mods/a.jar")).unwrap();
        assert!(bytes == b"first complete file" || bytes == b"second complete file");
        assert_eq!(fs::read_dir(root.join("mods")).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn enabling_and_disabling_preserve_file() {
        let root = std::env::temp_dir().join(format!("rust-hmcl-mods-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("a.jar"), b"mod").unwrap();
        toggle(&root, "a.jar").unwrap();
        assert!(!list(&root).unwrap()[0].enabled);
        assert_eq!(fs::read(root.join("a.jar.disabled")).unwrap(), b"mod");
        toggle(&root, "a.jar.disabled").unwrap();
        assert!(list(&root).unwrap()[0].enabled);
        fs::remove_dir_all(root).unwrap();
    }
}
