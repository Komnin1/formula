//! Tool palette icons, drawn with simple shapes.

use crate::doc::{ShapeKind, Stereo};
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Tool {
    Lasso,
    Marquee,
    Eraser,
    Text,
    Bond(u8, Stereo),
    Chain,
    Ring(usize, bool),
    Arrow,
    Shape,
    Charge(i8),
    Radical,
    Element,
    Template,
}

impl Tool {
    pub fn name(&self) -> String {
        match self {
            Tool::Lasso => "Lasso".into(),
            Tool::Marquee => "Marquee".into(),
            Tool::Eraser => "Eraser".into(),
            Tool::Text => "Text".into(),
            Tool::Bond(1, Stereo::None) => "Solid Bond".into(),
            Tool::Bond(2, _) => "Double Bond".into(),
            Tool::Bond(3, _) => "Triple Bond".into(),
            Tool::Bond(_, Stereo::Dashed) => "Dashed Bond".into(),
            Tool::Bond(_, Stereo::Wedge) => "Wedged Bond".into(),
            Tool::Bond(_, Stereo::Hash) => "Hashed Wedged Bond".into(),
            Tool::Bond(_, Stereo::Bold) => "Bold Bond".into(),
            Tool::Bond(_, Stereo::Wavy) => "Wavy Bond".into(),
            Tool::Bond(..) => "Bond".into(),
            Tool::Chain => "Acyclic Chain".into(),
            Tool::Ring(6, true) => "Benzene".into(),
            Tool::Ring(5, true) => "Cyclopentadiene".into(),
            Tool::Ring(n, _) => format!("Cyclo{}ane", ["", "", "", "prop", "but", "pent", "hex", "hept", "oct"].get(*n).unwrap_or(&"")),
            Tool::Arrow => "Arrows".into(),
            Tool::Shape => "Shapes and Brackets".into(),
            Tool::Charge(1) => "Positive Charge".into(),
            Tool::Charge(_) => "Negative Charge".into(),
            Tool::Radical => "Radical".into(),
            Tool::Element => "Atom Label".into(),
            Tool::Template => "Templates".into(),
        }
    }

    pub fn hint(&self) -> &'static str {
        match self {
            Tool::Lasso => "Drag around objects to select them. Drag a selection to move it. Double-click selects a whole structure.",
            Tool::Marquee => "Drag a box to select. Drag a selection to move it. Double-click selects a whole structure.",
            Tool::Eraser => "Click or drag over atoms, bonds and objects to delete them.",
            Tool::Text => "Click an atom to type its label (e.g. N, OH, CO2Me). Click empty space to add a caption.",
            Tool::Bond(..) => "Click to add a bond, drag to draw one in any direction. Click a bond to change it.",
            Tool::Chain => "Drag to draw a zig-zag carbon chain.",
            Tool::Ring(..) => "Click empty space for a new ring, an atom for a spiro ring, or a bond to fuse a ring.",
            Tool::Arrow => "Drag to draw an arrow. Pick the style in the toolbar above or from the list under this button.",
            Tool::Shape => "Drag to draw a shape or brackets. Pick the type in the toolbar above.",
            Tool::Charge(_) => "Click an atom to change its charge.",
            Tool::Radical => "Click an atom to add or remove a radical dot.",
            Tool::Element => "Click an atom to change its element, or empty space to add an atom.",
            Tool::Template => "Pick a template from the list, then click in the drawing to place it.",
        }
    }
}

pub const PALETTE: &[Tool] = &[
    Tool::Lasso,
    Tool::Marquee,
    Tool::Eraser,
    Tool::Text,
    Tool::Bond(1, Stereo::None),
    Tool::Bond(2, Stereo::None),
    Tool::Bond(3, Stereo::None),
    Tool::Bond(1, Stereo::Dashed),
    Tool::Bond(1, Stereo::Wedge),
    Tool::Bond(1, Stereo::Hash),
    Tool::Bond(1, Stereo::Bold),
    Tool::Bond(1, Stereo::Wavy),
    Tool::Chain,
    Tool::Arrow,
    Tool::Charge(1),
    Tool::Charge(-1),
    Tool::Radical,
    Tool::Shape,
    Tool::Ring(3, false),
    Tool::Ring(4, false),
    Tool::Ring(5, false),
    Tool::Ring(6, false),
    Tool::Ring(7, false),
    Tool::Ring(8, false),
    Tool::Ring(6, true),
    Tool::Ring(5, true),
    Tool::Element,
    Tool::Template,
];

