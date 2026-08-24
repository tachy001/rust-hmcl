//! Download providers and remote resources.
//!
//! Port of HMCL's `org.jackhuang.hmcl.download` package: currently the
//! Mojang version manifest (piston-meta), file downloading and the
//! vanilla install task.

pub mod file;
pub mod install;
pub mod version_list;

pub use file::{DownloadProgress, download_file, file_matches_sha1, file_matches_size};
pub use install::{InstallStatus, InstallTask, fetch_resolved_version, spawn_install};
pub use version_list::{RemoteVersion, VersionManifest, VersionType, fetch_version_manifest};

/// The Mojang download provider, mirroring `MojangDownloadProvider`.
pub const MOJANG_VERSION_MANIFEST_URL: &str =
    "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";

/// The default game directory (`<data dir>/.minecraft`).
pub fn default_game_dir(data_dir: &std::path::Path) -> std::path::PathBuf {
    data_dir.join(".minecraft")
}
