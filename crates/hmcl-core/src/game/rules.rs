//! Compatibility rules (`rules` in version JSON and libraries).
//!
//! Port of HMCL's `game.CompatibilityRule` and `game.OSRestriction`.

use serde::{Deserialize, Serialize};

/// An operating system family, mirroring `Platform.OperatingSystem`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatingSystem {
    Windows,
    Linux,
    Osx,
    Unknown,
}

impl OperatingSystem {
    /// The name used by the version JSON `os.name` field.
    pub fn name(&self) -> &'static str {
        match self {
            OperatingSystem::Windows => "windows",
            OperatingSystem::Linux => "linux",
            OperatingSystem::Osx => "osx",
            OperatingSystem::Unknown => "unknown",
        }
    }

    /// The JVM os.name property value (lowercased), matching HMCL behavior.
    pub fn java_property(&self) -> &'static str {
        match self {
            OperatingSystem::Windows => "windows",
            OperatingSystem::Linux => "linux",
            OperatingSystem::Osx => "osx",
            OperatingSystem::Unknown => "unknown",
        }
    }

    /// The natives suffix used in library names.
    pub fn natives_suffix(&self) -> &'static str {
        match self {
            OperatingSystem::Windows => "windows",
            OperatingSystem::Linux => "linux",
            OperatingSystem::Osx => "osx",
            OperatingSystem::Unknown => "unknown",
        }
    }

    /// The current operating system.
    pub fn current() -> Self {
        match std::env::consts::OS {
            "windows" => OperatingSystem::Windows,
            "macos" => OperatingSystem::Osx,
            "linux" => OperatingSystem::Linux,
            _ => OperatingSystem::Unknown,
        }
    }
}

/// The `os` restriction of a compatibility rule.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OSRestriction {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub arch: Option<String>,
}

impl OSRestriction {
    /// Whether this restriction allows `os`.
    pub fn allows(&self, os: &OperatingSystem) -> bool {
        if let Some(name) = &self.name
            && !os.name().eq_ignore_ascii_case(name)
        {
            return false;
        }
        if let Some(pattern) = &self.version {
            let version = current_os_version().unwrap_or_default();
            let matched = regex::Regex::new(pattern)
                .map(|regex| regex.is_match(&version))
                .unwrap_or(false);
            if !matched {
                return false;
            }
        }
        if let Some(arch) = &self.arch {
            let current_arch = std::env::consts::ARCH;
            let allowed = match arch.as_str() {
                "x86" => matches!(current_arch, "x86" | "x86_64"),
                "x86_64" => current_arch == "x86_64",
                "arm" => matches!(current_arch, "arm" | "aarch64"),
                other => current_arch == other,
            };
            if !allowed {
                return false;
            }
        }
        true
    }
}

/// The current OS version as `major.minor` (e.g. `10.0` for Windows 10/11).
pub fn current_os_version() -> Option<String> {
    #[cfg(windows)]
    {
        let version = winver::WindowsVersion::detect()?;
        Some(format!("{}.{}", version.major, version.minor))
    }
    #[cfg(not(windows))]
    {
        None
    }
}

/// A single compatibility rule with `action` + `os` + `features`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CompatibilityRule {
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default)]
    pub os: Option<OSRestriction>,
    #[serde(default)]
    pub features: Option<std::collections::HashMap<String, bool>>,
}

impl CompatibilityRule {
    /// Whether this rule allows the current platform.
    pub fn allows(&self, os: &OperatingSystem) -> bool {
        let os_allowed = self.os.as_ref().map(|restriction| restriction.allows(os)).unwrap_or(true);
        let features_allowed = true; // feature flags (e.g. custom_resolution) are client-driven
        let allowed = os_allowed && features_allowed;
        match self.action.as_deref() {
            Some("disallow") => !allowed,
            _ => allowed,
        }
    }
}

/// The current platform (used by library/rule resolution).
pub fn current_os() -> OperatingSystem {
    OperatingSystem::current()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_os_restriction() {
        let restriction: OSRestriction =
            serde_json::from_str(r#"{"name": "windows"}"#).unwrap();
        assert!(restriction.allows(&OperatingSystem::Windows));
        assert!(!restriction.allows(&OperatingSystem::Linux));
    }

    #[test]
    fn test_rule_allow_disallow() {
        let allow: CompatibilityRule =
            serde_json::from_str(r#"{"action": "allow", "os": {"name": "windows"}}"#).unwrap();
        assert!(allow.allows(&OperatingSystem::Windows));
        assert!(!allow.allows(&OperatingSystem::Osx));

        let disallow: CompatibilityRule =
            serde_json::from_str(r#"{"action": "disallow", "os": {"name": "osx"}}"#).unwrap();
        assert!(disallow.allows(&OperatingSystem::Windows));
        assert!(!disallow.allows(&OperatingSystem::Osx));
    }
}