fn polygon(c: Pos2, r: f32, n: usize) -> Vec<Pos2> {
    (0..n)
        .map(|k| {
            let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * k as f32 / n as f32;
            c + vec2(a.cos(), a.sin()) * r
        })
        .collect()
}

pub fn draw(p: &Painter, rect: Rect, tool: Tool, element: &str, col: Color32) {
    let r = rect.shrink(7.0);
    let s = Stroke::new(1.5, col);
    let a = pos2(r.min.x + 1.0, r.max.y - 1.0);
    let b = pos2(r.max.x - 1.0, r.min.y + 1.0);
    let d = (b - a).normalized();
    let n = vec2(-d.y, d.x);
    match tool {
        Tool::Template => {
            // A rubber stamp: knob, neck, block and the line it prints.
            let c = r.center();
            p.circle_stroke(pos2(c.x, r.min.y + 3.0), 2.6, s);
            p.line_segment([pos2(c.x - 1.5, r.min.y + 5.5), pos2(c.x - 1.5, c.y + 1.0)], s);
            p.line_segment([pos2(c.x + 1.5, r.min.y + 5.5), pos2(c.x + 1.5, c.y + 1.0)], s);
            let block = Rect::from_min_max(pos2(r.min.x + 2.0, c.y + 1.0), pos2(r.max.x - 2.0, r.max.y - 3.0));
            p.rect_filled(block, 1.5, col);
            p.line_segment([pos2(r.min.x, r.max.y), pos2(r.max.x, r.max.y)], s);
        }
        Tool::Lasso => {
            let pts: Vec<Pos2> = (0..=24)
                .map(|k| {
                    let t = std::f32::consts::TAU * k as f32 / 24.0;
                    r.center() + vec2(t.cos() * r.width() * 0.45, t.sin() * r.height() * 0.32 - 2.0)
                })
                .collect();
            p.add(Shape::dashed_line(&pts, s, 2.5, 2.0));
            p.line_segment([r.center() + vec2(-2.0, r.height() * 0.3), pos2(r.center().x - 4.0, r.max.y)], s);
        }
        Tool::Marquee => {
            let rr = r.shrink(1.0);
            let pts = vec![rr.min, pos2(rr.max.x, rr.min.y), rr.max, pos2(rr.min.x, rr.max.y), rr.min];
            p.add(Shape::dashed_line(&pts, s, 3.0, 2.0));
        }
        Tool::Eraser => {
            let c = r.center();
            let u = vec2(0.7, -0.7);
            let v = vec2(0.7, 0.7);
            let pts = vec![c - u * 9.0 - v * 4.0, c + u * 9.0 - v * 4.0, c + u * 9.0 + v * 4.0, c - u * 9.0 + v * 4.0];
            p.add(Shape::convex_polygon(pts.clone(), Color32::from_rgb(240, 160, 170), s));
            p.line_segment([c - v * 4.0, c + v * 4.0], s);
        }
        Tool::Text => {
            p.text(r.center(), egui::Align2::CENTER_CENTER, "A", egui::FontId::proportional(17.0), col);
        }
        Tool::Bond(order, stereo) => match (order, stereo) {
            (2, _) => {
                p.line_segment([a + n * 2.5, b + n * 2.5], s);
                p.line_segment([a - n * 2.5, b - n * 2.5], s);
            }
            (3, _) => {
                p.line_segment([a, b], s);
                p.line_segment([a + n * 4.0, b + n * 4.0], s);
                p.line_segment([a - n * 4.0, b - n * 4.0], s);
            }
            (_, Stereo::Dashed) => {
                p.add(Shape::dashed_line(&[a, b], s, 3.0, 2.5));
            }
            (_, Stereo::Wedge) => {
                p.add(Shape::convex_polygon(vec![a, b + n * 4.0, b - n * 4.0], col, Stroke::NONE));
            }
            (_, Stereo::Hash) => {
                for k in 0..=6 {
                    let t = k as f32 / 6.0;
                    let c = a + (b - a) * t;
                    let w = 0.5 + 3.5 * t;
                    p.line_segment([c + n * w, c - n * w], Stroke::new(1.2, col));
                }
            }
            (_, Stereo::Bold) => {
                p.line_segment([a, b], Stroke::new(4.0, col));
            }
            (_, Stereo::Wavy) => {
                let pts: Vec<Pos2> = (0..=30)
                    .map(|k| {
                        let t = k as f32 / 30.0;
                        a + (b - a) * t + n * (t * std::f32::consts::TAU * 3.0).sin() * 2.0
                    })
                    .collect();
                p.add(Shape::line(pts, s));
            }
            _ => {
                p.line_segment([a, b], s);
            }
        },
        Tool::Chain => {
            let pts = vec![
                pos2(r.min.x, r.center().y + 4.0),
                pos2(r.min.x + r.width() * 0.25, r.center().y - 4.0),
                pos2(r.min.x + r.width() * 0.5, r.center().y + 4.0),
                pos2(r.min.x + r.width() * 0.75, r.center().y - 4.0),
                pos2(r.max.x, r.center().y + 4.0),
            ];
            p.add(Shape::line(pts, s));
        }
        Tool::Ring(nr, arom) => {
            let pts = polygon(r.center(), r.width() * 0.5, nr);
            let mut closed = pts.clone();
            closed.push(pts[0]);
            p.add(Shape::line(closed, s));
            if arom {
                let c = r.center();
                let pairs: Vec<usize> = if nr == 6 { vec![0, 2, 4] } else { vec![0, 2] };
                for k in pairs {
                    let x = pts[k];
                    let y = pts[(k + 1) % nr];
                    let x2 = x + (c - x) * 0.25;
                    let y2 = y + (c - y) * 0.25;
                    p.line_segment([x2 + (y2 - x2) * 0.12, y2 - (y2 - x2) * 0.12], Stroke::new(1.2, col));
                }
            }
        }
        Tool::Arrow => {
            let a = pos2(r.min.x, r.center().y);
            let b = pos2(r.max.x, r.center().y);
            p.line_segment([a, b - vec2(4.0, 0.0)], s);
            p.add(Shape::convex_polygon(vec![b, b + vec2(-7.0, -3.5), b + vec2(-7.0, 3.5)], col, Stroke::NONE));
        }
        Tool::Shape => {
            let rr = r.shrink(2.0);
            p.line_segment([pos2(rr.min.x + 3.0, rr.min.y), rr.min], s);
            p.line_segment([rr.min, pos2(rr.min.x, rr.max.y)], s);
            p.line_segment([pos2(rr.min.x, rr.max.y), pos2(rr.min.x + 3.0, rr.max.y)], s);
            p.line_segment([pos2(rr.max.x - 3.0, rr.min.y), pos2(rr.max.x, rr.min.y)], s);
            p.line_segment([pos2(rr.max.x, rr.min.y), rr.max], s);
            p.line_segment([rr.max, pos2(rr.max.x - 3.0, rr.max.y)], s);
            p.circle_stroke(rr.center(), 3.5, Stroke::new(1.2, col));
        }
        Tool::Charge(c) => {
            p.circle_stroke(r.center(), r.width() * 0.42, Stroke::new(1.2, col));
            p.line_segment([r.center() - vec2(4.5, 0.0), r.center() + vec2(4.5, 0.0)], s);
            if c > 0 {
                p.line_segment([r.center() - vec2(0.0, 4.5), r.center() + vec2(0.0, 4.5)], s);
            }
        }
        Tool::Radical => {
            p.circle_filled(r.center(), 3.0, col);
        }
        Tool::Element => {
            p.rect_stroke(r.shrink(1.0), 2.0, Stroke::new(1.0, col), egui::StrokeKind::Middle);
            p.text(r.center(), egui::Align2::CENTER_CENTER, element, egui::FontId::proportional(14.0), col);
        }
    }
}


pub fn shape_icon(p: &Painter, rect: Rect, kind: ShapeKind) {
    let r = rect.shrink2(vec2(6.0, 5.0));
    let mut cv = crate::render::EguiCanvas { painter: p, origin: Pos2::ZERO, zoom: 1.0 };
    let s = crate::doc::Shape::new(r.min, r.max, kind);
    crate::render::draw_shape(&s, &mut cv, 1.2, Color32::BLACK, 10.0);
}
