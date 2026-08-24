//! Library definitions and artifact resolution.
//!
//! Port of HMCL's `game.Library` / `game.LibraryDownloadInfo`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::CompatibilityRule;

/// Rules applied to a library artifact's native classifier files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractRules {
    #[serde(default)]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub include: Vec<String>,
}

impl ExtractRules {
    pub fn should_extract(&self, path: &str) -> bool {
        if !self.include.is_empty() && !self.include.iter().any(|p| path.starts_with(p)) {
            return false;
        }
        !self.exclude.iter().any(|p| path.starts_with(p))
    }
}

/// The `downloads` section of a library.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LibraryDownloads {
    #[serde(default)]
    pub artifact: Option<LibraryDownloadInfo>,
    #[serde(default)]
    pub classifiers: Option<HashMap<String, LibraryDownloadInfo>>,
}

/// One library file download entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryDownloadInfo {
    pub path: Option<String>,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

/// A single library entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Library {
    /// The Maven coordinate `group:artifact:version`.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub downloads: Option<LibraryDownloads>,
    #[serde(default)]
    pub rules: Vec<CompatibilityRule>,
    /// OS name → native classifier.
    #[serde(default)]
    pub natives: Option<HashMap<String, String>>,
    #[serde(default)]
    pub extract: Option<ExtractRules>,
}

/// A Maven coordinate parsed from a library name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    pub group: String,
    pub artifact: String,
    pub version: String,
    pub classifier: Option<String>,
    pub extension: String,
}

impl Artifact {
    /// Parse a Maven coordinate in HMCL's descriptor form:
    /// `group:name:version[:classifier][@extension]`.
    pub fn parse(name: &str) -> Option<Self> {
        let mut parts: Vec<&str> = name.splitn(4, ':').collect();
        if parts.len() < 3 {
            return None;
        }
        // The extension may be attached to the last part with `@`.
        let mut extension = "jar".to_owned();
        let last_index = parts.len() - 1;
        if let Some((value, ext)) = parts[last_index].split_once('@') {
            parts[last_index] = value;
            extension = ext.to_owned();
        }
        let group = parts[0].replace('\\', "/");
        let artifact = parts[1].to_owned();
        let version = parts[2].to_owned();
        let classifier = parts.get(3).map(|s| (*s).to_owned());
        if version.is_empty() || extension.is_empty() {
            return None;
        }
        Some(Self {
            group,
            artifact,
            version,
            classifier,
            extension,
        })
    }

    /// The relative repository path, e.g. `net/minecraft/launchwrapper/1.12/launchwrapper-1.12.jar`.
    pub fn path(&self) -> String {
        let mut name = format!("{}-{}", self.artifact, self.version);
        if let Some(classifier) = &self.classifier {
            name.push('-');
            name.push_str(classifier);
        }
        name.push('.');
        name.push_str(&self.extension);
        format!(
            "{}/{}/{}/{}",
            self.group.replace('.', "/"),
            self.artifact,
            self.version,
            name
        )
    }

    /// The classifier used for native libraries on `os` (e.g. `natives-windows`).
    pub fn native_classifier(&self, os: &str) -> Option<String> {
        self.classifier
            .clone()
            .or_else(|| Some(format!("natives-{os}")))
    }
}

impl Library {
    /// Whether this library applies to the current platform.
    pub fn applies_to_current_platform(&self) -> bool {
        let os = super::rules::current_os();
        self.rules.iter().all(|rule| rule.allows(&os))
    }

    /// The library's artifact download entry.
    ///
    /// Returns `None` when a `downloads` section exists without an
    /// `artifact` entry (natives-only libraries). Falls back to the default
    /// Maven repository URL for legacy entries without `downloads`.
    pub fn artifact_download(&self) -> Option<LibraryDownloadInfo> {
        let artifact = Artifact::parse(self.name.as_deref()?)?;
        if let Some(downloads) = &self.downloads {
            let mut entry = downloads.artifact.clone()?;
            if entry.path.is_none() {
                entry.path = Some(artifact.path());
            }
            return Some(entry);
        }
        let base = self
            .url
            .as_deref()
            .unwrap_or("https://libraries.minecraft.net/");
        Some(LibraryDownloadInfo {
            path: Some(artifact.path()),
            url: format!("{}/{}", base.trim_end_matches('/'), artifact.path()),
            sha1: None,
            size: None,
        })
    }

