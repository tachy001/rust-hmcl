//! Compatibility facade for the legacy Worker API. New code uses OfficialInstaller directly.
pub(crate) use crate::official_installer::validate_artifacts;
pub use crate::official_installer::Installed;

pub fn validate_version(version: &str) -> anyhow::Result<()> {
    crate::loader_catalog::LoaderKind::Forge.validate_version(version)
}

impl crate::game::Worker {
    pub fn install_forge(
        &self,
        config: &crate::Config,
        selected: &str,
        alias: Option<&str>,
        requested: Option<&str>,
        repair: bool,
    ) -> anyhow::Result<Installed> {
        crate::official_installer::OfficialInstaller::new(
            self,
            crate::official_installer::InstallerFamily::Forge,
        )
        .install(config, selected, alias, requested, repair)
    }
}
