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

#[derive(Clone, Debug, serde::Serialize)]
pub struct Runtime {
    pub path: PathBuf,
    pub major: u32,
    pub version: String,
    pub vendor: String,
    pub architecture: String,
    pub is_jdk: bool,
    pub compatible: bool,
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
            let value = crate::game::java_compatible_path(PathBuf::from(value));
            let normalized = crate::game::java_compatible_path(path.to_path_buf());
            value
                .to_string_lossy()
                .replace('/', "\\")
                .eq_ignore_ascii_case(&normalized.to_string_lossy().replace('/', "\\"))
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
    let (output_tx, output_rx) = std::sync::mpsc::channel();
    let read = |stream: Box<dyn Read + Send>| {
        let output_tx = output_tx.clone();
        std::thread::spawn(move || {
            let mut bytes = vec![];
            let _ = stream.take(128 * 1024).read_to_end(&mut bytes);
            let _ = output_tx.send(bytes);
        })
    };
    let _stdout = read(Box::new(stdout));
    let _stderr = read(Box::new(stderr));
    drop(output_tx);
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
    let mut bytes = vec![];
    for _ in 0..2 {
        let remaining = Duration::from_secs(5).saturating_sub(started.elapsed());
        bytes.extend(
            output_rx
                .recv_timeout(remaining)
                .context("Java 输出读取超时")?,
        );
    }
    parse(path, &String::from_utf8_lossy(&bytes))
}
pub fn probe_selected(path: &str) -> Result<Runtime> {
    let path = Path::new(path.trim());
    if !path.is_absolute() {
        bail!("请选择 Java 可执行文件的完整路径");
    }
    let mut path = path.to_path_buf();
    if cfg!(windows)
        && path
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case("javaw.exe"))
    {
        path.set_file_name("java.exe");
    }
    if !path
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case(executable()))
    {
        bail!("请选择 bin 目录中的 Java 可执行文件");
    }
    let path = crate::game::java_compatible_path(path.canonicalize().context("Java 路径不存在")?);
    if !path.is_file() {
        bail!("Java 路径不是可执行文件");
    }
    probe(&path)
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
    let architecture = property("os.arch").unwrap_or_else(|| "未知架构".into());
    Ok(Runtime {
        compatible: matches_architecture(&architecture),
        is_jdk: path.parent().is_some_and(|directory| {
            directory
                .join(if cfg!(windows) { "javac.exe" } else { "javac" })
                .is_file()
        }),
        path,
        major,
        version,
        vendor: property("java.vendor").unwrap_or_else(|| "Java".into()),
        architecture,
    })
}

pub fn matches_architecture(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    match crate::game::arch() {
        "x86_64" => ["amd64", "x86_64", "x64"].contains(&value.as_str()),
        "aarch64" => ["aarch64", "arm64"].contains(&value.as_str()),
        "x86" => ["x86", "i386", "i486", "i586", "i686"].contains(&value.as_str()),
        _ => false,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manual_selection_rejects_other_programs_without_execution() {
        assert!(probe_selected("java").is_err());
        let path = std::env::temp_dir().join("aster-not-java.txt");
        assert!(probe_selected(path.to_str().unwrap()).is_err());
    }
    #[test]
    fn automatic_selection_only_accepts_matching_architecture_aliases() {
        assert!(matches_architecture(crate::game::arch()));
        assert!(!matches_architecture("unknown"));
        if cfg!(target_arch = "x86_64") {
            assert!(matches_architecture("amd64"));
            assert!(!matches_architecture("x86"));
            assert!(!matches_architecture("aarch64"));
        }
        if cfg!(target_arch = "aarch64") {
            assert!(matches_architecture("arm64"));
            assert!(!matches_architecture("amd64"));
        }
    }
    #[cfg(unix)]
    #[test]
    fn inherited_output_handles_do_not_make_a_probe_wait_forever() {
        use std::os::unix::fs::PermissionsExt;
        let root = std::env::temp_dir().join(format!("aster-java-output-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let pid_path = root.join("child.pid");
        let quoted = pid_path.to_string_lossy().replace('\'', "'\\''");
        let script = root.join("java");
        std::fs::write(&script, format!("#!/bin/sh\nsleep 30 &\nprintf '%s' \"$!\" > '{quoted}'\necho 'java.version = 17'\nexit 0\n")).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let start = Instant::now();
        let result = probe(&script);
        let elapsed = start.elapsed();
        let child: u32 = std::fs::read_to_string(&pid_path).unwrap().parse().unwrap();
        assert!(child > 1);
        let _ = Command::new("/bin/kill").arg(child.to_string()).status();
        std::fs::remove_dir_all(root).unwrap();
        assert!(result.is_err());
        assert!(elapsed < Duration::from_secs(10));
    }
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
