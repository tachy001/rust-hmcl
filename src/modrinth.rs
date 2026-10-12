use crate::{
    config::Config,
    game::{Event, Worker},
};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashSet, path::Path};

#[derive(Clone, Deserialize)]
pub struct Project {
    pub project_id: String,
    pub title: String,
    pub description: String,
    pub slug: String,
    pub downloads: u64,
}
pub fn environment(config: &Config) -> Result<(String, String)> {
    let meta = crate::metadata::resolve(Path::new(&config.game_dir), &config.selected)?;
    let game = meta["_clientVersion"]
        .as_str()
        .context("缺少基础版本")?
        .to_owned();
    let libraries = meta["libraries"].as_array().context("缺少依赖库")?;
    let loader = if libraries.iter().any(|l| {
        l["name"]
            .as_str()
            .is_some_and(|n| n.starts_with("org.quiltmc:quilt-loader:"))
    }) {
        "quilt"
    } else if libraries.iter().any(|l| {
        l["name"]
            .as_str()
            .is_some_and(|n| n.starts_with("net.fabricmc:fabric-loader:"))
    }) {
        "fabric"
    } else {
        bail!("请先选择 Fabric 或 Quilt 游戏版本，再搜索和安装模组");
    };
    Ok((game, loader.into()))
}
impl Worker {
    pub fn search_mods(&self, config: &Config, query: &str) -> Result<()> {
        let (game, loader) = environment(config)?;
        let facets = json!([
            ["project_type:mod"],
            [format!("versions:{game}")],
            [format!("categories:{loader}")]
        ])
        .to_string();
        let mut url = reqwest::Url::parse("https://api.modrinth.com/v2/search")?;
        url.query_pairs_mut()
            .append_pair("query", query)
            .append_pair("facets", &facets)
            .append_pair("limit", "20");
        self.status(format!("搜索 {game} / {loader} 兼容模组…"));
        let data = self.json(url.as_str())?;
        let projects: Vec<Project> = serde_json::from_value(data["hits"].clone())?;
        self.status(format!("找到 {} 个模组（最多显示 20 个）", projects.len()));
        self.tx.send(Event::Projects(projects))?;
        Ok(())
    }
    fn compatible_version(&self, project: &str, game: &str, loader: &str) -> Result<Value> {
        if !project
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            bail!("无效 Modrinth 项目 ID");
        }
        let mut url = reqwest::Url::parse(&format!(
            "https://api.modrinth.com/v2/project/{project}/version"
        ))?;
        url.query_pairs_mut()
            .append_pair("game_versions", &json!([game]).to_string())
            .append_pair("loaders", &json!([loader]).to_string());
        let versions = self.json(url.as_str())?;
        let versions = versions.as_array().context("模组版本列表格式错误")?;
        versions
            .iter()
            .find(|v| v["version_type"] == "release")
            .or_else(|| versions.first())
            .cloned()
            .context("没有与当前游戏及加载器兼容的模组版本")
    }
    pub fn install_mod(&self, config: &Config, project: &str) -> Result<()> {
        let (game, loader) = environment(config)?;
        let dir = crate::mods::directory(config)?;
        std::fs::create_dir_all(&dir)?;
        let version = self.compatible_version(project, &game, &loader)?;
        self.install_mod_version(&dir, &game, &loader, &version, &mut HashSet::new(), 0)?;
        self.status("模组及必需依赖安装完成");
        Ok(())
    }
    fn install_mod_version(
        &self,
        dir: &Path,
        game: &str,
        loader: &str,
        version: &Value,
        seen: &mut HashSet<String>,
        depth: usize,
    ) -> Result<()> {
        self.check()?;
        if depth > 32 {
            bail!("模组依赖层级过深");
        }
        let id = version["id"].as_str().context("模组版本缺少 ID")?;
        if !seen.insert(id.into()) {
            return Ok(());
        }
        validate_compatibility(version, game, loader)?;
        if let Some(dependencies) = version["dependencies"].as_array() {
            for dependency in dependencies {
                if dependency["dependency_type"] != "required" {
                    continue;
                }
                let required = if let Some(id) = dependency["version_id"].as_str() {
                    if !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
                        bail!("无效模组依赖 ID");
                    }
                    self.json(&format!("https://api.modrinth.com/v2/version/{id}"))?
                } else if let Some(project) = dependency["project_id"].as_str() {
                    self.compatible_version(project, game, loader)?
                } else {
                    bail!("模组包含无法自动下载的必需依赖，请手动安装");
                };
                self.install_mod_version(dir, game, loader, &required, seen, depth + 1)?;
            }
        }
        let files = version["files"].as_array().context("模组版本缺少文件")?;
        let file = files
            .iter()
            .find(|f| f["primary"].as_bool() == Some(true))
            .or_else(|| files.first())
            .context("模组没有可下载文件")?;
        let name = file["filename"].as_str().context("模组文件缺少名称")?;
        if !name.ends_with(".jar") {
            bail!("该项目文件不是 .jar 模组");
        }
        let destination = crate::game::safe_join(dir, name)?;
        let url = file["url"].as_str().context("模组缺少下载地址")?;
        crate::modpack::validate_url(url)?;
        let sha = file["hashes"]["sha1"]
            .as_str()
            .context("模组缺少 SHA-1 校验值")?;
        let registry = dir.join(".rust-modrinth.json");
        let mut state: Value = if registry.exists() {
            serde_json::from_slice(&std::fs::read(&registry)?)?
        } else {
            json!({})
        };
        let project = version["project_id"]
            .as_str()
            .context("模组版本缺少项目 ID")?;
        if destination.exists()
            && state[project]["filename"].as_str() != Some(name)
            && crate::game::sha1_file(&destination)? != sha
        {
            bail!("同名模组已存在，不会覆盖: {name}");
        }
        self.status(format!("下载模组 {name}"));
        self.download(url, &destination, Some(sha))?;
        if let Some(old) = state[project]["filename"].as_str() {
            let old_file = crate::game::safe_join(dir, old)?;
            if old != name && old_file.exists() {
                crate::mods::toggle(dir, old)?;
            }
        }
        state[project] = json!({"filename":name,"version":id});
        let pending = registry.with_extension("tmp");
        std::fs::write(&pending, serde_json::to_vec_pretty(&state)?)?;
        std::fs::rename(pending, registry)?;
        Ok(())
    }
}
fn validate_compatibility(version: &Value, game: &str, loader: &str) -> Result<()> {
    if !version["game_versions"]
        .as_array()
        .is_some_and(|v| v.iter().any(|v| v.as_str() == Some(game)))
        || !version["loaders"]
            .as_array()
            .is_some_and(|v| v.iter().any(|v| v.as_str() == Some(loader)))
    {
        bail!("模组依赖与当前 Minecraft / 加载器不兼容");
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn incompatible_dependency_is_rejected() {
        let value = json!({"game_versions":["1.20.1"],"loaders":["fabric"]});
        assert!(validate_compatibility(&value, "1.20.1", "fabric").is_ok());
        assert!(validate_compatibility(&value, "1.21", "fabric").is_err());
        assert!(validate_compatibility(&value, "1.20.1", "quilt").is_err());
    }
    #[test]
    #[ignore = "requires official Modrinth endpoint"]
    fn official_compatible_version() {
        let (tx, _rx) = std::sync::mpsc::channel();
        let worker = Worker::new(
            tx,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        )
        .unwrap();
        let version = worker
            .compatible_version("fabric-api", "1.20.1", "fabric")
            .unwrap();
        validate_compatibility(&version, "1.20.1", "fabric").unwrap();
        assert!(version["files"][0]["hashes"]["sha1"].is_string());
    }
}
