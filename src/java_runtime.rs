//! Local Java discovery and bounded probes. No Java is required to start the launcher.
use anyhow::{bail, Context, Result};
use std::{
    collections::HashSet,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

#[derive(Clone, Debug)]
pub struct Runtime {
    pub path: PathBuf,
    pub major: u32,
    pub version: String,
    pub vendor: String,
    pub architecture: String,
    pub is_jdk: bool,
}
pub fn executable() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

pub fn candidates(registered: &[String]) -> Vec<PathBuf> {
    let mut paths = registered.iter().map(PathBuf::from).collect::<Vec<_>>();
    let mut roots = vec![crate::config::data_dir().join("java")];
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        paths.push(PathBuf::from(home).join("bin").join(executable()));
    }
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(std::env::split_paths(&path).map(|dir| dir.join(executable())));
    }
    if cfg!(windows) {
        for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(root) = std::env::var_os(variable) {
                for vendor in [
                    "Java",
                    "Eclipse Adoptium",
                    "Microsoft",
                    "Zulu",
                    "BellSoft",
                    "Amazon Corretto",
                ] {
                    let directory = PathBuf::from(&root).join(vendor);
                    if vendor == "Microsoft" {
                        if let Ok(entries) = std::fs::read_dir(directory) {
                            roots.extend(
                                entries
                                    .filter_map(Result::ok)
                                    .filter(|entry| {
                                        entry
                                            .file_name()
                                            .to_string_lossy()
                                            .to_lowercase()
                                            .starts_with("jdk")
                                    })
                                    .map(|entry| entry.path()),
                            );
                        }
                    } else {
                        roots.push(directory);
                    }
                }
            }
        }
    } else if cfg!(target_os = "macos") {
        roots.push(PathBuf::from("/Library/Java/JavaVirtualMachines"));
    } else {
        roots.extend([PathBuf::from("/usr/lib/jvm"), PathBuf::from("/usr/java")]);
    }
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".sdkman/candidates/java"));
    }
    for root in roots {
        for entry in walkdir::WalkDir::new(root)
            .max_depth(7)
            .into_iter()
            .filter_map(Result::ok)
        {
            if entry.file_type().is_file() && entry.file_name() == executable() {
                paths.push(entry.into_path());
            }
        }
    }
    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter_map(|path| {
            let path = crate::game::java_compatible_path(path.canonicalize().ok()?);
            let key = if cfg!(windows) {
                path.to_string_lossy().to_lowercase()
            } else {
                path.to_string_lossy().into_owned()
            };
            seen.insert(key).then_some(path)
        })
        .collect()
}

pub fn is_hidden(path: &Path, hidden: &[String]) -> bool {
    let display = path.to_string_lossy();
    hidden.iter().any(|value| {
        if cfg!(windows) {
            value.eq_ignore_ascii_case(&display)
        } else {
            value == display.as_ref()
        }
    })
}
pub fn discover(
    registered: &[String],
    hidden: &[String],
    cancel: &AtomicBool,
) -> Result<Vec<Runtime>> {
    let mut runtimes = vec![];
    let mut seen = HashSet::new();
    for path in candidates(registered) {
        if cancel.load(Ordering::Relaxed) {
            bail!("Java 检测已取消");
        }
        if let Ok(runtime) = probe(&path) {
            if is_hidden(&runtime.path, hidden) {
                continue;
            }
            let key = if cfg!(windows) {
                runtime.path.to_string_lossy().to_lowercase()
            } else {
                runtime.path.to_string_lossy().into_owned()
            };
            if !seen.insert(key) {
                continue;
            }
            runtimes.push(runtime);
        }
    }
    runtimes.sort_by(|a, b| b.major.cmp(&a.major).then(a.path.cmp(&b.path)));
    Ok(runtimes)
}

