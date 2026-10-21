//! Loader discovery and recommendation, independent of UI and installation state.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{cmp::Ordering, collections::BTreeMap};

const FORGE_VERSIONS: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json";
const FORGE_PROMOTIONS: &str =
    "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoaderKind {
    Fabric,
    Quilt,
    Forge,
}

impl LoaderKind {
    pub fn validate_version(self, version: &str) -> Result<()> {
        if self == Self::Forge {
            if version.is_empty()
                || version.len() > 80
                || !version
                    .bytes()
                    .all(|c| c.is_ascii_digit() || c == b'.' || c == b'-')
                || !version.bytes().any(|c| c.is_ascii_digit())
            {
                bail!("Forge 版本只能包含数字、点和连字符");
            }
            Ok(())
        } else {
            validate_component(version)
        }
    }
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct LoaderRelease {
    pub version: String,
    pub stable: bool,
    pub recommended: bool,
    pub latest: bool,
}

#[derive(Debug, Serialize)]
pub struct LoaderVersions {
    pub instance_id: String,
    pub minecraft: String,
    pub loader: LoaderKind,
    pub install_supported: bool,
    pub reason: Option<String>,
    pub versions: Vec<LoaderRelease>,
}

/// Discovery only needs official JSON metadata; implementations must not install or launch anything.
pub trait CatalogSource {
    fn fetch_json(&self, url: &str) -> Result<Value>;
}

pub struct LoaderCatalog<'a> {
    source: &'a dyn CatalogSource,
}

impl<'a> LoaderCatalog<'a> {
    pub fn new(source: &'a dyn CatalogSource) -> Self {
        Self { source }
    }

