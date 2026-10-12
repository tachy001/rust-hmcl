use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::{collections::HashSet, fs, path::Path};

/// Resolve launcher profiles without mutating their on-disk representation.
pub fn resolve(root: &Path, id: &str) -> Result<Value> {
    resolve_inner(root, id, &mut HashSet::new(), 0)
}
fn resolve_inner(root: &Path, id: &str, seen: &mut HashSet<String>, depth: usize) -> Result<Value> {
    if depth > 16 || !seen.insert(id.into()) {
        bail!("版本继承存在循环或层级过深: {id}");
    }
    let dir = crate::game::version_dir(root, id)?;
    let child: Value = serde_json::from_slice(
        &fs::read(dir.join(format!("{id}.json")))
            .with_context(|| format!("找不到版本 {id} 的配置"))?,
    )?;
    if !child.is_object() {
        bail!("版本配置必须是 JSON 对象: {id}");
    }
    let mut merged = if let Some(parent) = child["inheritsFrom"].as_str() {
        merge(&resolve_inner(root, parent, seen, depth + 1)?, &child)?
    } else {
        let mut value = child;
        value["_clientVersion"] = json!(id);
        value
    };
    merged["id"] = json!(id);
    normalize_libraries(&mut merged)?;
    Ok(merged)
}
pub fn merge(parent: &Value, child: &Value) -> Result<Value> {
    if !parent.is_object() {
        bail!("基础版本配置应为 JSON 对象");
    }
    let mut result = parent.clone();
    let mut libraries = parent["libraries"].as_array().cloned().unwrap_or_default();
    if let Some(extra) = child["libraries"].as_array() {
        for lib in extra {
            let key = library_key(lib);
            libraries.retain(|old| library_key(old) != key);
            libraries.push(lib.clone());
        }
    }
    for (key, value) in child.as_object().context("版本配置应为 JSON 对象")? {
        if key == "libraries" || key == "inheritsFrom" {
            continue;
        }
        if key == "arguments" {
            for group in ["jvm", "game"] {
                let mut args = parent["arguments"][group]
                    .as_array()
                    .cloned()
                    .unwrap_or_default();
                if let Some(extra) = value[group].as_array() {
                    args.extend(extra.iter().cloned());
                }
                result["arguments"][group] = json!(args);
            }
        } else {
            result[key] = value.clone();
        }
    }
    if let Some(jar) = child["jar"].as_str() {
        result["_clientVersion"] = json!(jar);
    }
    result["libraries"] = json!(libraries);
    if let Some(object) = result.as_object_mut() {
        object.remove("inheritsFrom");
    }
    Ok(result)
}
fn library_key(lib: &Value) -> String {
    let name = lib["name"].as_str().unwrap_or("");
    let parts = name.split(':').collect::<Vec<_>>();
    format!(
        "{}:{}:{}",
        parts.first().unwrap_or(&""),
        parts.get(1).unwrap_or(&""),
        parts.get(3).unwrap_or(&"")
    )
}
pub fn maven_path(name: &str) -> Result<String> {
    let (name, extension) = name.split_once('@').unwrap_or((name, "jar"));
    let parts = name.split(':').collect::<Vec<_>>();
    if !(3..=4).contains(&parts.len()) || parts.iter().any(|s| s.is_empty()) {
        bail!("无效 Maven 坐标: {name}");
    }
    let suffix = parts.get(3).map(|c| format!("-{c}")).unwrap_or_default();
    let path = format!(
        "{}/{}/{}/{}-{}{suffix}.{extension}",
        parts[0].replace('.', "/"),
        parts[1],
        parts[2],
        parts[1],
        parts[2]
    );
    crate::game::safe_join(Path::new("libraries"), &path)?;
    Ok(path)
}
pub fn normalize_libraries(meta: &mut Value) -> Result<()> {
    if let Some(libraries) = meta["libraries"].as_array_mut() {
        for lib in libraries {
            if lib["downloads"]["artifact"].is_object() {
                continue;
            }
            // Native-only entries must not be added to the Java classpath.
            if lib.get("natives").is_some() {
                continue;
            }
            let path = maven_path(lib["name"].as_str().context("依赖库缺少名称")?)?;
            let base = lib["url"]
                .as_str()
                .unwrap_or("https://libraries.minecraft.net/");
            let url = format!("{}/{}", base.trim_end_matches('/'), path);
            lib["downloads"] = json!({"artifact":{"path":path,"url":url}});
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn maven_coordinates() {
        assert_eq!(
            maven_path("net.fabricmc:fabric-loader:0.16.0").unwrap(),
            "net/fabricmc/fabric-loader/0.16.0/fabric-loader-0.16.0.jar"
        );
        assert_eq!(
            maven_path("a.b:c:1:natives-linux").unwrap(),
            "a/b/c/1/c-1-natives-linux.jar"
        );
        assert!(maven_path("bad").is_err());
        assert!(maven_path("a:b:../../x").is_err());
    }
    #[test]
    fn child_overrides_library_and_appends_arguments() {
        let base = json!({"id":"1.20.1","_clientVersion":"1.20.1","mainClass":"vanilla",
            "libraries":[{"name":"a:b:1"},{"name":"a:c:1"}],"arguments":{"jvm":["-cp","${classpath}"],"game":["--username","${auth_player_name}"]}});
        let child = json!({"id":"fabric","inheritsFrom":"1.20.1","mainClass":"fabric.Main","libraries":[{"name":"a:b:2"}],"arguments":{"game":["--custom"]}});
        let merged = merge(&base, &child).unwrap();
        assert_eq!(merged["mainClass"], "fabric.Main");
        assert_eq!(merged["_clientVersion"], "1.20.1");
        assert_eq!(merged["libraries"].as_array().unwrap().len(), 2);
        assert_eq!(merged["arguments"]["game"].as_array().unwrap().len(), 3);
        assert!(merged.get("inheritsFrom").is_none());
    }
    #[test]
    fn cyclic_inheritance_is_rejected() {
        let root = std::env::temp_dir().join(format!("rust-hmcl-cycle-{}", std::process::id()));
        let dir = crate::game::version_dir(&root, "a").unwrap();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.json"), br#"{"inheritsFrom":"a"}"#).unwrap();
        assert!(resolve(&root, "a")
            .unwrap_err()
            .to_string()
            .contains("循环"));
        fs::remove_dir_all(root).unwrap();
    }
}