pub fn probe(path: &Path) -> Result<Runtime> {
    let mut command = Command::new(path);
    command
        .args(["-XshowSettings:properties", "-version"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = command.spawn().context("无法运行所选 Java")?;
    let stdout = child.stdout.take().context("无法读取 Java 输出")?;
    let stderr = child.stderr.take().context("无法读取 Java 版本")?;
    let read = |stream: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut bytes = vec![];
            let _ = stream.take(128 * 1024).read_to_end(&mut bytes);
            bytes
        })
    };
    let stdout = read(Box::new(stdout));
    let stderr = read(Box::new(stderr));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > Duration::from_secs(5) {
            let _ = child.kill();
            let _ = child.wait();
            bail!("Java 检测超时");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    if !status.success() {
        bail!("Java 检测失败");
    }
    let bytes = [
        stdout.join().unwrap_or_default(),
        stderr.join().unwrap_or_default(),
    ]
    .concat();
    parse(path, &String::from_utf8_lossy(&bytes))
}

fn parse(path: &Path, text: &str) -> Result<Runtime> {
    let property = |key: &str| {
        text.lines()
            .filter_map(|line| line.trim().split_once('='))
            .find(|(name, _)| name.trim() == key)
            .map(|(_, value)| value.trim().to_owned())
    };
    let version = property("java.version")
        .or_else(|| text.split('"').nth(1).map(str::to_owned))
        .context("无法识别 Java 版本")?;
    let number = version
        .strip_prefix("1.")
        .unwrap_or(&version)
        .split(['.', '_', '-', '+'])
        .next()
        .context("无效 Java 版本")?;
    let major = number.parse::<u32>().context("无效 Java 主版本")?;
    if major < 5 {
        bail!("不支持的 Java 版本");
    }
    let path = property("java.home")
        .map(|home| PathBuf::from(home).join("bin").join(executable()))
        .filter(|candidate| candidate.is_file())
        .and_then(|candidate| candidate.canonicalize().ok())
        .map(crate::game::java_compatible_path)
        .unwrap_or_else(|| path.into());
    Ok(Runtime {
        is_jdk: path.parent().is_some_and(|directory| {
            directory
                .join(if cfg!(windows) { "javac.exe" } else { "javac" })
                .is_file()
        }),
        path,
        major,
        version,
        vendor: property("java.vendor").unwrap_or_else(|| "Java".into()),
        architecture: property("os.arch").unwrap_or_else(|| "未知架构".into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recognizes_legacy_current_and_early_access_java() {
        let old = parse(
            Path::new("java"),
            "java.version = 1.8.0_482\njava.vendor = Eclipse Adoptium\nos.arch = amd64",
        )
        .unwrap();
        assert_eq!(old.major, 8);
        assert_eq!(old.vendor, "Eclipse Adoptium");
        let modern = parse(
            Path::new("java"),
            "java.version = 21.0.6\nos.arch = aarch64",
        )
        .unwrap();
        assert_eq!(modern.major, 21);
        assert_eq!(modern.architecture, "aarch64");
        assert_eq!(
            parse(Path::new("java"), "openjdk version \"25-ea\"")
                .unwrap()
                .major,
            25
        );
        assert!(parse(Path::new("java"), "not a runtime").is_err());
    }
    #[test]
    fn hidden_paths_match_windows_case_rules_and_config_round_trips() {
        let config = crate::config::Config {
            java_paths: vec!["custom/java".into()],
            hidden_java: vec!["custom/java".into()],
            ..Default::default()
        };
        let config: crate::config::Config =
            serde_json::from_slice(&serde_json::to_vec(&config).unwrap()).unwrap();
        assert!(is_hidden(Path::new("custom/java"), &config.hidden_java));
        assert_eq!(
            is_hidden(Path::new("CUSTOM/JAVA"), &config.hidden_java),
            cfg!(windows)
        );
        assert!(!is_hidden(Path::new("other/java"), &config.hidden_java));
        assert_eq!(config.java_paths, vec!["custom/java"]);
    }
}
