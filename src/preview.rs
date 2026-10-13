//! Software rendering of the actual egui UI, without a window or OS input.
//! This is a layout preview, not a desktop screenshot or a GPU validation.
use anyhow::{bail, Context, Result};
use eframe::egui::{
    self,
    epaint::{ClippedPrimitive, ImageData, Primitive},
    Color32, TextureId,
};
use std::{collections::HashMap, path::Path};

struct Texture {
    size: [usize; 2],
    pixels: Vec<Color32>,
    nearest: bool,
}

pub fn motion(args: &[String]) -> Result<()> {
    let path = Path::new(
        args.first()
            .context("用法：--render-motion <输出目录> [light|dark] [normal|reduced] [宽] [高]")?,
    );
    if path.exists() && std::fs::read_dir(path)?.next().is_some() {
        bail!("动画预览输出目录必须为空，避免覆盖已有文件");
    }
    let mut config = crate::Config::load()?;
    config.start_page = "instance".into();
    config.dark_mode = match args.get(1).map(String::as_str).unwrap_or("light") {
        "light" => false,
        "dark" => true,
        _ => bail!("主题应为 light 或 dark"),
    };
    config.reduce_motion = match args.get(2).map(String::as_str).unwrap_or("normal") {
        "normal" => false,
        "reduced" => true,
        _ => bail!("动画模式应为 normal 或 reduced"),
    };
    let width = args
        .get(3)
        .map(|v| v.parse::<u32>())
        .transpose()?
        .unwrap_or(1080);
    let height = args
        .get(4)
        .map(|v| v.parse::<u32>())
        .transpose()?
        .unwrap_or(700);
    if !(800..=2560).contains(&width) || !(490..=1440).contains(&height) {
        bail!("预览尺寸超出支持范围");
    }
    let ctx = egui::Context::default();
    let mut launcher = crate::Launcher::with_config(&ctx, config, String::new());
    if !launcher.installed.contains(&launcher.config.selected) {
        bail!("实例分页动画预览需要选择已安装实例");
    }
    let mut renderer = SoftwareFrames::default();
    for time in [0., 0.2, 0.6, 0.8] {
        renderer.draw(&ctx, &mut launcher, time, width, height)?;
    }
    std::fs::create_dir_all(path)?;
    let before = renderer.draw(&ctx, &mut launcher, 0.9, width, height)?;
    before.save(path.join("before.png"))?;
    launcher.instance_tab = crate::instance_ui::Tab::Components;
    for ms in [0, 40, 80, 160, 240, 400, 500] {
        renderer
            .draw(&ctx, &mut launcher, 1. + ms as f64 / 1000., width, height)?
            .save(path.join(format!("frame-{ms:03}ms.png")))?;
    }
    std::fs::write(path.join("README.txt"),"Actual egui instance page, Settings -> Components. Independent offscreen frames; no desktop capture, OS input, Java, installation or config save. before.png precedes the tab change; frame timestamps are measured from the change. Native window corner clipping is not part of these frames.\n")?;
    println!("实际分页动画离屏帧：{}", path.display());
    Ok(())
}

#[derive(Default)]
struct SoftwareFrames {
    textures: HashMap<TextureId, Texture>,
}
impl SoftwareFrames {
    fn draw(
        &mut self,
        ctx: &egui::Context,
        launcher: &mut crate::Launcher,
        time: f64,
        width: u32,
        height: u32,
    ) -> Result<image::RgbaImage> {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width as f32, height as f32),
                )),
                time: Some(time),
                ..Default::default()
            },
            |ctx| launcher.draw(ctx),
        );
        for (id, delta) in output.textures_delta.set {
            let size = delta.image.size();
            let nearest = delta.options.magnification == egui::TextureFilter::Nearest;
            let pixels = match delta.image {
                ImageData::Color(image) => image.pixels.clone(),
                ImageData::Font(image) => image.srgba_pixels(None).collect(),
            };
            if let Some([x, y]) = delta.pos {
                let texture = self
                    .textures
                    .get_mut(&id)
                    .context("预览纹理增量缺少基础图像")?;
                if x + size[0] > texture.size[0] || y + size[1] > texture.size[1] {
                    bail!("预览纹理增量越界");
                }
                for row in 0..size[1] {
                    let start = (y + row) * texture.size[0] + x;
                    texture.pixels[start..start + size[0]]
                        .copy_from_slice(&pixels[row * size[0]..(row + 1) * size[0]]);
                }
            } else {
                self.textures.insert(
                    id,
                    Texture {
                        size,
                        pixels,
                        nearest,
                    },
                );
            }
        }
        let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
        let image = rasterize(
            &primitives,
            &self.textures,
            width,
            height,
            output.pixels_per_point,
        )?;
        for id in output.textures_delta.free {
            self.textures.remove(&id);
        }
        Ok(image)
    }
}

