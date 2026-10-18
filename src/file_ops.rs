//! Exclusive publication shared by local imports and resource-pack staging.
#[cfg(target_os = "windows")]
use anyhow::Context;
use anyhow::Result;
use std::{fs, path::Path};

pub(crate) fn remove_owned_directory(path: &Path, parent: &Path) -> Result<()> {
    let attributes = fs::symlink_metadata(path)?;
    anyhow::ensure!(
        attributes.is_dir() && !attributes.file_type().is_symlink(),
        "暂存路径不是普通目录"
    );
    let parent = fs::canonicalize(parent)?;
    let resolved = fs::canonicalize(path)?;
    anyhow::ensure!(
        resolved.parent() == Some(parent.as_path()) && resolved.file_name() == path.file_name(),
        "暂存目录超出指定范围"
    );
    fs::remove_dir_all(resolved)?;
    Ok(())
}

pub(crate) fn commit_new(source: &Path, destination: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn MoveFileExW(source: *const u16, destination: *const u16, flags: u32) -> i32;
        }
        let source = fs::canonicalize(source)?;
        let destination = fs::canonicalize(destination.parent().context("缺少目标目录")?)?
            .join(destination.file_name().context("缺少目标名称")?);
        let from: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<u16> = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        // No REPLACE_EXISTING: files and even empty directories must be retained.
        if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 0) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        if source.is_dir() {
            // Reserve exclusively; Unix rename can replace this owned empty dir.
            fs::create_dir(destination)?;
            if let Err(error) = fs::rename(source, destination) {
                let _ = fs::remove_dir(destination);
                return Err(error.into());
            }
        } else {
            fs::hard_link(source, destination)?;
            fs::remove_file(source)?;
        }
        Ok(())
    }
}
