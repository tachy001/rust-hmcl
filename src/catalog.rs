//! Read-only information for the HMCL game list. Refreshed with the local catalog,
//! rather than reading and resolving profiles during every UI frame.
use serde_json::Value;
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug)]
pub struct Description {
    pub icon: &'static str,
    pub subtitle: String,
}

pub fn load(root: &Path, ids: &[String]) -> BTreeMap<String, Description> {
    ids.iter()
        .map(|id| {
            let description = match crate::metadata::resolve(root, id) {
                Ok(profile) => describe(&profile),
                Err(_) => Description {
                    icon: "grass",
                    subtitle: "版本配置无法读取 · 可尝试校验 / 修复".into(),
                },
            };
            (id.clone(), description)
        })
        .collect()
}

fn describe(profile: &Value) -> Description {
    let minecraft = profile["_clientVersion"]
        .as_str()
        .or_else(|| profile["id"].as_str())
        .unwrap_or("未知版本");
    let libraries = profile["libraries"].as_array();
    let loader = libraries
        .into_iter()
        .flatten()
        .filter_map(|lib| lib["name"].as_str())
        .find_map(|name| {
            if let Some(version) = name.strip_prefix("net.fabricmc:fabric-loader:") {
                Some(("fabric", "Fabric", version))
            } else {
                name.strip_prefix("org.quiltmc:quilt-loader:")
                    .map(|version| ("quilt", "Quilt", version))
            }
        });
    match loader {
        Some((icon, name, version)) => Description {
            icon,
            subtitle: format!("Minecraft {minecraft} · {name} {version}"),
        },
        None => Description {
            icon: "grass",
            subtitle: format!("Minecraft {minecraft}"),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn inherited_profiles_show_game_and_loader_instead_of_instance_id() {
        let profile = json!({"id":"my-long-instance","_clientVersion":"1.20.1","libraries":[{"name":"net.fabricmc:fabric-loader:0.19.5"}]});
        let info = describe(&profile);
        assert_eq!(info.icon, "fabric");
        assert_eq!(info.subtitle, "Minecraft 1.20.1 · Fabric 0.19.5");
        let quilt = describe(
            &json!({"id":"1.21","libraries":[{"name":"org.quiltmc:quilt-loader:0.28.0"}]}),
        );
        assert_eq!(quilt.icon, "quilt");
        assert!(quilt.subtitle.contains("Quilt 0.28.0"));
    }
}
