//! HMCL Motion / ContainerAnimations: retain visuals, never rerun old page actions.
use eframe::egui::{
    self,
    epaint::{ClippedShape, Shape},
    layers::ShapeIdx,
    TextureHandle, TextureId,
};
use std::collections::HashSet;

fn cubic(t: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let sample = |u: f64, a: f64, b: f64| {
        3. * (1. - u).powi(2) * u * a + 3. * (1. - u) * u * u * b + u * u * u
    };
    let mut low = 0.;
    let mut high = 1.;
    for _ in 0..32 {
        let mid = (low + high) * 0.5;
        if sample(mid, x1, x2) < t {
            low = mid;
        } else {
            high = mid;
        }
    }
    sample((low + high) * 0.5, y1, y2)
}
// Motion.EASE_IN_OUT_CUBIC_EMPHASIZED, including its exact midpoint/control points.
pub fn emphasized(t: f64) -> f32 {
    let t = t.clamp(0., 1.);
    let (mx, my) = (0.166666, 0.4);
    if t == 0. || t == 1. {
        return t as f32;
    }
    if t < mx {
        (cubic(t / mx, 0.05 / mx, 0., 0.133333 / mx, 0.06 / my) * my) as f32
    } else {
        (cubic(
            (t - mx) / (1. - mx),
            (0.208333 - mx) / (1. - mx),
            (0.82 - my) / (1. - my),
            (0.25 - mx) / (1. - mx),
            1.,
        ) * (1. - my)
            + my) as f32
    }
}
#[derive(Default)]
struct Snapshot {
    shapes: Vec<ClippedShape>,
    // Retain textures while their original page can release its own handles.
    _textures: Vec<TextureHandle>,
}
fn texture_ids(shape: &Shape, ids: &mut HashSet<TextureId>) {
    if let Shape::Vec(shapes) = shape {
        for shape in shapes {
            texture_ids(shape, ids);
        }
    } else {
        ids.insert(shape.texture_id());
    }
}
impl Snapshot {
    fn new(ctx: &egui::Context, shapes: Vec<ClippedShape>) -> Self {
        let mut ids = HashSet::new();
        for shape in &shapes {
            texture_ids(&shape.shape, &mut ids);
        }
        let manager = ctx.tex_manager();
        let mut textures = vec![];
        {
            let mut locked = manager.write();
            for id in ids {
                if matches!(id, TextureId::Managed(_)) && locked.meta(id).is_some() {
                    locked.retain(id);
                    textures.push(TextureHandle::new(manager.clone(), id));
                }
            }
        }
        Self {
            shapes,
            _textures: textures,
        }
    }
}
#[derive(Default)]
pub struct TabTransition {
    key: Option<String>,
    last: Snapshot,
    outgoing: Snapshot,
    started: Option<f64>,
}
pub struct Frame {
    layer: egui::LayerId,
    start: ShapeIdx,
    clip: egui::Rect,
    progress: f64,
}
impl TabTransition {
    pub fn begin(&mut self, ui: &egui::Ui, key: String, reduced: bool) -> Frame {
        let now = ui.input(|input| input.time);
        if self.key.as_ref() != Some(&key) {
            let initial = self.key.is_none();
            self.key = Some(key);
            self.outgoing = std::mem::take(&mut self.last);
            self.started =
                (!initial && !reduced && !self.outgoing.shapes.is_empty()).then_some(now);
        }
        let progress = if reduced {
            1.
        } else {
            self.started
                .map(|start| ((now - start) / 0.4).clamp(0., 1.))
                .unwrap_or(1.)
        };
        if progress >= 1. {
            self.outgoing = Snapshot::default();
            self.started = None;
        }
        let layer = ui.layer_id();
        let start = ui
            .ctx()
            .graphics_mut(|graphics| graphics.entry(layer).next_idx());
        Frame {
            layer,
            start,
            clip: ui.clip_rect(),
            progress,
        }
    }
    pub fn finish(&mut self, ui: &egui::Ui, frame: Frame) {
        let ctx = ui.ctx();
        let (end, shapes) = ctx.graphics(|graphics| {
            let list = graphics.get(frame.layer).expect("animation layer exists");
            (
                list.next_idx(),
                list.all_entries()
                    .skip(frame.start.0)
                    .cloned()
                    .collect::<Vec<_>>(),
            )
        });
        self.last = Snapshot::new(ctx, shapes);
        if frame.progress >= 1. {
            return;
        }
        let incoming = emphasized(frame.progress);
        let outgoing = 1. - emphasized((frame.progress * 2.).min(1.));
        let shift = egui::vec2(0., frame.clip.height() * 0.2 * (1. - incoming));
        let mut visible = vec![];
        for (snapshot, alpha, offset) in [
            (&self.outgoing, outgoing, egui::Vec2::ZERO),
            (&self.last, incoming, shift),
        ] {
            if alpha <= 0. {
                continue;
            }
            for clipped in &snapshot.shapes {
                let mut clipped = clipped.clone();
                clipped
                    .shape
                    .transform(egui::emath::TSTransform::from_translation(offset));
                egui::epaint::shape_transform::adjust_colors(&mut clipped.shape, move |color| {
                    if *color != egui::Color32::PLACEHOLDER {
                        *color = color.gamma_multiply(alpha);
                    }
                });
                // Move inner scroll clipping with content, keep the viewport fixed.
                clipped.clip_rect = clipped.clip_rect.translate(offset).intersect(frame.clip);
                visible.push(clipped);
            }
        }
        ctx.graphics_mut(|graphics| {
            let list = graphics.entry(frame.layer);
            for index in frame.start.0..end.0 {
                list.reset_shape(ShapeIdx(index));
            }
            for shape in visible {
                list.add(shape.clip_rect, shape.shape);
            }
        });
        ctx.request_repaint();
    }
}
impl Frame {
    pub fn interactive(&self) -> bool {
        self.progress >= 1.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emphasized_curve_preserves_original_midpoint_and_endpoints() {
        assert_eq!(emphasized(0.), 0.);
        assert_eq!(emphasized(1.), 1.);
        assert!((emphasized(0.166666) - 0.4).abs() < 0.00001);
        let mut previous = 0.;
        for step in 0..=1000 {
            let current = emphasized(step as f64 / 1000.);
            assert!(current >= previous);
            previous = current;
        }
        assert!(emphasized(0.5) > 0.9);
    }
    #[test]
    fn tab_snapshots_keep_old_images_alive_and_release_after_transition() {
        let ctx = egui::Context::default();
        let texture = ctx.load_texture(
            "old-image",
            egui::ColorImage::new([2, 2], egui::Color32::WHITE),
            Default::default(),
        );
        let id = texture.id();
        let mut animation = TabTransition::default();
        let mut draw = |time, key: &str| {
            let _ = ctx.run(
                egui::RawInput {
                    time: Some(time),
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(400., 300.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let frame = animation.begin(ui, key.into(), false);
                        let rect =
                            egui::Rect::from_min_size(ui.min_rect().min, egui::vec2(100., 100.));
                        if key == "old" {
                            ui.painter().image(
                                id,
                                rect,
                                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1., 1.)),
                                egui::Color32::WHITE,
                            );
                        } else {
                            ui.painter().rect_filled(rect, 0., egui::Color32::BLUE);
                        }
                        animation.finish(ui, frame);
                    });
                },
            );
        };
        draw(0., "old");
        drop(texture);
        assert!(ctx.tex_manager().read().meta(id).is_some());
        draw(1., "new");
        assert!(ctx.tex_manager().read().meta(id).is_some());
        draw(1.5, "new");
        assert!(ctx.tex_manager().read().meta(id).is_none());
    }
}
