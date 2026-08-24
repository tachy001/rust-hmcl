//! The Mojang version manifest (piston-meta).
//!
//! Port of `org.jackhuang.hmcl.download.game.GameRemoteVersions`.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// One entry of the version manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteVersion {
    pub id: String,
    #[serde(rename = "type")]
    pub version_type: String,
    pub url: String,
    #[serde(rename = "releaseTime")]
    pub release_time: String,
    #[serde(rename = "sha1")]
    pub sha1: String,
}

/// The root of the version manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionManifest {
    pub latest: LatestVersions,
    pub versions: Vec<RemoteVersion>,
}

/// The `latest` snapshot/release pointers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LatestVersions {
    pub release: String,
    pub snapshot: String,
}

/// Release channel of a version entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionType {
    Release,
    Snapshot,
    OldBeta,
    OldAlpha,
    Unknown,
}

impl VersionType {
    pub fn of(version_type: &str) -> Self {
        match version_type {
            "release" => VersionType::Release,
            "snapshot" => VersionType::Snapshot,
            "old_beta" => VersionType::OldBeta,
            "old_alpha" => VersionType::OldAlpha,
            _ => VersionType::Unknown,
        }
    }

    pub fn label_key(&self) -> &'static str {
        match self {
            VersionType::Release => "version.category.release",
            VersionType::Snapshot => "version.category.snapshot",
            VersionType::OldBeta => "version.category.old_beta",
            VersionType::OldAlpha => "version.category.old_alpha",
            VersionType::Unknown => "version.category.unknown",
        }
    }
}

/// Fetch the version manifest from Mojang.
pub async fn fetch_version_manifest() -> anyhow::Result<VersionManifest> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;
    let response = client
        .get(crate::download::MOJANG_VERSION_MANIFEST_URL)
        .send()
        .await?
        .error_for_status()?;
    Ok(response.json().await?)
}