    pub fn releases(&self, minecraft: &str, loader: LoaderKind) -> Result<Vec<LoaderRelease>> {
        validate_component(minecraft)?;
        let (mut releases, recommended, latest) = match loader {
            LoaderKind::Fabric | LoaderKind::Quilt => {
                let api = match loader {
                    LoaderKind::Fabric => "https://meta.fabricmc.net/v2",
                    _ => "https://meta.quiltmc.org/v3",
                };
                let data = self
                    .source
                    .fetch_json(&format!("{api}/versions/loader/{minecraft}"))?;
                let mut releases = BTreeMap::<String, LoaderRelease>::new();
                for item in data.as_array().context("加载器版本列表格式错误")? {
                    let entry = &item["loader"];
                    let version = entry["version"].as_str().context("加载器缺少版本号")?;
                    loader.validate_version(version)?;
                    let stable = entry["stable"]
                        .as_bool()
                        .unwrap_or_else(|| !version.contains('-'));
                    let release = releases.entry(version.into()).or_insert(LoaderRelease {
                        version: version.into(),
                        stable,
                        recommended: false,
                        latest: false,
                    });
                    // Conflicting duplicate records must never promote an explicitly unstable build.
                    release.stable &= stable;
                }
                (releases.into_values().collect::<Vec<_>>(), None, None)
            }
            LoaderKind::Forge => {
                let data = self.source.fetch_json(FORGE_VERSIONS)?;
                let values = match data
                    .as_object()
                    .context("Forge 版本列表格式错误")?
                    .get(minecraft)
                {
                    Some(value) => value.as_array().context("Forge 游戏版本列表格式错误")?,
                    None => return Ok(vec![]),
                };
                let mut releases = BTreeMap::new();
                let prefix = format!("{minecraft}-");
                for item in values {
                    let version = item
                        .as_str()
                        .and_then(|v| v.strip_prefix(&prefix))
                        .context("Forge 版本与基础游戏不匹配")?;
                    loader.validate_version(version)?;
                    releases.insert(
                        version.to_owned(),
                        LoaderRelease {
                            version: version.into(),
                            stable: true,
                            recommended: false,
                            latest: false,
                        },
                    );
                }
                let promotions = self.source.fetch_json(FORGE_PROMOTIONS)?;
                let promotions = promotions["promos"]
                    .as_object()
                    .context("Forge 推荐版本格式错误")?;
                let recommended = promotions
                    .get(&format!("{minecraft}-recommended"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                let latest = promotions
                    .get(&format!("{minecraft}-latest"))
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                // A promotion absent from the official version list is stale, not installable metadata.
                (releases.into_values().collect(), recommended, latest)
            }
        };
        releases.sort_by(|a, b| compare_versions(&b.version, &a.version));
        let latest = latest
            .filter(|value| releases.iter().any(|r| &r.version == value))
            .or_else(|| releases.first().map(|r| r.version.clone()));
        let recommended = recommended
            .filter(|value| releases.iter().any(|r| &r.version == value))
            .or_else(|| {
                if loader == LoaderKind::Forge {
                    latest.clone()
                } else {
                    releases
                        .iter()
                        .find(|r| r.stable)
                        .or(releases.first())
                        .map(|r| r.version.clone())
                }
            });
        for release in &mut releases {
            release.latest = latest.as_deref() == Some(&release.version);
            release.recommended = recommended.as_deref() == Some(&release.version);
        }
        Ok(releases)
    }

    pub fn recommended(&self, minecraft: &str, loader: LoaderKind) -> Result<String> {
        self.releases(minecraft, loader)?
            .into_iter()
            .find(|r| r.recommended)
            .map(|r| r.version)
            .context("该 Minecraft 版本没有可用加载器")
    }
}

pub(crate) fn validate_component(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value.as_bytes()[0].is_ascii_alphanumeric()
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b".-_+".contains(&c))
    {
        bail!("无效的游戏或加载器版本号");
    }
    Ok(())
}

fn compare_parts(a: &str, b: &str) -> Ordering {
    let mut a = a.split('.');
    let mut b = b.split('.');
    loop {
        let ordering = match (a.next(), b.next()) {
            (Some(a), Some(b)) => match (a.parse::<u64>(), b.parse::<u64>()) {
                (Ok(a), Ok(b)) => a.cmp(&b),
                _ => a.cmp(b),
            },
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            (None, None) => return Ordering::Equal,
        };
        if ordering != Ordering::Equal {
            return ordering;
        }
    }
}

fn compare_versions(a: &str, b: &str) -> Ordering {
    let a = a.split('+').next().unwrap_or(a);
    let b = b.split('+').next().unwrap_or(b);
    let (a, a_pre) = a.split_once('-').map_or((a, None), |(v, p)| (v, Some(p)));
    let (b, b_pre) = b.split_once('-').map_or((b, None), |(v, p)| (v, Some(p)));
    compare_parts(a, b).then_with(|| match (a_pre, b_pre) {
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => compare_parts(a, b),
        _ => Ordering::Equal,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;
    struct Source {
        responses: Vec<Value>,
        urls: RefCell<Vec<String>>,
    }
    impl CatalogSource for Source {
        fn fetch_json(&self, url: &str) -> Result<Value> {
            let index = self.urls.borrow().len();
            self.urls.borrow_mut().push(url.into());
            self.responses
                .get(index)
                .cloned()
                .context("unexpected network request")
        }
    }
    fn source(responses: Vec<Value>) -> Source {
        Source {
            responses,
            urls: RefCell::new(vec![]),
        }
    }

    #[test]
    fn shuffled_quilt_versions_choose_newest_stable_instead_of_first_beta() {
        let source = source(vec![json!([
            {"loader":{"version":"0.20.0-beta.9"}},
            {"loader":{"version":"0.29.0-beta.11"}},
            {"loader":{"version":"0.28.1"}},
            {"loader":{"version":"0.29.0-beta.9"}},
            {"loader":{"version":"0.28.1"}}
        ])]);
        let versions = LoaderCatalog::new(&source)
            .releases("1.20.1", LoaderKind::Quilt)
            .unwrap();
        assert_eq!(versions.len(), 4);
        assert_eq!(versions[0].version, "0.29.0-beta.11");
        assert!(versions[0].latest && !versions[0].recommended);
        assert_eq!(
            versions.iter().find(|v| v.recommended).unwrap().version,
            "0.28.1"
        );
    }

    #[test]
    fn explicit_stability_and_duplicate_conflicts_are_respected() {
        let source = source(vec![json!([
            {"loader":{"version":"0.20.1","stable":false}},
            {"loader":{"version":"0.19.5","stable":true}},
            {"loader":{"version":"0.19.5","stable":false}},
            {"loader":{"version":"0.9.0","stable":true}}
        ])]);
        assert_eq!(
            LoaderCatalog::new(&source)
                .recommended("1.20.1", LoaderKind::Fabric)
                .unwrap(),
            "0.9.0"
        );
    }

    #[test]
    fn forge_promotions_and_missing_game_are_handled_without_inventing_versions() {
        let response_source = source(vec![
            json!({"1.20.1":["1.20.1-47.4.9","1.20.1-47.4.10"]}),
            json!({"promos":{"1.20.1-recommended":"47.4.9","1.20.1-latest":"99.0.0"}}),
        ]);
        let versions = LoaderCatalog::new(&response_source)
            .releases("1.20.1", LoaderKind::Forge)
            .unwrap();
        assert_eq!(versions[0].version, "47.4.10");
        assert!(versions[0].latest);
        assert!(versions[1].recommended);
        let absent = source(vec![json!({})]);
        assert!(LoaderCatalog::new(&absent)
            .releases("1.20.1", LoaderKind::Forge)
            .unwrap()
            .is_empty());
        assert_eq!(absent.urls.borrow().len(), 1);
    }

    #[test]
    fn a_game_with_only_prereleases_recommends_the_newest_available_build() {
        let source = source(vec![json!([
            {"loader":{"version":"0.31.0-beta.9","stable":false}},
            {"loader":{"version":"0.31.0-beta.11","stable":false}}
        ])]);
        let versions = LoaderCatalog::new(&source)
            .releases("1.20.1", LoaderKind::Fabric)
            .unwrap();
        assert_eq!(versions[0].version, "0.31.0-beta.11");
        assert!(versions[0].recommended && versions[0].latest && !versions[0].stable);
    }

    #[test]
    fn invalid_requests_fail_before_network_and_invalid_remote_entries_fail_closed() {
        for value in [
            "",
            "../escape",
            "1.20.1?x=1",
            "1.20.1/other",
            "1.20.1#fragment",
        ] {
            let source = source(vec![]);
            assert!(LoaderCatalog::new(&source)
                .releases(value, LoaderKind::Fabric)
                .is_err());
            assert!(source.urls.borrow().is_empty());
        }
        for value in [json!({}), json!([{"loader":{"version":"../escape"}}])] {
            assert!(LoaderCatalog::new(&source(vec![value]))
                .releases("1.20.1", LoaderKind::Quilt)
                .is_err());
        }
        let malformed_forge = source(vec![json!({"1.20.1":["1.19.4-45.0.0"]})]);
        assert!(LoaderCatalog::new(&malformed_forge)
            .releases("1.20.1", LoaderKind::Forge)
            .is_err());
    }
}
