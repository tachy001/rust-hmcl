//! Screenshot files and asynchronous previews, following ScreenshotsPage.java.
use crate::theme;
use anyhow::{bail, Context, Result};
use eframe::egui::{self, RichText};
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver},
    time::SystemTime,
};

#[derive(Clone)]
struct File {
    path: PathBuf,
    root: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
}
impl File {
    fn name(&self) -> String {
        self.path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into()
    }
    fn verify(&self, directory: &Path) -> Result<()> {
        if self.path.parent() != Some(directory) {
            bail!("截图不属于当前目录");
        }
        if fs::canonicalize(directory)? != self.root {
            bail!("截图目录已改变，请刷新后重试");
        }
        let meta = fs::symlink_metadata(&self.path)?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            bail!("截图已被替换或不是普通文件");
        }
        if meta.len() != self.size || meta.modified().ok() != self.modified {
            bail!("截图已改变，请刷新后重试：{}", self.name());
        }
        Ok(())
    }
}

fn list(directory: &Path) -> Result<Vec<File>> {
    if !directory.exists() {
        return Ok(vec![]);
    }
    let mut files = vec![];
    let root = fs::canonicalize(directory)?;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        if !path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        {
            continue;
        }
        let meta = fs::symlink_metadata(&path)?;
        if !meta.is_file() || meta.file_type().is_symlink() {
            continue;
        }
        files.push(File {
            path,
            root: root.clone(),
            size: meta.len(),
            modified: meta.modified().ok(),
        });
    }
    files.sort_by(|a, b| b.path.cmp(&a.path));
    Ok(files)
}

fn decode(file: &File, directory: &Path, thumbnail: bool) -> Result<egui::ColorImage> {
    file.verify(directory)?;
    let mut bytes = vec![];
    fs::File::open(&file.path)?
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 64 * 1024 * 1024 {
        bail!("截图文件超过 64 MB");
    }
    file.verify(directory)?;
    let dimensions =
        image::ImageReader::with_format(std::io::Cursor::new(&bytes), image::ImageFormat::Png)
            .into_dimensions()?;
    if u64::from(dimensions.0) * u64::from(dimensions.1) > 32_000_000 {
        bail!("截图分辨率超过 3200 万像素");
    }
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(16384);
    limits.max_image_height = Some(16384);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().context("无法读取 PNG 截图")?;
    let rgba = if thumbnail {
        image.thumbnail(72, 72).to_rgba8()
    } else {
        image.to_rgba8()
    };
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        [rgba.width() as usize, rgba.height() as usize],
        &rgba,
    ))
}

