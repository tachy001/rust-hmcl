//! Same default-skin selection as HMCL TexturesLoader / Java UUID.hashCode.
pub fn skin(player: &str) -> (&'static str, &'static [u8]) {
    let uuid = crate::game::offline_uuid(player);
    let bits = u128::from_str_radix(&uuid.replace('-', ""), 16).expect("generated offline UUID");
    let hash =
        ((bits >> 96) as u32 ^ (bits >> 64) as u32 ^ (bits >> 32) as u32 ^ bits as u32) as i32;
    let index = hash.rem_euclid(18) as usize;
    macro_rules! assets {
        ($model:literal) => {
            [
                (
                    concat!($model, "/alex"),
                    include_bytes!(concat!("../assets/hmcl/skins/", $model, "/alex.png"))
                        as &[u8],
                ),
                (
                    concat!($model, "/ari"),
                    include_bytes!(concat!("../assets/hmcl/skins/", $model, "/ari.png")) as &[u8],
                ),
                (
                    concat!($model, "/efe"),
                    include_bytes!(concat!("../assets/hmcl/skins/", $model, "/efe.png")) as &[u8],
                ),
                (
                    concat!($model, "/kai"),
                    include_bytes!(concat!("../assets/hmcl/skins/", $model, "/kai.png")) as &[u8],
                ),
                (
                    concat!($model, "/makena"),
                    include_bytes!(concat!("../assets/hmcl/skins/", $model, "/makena.png"))
                        as &[u8],
                ),
                (
                    concat!($model, "/noor"),
                    include_bytes!(concat!("../assets/hmcl/skins/", $model, "/noor.png"))
                        as &[u8],
                ),
                (
                    concat!($model, "/steve"),
                    include_bytes!(concat!("../assets/hmcl/skins/", $model, "/steve.png"))
                        as &[u8],
                ),
                (
                    concat!($model, "/sunny"),
                    include_bytes!(concat!("../assets/hmcl/skins/", $model, "/sunny.png"))
                        as &[u8],
                ),
                (
                    concat!($model, "/zuri"),
                    include_bytes!(concat!("../assets/hmcl/skins/", $model, "/zuri.png"))
                        as &[u8],
                ),
            ]
        };
    }
    if index < 9 {
        assets!("slim")[index]
    } else {
        assets!("wide")[index - 9]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_avatar_matches_java_uuid_hash_selection() {
        // Known Mojang offline UUID b50ad385-829d-3141-a216-7e7d7539ba7f.
        // UUID.hashCode with floorMod(hash, 18) selects index 4, slim Makena.
        assert_eq!(skin("Notch").0, "slim/makena");
        let (_, bytes) = skin("Notch");
        let image = image::load_from_memory(bytes).unwrap();
        assert_eq!(image.width(), 64);
        assert_eq!(image.height(), 64);
    }
}
