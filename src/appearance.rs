use anyhow::{bail, Context, Result};
use sha1::{Digest, Sha1};
use std::{path::Path, sync::Arc};

pub struct Wallpaper {
    pub uri: String,
    pub bytes: Arc<[u8]>,
    pub dimensions: (u32, u32),
}
impl Wallpaper {
    pub fn load(path: &Path) -> Result<Self> {
        let metadata = std::fs::metadata(path).context("找不到壁纸文件")?;
        if metadata.len() > 32 * 1024 * 1024 {
            bail!("壁纸文件不能超过 32 MB");
        }
        let bytes = std::fs::read(path)?;
        let format = image::guess_format(&bytes).context("无法识别图片格式")?;
        if !matches!(format, image::ImageFormat::Png | image::ImageFormat::Jpeg) {
            bail!("请选择 PNG 或 JPEG 壁纸");
        }
        let dimensions = image::ImageReader::new(std::io::Cursor::new(&bytes))
            .with_guessed_format()?
            .into_dimensions()?;
        if dimensions.0 == 0
            || dimensions.1 == 0
            || dimensions.0 as u64 * dimensions.1 as u64 > 32_000_000
        {
            bail!("壁纸分辨率超出 3200 万像素限制");
        }
        let extension = if format == image::ImageFormat::Png {
            "png"
        } else {
            "jpg"
        };
        Ok(Self {
            uri: format!("bytes://wallpaper/{:x}.{extension}", Sha1::digest(&bytes)),
            bytes: bytes.into(),
            dimensions,
        })
    }
}
pub fn crop(image: (u32, u32), view: eframe::egui::Vec2) -> eframe::egui::Rect {
    let view = (view.x / view.y.max(1.)).max(0.01);
    let source = image.0.max(1) as f32 / image.1.max(1) as f32;
    let (u, v) = if view > source {
        (1., source / view)
    } else {
        (view / source, 1.)
    };
    eframe::egui::Rect::from_center_size(eframe::egui::pos2(0.5, 0.5), eframe::egui::vec2(u, v))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crop_preserves_aspect_and_stays_in_texture() {
        let uv = crop((1920, 1080), eframe::egui::vec2(800., 800.));
        assert!((uv.width() - 0.5625).abs() < 0.001);
        assert_eq!(uv.height(), 1.);
        let uv = crop((800, 800), eframe::egui::vec2(1600., 900.));
        assert_eq!(uv.width(), 1.);
        assert!((uv.height() - 0.5625).abs() < 0.001);
    }
}
