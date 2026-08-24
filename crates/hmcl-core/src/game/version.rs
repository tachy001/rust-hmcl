//! The version JSON model (`versions/<id>/<id>.json`).
//!
//! Port of HMCL's `GameInstanceManifest` fields (vanilla subset plus
//! inheritance support).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::{Arguments, CompatibilityRule, Library};

/// Asset index metadata (`assetIndex`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetIndexInfo {
    pub id: String,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(rename = "totalSize", default)]
    pub total_size: Option<u64>,
}

/// Download metadata for a file (`downloads` / `artifact`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DownloadInfo {
    #[serde(default)]
    pub path: Option<String>,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

/// Logging client configuration (`logging`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingInfo {
    pub file: DownloadInfo,
    pub argument: String,
    #[serde(rename = "type")]
    pub logging_type: String,
}

/// The `javaVersion` requirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JavaVersion {
    pub component: String,
    #[serde(rename = "majorVersion", default)]
    pub major_version: Option<u32>,
}

/// The release channel of a version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionType {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
    Custom,
    #[serde(other)]
    Unknown,
}

/// A Minecraft version manifest, supporting `inheritsFrom` chains.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GameVersion {
    pub id: String,
    #[serde(rename = "inheritsFrom", default)]
    pub inherits_from: Option<String>,
    #[serde(default)]
    pub jar: Option<String>,
    #[serde(rename = "minecraftArguments", default)]
    pub minecraft_arguments: Option<String>,
    #[serde(default)]
    pub arguments: Option<Arguments>,
    #[serde(rename = "mainClass", default)]
    pub main_class: Option<String>,
    #[serde(rename = "assetIndex", default)]
    pub asset_index: Option<AssetIndexInfo>,
    #[serde(default)]
    pub assets: Option<String>,
    #[serde(rename = "javaVersion", default)]
    pub java_version: Option<JavaVersion>,
    #[serde(default)]
    pub libraries: Vec<Library>,
    #[serde(default)]
    pub compatibility_rules: Vec<CompatibilityRule>,
    #[serde(default)]
    pub downloads: HashMap<String, DownloadInfo>,
    #[serde(default)]
    pub logging: HashMap<String, LoggingInfo>,
    #[serde(default, rename = "type")]
    pub release_type: Option<VersionType>,
    #[serde(rename = "minimumLauncherVersion", default)]
    pub minimum_launcher_version: Option<i32>,
    #[serde(rename = "mainClassOverride", default)]
    pub main_class_override: Option<String>,
}

impl GameVersion {
    /// The client jar download info, falling back to the legacy URL scheme.
    pub fn client_download(&self) -> DownloadInfo {
        if let Some(client) = self.downloads.get("client") {
            return client.clone();
        }
        let jar_id = self.jar.as_deref().unwrap_or(&self.id);
        DownloadInfo {
            path: None,
            url: format!("https://launcher.mojang.com/v1/objects/{jar_id}/{jar_id}.jar"),
            sha1: None,
            size: None,
        }
    }

    /// The asset index info, falling back to known legacy indexes.
    pub fn asset_index_info(&self) -> AssetIndexInfo {
        if let Some(index) = &self.asset_index {
            return index.clone();
        }
        let assets_id = self.assets.as_deref().unwrap_or("legacy");
        let hash = match assets_id {
            "1.8" => "f6ad102bcaa53b1a58358f16e376d548d44933ec",
            "1.7.10" => "1863782e33ce7b584fc45b037325a1964e095d3e",
            "1.7.4" => "545510a60f526b9aa8a38f9c0bc7a74235d21675",
            "pre-1.6" => "3d8e55480977e32acd9844e545177e69a52f594b",
            _ => "770572e819335b6c0a053f8378ad88eda189fc14",
        };
        AssetIndexInfo {
            id: assets_id.to_owned(),
            url: format!("https://piston-meta.mojang.com/v1/packages/{hash}/{assets_id}.json"),
            sha1: None,
            size: None,
            total_size: None,
        }
    }

    /// Merge `parent` into `self`, resolving one inheritance level.
    pub fn merge_with(&self, parent: &GameVersion) -> GameVersion {
        let mut merged = self.clone();
        merged.inherits_from = None;
        if merged.minecraft_arguments.is_none() {
            merged.minecraft_arguments = parent.minecraft_arguments.clone();
        }
        if merged.arguments.is_none() {
            merged.arguments = parent.arguments.clone();
        }
        if merged.main_class.is_none() {
            merged.main_class = parent.main_class.clone();
        }
        if merged.asset_index.is_none() {
            merged.asset_index = parent.asset_index.clone();
        }
        if merged.assets.is_none() {
            merged.assets = parent.assets.clone();
        }
        if merged.java_version.is_none() {
            merged.java_version = parent.java_version.clone();
        }
        merged.libraries.extend(parent.libraries.iter().cloned());
        merged
            .compatibility_rules
            .extend(parent.compatibility_rules.iter().cloned());
        for (key, value) in &parent.downloads {
            merged
                .downloads
                .entry(key.clone())
                .or_insert_with(|| value.clone());
        }
        if merged.logging.is_empty() {
            merged.logging = parent.logging.clone();
        }
        merged
    }

    /// The java major version required by this version (default 8).
    pub fn java_major_version(&self) -> u32 {
        self.java_version
            .as_ref()
            .and_then(|java| java.major_version)
            .unwrap_or(8)
    }
}
