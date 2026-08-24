//! Minecraft game metadata: version JSON, libraries, rules and assets.
//!
//! Port of HMCL's `org.jackhuang.hmcl.game` package (serde-based model of
//! the Minecraft version manifest format).

pub mod arguments;
pub mod asset_index;
pub mod library;
pub mod rules;
pub mod version;

pub use arguments::{Argument, Arguments, StringArgument};
pub use asset_index::{AssetIndex, AssetObject};
pub use library::{Library, LibraryDownloadInfo, LibraryDownloads};
pub use rules::{CompatibilityRule, OperatingSystem, OSRestriction};
pub use version::{AssetIndexInfo, DownloadInfo, GameVersion, JavaVersion, LoggingInfo, VersionType};
