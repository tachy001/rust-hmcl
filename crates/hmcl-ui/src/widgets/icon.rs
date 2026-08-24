//! Vector icon rendering.
//!
//! Port of HMCL's `SVG.java`: Material Symbols path data rendered as filled
//! egui shapes. All icons use a 24x24 viewBox.

use super::icons_data;

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use egui::{Color32, Pos2, Rect, Response, Sense, Shape, Ui, Vec2};

/// Parse the `d` attribute of an SVG path into subpaths of points
/// (curves flattened to line segments, arcs approximated).
pub fn parse_path(data: &str) -> Vec<Vec<Pos2>> {
    let mut parser = Parser::new(data);
    parser.parse()
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(data: &str) -> Self {
        Self {
            chars: data.chars().collect(),
            pos: 0,
        }
    }

    fn skip_separators(&mut self) {
        while let Some(c) = self.chars.get(self.pos) {
            if c.is_whitespace() || *c == ',' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn peek_command(&mut self) -> Option<char> {
        self.skip_separators();
        let c = *self.chars.get(self.pos)?;
        if c.is_ascii_alphabetic() {
            Some(c)
        } else {
            None
        }
    }

    fn take_command(&mut self) -> Option<char> {
        let c = self.peek_command()?;
        self.pos += 1;
        Some(c)
    }

    fn number(&mut self) -> Option<f32> {
        self.skip_separators();
        let start = self.pos;
        if let Some(c) = self.chars.get(self.pos)
            && (*c == '+' || *c == '-') {
                self.pos += 1;
            }
        let mut seen_digit = false;
        let mut seen_dot = false;
        while let Some(c) = self.chars.get(self.pos) {
            if c.is_ascii_digit() {
                seen_digit = true;
                self.pos += 1;
            } else if *c == '.' && !seen_dot {
                seen_dot = true;
                self.pos += 1;
            } else if (*c == 'e' || *c == 'E') && seen_digit {
                let save = self.pos;
                self.pos += 1;
                if let Some(sign) = self.chars.get(self.pos)
                    && (*sign == '+' || *sign == '-') {
                        self.pos += 1;
                    }
                if self.chars.get(self.pos).is_some_and(|c| c.is_ascii_digit()) {
                    seen_dot = true; // block a second exponent
                    while self.chars.get(self.pos).is_some_and(|c| c.is_ascii_digit()) {
                        self.pos += 1;
                    }
                } else {
                    self.pos = save;
                    break;
                }
            } else {
                break;
            }
        }
        if !seen_digit || self.pos == start {
            return None;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        text.parse().ok()
    }

    fn flag(&mut self) -> Option<f32> {
        self.skip_separators();
        let c = *self.chars.get(self.pos)?;
        if c == '0' || c == '1' {
            self.pos += 1;
            Some(if c == '1' { 1.0 } else { 0.0 })
        } else {
            None
        }
    }

    fn parse(&mut self) -> Vec<Vec<Pos2>> {
        let mut subpaths: Vec<Vec<Pos2>> = Vec::new();
        let mut current = Pos2::ZERO;
        let mut start = Pos2::ZERO;
        let mut last_quad_control: Option<Pos2> = None;
        let mut last_cubic_control: Option<Pos2> = None;

        while let Some(command) = self.take_command() {
            match command {
                'M' | 'm' => {
                    let relative = command == 'm';
                    let (x, y) = match (self.number(), self.number()) {
                        (Some(x), Some(y)) => (x, y),
                        _ => break,
                    };
                    let point = if relative {
                        Pos2::new(current.x + x, current.y + y)
                    } else {
                        Pos2::new(x, y)
                    };
                    subpaths.push(vec![point]);
                    current = point;
                    start = point;
                    last_quad_control = None;
                    last_cubic_control = None;
                    // Implicit lineto repeats for subsequent coordinate pairs.
                    while let (Some(x), Some(y)) = (self.number(), self.number()) {
                        let point = if relative {
                            Pos2::new(current.x + x, current.y + y)
                        } else {
                            Pos2::new(x, y)
                        };
                        subpaths.last_mut().unwrap().push(point);
                        current = point;
                    }
                }
                'L' | 'l' => {
                    let relative = command == 'l';
                    while let (Some(x), Some(y)) = (self.number(), self.number()) {
                        let point = if relative {
                            Pos2::new(current.x + x, current.y + y)
                        } else {
                            Pos2::new(x, y)
                        };
                        subpaths.last_mut().unwrap().push(point);
                        current = point;
                    }
                }
                'H' | 'h' => {
                    let relative = command == 'h';
                    while let Some(x) = self.number() {
                        let point = Pos2::new(if relative { current.x + x } else { x }, current.y);
                        subpaths.last_mut().unwrap().push(point);
                        current = point;
                    }
                }
                'V' | 'v' => {
                    let relative = command == 'v';
                    while let Some(y) = self.number() {
                        let point = Pos2::new(current.x, if relative { current.y + y } else { y });
                        subpaths.last_mut().unwrap().push(point);
                        current = point;
                    }
                }
                'Q' | 'q' => {
                    let relative = command == 'q';
                    while let (Some(x1), Some(y1), Some(x), Some(y)) =
                        (self.number(), self.number(), self.number(), self.number())
                    {
                        let (cx, cy) = if relative {
                            (current.x + x1, current.y + y1)
                        } else {
                            (x1, y1)
                        };
                        let control = Pos2::new(cx, cy);
                        let end = Pos2::new(
                            if relative { current.x + x } else { x },
                            if relative { current.y + y } else { y },
                        );
                        flatten_quadratic(current, control, end, subpaths.last_mut().unwrap());
                        current = end;
                        last_quad_control = Some(control);
                    }
                }
                'T' | 't' => {
                    let relative = command == 't';
                    while let (Some(x), Some(y)) = (self.number(), self.number()) {
                        let end = Pos2::new(
                            if relative { current.x + x } else { x },
                            if relative { current.y + y } else { y },
                        );
                        let control = last_quad_control
                            .map(|c| Pos2::new(2.0 * current.x - c.x, 2.0 * current.y - c.y))
                            .unwrap_or(current);
                        flatten_quadratic(current, control, end, subpaths.last_mut().unwrap());
                        current = end;
                        last_quad_control = Some(control);
                    }
                }
                'C' | 'c' => {
                    let relative = command == 'c';
                    while let (Some(x1), Some(y1), Some(x2), Some(y2), Some(x), Some(y)) = (
                        self.number(),
                        self.number(),
                        self.number(),
                        self.number(),
                        self.number(),
                        self.number(),
                    ) {
                        let (c1, c2) = if relative {
                            (
                                Pos2::new(current.x + x1, current.y + y1),
                                Pos2::new(current.x + x2, current.y + y2),
                            )
                        } else {
                            (Pos2::new(x1, y1), Pos2::new(x2, y2))
                        };
                        let end = Pos2::new(
                            if relative { current.x + x } else { x },
                            if relative { current.y + y } else { y },
                        );
                        flatten_cubic(current, c1, c2, end, subpaths.last_mut().unwrap());
                        current = end;
                        last_cubic_control = Some(c2);
                    }
                }
                'S' | 's' => {
                    let relative = command == 's';
                    while let (Some(x2), Some(y2), Some(x), Some(y)) =
                        (self.number(), self.number(), self.number(), self.number())
                    {
                        let c1 = last_cubic_control
                            .map(|c| Pos2::new(2.0 * current.x - c.x, 2.0 * current.y - c.y))
                            .unwrap_or(current);
                        let c2 = Pos2::new(
                            if relative { current.x + x2 } else { x2 },
                            if relative { current.y + y2 } else { y2 },
                        );
                        let end = Pos2::new(
                            if relative { current.x + x } else { x },
                            if relative { current.y + y } else { y },
                        );
                        flatten_cubic(current, c1, c2, end, subpaths.last_mut().unwrap());
                        current = end;
                        last_cubic_control = Some(c2);
                    }
                }
                'A' | 'a' => {
                    let relative = command == 'a';
                    while let (Some(rx), Some(ry), Some(rotation), Some(large_arc), Some(sweep)) =
                        (self.number(), self.number(), self.number(), self.flag(), self.flag())
                    {
                        let (x, y) = match (self.number(), self.number()) {
                            (Some(x), Some(y)) => (x, y),
                            _ => break,
                        };
                        let end = Pos2::new(
                            if relative { current.x + x } else { x },
                            if relative { current.y + y } else { y },
                        );
                        flatten_arc(
                            current,
                            end,
                            rx,
                            ry,
                            rotation,
                            large_arc != 0.0,
                            sweep != 0.0,
                            subpaths.last_mut().unwrap(),
                        );
                        current = end;
                    }
                }
                'Z' | 'z' => {
                    if let Some(subpath) = subpaths.last_mut() {
                        subpath.push(start);
                    }
                    current = start;
                }
                _ => break,
            }
        }
        subpaths
    }
}

const CURVE_STEPS: usize = 12;

fn flatten_quadratic(from: Pos2, control: Pos2, to: Pos2, out: &mut Vec<Pos2>) {
    for i in 1..=CURVE_STEPS {
        let t = i as f32 / CURVE_STEPS as f32;
        let mt = 1.0 - t;
        let point = mt * mt * from.to_vec2() + 2.0 * mt * t * control.to_vec2() + t * t * to.to_vec2();
        out.push(point.to_pos2());
    }
}

fn flatten_cubic(from: Pos2, c1: Pos2, c2: Pos2, to: Pos2, out: &mut Vec<Pos2>) {
    for i in 1..=CURVE_STEPS {
        let t = i as f32 / CURVE_STEPS as f32;
        let mt = 1.0 - t;
        let point = mt * mt * mt * from.to_vec2()
            + 3.0 * mt * mt * t * c1.to_vec2()
            + 3.0 * mt * t * t * c2.to_vec2()
            + t * t * t * to.to_vec2();
        out.push(point.to_pos2());
    }
}

/// Approximate an SVG elliptical arc using the SVG 1.1 F.6.5 algorithm.
#[allow(clippy::too_many_arguments)]
fn flatten_arc(
    from: Pos2,
    to: Pos2,
    rx: f32,
    ry: f32,
    rotation_deg: f32,
    large_arc: bool,
    sweep: bool,
    out: &mut Vec<Pos2>,
) {
    let mut rx = rx.abs();
    let mut ry = ry.abs();
    if rx == 0.0 || ry == 0.0 || from == to {
        out.push(to);
        return;
    }

    let phi = rotation_deg.to_radians();
    let (sin_phi, cos_phi) = phi.sin_cos();

    let dx2 = (from.x - to.x) / 2.0;
    let dy2 = (from.y - to.y) / 2.0;
    let x1p = cos_phi * dx2 + sin_phi * dy2;
    let y1p = -sin_phi * dx2 + cos_phi * dy2;

    let lambda = (x1p * x1p) / (rx * rx) + (y1p * y1p) / (ry * ry);
    if lambda > 1.0 {
        let sqrt_lambda = lambda.sqrt();
        rx *= sqrt_lambda;
        ry *= sqrt_lambda;
    }

    let rx2 = rx * rx;
    let ry2 = ry * ry;
    let x1p2 = x1p * x1p;
    let y1p2 = y1p * y1p;
    let numerator = (rx2 * ry2 - rx2 * y1p2 - ry2 * x1p2).max(0.0);
    let denominator = (rx2 * y1p2 + ry2 * x1p2).max(f32::EPSILON);
    let mut coef = (numerator / denominator).sqrt();
    if large_arc == sweep {
        coef = -coef;
    }
    let cxp = coef * (rx * y1p) / ry;
    let cyp = -coef * (ry * x1p) / rx;

    let cx = cos_phi * cxp - sin_phi * cyp + (from.x + to.x) / 2.0;
    let cy = sin_phi * cxp + cos_phi * cyp + (from.y + to.y) / 2.0;

    let angle = |ux: f32, uy: f32, vx: f32, vy: f32| -> f32 {
        let dot = (ux * vx + uy * vy).clamp(-1.0, 1.0);
        let mut angle = dot.acos();
        if ux * vy - uy * vx < 0.0 {
            angle = -angle;
        }
        angle
    };

    let theta1 = angle(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry);
    let mut delta_theta = angle(
        (x1p - cxp) / rx,
        (y1p - cyp) / ry,
        (-x1p - cxp) / rx,
        (-y1p - cyp) / ry,
    );

    if !sweep && delta_theta > 0.0 {
        delta_theta -= std::f32::consts::TAU;
    } else if sweep && delta_theta < 0.0 {
        delta_theta += std::f32::consts::TAU;
    }

    let steps = ((delta_theta.abs() / (std::f32::consts::PI / 8.0)).ceil() as usize).max(1);
    for i in 1..=steps {
        let theta = theta1 + delta_theta * i as f32 / steps as f32;
        let (sin_t, cos_t) = theta.sin_cos();
        let point = Pos2::new(
            cos_phi * rx * cos_t - sin_phi * ry * sin_t + cx,
            sin_phi * rx * cos_t + cos_phi * ry * sin_t + cy,
        );
        out.push(point);
    }
}

fn parsed_icons() -> &'static Mutex<HashMap<&'static str, Vec<Vec<Pos2>>>> {
    static CACHE: OnceLock<Mutex<HashMap<&'static str, Vec<Vec<Pos2>>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Build egui `Shape`s for the named icon, scaled to fit `size` at `origin`.
pub fn icon_shapes(name: &str, origin: Pos2, size: f32, color: Color32) -> Option<Vec<Shape>> {
    let path = icons_data::find(name)?;
    if path.is_empty() {
        return None;
    }

    let mut cache = parsed_icons().lock().unwrap();
    let subpaths = if let Some(subpaths) = cache.get(name) {
        subpaths.clone()
    } else {
        let subpaths = parse_path(path);
        cache.insert(
            icons_data::ICONS
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(n, _)| *n)
                .unwrap(),
            subpaths.clone(),
        );
        subpaths
    };
    drop(cache);

    let scale = size / 24.0;
    let shapes = subpaths
        .into_iter()
        .filter(|subpath| subpath.len() >= 2)
        .map(|subpath| {
            let points: Vec<Pos2> = subpath
                .into_iter()
                .map(|p| Pos2::new(origin.x + p.x * scale, origin.y + p.y * scale))
                .collect();
            let closed = points.first() == points.last();
            Shape::Path(egui::epaint::PathShape {
                points,
                closed,
                fill: color,
                stroke: egui::epaint::PathStroke::NONE,
            })
        })
        .collect();
    Some(shapes)
}

/// Paint an icon centered in an allocated rect of `size`.
pub fn icon(ui: &mut Ui, name: &str, size: f32, color: Color32) -> Option<Response> {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    if let Some(shapes) = icon_shapes(name, rect.min, size, color) {
        ui.painter().extend(shapes);
    }
    Some(response)
}

/// Paint an icon inside an existing rect.
pub fn icon_in_rect(painter: &egui::Painter, rect: Rect, name: &str, color: Color32) {
    let size = rect.width().min(rect.height());
    let origin = rect.center() - Vec2::splat(size / 2.0);
    if let Some(shapes) = icon_shapes(name, origin, size, color) {
        painter.extend(shapes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let subpaths = parse_path("M11 13H5V11H11V5H13V11H19V13H13V19H11V13Z");
        assert_eq!(subpaths.len(), 1);
        let subpath = &subpaths[0];
        assert_eq!(subpath.len(), 14);
        assert_eq!(subpath.first(), subpath.last());
    }

    #[test]
    fn test_parse_relative_and_arc() {
        let subpaths = parse_path("M11,7H13A2,2 0 0,1 15,9V17H13V13H11V17H9V9A2,2 0 0,1 11,7M11,9V11H13V9H11Z");
        assert_eq!(subpaths.len(), 2);
    }

    #[test]
    fn test_parse_all_icons() {
        for (name, path) in icons_data::ICONS {
            if path.is_empty() {
                continue;
            }
            let subpaths = parse_path(path);
            assert!(
                subpaths.iter().all(|s| !s.is_empty()),
                "icon {name} produced an empty subpath"
            );
        }
    }
}

