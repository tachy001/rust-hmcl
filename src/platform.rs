//! Host conventions shared by Java discovery, downloads and launch arguments.
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OperatingSystem {
    Windows,
    Macos,
    Linux,
}

#[derive(Clone, Copy, Debug)]
pub struct Platform {
    system: OperatingSystem,
    architecture: &'static str,
}

#[derive(serde::Serialize)]
pub struct PlatformInfo {
    pub system: OperatingSystem,
    pub architecture: &'static str,
    pub java_executable: &'static str,
    pub classpath_separator: &'static str,
    pub data_directory: PathBuf,
}

impl Platform {
    pub fn current() -> Self {
        let system = if cfg!(windows) {
            OperatingSystem::Windows
        } else if cfg!(target_os = "macos") {
            OperatingSystem::Macos
        } else {
            OperatingSystem::Linux
        };
        Self {
            system,
            architecture: std::env::consts::ARCH,
        }
    }
    pub fn system(self) -> OperatingSystem {
        self.system
    }
    pub fn architecture(self) -> &'static str {
        self.architecture
    }
    pub fn minecraft_os(self) -> &'static str {
        match self.system {
            OperatingSystem::Windows => "windows",
            OperatingSystem::Macos => "osx",
            OperatingSystem::Linux => "linux",
        }
    }
    pub fn adoptium_os(self) -> &'static str {
        match self.system {
            OperatingSystem::Macos => "mac",
            _ => self.minecraft_os(),
        }
    }
    pub fn adoptium_arch(self) -> anyhow::Result<&'static str> {
        match self.architecture {
            "x86_64" => Ok("x64"),
            "aarch64" => Ok("aarch64"),
            "x86" => Ok("x86"),
            _ => anyhow::bail!("当前架构暂不支持自动下载 Java：{}", self.architecture),
        }
    }
    pub fn java_executable(self) -> &'static str {
        if self.system == OperatingSystem::Windows {
            "java.exe"
        } else {
            "java"
        }
    }
    pub fn classpath_separator(self) -> &'static str {
        if self.system == OperatingSystem::Windows {
            ";"
        } else {
            ":"
        }
    }
    pub fn is_java_stub(self, path: &Path) -> bool {
        self.system == OperatingSystem::Macos
            && (path == Path::new("/usr/bin/java")
                || path.starts_with("/System/Library/Frameworks/JavaVM.framework"))
    }
    pub fn system_java_roots(self, home: Option<&Path>) -> Vec<PathBuf> {
        let mut roots = match self.system {
            OperatingSystem::Windows => vec![],
            OperatingSystem::Macos => vec![PathBuf::from("/Library/Java/JavaVirtualMachines")],
            OperatingSystem::Linux => [
                "/usr/lib/jvm",
                "/usr/lib64/jvm",
                "/usr/java",
                "/opt/java",
                "/opt/jdks",
            ]
            .map(PathBuf::from)
            .to_vec(),
        };
        if let Some(home) = home {
            if self.system == OperatingSystem::Macos {
                roots.push(home.join("Library/Java/JavaVirtualMachines"));
            }
            roots.push(home.join(".sdkman/candidates/java"));
            roots.push(home.join(".asdf/installs/java"));
        }
        roots
    }
    pub fn info(self) -> PlatformInfo {
        PlatformInfo {
            system: self.system,
            architecture: self.architecture,
            java_executable: self.java_executable(),
            classpath_separator: self.classpath_separator(),
            data_directory: crate::config::data_dir(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_conventions_do_not_mix_windows_macos_and_linux() {
        for (os, arch, java, separator, mojang, adoptium, cpu) in [
            (
                OperatingSystem::Windows,
                "x86_64",
                "java.exe",
                ";",
                "windows",
                "windows",
                "x64",
            ),
            (
                OperatingSystem::Macos,
                "aarch64",
                "java",
                ":",
                "osx",
                "mac",
                "aarch64",
            ),
            (
                OperatingSystem::Linux,
                "x86_64",
                "java",
                ":",
                "linux",
                "linux",
                "x64",
            ),
        ] {
            let platform = Platform {
                system: os,
                architecture: arch,
            };
            assert_eq!(
                (
                    platform.java_executable(),
                    platform.classpath_separator(),
                    platform.minecraft_os(),
                    platform.adoptium_os(),
                    platform.adoptium_arch().unwrap()
                ),
                (java, separator, mojang, adoptium, cpu)
            );
        }
        assert!(Platform {
            system: OperatingSystem::Linux,
            architecture: "unknown"
        }
        .adoptium_arch()
        .is_err());
    }
    #[test]
    fn macos_discovery_includes_user_jdks_and_never_runs_the_apple_stub() {
        let mac = Platform {
            system: OperatingSystem::Macos,
            architecture: "aarch64",
        };
        assert!(mac
            .system_java_roots(Some(Path::new("user")))
            .contains(&PathBuf::from("user/Library/Java/JavaVirtualMachines")));
        assert!(mac.is_java_stub(Path::new("/usr/bin/java")));
        assert!(mac.is_java_stub(Path::new(
            "/System/Library/Frameworks/JavaVM.framework/Versions/A/Commands/java"
        )));
        assert!(!mac.is_java_stub(Path::new(
            "/Library/Java/JavaVirtualMachines/jdk/Contents/Home/bin/java"
        )));
        let linux = Platform {
            system: OperatingSystem::Linux,
            architecture: "x86_64",
        };
        assert!(!linux.is_java_stub(Path::new("/usr/bin/java")));
    }
}
