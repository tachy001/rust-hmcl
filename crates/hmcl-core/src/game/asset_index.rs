//! The asset index (`assets/indexes/<id>.json`).
//!
//! Port of HMCL's `game.AssetIndex` / `game.AssetObject`.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// The root of an asset index file.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AssetIndex {
    #[serde(default)]
    pub objects: HashMap<String, AssetObject>,
    #[serde(rename = "virtual", default)]
    pub virtual_: Option<bool>,
    #[serde(rename = "map_to_resources", default)]
    pub map_to_resources: Option<bool>,
}

/// One asset object: hash-addressed content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    #[serde(default)]
    pub size: u64,
}

impl AssetObject {
    /// The relative path under `assets/objects`: `<h>/<hash>`.
    pub fn object_path(&self) -> String {
        format!("{}/{}", &self.hash[0..2], self.hash)
    }
}

impl AssetIndex {
    /// Whether assets are stored in the hashed `objects` layout or mapped
    /// back into `resources`/`virtual` directories.
    pub fn is_virtual(&self) -> bool {
        self.virtual_.unwrap_or(false)
    }

    pub fn is_map_to_resources(&self) -> bool {
        self.map_to_resources.unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_object_path() {
        let object = AssetObject {
            hash: "0123456789abcdef0123456789abcdef01234567".to_owned(),
            size: 1234,
        };
        assert_eq!(object.object_path(), "01/0123456789abcdef0123456789abcdef01234567");
    }

    #[test]
    fn test_asset_index_parse() {
        let json = r#"{
            "virtual": true,
            "objects": {
                "minecraft/textures/block/stone.png": {"hash": "abc", "size": 42}
            }
        }"#;
        let index: AssetIndex = serde_json::from_str(json).unwrap();
        assert!(index.is_virtual());
        assert_eq!(index.objects.len(), 1);
    }
}
