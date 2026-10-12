//! Profile operations preserve unknown JSON fields and never rewrite game files.
use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn validate_name(id: &str) -> Result<()> {
    if id.is_empty()
        || id.encode_utf16().count() > 100
        || id.ends_with(['.', ' '])
        || id
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        || id == "."
        || id == ".."
    {
        bail!("实例名称应为 1–100 字，不能包含路径符号或以点、空格结尾");
    }
    let stem = id.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix)
                .is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
        })
    {
        bail!("实例名称是系统保留名称");
    }
    Ok(())
}
fn check(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        bail!("操作已取消");
    }
    Ok(())
}
fn paths(root: &Path, old: &str, new: &str) -> Result<(PathBuf, PathBuf, PathBuf)> {
    validate_name(new)?;
    if old == new {
        bail!("新名称与原名称相同");
    }
    let versions = root
        .join("versions")
        .canonicalize()
        .context("实例目录不存在")?;
    let source = crate::game::version_dir(root, old)?;
    if fs::symlink_metadata(&source)?.file_type().is_symlink() {
        bail!("暂不支持操作链接形式的实例目录");
    }
    let source = source.canonicalize()?;
    if source.parent() != Some(versions.as_path()) {
        bail!("实例目录超出游戏目录");
    }
    let destination = versions.join(new);
    if fs::symlink_metadata(&destination).is_ok() {
        bail!("同名实例已存在，不能覆盖");
    }
    if fs::symlink_metadata(source.join(format!("{new}.json"))).is_ok()
        || source.join(format!("{old}.jar")).is_file()
            && fs::symlink_metadata(source.join(format!("{new}.jar"))).is_ok()
    {
        bail!("实例内已有与目标名称冲突的文件，不能覆盖");
    }
    for entry in walkdir::WalkDir::new(&source) {
        if entry?.file_type().is_symlink() {
            bail!("暂不支持复制或重命名含链接的实例，请保留原实例");
        }
    }
    crate::metadata::resolve(root, old)?;
    Ok((versions, source, destination))
}
fn rewrite(bytes: &[u8], old: &str, new: &str, own: bool) -> Result<Vec<u8>> {
    let mut profile: Value = serde_json::from_slice(bytes)?;
    if !profile.is_object() {
        bail!("实例配置不是 JSON 对象");
    }
    if own {
        profile["id"] = new.into();
    }
    for field in ["jar", "inheritsFrom"] {
        if profile[field].as_str() == Some(old) {
            profile[field] = new.into();
        }
    }
    Ok(serde_json::to_vec_pretty(&profile)?)
}
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension("json.rust-hmcl-tmp");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .context("无法准备配置更新，临时文件可能已存在")?;
    let result = (|| -> Result<()> {
        file.write_all(bytes)?;
        fs::set_permissions(&temporary, fs::metadata(path)?.permissions())?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = remove_owned_file(&temporary);
    }
    result
}

pub fn duplicate(root: &Path, old: &str, new: &str, cancel: &AtomicBool) -> Result<()> {
    let (versions, source, destination) = paths(root, old, new)?;
    check(cancel)?;
    let stage = versions.join(format!(
        ".rust-hmcl-copy-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    fs::create_dir(&stage)?;
    let result = (|| -> Result<()> {
        for entry in walkdir::WalkDir::new(&source).min_depth(1) {
            check(cancel)?;
            let entry = entry?;
            if entry.file_type().is_symlink() {
                bail!(
                    "复制遇到链接文件，原实例保持不变：{}",
                    entry.path().display()
                );
            }
            let relative = entry.path().strip_prefix(&source)?;
            let target = stage.join(relative);
            if entry.file_type().is_dir() {
                fs::create_dir_all(&target)?;
                continue;
            }
            if !entry.file_type().is_file() {
                bail!("实例中包含无法复制的特殊文件");
            }
            fs::create_dir_all(target.parent().context("复制路径缺少目录")?)?;
            let mut input = fs::File::open(entry.path())?;
            let mut output = fs::File::create(&target)?;
            let mut buffer = [0u8; 65536];
            loop {
                check(cancel)?;
                let count = input.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                output.write_all(&buffer[..count])?;
            }
            fs::set_permissions(&target, entry.metadata()?.permissions())?;
        }
        let old_profile = stage.join(format!("{old}.json"));
        let bytes = rewrite(&fs::read(&old_profile)?, old, new, true)?;
        let new_profile = stage.join(format!("{new}.json"));
        fs::write(&new_profile, bytes)?;
        fs::set_permissions(&new_profile, fs::metadata(&old_profile)?.permissions())?;
        remove_owned_file(&old_profile)?;
        let jar = stage.join(format!("{old}.jar"));
        if jar.is_file() {
            fs::rename(jar, stage.join(format!("{new}.jar")))?;
        }
        check(cancel)?;
        if fs::symlink_metadata(&destination).is_ok() {
            bail!("复制期间出现了同名实例，未覆盖");
        }
        fs::rename(&stage, &destination)?;
        Ok(())
    })();
    if result.is_err() && stage.exists() {
        cleanup_stage(&stage, &versions)?;
    }
    result
}
fn remove_owned_file(path: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        make_owned_file_writable(path)?;
    }
    fs::remove_file(path)?;
    Ok(())
}
fn cleanup_stage(stage: &Path, versions: &Path) -> Result<()> {
    let resolved = stage.canonicalize()?;
    if resolved.parent() != Some(versions)
        || !stage
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with(".rust-hmcl-copy-"))
    {
        bail!("临时复制目录检查失败，已保留以供恢复");
    }
    #[cfg(windows)]
    for entry in walkdir::WalkDir::new(stage)
        .into_iter()
        .filter_map(Result::ok)
    {
        if entry.file_type().is_file() {
            make_owned_file_writable(entry.path())?;
        }
    }
    fs::remove_dir_all(stage)?;
    Ok(())
}
#[cfg(windows)]
#[allow(clippy::permissions_set_readonly_false)] // Windows attribute only; never changes Unix mode bits.
fn make_owned_file_writable(path: &Path) -> Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions)?;
    Ok(())
}