    /// The native artifact download entry for `os`.
    ///
    /// Returns `None` when the library has no natives for `os`, or when a
    /// `downloads` section exists without a matching classifier entry.
    pub fn native_download(&self, os: &str) -> Option<LibraryDownloadInfo> {
        let artifact = Artifact::parse(self.name.as_deref()?)?;
        let classifier = self.natives.as_ref()?.get(os)?;
        let mut artifact = artifact;
        artifact.classifier = Some(classifier.clone());
        artifact.extension = "jar".to_owned();

        if let Some(downloads) = &self.downloads {
            let mut entry = downloads.classifiers.as_ref()?.get(classifier)?.clone();
            if entry.path.is_none() {
                entry.path = Some(artifact.path());
            }
            return Some(entry);
        }
        let base = self
            .url
            .as_deref()
            .unwrap_or("https://libraries.minecraft.net/");
        Some(LibraryDownloadInfo {
            path: Some(artifact.path()),
            url: format!("{}/{}", base.trim_end_matches('/'), artifact.path()),
            sha1: None,
            size: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artifact_parse() {
        let artifact = Artifact::parse("net.minecraft:launchwrapper:1.12").unwrap();
        assert_eq!(artifact.group, "net.minecraft");
        assert_eq!(artifact.artifact, "launchwrapper");
        assert_eq!(artifact.version, "1.12");
        assert_eq!(artifact.classifier, None);
        assert_eq!(
            artifact.path(),
            "net/minecraft/launchwrapper/1.12/launchwrapper-1.12.jar"
        );

        let native = Artifact::parse("org.lwjgl:lwjgl:3.3.3:natives-windows").unwrap();
        assert_eq!(native.classifier.as_deref(), Some("natives-windows"));
        assert_eq!(
            native.path(),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-windows.jar"
        );

        let with_ext = Artifact::parse("org.ow2.asm:asm-all:5.2:zip").unwrap();
        assert_eq!(with_ext.classifier.as_deref(), Some("zip"));
        assert_eq!(with_ext.extension, "jar");

        let with_at_ext = Artifact::parse("com.example:mod:1.0:universal@zip").unwrap();
        assert_eq!(with_at_ext.classifier.as_deref(), Some("universal"));
        assert_eq!(with_at_ext.extension, "zip");
        assert_eq!(
            with_at_ext.path(),
            "com/example/mod/1.0/mod-1.0-universal.zip"
        );
    }

    #[test]
    fn test_library_download_resolution() {
        let json = r#"{
            "name": "net.minecraft:launchwrapper:1.12",
            "downloads": {
                "artifact": {
                    "path": "net/minecraft/launchwrapper/1.12/launchwrapper-1.12.jar",
                    "url": "https://libraries.minecraft.net/net/minecraft/launchwrapper/1.12/launchwrapper-1.12.jar",
                    "sha1": "1111111111111111111111111111111111111111",
                    "size": 30935
                }
            }
        }"#;
        let library: Library = serde_json::from_str(json).unwrap();
        let download = library.artifact_download().unwrap();
        assert!(download.url.contains("libraries.minecraft.net"));
        assert_eq!(download.size, Some(30935));
    }

    #[test]
    fn test_native_resolution() {
        let json = r#"{
            "name": "org.lwjgl:lwjgl:3.3.3",
            "natives": {
                "windows": "natives-windows",
                "linux": "natives-linux",
                "osx": "natives-macos"
            }
        }"#;
        let library: Library = serde_json::from_str(json).unwrap();
        let download = library.native_download("windows").unwrap();
        assert!(download.path.unwrap().contains("natives-windows"));
        assert!(library.native_download("linux").is_some());
        assert!(library.native_download("freebsd").is_none());
    }
}
