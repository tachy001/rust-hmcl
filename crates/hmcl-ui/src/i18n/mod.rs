//! Localization.
//!
//! Port of HMCL's i18n system (`org.jackhuang.hmcl.util.i18n`). Language
//! packs are Java `.properties` files stored as UTF-8 and shipped under
//! `assets/lang` (format-compatible with HMCL's `I18N_*.properties`).

mod properties;

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

pub use properties::parse_properties;

/// The list of locales we ship, in preference order for fallback.
const BUNDLED_LOCALES: &[&str] = &[
    "en", "zh_CN", "zh", "ja", "de", "es", "ar", "ru", "uk", "lzh",
];

/// Resolve the locale file name for `locale` (e.g. `zh_CN`), with fallbacks.
fn resolve_file(locale: &str) -> Option<String> {
    let normalized = locale.replace('-', "_");
    let candidates: Vec<String> = if normalized.contains('_') {
        let (lang, region) = normalized.split_once('_').unwrap();

        vec![
            format!("I18N_{lang}_{region}.properties"),
            format!("I18N_{lang}.properties"),
            "I18N.properties".to_owned(),
        ]
    } else {
        vec![
            format!("I18N_{normalized}.properties"),
            "I18N.properties".to_owned(),
        ]
    };
    candidates
        .into_iter()
        .find(|name| lang_dir().join(name).exists())
}

/// The directory containing the language pack files.
pub fn lang_dir() -> std::path::PathBuf {
    crate::assets_dir().join("lang")
}

/// A loaded language pack.
#[derive(Debug, Default)]
pub struct LanguagePack {
    entries: HashMap<String, String>,
}

impl LanguagePack {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()))?;
        Ok(Self {
            entries: parse_properties(&text),
        })
    }

    /// Translate `key`, returning the key itself when missing.
    pub fn tr(&self, key: &str) -> String {
        self.entries
            .get(key)
            .cloned()
            .unwrap_or_else(|| key.to_owned())
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }
}

/// The currently active language pack (defaults to system locale).
pub fn current() -> &'static LanguagePack {
    static PACK: OnceLock<LanguagePack> = OnceLock::new();
    PACK.get_or_init(|| {
        let locale = system_locale().unwrap_or_else(|| "en".to_owned());
        load_for_locale(&locale)
            .unwrap_or_else(|| load_for_locale("en").expect("bundled english language pack"))
    })
}

/// Load the best matching pack for `locale`.
pub fn load_for_locale(locale: &str) -> Option<LanguagePack> {
    resolve_file(locale).and_then(|file| LanguagePack::load(&lang_dir().join(file)).ok())
}

/// Detect the system locale as a `lang[_REGION]` string.
pub fn system_locale() -> Option<String> {
    let raw = sys_locale::get_locale()?;
    Some(raw.replace('-', "_"))
}

/// Translate `key` in the active language pack, returning an owned string.
pub fn tr(key: &str) -> String {
    current().tr(key)
}

/// Translate `key`, falling back to `default` when the key is missing.
pub fn tr_or(key: &str, default: &str) -> String {
    let pack = current();
    if pack.contains_key(key) {
        pack.tr(key)
    } else {
        default.to_owned()
    }
}

/// All bundled locales, in preference order.
pub fn bundled_locales() -> &'static [&'static str] {
    BUNDLED_LOCALES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let pack = LanguagePack::load(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/lang/I18N_zh_CN.properties"),
        )
        .unwrap();
        assert_eq!(pack.tr("launcher"), "启动器");
        assert!(pack.contains_key("launcher.agreement"));
    }

    #[test]
    fn test_parse_escapes() {
        let text = r#"# comment
! another comment
key=value
spaced=trimmed value
escaped=line1\
line2
unicode=\u4e2d
empty=
"#;
        let entries = parse_properties(text);
        assert_eq!(entries.get("key").unwrap(), "value");
        assert_eq!(entries.get("spaced").unwrap(), "trimmed value");
        assert_eq!(entries.get("escaped").unwrap(), "line1line2");
        assert_eq!(entries.get("unicode").unwrap(), "中");
        assert_eq!(entries.get("empty").unwrap(), "");
        assert!(!entries.contains_key("comment"));
    }

    #[test]
    fn test_resolve_file() {
        assert_eq!(
            resolve_file("zh_CN"),
            Some("I18N_zh_CN.properties".to_owned())
        );
        assert_eq!(resolve_file("zh-TW"), Some("I18N_zh.properties".to_owned()));
        assert_eq!(resolve_file("fr"), Some("I18N.properties".to_owned()));
    }
}
