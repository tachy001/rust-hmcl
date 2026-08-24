//! Download providers and remote resources.
//!
//! Port of HMCL's `org.jackhuang.hmcl.download` package: currently the
//! Mojang version manifest (piston-meta) and the download provider
//! abstraction (official source only, per project scope).

pub mod version_list;

pub use version_list::{fetch_version_manifest, RemoteVersion, VersionManifest, VersionType};

/// The Mojang download provider, mirroring `MojangDownloadProvider`.
pub const MOJANG_VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