pub fn run(args: &[String]) -> Result<()> {
    let path = Path::new(
        args.first()
            .context("用法：--render-ui <PNG 路径> [页面] [light|dark] [宽] [高]")?,
    );
    let page = crate::Page::from_key(args.get(1).map(String::as_str).unwrap_or("home"));
    if let Some(key) = args.get(1) {
        if page.key() != key {
            bail!("未知页面：{key}");
        }
    }
    let mut config = crate::Config::load()?;
    if let Some(theme) = args.get(2) {
        config.dark_mode = match theme.as_str() {
            "light" => false,
            "dark" => true,
            _ => bail!("主题应为 light 或 dark"),
        };
    }
    let width = args
        .get(3)
        .map(|v| v.parse::<u32>())
        .transpose()?
        .unwrap_or(1080);
    let height = args
        .get(4)
        .map(|v| v.parse::<u32>())
        .transpose()?
        .unwrap_or(700);
    if !(800..=2560).contains(&width) || !(490..=1440).contains(&height) {
        bail!("预览尺寸应在 800×490 至 2560×1440 之间");
    }
    config.start_page = page.key().into();
    let operation = args.get(5).and_then(|key| match key.as_str() {
        "duplicate" => Some(false),
        "rename" => Some(true),
        _ => None,
    });
    let tab = args
        .get(5)
        .filter(|_| operation.is_none())
        .map(|key| crate::instance_ui::Tab::from_key(key).context("未知实例管理分页"))
        .transpose()?;
    if tab.is_some() && page != crate::Page::Instance {
        bail!("仅 instance 页面支持指定分页");
    }
    let screenshot_dialog = args.get(6).map(String::as_str);
    if screenshot_dialog.is_some()
        && (page != crate::Page::Instance
            || !matches!(
                tab,
                Some(crate::instance_ui::Tab::Screenshots | crate::instance_ui::Tab::Resources)
            ))
    {
        bail!("内容弹窗预览仅支持 instance 页的 screenshots 或 resources 分页");
    }
    let image = render(config, width, height, tab, operation, screenshot_dialog)?;
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    image.save(path)?;
    println!("离屏界面预览（非桌面截图）：{}", path.display());
    Ok(())
}

fn render(
    config: crate::Config,
    width: u32,
    height: u32,
    tab: Option<crate::instance_ui::Tab>,
    operation: Option<bool>,
    screenshot_dialog: Option<&str>,
) -> Result<image::RgbaImage> {
    let page = crate::Page::from_key(&config.start_page);
    let ctx = egui::Context::default();
    let mut launcher = crate::Launcher::with_config(&ctx, config, String::new());
    if let Some(rename) = operation {
        let id = launcher.config.selected.clone();
        if !launcher.installed.contains(&id) {
            bail!("预览操作对话框需要选择已安装实例");
        }
        launcher.apply_version_action(
            &id,
            if rename {
                crate::VersionAction::Rename
            } else {
                crate::VersionAction::Duplicate
            },
        );
    }
    if page == crate::Page::Java {
        launcher.runtimes = crate::java_runtime::discover(
            &launcher.config.java_paths,
            &launcher.config.hidden_java,
            &std::sync::atomic::AtomicBool::new(false),
        )?;
        launcher.java_loaded = true;
    }
    if let Some(tab) = tab {
        launcher.instance_tab = tab;
        if tab == crate::instance_ui::Tab::Resources {
            launcher
                .resourcepacks
                .prepare_preview(&launcher.config, &ctx)?;
            if let Some(mode) = screenshot_dialog {
                launcher.resourcepacks.prepare_dialog_preview(mode)?;
            }
        }
        if tab == crate::instance_ui::Tab::Screenshots {
            launcher.screenshots.prepare_preview(
                crate::game::instance_dir(&launcher.config)?.join("screenshots"),
                &ctx,
            )?;
            if let Some(mode) = screenshot_dialog {
                launcher.screenshots.prepare_dialog_preview(mode, &ctx)?;
            }
        }
    }
    if page == crate::Page::Mods {
        launcher.refresh_mods();
    }
    let mut textures = HashMap::<TextureId, Texture>::new();
    let mut primitives = vec![];
    let mut pixels_per_point = 1.;
    for frame in 0..6 {
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width as f32, height as f32),
                )),
                time: Some(frame as f64 / 5.), // Inspect settled layouts, after area/modal fade-in.
                ..Default::default()
            },
            |ctx| launcher.draw(ctx),
        );
        for (id, delta) in output.textures_delta.set {
            let size = delta.image.size();
            let nearest = delta.options.magnification == egui::TextureFilter::Nearest;
            let pixels = match delta.image {
                ImageData::Color(image) => image.pixels.clone(),
                ImageData::Font(image) => image.srgba_pixels(None).collect(),
            };
            if let Some([x, y]) = delta.pos {
                let texture = textures.get_mut(&id).context("预览纹理增量缺少基础图像")?;
                if x + size[0] > texture.size[0] || y + size[1] > texture.size[1] {
                    bail!("预览纹理增量越界");
                }
                for row in 0..size[1] {
                    let start = (y + row) * texture.size[0] + x;
                    texture.pixels[start..start + size[0]]
                        .copy_from_slice(&pixels[row * size[0]..(row + 1) * size[0]]);
                }
            } else {
                textures.insert(
                    id,
                    Texture {
                        size,
                        pixels,
                        nearest,
                    },
                );
            }
        }
        pixels_per_point = output.pixels_per_point;
        primitives = ctx.tessellate(output.shapes, pixels_per_point);
        if frame < 5 {
            for id in output.textures_delta.free {
                textures.remove(&id);
            }
        }
    }
    rasterize(&primitives, &textures, width, height, pixels_per_point)
}