pub fn rename(root: &Path, old: &str, new: &str, cancel: &AtomicBool) -> Result<()> {
    let (versions, source, destination) = paths(root, old, new)?;
    let original = fs::read(source.join(format!("{old}.json")))?;
    let updated = rewrite(&original, old, new, true)?;
    let mut dependents = vec![];
    for id in crate::game::installed(root) {
        if id == old {
            continue;
        }
        let path = crate::game::version_dir(root, &id)?.join(format!("{id}.json"));
        let bytes = fs::read(&path)?;
        let json: Value =
            serde_json::from_slice(&bytes).context("其他实例配置损坏，无法核验继承关系")?;
        if json["inheritsFrom"].as_str() == Some(old) || json["jar"].as_str() == Some(old) {
            if fs::symlink_metadata(&path)?.file_type().is_symlink()
                || !path.canonicalize()?.starts_with(&versions)
            {
                bail!("依赖实例的配置位于游戏目录外，不能修改");
            }
            let updated = rewrite(&bytes, old, new, false)?;
            dependents.push((path, bytes, updated));
        }
    }
    check(cancel)?;
    fs::rename(&source, &destination)?;
    let old_profile = destination.join(format!("{old}.json"));
    let new_profile = destination.join(format!("{new}.json"));
    let old_jar = destination.join(format!("{old}.jar"));
    let new_jar = destination.join(format!("{new}.jar"));
    let mut profile_moved = false;
    let mut jar_moved = false;
    let mut profile_written = false;
    let mut written = 0;
    let result = (|| -> Result<()> {
        fs::rename(&old_profile, &new_profile)?;
        profile_moved = true;
        if old_jar.is_file() {
            fs::rename(&old_jar, &new_jar)?;
            jar_moved = true;
        }
        atomic_write(&new_profile, &updated)?;
        profile_written = true;
        for (path, original, updated) in &dependents {
            check(cancel)?;
            if fs::read(path)? != *original {
                bail!("依赖实例配置在操作期间发生改变");
            }
            atomic_write(path, updated)?;
            written += 1;
        }
        check(cancel)?;
        Ok(())
    })();
    if let Err(error) = result {
        let rollback = (|| -> Result<()> {
            for (path, original, updated) in dependents[..written].iter().rev() {
                if fs::read(path)? != *updated {
                    bail!("依赖配置再次改变，未覆盖外部修改：{}", path.display());
                }
                atomic_write(path, original)?;
            }
            if profile_written {
                atomic_write(&new_profile, &original)?;
            }
            if jar_moved {
                fs::rename(&new_jar, &old_jar)?;
            }
            if profile_moved {
                fs::rename(&new_profile, &old_profile)?;
            }
            fs::rename(&destination, &source)?;
            Ok(())
        })();
        if let Err(rollback_error) = rollback {
            bail!("重命名失败：{error:#}；恢复未完成：{rollback_error:#}。请保留实例目录检查。");
        }
        return Err(error.context("重命名未完成，已恢复原实例"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    struct Fixture {
        root: PathBuf,
        canonical: PathBuf,
    }
    impl Fixture {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir()
                .join(format!("rust-hmcl-instance-{}-{name}", std::process::id()));
            fs::create_dir(&root).unwrap();
            fs::create_dir(root.join("versions")).unwrap();
            let canonical = root.canonicalize().unwrap();
            Self { root, canonical }
        }
        fn profile(&self, id: &str, parent: Option<&str>) {
            let dir = self.root.join("versions").join(id);
            fs::create_dir(&dir).unwrap();
            let mut profile =
                json!({"id":id,"libraries":[],"mainClass":"test.Main","custom":{"keep":"value"}});
            if let Some(parent) = parent {
                profile["inheritsFrom"] = parent.into();
            }
            fs::write(
                dir.join(format!("{id}.json")),
                serde_json::to_vec_pretty(&profile).unwrap(),
            )
            .unwrap();
            if parent.is_none() {
                fs::write(dir.join(format!("{id}.jar")), b"unchanged-client-jar").unwrap();
            }
            fs::create_dir(dir.join("game")).unwrap();
            fs::write(dir.join("game/level.dat"), b"unchanged-save").unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            // Delete only this freshly created, unchanged, private fixture directory.
            if self.root.canonicalize().ok().as_ref() == Some(&self.canonical)
                && self.canonical.parent() == std::env::temp_dir().canonicalize().ok().as_deref()
            {
                let _ = fs::remove_dir_all(&self.root);
            }
        }
    }
    #[test]
    fn duplicate_preserves_source_game_and_unknown_profile_fields() {
        let fixture = Fixture::new("copy");
        fixture.profile("base", None);
        duplicate(&fixture.root, "base", "copied", &AtomicBool::new(false)).unwrap();
        let target = fixture.root.join("versions/copied");
        assert_eq!(
            fs::read(target.join("copied.jar")).unwrap(),
            b"unchanged-client-jar"
        );
        assert_eq!(
            fs::read(target.join("game/level.dat")).unwrap(),
            b"unchanged-save"
        );
        assert!(fixture.root.join("versions/base/base.jar").is_file());
        assert!(!target.join("base.json").exists());
        let meta = crate::metadata::resolve(&fixture.root, "copied").unwrap();
        assert_eq!(meta["_clientVersion"], "copied");
        assert_eq!(meta["custom"]["keep"], "value");
        assert!(duplicate(&fixture.root, "base", "copied", &AtomicBool::new(false)).is_err());
    }
    #[test]
    fn rename_updates_inheritance_and_retains_saves() {
        let fixture = Fixture::new("rename");
        fixture.profile("base", None);
        fixture.profile("child", Some("base"));
        rename(&fixture.root, "base", "renamed", &AtomicBool::new(false)).unwrap();
        assert!(!fixture.root.join("versions/base").exists());
        assert_eq!(
            fs::read(fixture.root.join("versions/renamed/game/level.dat")).unwrap(),
            b"unchanged-save"
        );
        let meta = crate::metadata::resolve(&fixture.root, "child").unwrap();
        assert_eq!(meta["_clientVersion"], "renamed");
        let child: Value = serde_json::from_slice(
            &fs::read(fixture.root.join("versions/child/child.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(child["inheritsFrom"], "renamed");
    }
    #[test]
    fn failed_dependent_write_rolls_back_folder_metadata_and_jar() {
        let fixture = Fixture::new("rollback");
        fixture.profile("base", None);
        fixture.profile("a-child", Some("base"));
        fixture.profile("b-child", Some("base"));
        let base = fixture.root.join("versions/base/base.json");
        let original = fs::read(&base).unwrap();
        let child = fixture.root.join("versions/a-child/a-child.json");
        let child_original = fs::read(&child).unwrap();
        let blocker = fixture
            .root
            .join("versions/b-child/b-child.json.rust-hmcl-tmp");
        fs::write(&blocker, b"existing-recovery-file").unwrap();
        assert!(rename(&fixture.root, "base", "renamed", &AtomicBool::new(false)).is_err());
        assert_eq!(fs::read(base).unwrap(), original);
        assert_eq!(fs::read(child).unwrap(), child_original);
        assert!(!fixture.root.join("versions/renamed").exists());
        assert!(fixture.root.join("versions/base/base.jar").is_file());
        assert_eq!(fs::read(blocker).unwrap(), b"existing-recovery-file");
        assert_eq!(
            crate::metadata::resolve(&fixture.root, "b-child").unwrap()["_clientVersion"],
            "base"
        );
    }
    #[test]
    fn cancelled_or_invalid_names_do_not_create_an_instance() {
        let fixture = Fixture::new("cancel");
        fixture.profile("base", None);
        assert!(duplicate(&fixture.root, "base", "new", &AtomicBool::new(true)).is_err());
        assert!(rename(&fixture.root, "base", "new", &AtomicBool::new(true)).is_err());
        assert!(!fixture.root.join("versions/new").exists());
        for name in ["../escape", "CON", "LPT1.json", "trailing.", "bad:name", ""] {
            assert!(validate_name(name).is_err());
        }
        assert!(validate_name("我的实例-1.20.1").is_ok());
    }
}
