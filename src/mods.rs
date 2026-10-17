use anyhow::{bail, Context, Result};
use std::{
    fs,
    path::{Path, PathBuf},
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
    fs::create_dir_all(dir)?;
    let mut count = 0;
    for source in files {
        if source.extension().and_then(|s| s.to_str()) != Some("jar") {
            bail!("请选择 .jar 模组文件");
        }
        let name = source.file_name().context("缺少文件名")?.to_string_lossy();
        let dest = crate::game::safe_join(dir, &name)?;
        if dest.exists() {
            bail!("模组 {name} 已存在，不会覆盖");
        }
        fs::copy(source, dest)?;
        count += 1;
    }
    Ok(count)
}
#[cfg(test)]
mod tests {
    use super::*;
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
