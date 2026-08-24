//! Image loading and texture caching.
//!
//! Port of HMCL's `ui.image.ImageLoader`: decodes PNG/JPEG resources into
//! egui textures. Animated images (APNG) are handled by `AnimatedImage`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

use egui::{ColorImage, Context, TextureHandle, TextureOptions};

/// Global texture cache keyed by asset path.
pub fn cache() -> &'static Mutex<HashMap<String, TextureHandle>> {
    static CACHE: OnceLock<Mutex<HashMap<String, TextureHandle>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Decode an image file into a `ColorImage`.
pub fn decode_file(path: &Path) -> anyhow::Result<ColorImage> {
    let bytes = std::fs::read(path)
        .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()))?;
    decode_bytes(&bytes)
}

/// Decode raw PNG/JPEG bytes into a `ColorImage`.
pub fn decode_bytes(bytes: &[u8]) -> anyhow::Result<ColorImage> {
    let image = image::load_from_memory(bytes)
        .map_err(|e| anyhow::anyhow!("failed to decode image: {e}"))?
        .to_rgba8();
    let (width, height) = image.dimensions();
    let pixels = image.into_raw();
    Ok(ColorImage::from_rgba_unmultiplied(
        [width as usize, height as usize],
        &pixels,
    ))
}

/// Load a texture from an asset path, caching the result.
pub fn texture(ctx: &Context, asset: &str) -> Option<TextureHandle> {
    if let Some(handle) = cache().lock().unwrap().get(asset) {
        return Some(handle.clone());
    }
    let path = crate::assets_dir().join(asset);
    let image = decode_file(&path).ok()?;
    let handle = ctx.load_texture(
        asset.to_owned(),
        image,
        TextureOptions::LINEAR,
    );
    cache()
        .lock()
        .unwrap()
        .insert(asset.to_owned(), handle.clone());
    Some(handle)
}

/// Load a built-in wallpaper by id (e.g. `2021-08-26`).
pub fn wallpaper(ctx: &Context, id: &str) -> Option<TextureHandle> {
    if id == "none" {
        return None;
    }
    texture(ctx, &format!("img/wallpapers/{id}.jpg"))
}

/// A playing APNG animation, mirroring HMCL's `AnimationImage`.
pub struct AnimatedImage {
    frames: Vec<AnimatedFrame>,
    current: usize,
    elapsed: f32,
}

/// One decoded APNG frame with its display duration in seconds.
pub struct AnimatedFrame {
    pixels: Vec<u8>,
    width: u32,
    height: u32,
    delay: f32,
}

impl AnimatedImage {
    /// Load an APNG file.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(path)
            .map_err(|e| anyhow::anyhow!("failed to read {}: {e}", path.display()))?;
        Self::load_bytes(&bytes)
    }

    pub fn load_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        let apng = oxideav_png::decoder::decode_apng(bytes)
            .map_err(|e| anyhow::anyhow!("apng decode error: {e}"))?;
        let frames: Vec<AnimatedFrame> = apng
            .frames
            .into_iter()
            .map(|frame| {
                let delay = frame.delay_cs as f32 / 100.0;
                let (pixels, width, height) = png_image_to_rgba(&frame.image);
                AnimatedFrame {
                    pixels,
                    width,
                    height,
                    delay: if delay > 0.0 { delay } else { 0.1 },
                }
            })
            .collect();
        if frames.is_empty() {
            anyhow::bail!("apng contains no frames");
        }
        Ok(Self {
            frames,
            current: 0,
            elapsed: 0.0,
        })
    }

    pub fn is_animated(&self) -> bool {
        self.frames.len() > 1
    }

    pub fn width(&self) -> u32 {
        self.frames[0].width
    }

    pub fn height(&self) -> u32 {
        self.frames[0].height
    }

    /// Advance the animation by `dt` seconds and return the current frame.
    pub fn update(&mut self, dt: f32) -> &AnimatedFrame {
        self.elapsed += dt;
        while self.elapsed >= self.frames[self.current].delay {
            self.elapsed -= self.frames[self.current].delay;
            self.current = (self.current + 1) % self.frames.len();
        }
        &self.frames[self.current]
    }

    /// The current frame converted to an egui image.
    pub fn current_color_image(&self) -> ColorImage {
        let frame = &self.frames[self.current];
        ColorImage::from_rgba_unmultiplied(
            [frame.width as usize, frame.height as usize],
            &frame.pixels,
        )
    }
}

/// Convert an oxideav `PngImage` to flat RGBA8 pixels.
fn png_image_to_rgba(image: &oxideav_png::PngImage) -> (Vec<u8>, u32, u32) {
    use oxideav_png::PngPixelFormat;
    let width = image.width;
    let height = image.height;
    let count = width as usize * height as usize;
    let data = &image.data;
    let palette = &image.palette;
    let mut out = Vec::with_capacity(count * 4);
    match image.pixel_format {
        PngPixelFormat::Rgba => out.extend_from_slice(data),
        PngPixelFormat::Rgb24 => {
            for p in data.as_chunks::<3>().0 {
                out.extend_from_slice(&[p[0], p[1], p[2], 255]);
            }
        }
        PngPixelFormat::Gray8 => {
            for &v in data {
                out.extend_from_slice(&[v, v, v, 255]);
            }
        }
        PngPixelFormat::Gray16Le => {
            for p in data.as_chunks::<2>().0 {
                let v = p[1];
                out.extend_from_slice(&[v, v, v, 255]);
            }
        }
        PngPixelFormat::Ya8 => {
            for p in data.as_chunks::<2>().0 {
                out.extend_from_slice(&[p[0], p[0], p[0], p[1]]);
            }
        }
        PngPixelFormat::Rgb48Le => {
            for p in data.as_chunks::<6>().0 {
                out.extend_from_slice(&[p[1], p[3], p[5], 255]);
            }
        }
        PngPixelFormat::Rgba64Le => {
            for p in data.as_chunks::<8>().0 {
                out.extend_from_slice(&[p[1], p[3], p[5], p[7]]);
            }
        }
        PngPixelFormat::Pal8 => {
            for &index in data {
                let at = index as usize * 4;
                if at + 4 <= palette.len() {
                    out.extend_from_slice(&palette[at..at + 4]);
                } else {
                    out.extend_from_slice(&[0, 0, 0, 255]);
                }
            }
        }
    }
    if out.len() != count * 4 {
        tracing::warn!("unexpected decoded pixel buffer size for APNG frame");
        out.resize(count * 4, 0);
    }
    (out, width, height)
}