struct Row {
    file: File,
    thumbnail: Option<egui::TextureHandle>,
    error: Option<String>,
}
enum Load {
    Files(Result<Vec<File>, String>),
    Thumbnail(PathBuf, Result<egui::ColorImage, String>),
}
struct Preview {
    file: File,
    rx: Option<Receiver<Result<egui::ColorImage, String>>>,
    image: Option<egui::ColorImage>,
    texture: Option<egui::TextureHandle>,
    error: String,
}
#[derive(Default)]
pub struct Browser {
    // On Linux the clipboard owner must stay alive after set_image returns.
    clipboard: Option<arboard::Clipboard>,
    directory: Option<PathBuf>,
    rows: Vec<Row>,
    rx: Option<Receiver<Load>>,
    selected: BTreeSet<PathBuf>,
    preview: Option<Preview>,
    delete: Option<(PathBuf, Vec<File>)>,
    pub error: String,
}
impl Browser {
    // The offscreen renderer uses the same scanner/decoder, without timing races.
    pub(super) fn prepare_preview(
        &mut self,
        directory: PathBuf,
        ctx: &egui::Context,
    ) -> Result<()> {
        self.rows = list(&directory)?
            .into_iter()
            .map(|file| {
                let (thumbnail, error) = match decode(&file, &directory, true) {
                    Ok(image) => (
                        Some(ctx.load_texture(
                            format!("screenshot-thumb:{}", file.path.display()),
                            image,
                            egui::TextureOptions::LINEAR,
                        )),
                        None,
                    ),
                    Err(error) => (None, Some(error.to_string())),
                };
                Row {
                    file,
                    thumbnail,
                    error,
                }
            })
            .collect();
        self.directory = Some(directory);
        Ok(())
    }
    fn refresh(&mut self, directory: PathBuf, ctx: &egui::Context) {
        // Reset both the channel and directory-bound actions when changing instances.
        self.directory = Some(directory.clone());
        self.rows.clear();
        self.selected.clear();
        self.preview = None;
        self.delete = None;
        self.error.clear();
        let (tx, rx) = mpsc::sync_channel(8);
        self.rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let files = match list(&directory) {
                Ok(files) => files,
                Err(error) => {
                    let _ = tx.send(Load::Files(Err(error.to_string())));
                    ctx.request_repaint();
                    return;
                }
            };
            if tx.send(Load::Files(Ok(files.clone()))).is_err() {
                return;
            }
            ctx.request_repaint();
            for file in files {
                let image = decode(&file, &directory, true).map_err(|e| e.to_string());
                if tx.send(Load::Thumbnail(file.path, image)).is_err() {
                    return;
                }
                ctx.request_repaint();
            }
        });
    }
    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.rx {
            for _ in 0..32 {
                match rx.try_recv() {
                    Ok(Load::Files(Ok(files))) => {
                        self.rows = files
                            .into_iter()
                            .map(|file| Row {
                                file,
                                thumbnail: None,
                                error: None,
                            })
                            .collect()
                    }
                    Ok(Load::Files(Err(error))) => self.error = error,
                    Ok(Load::Thumbnail(path, result)) => {
                        if let Some(row) = self.rows.iter_mut().find(|row| row.file.path == path) {
                            match result {
                                Ok(image) => {
                                    row.thumbnail = Some(ctx.load_texture(
                                        format!("screenshot-thumb:{}", path.display()),
                                        image,
                                        egui::TextureOptions::LINEAR,
                                    ))
                                }
                                Err(error) => row.error = Some(error),
                            }
                        }
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.rx = None;
                        break;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                }
            }
        }
        if self.rx.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }
    fn open_preview(&mut self, file: File, ctx: &egui::Context) {
        let directory = self.directory.clone().unwrap_or_default();
        let task_file = file.clone();
        let (tx, rx) = mpsc::channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(decode(&task_file, &directory, false).map_err(|e| e.to_string()));
            ctx.request_repaint();
        });
        self.preview = Some(Preview {
            file,
            rx: Some(rx),
            image: None,
            texture: None,
            error: String::new(),
        });
    }
    pub fn page(&mut self, ui: &mut egui::Ui, directory: PathBuf) {
        if self.directory.as_ref() != Some(&directory) {
            self.refresh(directory.clone(), ui.ctx());
        }
        self.poll(ui.ctx());
        let mut refresh = false;
        ui.horizontal_wrapped(|ui| {
            if self.selected.is_empty() {
                refresh = theme::button(ui, "刷新").clicked();
                if theme::button(ui, "打开目录").clicked() {
                    if let Err(error) = fs::create_dir_all(&directory)
                        .map_err(anyhow::Error::from)
                        .and_then(|()| open::that(&directory).map_err(anyhow::Error::from))
                    {
                        self.error = error.to_string();
                    }
                }
                if ui
                    .add_enabled(
                        self.rx.is_none() && !self.rows.is_empty(),
                        theme::MaterialButton::new("清空截图"),
                    )
                    .clicked()
                {
                    self.delete = Some((
                        directory.clone(),
                        self.rows.iter().map(|row| row.file.clone()).collect(),
                    ));
                }
                if theme::button(ui, "全选").clicked() {
                    self.selected = self.rows.iter().map(|row| row.file.path.clone()).collect();
                }
            } else {
                ui.label(format!("已选择 {} 张", self.selected.len()));
                if theme::button(ui, "删除选中").clicked() {
                    self.delete = Some((
                        directory.clone(),
                        self.rows
                            .iter()
                            .filter(|row| self.selected.contains(&row.file.path))
                            .map(|row| row.file.clone())
                            .collect(),
                    ));
                }
                if theme::button(ui, "全选").clicked() {
                    self.selected = self.rows.iter().map(|row| row.file.path.clone()).collect();
                }
                if theme::button(ui, "取消选择").clicked() {
                    self.selected.clear();
                }
            }
            if self.rx.is_some() {
                ui.spinner();
            }
        });
        if refresh {
            self.refresh(directory.clone(), ui.ctx());
        }
        if self.rows.is_empty() && self.rx.is_none() {
            theme::empty(
                ui,
                "maximize",
                "没有截图",
                "在游戏内按 F2 拍摄的 PNG 截图会显示在这里。",
            );
        }
        let mut preview = None;
        for row in &self.rows {
            ui.push_id(&row.file.path, |ui| {
                egui::Frame::new()
                    .fill(if self.selected.contains(&row.file.path) {
                        theme::container(ui)
                    } else {
                        theme::surface(ui)
                    })
                    .stroke(egui::Stroke::new(1_f32, theme::border(ui)))
                    .corner_radius(4.)
                    .inner_margin(8.)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let mut selected = self.selected.contains(&row.file.path);
                            if ui.checkbox(&mut selected, "").changed() {
                                if selected {
                                    self.selected.insert(row.file.path.clone());
                                } else {
                                    self.selected.remove(&row.file.path);
                                }
                            }
                            let (rect, response) =
                                ui.allocate_exact_size(egui::vec2(72., 72.), egui::Sense::click());
                            if let Some(texture) = &row.thumbnail {
                                let size = texture.size_vec2();
                                let fit = size * (72. / size.x).min(72. / size.y);
                                egui::Image::new(texture)
                                    .paint_at(ui, egui::Rect::from_center_size(rect.center(), fit));
                            } else {
                                theme::icon(
                                    ui,
                                    "maximize",
                                    rect.shrink(12.),
                                    ui.visuals().weak_text_color(),
                                );
                            }
                            if response.clicked() {
                                preview = Some(row.file.clone());
                            }
                            ui.vertical(|ui| {
                                ui.set_width((ui.available_width() - 100.).max(20.));
                                let response = ui.add(
                                    egui::Label::new(row.file.name())
                                        .truncate()
                                        .sense(egui::Sense::click()),
                                );
                                if response.double_clicked() {
                                    preview = Some(row.file.clone());
                                }
                                ui.label(
                                    RichText::new(format!(
                                        "{:.1} KB",
                                        row.file.size as f64 / 1024.
                                    ))
                                    .small()
                                    .color(ui.visuals().weak_text_color()),
                                );
                                if let Some(error) = &row.error {
                                    ui.label(RichText::new("预览不可用").small())
                                        .on_hover_text(error);
                                }
                            });
                            if ui
                                .add(theme::MaterialButton::new("").icon("info"))
                                .on_hover_text("预览截图")
                                .clicked()
                            {
                                preview = Some(row.file.clone());
                            }
                            if ui
                                .add(theme::MaterialButton::new("").icon("delete"))
                                .on_hover_text("删除截图")
                                .clicked()
                            {
                                self.delete = Some((directory.clone(), vec![row.file.clone()]));
                            }
                        });
                    });
            });
        }
        if let Some(file) = preview {
            self.open_preview(file, ui.ctx());
        }
        if !self.error.is_empty() {
            ui.label(&self.error);
        }
    }
    pub fn modal_open(&self) -> bool {
        self.preview.is_some() || self.delete.is_some()
    }
    pub(super) fn prepare_dialog_preview(&mut self, mode: &str, ctx: &egui::Context) -> Result<()> {
        let directory = self.directory.clone().context("没有截图目录")?;
        match mode {
            "preview" => {
                let file = self
                    .rows
                    .iter()
                    .find(|row| row.error.is_none())
                    .context("没有可预览的截图")?
                    .file
                    .clone();
                let image = decode(&file, &directory, false)?;
                let texture = ctx.load_texture(
                    "screenshot-preview",
                    image.clone(),
                    egui::TextureOptions::LINEAR,
                );
                self.preview = Some(Preview {
                    file,
                    rx: None,
                    image: Some(image),
                    texture: Some(texture),
                    error: String::new(),
                });
            }
            "delete" => {
                if self.rows.is_empty() {
                    bail!("没有可删除的截图");
                }
                self.delete = Some((
                    directory,
                    self.rows.iter().map(|row| row.file.clone()).collect(),
                ));
            }
            _ => bail!("截图预览模式应为 preview 或 delete"),
        }
        Ok(())
    }
    pub fn dialogs(&mut self, ctx: &egui::Context, busy: bool) -> Option<PathBuf> {
        let mut wallpaper = None;
        let mut close = false;
        if let Some(preview) = &mut self.preview {
            if let Some(rx) = &preview.rx {
                match rx.try_recv() {
                    Ok(result) => {
                        preview.rx = None;
                        match result {
                            Ok(image) => {
                                preview.texture = Some(ctx.load_texture(
                                    "screenshot-preview",
                                    image.clone(),
                                    egui::TextureOptions::LINEAR,
                                ));
                                preview.image = Some(image);
                            }
                            Err(error) => preview.error = error,
                        }
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        preview.rx = None;
                        preview.error = "图片加载任务中断".into();
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        ctx.request_repaint_after(std::time::Duration::from_millis(50))
                    }
                }
            }
            let modal = egui::Modal::new(egui::Id::new("screenshot-preview"))
                .frame(
                    egui::Frame::popup(&ctx.style())
                        .inner_margin(24.)
                        .corner_radius(4.),
                )
                .show(ctx, |ui| {
                    ui.set_width((ctx.screen_rect().width() * 0.75).min(860.));
                    ui.add(
                        egui::Label::new(RichText::new(preview.file.name()).size(20.).strong())
                            .truncate(),
                    );
                    ui.label(format!("{:.1} KB", preview.file.size as f64 / 1024.));
                    if let Some(texture) = &preview.texture {
                        let size = texture.size_vec2();
                        ui.label(format!("{} × {}", size.x as u32, size.y as u32));
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                            ui.add(
                                egui::Image::new(texture).fit_to_exact_size(
                                    size * (ui.available_width() / size.x)
                                        .min(ctx.screen_rect().height() * 0.57 / size.y)
                                        .min(1.),
                                ),
                            );
                        });
                    } else if preview.rx.is_some() {
                        ui.spinner();
                    } else {
                        ui.label(&preview.error);
                    }
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(
                                !busy && preview.image.is_some(),
                                theme::MaterialButton::new("设为背景"),
                            )
                            .clicked()
                        {
                            wallpaper = Some(preview.file.path.clone());
                        }
                        if ui
                            .add_enabled(
                                preview.image.is_some(),
                                theme::MaterialButton::new("复制图片"),
                            )
                            .clicked()
                        {
                            if let Some(image) = &preview.image {
                                let bytes: Vec<u8> = image
                                    .pixels
                                    .iter()
                                    .flat_map(|pixel| pixel.to_srgba_unmultiplied())
                                    .collect();
                                let result = (|| {
                                    if self.clipboard.is_none() {
                                        self.clipboard = Some(arboard::Clipboard::new()?);
                                    }
                                    self.clipboard
                                        .as_mut()
                                        .unwrap()
                                        .set_image(arboard::ImageData {
                                            width: image.size[0],
                                            height: image.size[1],
                                            bytes: std::borrow::Cow::Owned(bytes),
                                        })
                                })();
                                if let Err(error) = result {
                                    preview.error = error.to_string();
                                }
                            }
                        }
                        if theme::button(ui, "关闭").clicked() {
                            close = true;
                        }
                    });
                    if !preview.error.is_empty() && preview.image.is_some() {
                        ui.label(&preview.error);
                    }
                    if !self.error.is_empty() {
                        ui.label(&self.error);
                    }
                });
            close |= modal.should_close();
        }
        if close {
            self.preview = None;
        }
        if let Some((directory, files)) = self.delete.clone() {
            let mut confirmed = false;
            let mut cancelled = false;
            let modal = egui::Modal::new(egui::Id::new("delete-screenshots"))
                .frame(
                    egui::Frame::popup(&ctx.style())
                        .inner_margin(24.)
                        .corner_radius(4.),
                )
                .show(ctx, |ui| {
                    ui.set_width(440.);
                    ui.label(RichText::new("删除截图").size(20.).strong());
                    ui.label(format!("确定永久删除这 {} 张截图吗？", files.len()));
                    ui.label("此操作无法撤销。");
                    for file in files.iter().take(5) {
                        ui.add(egui::Label::new(file.name()).truncate());
                    }
                    ui.horizontal(|ui| {
                        confirmed = ui
                            .add_enabled(!busy, theme::MaterialButton::new("删除"))
                            .clicked();
                        cancelled = theme::button(ui, "取消").clicked();
                    });
                });
            if confirmed {
                self.delete = None;
                let result = delete(&directory, &files);
                self.refresh(directory, ctx);
                if let Err(error) = result {
                    self.error = error.to_string();
                }
            } else if cancelled || modal.should_close() {
                self.delete = None;
            }
        }
        wallpaper
    }
}
fn delete(directory: &Path, files: &[File]) -> Result<()> {
    // Validate the complete captured selection before deleting any file.
    for file in files {
        file.verify(directory)?;
    }
    for file in files {
        fs::remove_file(&file.path).with_context(|| format!("无法删除 {}", file.name()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "rust-hmcl-screenshots-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn png(&self, name: &str) {
            image::RgbaImage::from_pixel(160, 90, image::Rgba([22, 44, 66, 255]))
                .save(self.0.join(name))
                .unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let root = fs::canonicalize(&self.0).unwrap();
            assert_eq!(
                root.parent(),
                Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
            );
            assert!(root
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("rust-hmcl-screenshots-"));
            fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn only_png_files_are_listed_reverse_and_thumbnails_preserve_aspect() {
        let fixture = Fixture::new();
        fixture.png("2026-10-12.png");
        fixture.png("2026-10-13.PNG");
        fs::write(fixture.0.join("notes.txt"), b"keep").unwrap();
        fs::create_dir(fixture.0.join("folder.png")).unwrap();
        let files = list(&fixture.0).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].name(), "2026-10-13.PNG");
        assert_eq!(decode(&files[0], &fixture.0, true).unwrap().size, [72, 41]);
        assert_eq!(
            decode(&files[0], &fixture.0, false).unwrap().size,
            [160, 90]
        );
        delete(&fixture.0, &files[..1]).unwrap();
        assert!(fixture.0.join("2026-10-12.png").exists());
        assert!(fixture.0.join("notes.txt").exists());
        assert!(fixture.0.join("folder.png").exists());
    }
    #[test]
    fn changed_selection_is_rejected_before_any_deletion() {
        let fixture = Fixture::new();
        fixture.png("a.png");
        fixture.png("b.png");
        let files = list(&fixture.0).unwrap();
        fs::write(fixture.0.join("a.png"), b"new contents").unwrap();
        assert!(delete(&fixture.0, &files).is_err());
        assert!(fixture.0.join("b.png").exists());
        assert_eq!(fs::read(fixture.0.join("a.png")).unwrap(), b"new contents");
        assert!(delete(&fixture.0.join("other"), &files).is_err());
    }
    #[test]
    fn broken_images_keep_file_rows_and_directory_switch_resets_pending_actions() {
        let fixture = Fixture::new();
        fs::write(fixture.0.join("broken.png"), b"not a png").unwrap();
        let ctx = egui::Context::default();
        let mut browser = Browser::default();
        browser.prepare_preview(fixture.0.clone(), &ctx).unwrap();
        assert_eq!(browser.rows.len(), 1);
        assert!(browser.rows[0].error.is_some());
        browser.delete = Some((fixture.0.clone(), vec![browser.rows[0].file.clone()]));
        browser.selected.insert(browser.rows[0].file.path.clone());
        let other = fixture.0.join("other");
        fs::create_dir(&other).unwrap();
        browser.refresh(other, &ctx);
        assert!(!browser.modal_open());
        assert!(browser.selected.is_empty());
        assert!(fixture.0.join("broken.png").exists());
        // Drop receiver before cleaning up; the scanner may still be running.
        drop(browser);
    }
}