fn sample(texture: &Texture, uv: egui::Pos2) -> [f32; 4] {
    if texture.nearest {
        let x = (uv.x * texture.size[0] as f32)
            .floor()
            .max(0.)
            .min((texture.size[0] - 1) as f32) as usize;
        let y = (uv.y * texture.size[1] as f32)
            .floor()
            .max(0.)
            .min((texture.size[1] - 1) as f32) as usize;
        return texture.pixels[y * texture.size[0] + x]
            .to_array()
            .map(|v| v as f32 / 255.);
    }
    let x = (uv.x * texture.size[0] as f32 - 0.5).clamp(0., (texture.size[0] - 1) as f32);
    let y = (uv.y * texture.size[1] as f32 - 0.5).clamp(0., (texture.size[1] - 1) as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = (
        (x0 + 1).min(texture.size[0] - 1),
        (y0 + 1).min(texture.size[1] - 1),
    );
    let (dx, dy) = (x - x0 as f32, y - y0 as f32);
    let mut result = [0.; 4];
    for (px, py, weight) in [
        (x0, y0, (1. - dx) * (1. - dy)),
        (x1, y0, dx * (1. - dy)),
        (x0, y1, (1. - dx) * dy),
        (x1, y1, dx * dy),
    ] {
        let rgba = texture.pixels[py * texture.size[0] + px].to_array();
        for channel in 0..4 {
            result[channel] += rgba[channel] as f32 / 255. * weight;
        }
    }
    result
}

fn rasterize(
    primitives: &[ClippedPrimitive],
    textures: &HashMap<TextureId, Texture>,
    width: u32,
    height: u32,
    scale: f32,
) -> Result<image::RgbaImage> {
    let mut image = image::RgbaImage::from_pixel(width, height, image::Rgba([28, 30, 39, 255]));
    for primitive in primitives {
        let Primitive::Mesh(mesh) = &primitive.primitive else {
            bail!("软件预览不支持原生绘制回调");
        };
        let texture = textures
            .get(&mesh.texture_id)
            .context("界面使用了尚未加载的纹理")?;
        let clip = primitive.clip_rect;
        for indices in mesh.indices.as_chunks::<3>().0 {
            let v = [
                mesh.vertices[indices[0] as usize],
                mesh.vertices[indices[1] as usize],
                mesh.vertices[indices[2] as usize],
            ];
            let p = v.map(|v| v.pos * scale);
            let edge = |a: egui::Pos2, b: egui::Pos2, c: egui::Pos2| {
                (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
            };
            let area = edge(p[0], p[1], p[2]);
            if area.abs() < 0.00001 {
                continue;
            }
            let min_x = p
                .iter()
                .map(|p| p.x)
                .fold(f32::INFINITY, f32::min)
                .max(clip.left() * scale)
                .floor()
                .max(0.) as u32;
            let min_y = p
                .iter()
                .map(|p| p.y)
                .fold(f32::INFINITY, f32::min)
                .max(clip.top() * scale)
                .floor()
                .max(0.) as u32;
            let max_x = p
                .iter()
                .map(|p| p.x)
                .fold(f32::NEG_INFINITY, f32::max)
                .min(clip.right() * scale)
                .ceil()
                .max(0.)
                .min(width as f32) as u32;
            let max_y = p
                .iter()
                .map(|p| p.y)
                .fold(f32::NEG_INFINITY, f32::max)
                .min(clip.bottom() * scale)
                .ceil()
                .max(0.)
                .min(height as f32) as u32;
            for y in min_y..max_y {
                for x in min_x..max_x {
                    let q = egui::pos2(x as f32 + 0.5, y as f32 + 0.5);
                    if !clip.contains(q / scale) {
                        continue;
                    }
                    let weights = [
                        edge(p[1], p[2], q) / area,
                        edge(p[2], p[0], q) / area,
                        edge(p[0], p[1], q) / area,
                    ];
                    if weights.iter().any(|w| *w < 0.) {
                        continue;
                    }
                    // Shared triangle edges belong to exactly one triangle. Otherwise
                    // translucent panels would be blended twice along their diagonals.
                    let top_left = |mut a: egui::Pos2, mut b: egui::Pos2| {
                        if area < 0. {
                            std::mem::swap(&mut a, &mut b);
                        }
                        b.y < a.y || b.y == a.y && b.x > a.x
                    };
                    if [(1, 2), (2, 0), (0, 1)]
                        .iter()
                        .enumerate()
                        .any(|(i, (a, b))| weights[i] == 0. && !top_left(p[*a], p[*b]))
                    {
                        continue;
                    }
                    let uv = egui::pos2(
                        (0..3).map(|i| v[i].uv.x * weights[i]).sum(),
                        (0..3).map(|i| v[i].uv.y * weights[i]).sum(),
                    );
                    let texel = sample(texture, uv);
                    let colors = v.map(|v| v.color.to_array());
                    let rgba: [f32; 4] = std::array::from_fn(|c| {
                        texel[c]
                            * (0..3)
                                .map(|i| colors[i][c] as f32 / 255. * weights[i])
                                .sum::<f32>()
                    });
                    let dst = image.get_pixel_mut(x, y);
                    for c in 0..3 {
                        dst[c] = (rgba[c] * 255. + dst[c] as f32 * (1. - rgba[3]))
                            .round()
                            .clamp(0., 255.) as u8;
                    }
                }
            }
        }
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wallpaper_is_behind_title_and_page_panels() {
        for dark in [false, true] {
            let config = crate::Config {
                game_dir: "__absent_preview_game_directory__".into(),
                start_page: "versions".into(),
                dark_mode: dark,
                ..Default::default()
            };
            let image = render(config, 800, 490, None, None, None).unwrap();
            let expected = if dark { [49, 61, 113] } else { [222, 225, 255] };
            assert_eq!(&image.get_pixel(600, 10).0[..3], &expected);
            // The center page is almost opaque, unlike the wallpaper Area bug.
            let left = image.get_pixel(300, 470);
            let right = image.get_pixel(740, 470);
            assert!((left[0] as i32 - right[0] as i32).abs() < 20);
        }
    }
    #[test]
    fn clipping_and_alpha_are_applied_without_drawing_outside_ui() {
        let id = TextureId::Managed(0);
        let mut textures = HashMap::new();
        textures.insert(
            id,
            Texture {
                size: [1, 1],
                pixels: vec![Color32::WHITE],
                nearest: false,
            },
        );
        let mut mesh = egui::Mesh::with_texture(id);
        mesh.add_colored_rect(
            egui::Rect::from_min_max(egui::pos2(0., 0.), egui::pos2(8., 8.)),
            Color32::from_rgba_unmultiplied(255, 0, 0, 128),
        );
        let image = rasterize(
            &[ClippedPrimitive {
                clip_rect: egui::Rect::from_min_max(egui::pos2(2., 2.), egui::pos2(6., 6.)),
                primitive: Primitive::Mesh(mesh),
            }],
            &textures,
            8,
            8,
            1.,
        )
        .unwrap();
        assert_eq!(image.get_pixel(0, 0).0, [28, 30, 39, 255]);
        assert!(image.get_pixel(3, 2)[0] > 130);
        assert!(image.get_pixel(3, 2)[1] < 20);
        assert_eq!(image.get_pixel(7, 7).0, [28, 30, 39, 255]);
    }
}
