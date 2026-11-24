//! Arranging objects: split a selection into separate objects, align their edges,
//! distribute them evenly or with a fixed gap, and the rulers and grid on the sheet.

use crate::doc::*;
use crate::render::{self, Canvas, EguiCanvas};
use egui::{Color32, Pos2, Rect, Stroke, Vec2, pos2, vec2};
use std::collections::BTreeSet;

/// Drawing units in one millimetre on paper (a bond is `BOND_LEN` units and
/// `style.bond_length` points long).
pub fn units_per_mm(doc: &Doc) -> f32 {
    BOND_LEN / doc.style.bond_length.max(0.01) * 72.0 / 25.4
}

/// Scale the selected objects by `k` around `anchor`, keeping their proportions: positions
/// spread out, and atom labels, bond widths and gaps, and caption text grow with them.
pub fn scale(doc: &mut Doc, sel: &Selection, anchor: Pos2, k: f32) {
    if !(k.is_finite() && k > 0.0) {
        return;
    }
    // Captions scale as a whole box (corner and text size), the rest point by point.
    let mut rest = sel.clone();
    rest.captions.clear();
    doc.transform(&rest, |p| anchor + (p - anchor) * k);
    for &i in &sel.atoms {
        if let Some(a) = doc.atoms.get_mut(i) {
            a.size = (a.size * k).clamp(0.02, 50.0);
        }
    }
    for &i in &sel.captions {
        if let Some(c) = doc.captions.get_mut(i) {
            c.pos = anchor + (c.pos - anchor) * k;
            c.size = (c.size * k).clamp(0.5, 500.0);
        }
    }
}

/// One page's drawing area in drawing units (its top left corner is the origin, where the
/// rulers start).
pub fn page_size(doc: &Doc) -> Vec2 {
    let (w, h) = doc.style.page.area_cm();
    vec2(w, h) * 10.0 * units_per_mm(doc)
}

/// The pages the drawing covers: first page column and row, and how many across and down.
/// At least the first page; the drawing may also run left of or above it.
pub fn pages_used(doc: &Doc) -> (i32, i32, i32, i32) {
    let s = page_size(doc);
    let Some(b) = doc.bounds_of(None) else { return (0, 0, 1, 1) };
    let c0 = ((b.min.x / s.x).floor() as i32).clamp(-20, 0);
    let r0 = ((b.min.y / s.y).floor() as i32).clamp(-20, 0);
    let c1 = ((b.max.x / s.x).ceil() as i32).clamp(1, 40);
    let r1 = ((b.max.y / s.y).ceil() as i32).clamp(1, 40);
    (c0, r0, c1 - c0, r1 - r0)
}

/// The page on the canvas: grey outside the drawing area, white inside, dashed lines where
/// one page ends and the next begins.
pub fn draw_pages(painter: &egui::Painter, area: Rect, doc: &Doc, origin: Pos2, px_per_unit: f32) {
    let s = page_size(doc) * px_per_unit;
    let (c0, r0, nx, ny) = pages_used(doc);
    let min = origin + vec2(c0 as f32 * s.x, r0 as f32 * s.y);
    let sheet = Rect::from_min_size(min, vec2(nx as f32 * s.x, ny as f32 * s.y));
    let outside = if doc.style.dark { Color32::from_gray(52) } else { Color32::from_gray(214) };
    painter.rect_filled(area, 0.0, outside);
    // A soft shadow, then the sheet.
    painter.rect_filled(sheet.translate(vec2(3.0, 3.0)), 0.0, Color32::from_black_alpha(40));
    painter.rect_filled(sheet, 0.0, doc.style.paper());
    let edge = Stroke::new(1.0, Color32::from_gray(150));
    let brk = Stroke::new(1.0, Color32::from_gray(185));
    for k in 1..nx {
        let x = sheet.min.x + k as f32 * s.x;
        dashed(painter, pos2(x, sheet.min.y), pos2(x, sheet.max.y), brk);
    }
    for k in 1..ny {
        let y = sheet.min.y + k as f32 * s.y;
        dashed(painter, pos2(sheet.min.x, y), pos2(sheet.max.x, y), brk);
    }
    painter.rect_stroke(sheet, 0.0, edge, egui::StrokeKind::Outside);
}

fn dashed(painter: &egui::Painter, a: Pos2, b: Pos2, stroke: Stroke) {
    let len = (b - a).length();
    if len < 1.0 {
        return;
    }
    let d = (b - a) / len;
    let mut t = 0.0;
    while t < len {
        let e = (t + 6.0).min(len);
        painter.line_segment([a + d * t, a + d * e], stroke);
        t += 10.0;
    }
}

/// The separate objects in a selection: each connected piece of structure, and every
/// arrow, caption and shape on its own.
pub fn objects(doc: &Doc, sel: &Selection) -> Vec<Selection> {
    let mut out = Vec::new();
    let mut seen: BTreeSet<usize> = BTreeSet::new();
    let adj = doc.adjacency();
    for &start in &sel.atoms {
        if !seen.insert(start) {
            continue;
        }
        let mut group = Selection::default();
        let mut stack = vec![start];
        while let Some(a) = stack.pop() {
            group.atoms.insert(a);
            for &(n, _) in &adj[a] {
                if sel.atoms.contains(&n) && seen.insert(n) {
                    stack.push(n);
                }
            }
        }
        out.push(group);
    }
    for &i in &sel.arrows {
        out.push(Selection { arrows: [i].into(), ..Default::default() });
    }
    for &i in &sel.captions {
        out.push(Selection { captions: [i].into(), ..Default::default() });
    }
    for &i in &sel.shapes {
        out.push(Selection { shapes: [i].into(), ..Default::default() });
    }
    out
}

/// The box an object covers on paper, including atom labels and caption text.
pub fn visual_bounds(doc: &Doc, sel: &Selection, ctx: &egui::Context) -> Option<Rect> {
    let painter = egui::Painter::new(ctx.clone(), egui::LayerId::background(), Rect::EVERYTHING);
    let mut cv = EguiCanvas { painter: &painter, origin: Pos2::ZERO, zoom: 1.0 };
    let no_captions = Selection { captions: BTreeSet::new(), ..sel.clone() };
    let mut r = doc.bounds_of(Some(&no_captions)).unwrap_or(Rect::NOTHING);
    for &i in &sel.atoms {
        if render::shows_label(doc, i) {
            let l = render::layout_label(doc, i, &mut cv as &mut dyn Canvas);
            r = r.union(l.full_box);
        }
    }
    for &i in &sel.captions {
        let c = &doc.captions[i];
        let s = render::caption_size(c, &mut cv as &mut dyn Canvas);
        r = r.union(Rect::from_min_size(c.pos, s));
    }
    (r.min.x <= r.max.x && r.min.y <= r.max.y).then_some(r)
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Edge {
    Left,
    Right,
    Top,
    Bottom,
    CentreX,
    CentreY,
}

impl Edge {
    pub fn label(self) -> &'static str {
        match self {
            Edge::Left => "Left Edges",
            Edge::Right => "Right Edges",
            Edge::Top => "Top Edges",
            Edge::Bottom => "Bottom Edges",
            Edge::CentreX => "L/R Centers",
            Edge::CentreY => "T/B Centers",
        }
    }
}

fn boxes(doc: &Doc, sel: &Selection, ctx: &egui::Context) -> Vec<(Selection, Rect)> {
    objects(doc, sel).into_iter().filter_map(|o| visual_bounds(doc, &o, ctx).map(|r| (o, r))).collect()
}

/// Line the objects up on one edge. Returns how many objects took part.
pub fn align(doc: &mut Doc, sel: &Selection, edge: Edge, ctx: &egui::Context) -> usize {
    let items = boxes(doc, sel, ctx);
    if items.len() < 2 {
        return items.len();
    }
    let all = items.iter().fold(Rect::NOTHING, |a, (_, r)| a.union(*r));
    for (o, r) in &items {
        let d = match edge {
            Edge::Left => vec2(all.min.x - r.min.x, 0.0),
            Edge::Right => vec2(all.max.x - r.max.x, 0.0),
            Edge::Top => vec2(0.0, all.min.y - r.min.y),
            Edge::Bottom => vec2(0.0, all.max.y - r.max.y),
            Edge::CentreX => vec2(0.0, all.center().y - r.center().y),
            Edge::CentreY => vec2(all.center().x - r.center().x, 0.0),
        };
        doc.translate(o, d);
    }
    items.len()
}

/// Spread the objects along one axis. With `gap` (drawing units) each object starts that far
/// after the previous one ends and the first stays put; without it the first and last stay put
/// and the space between them is shared out equally. Returns how many objects took part.
pub fn distribute(doc: &mut Doc, sel: &Selection, horizontal: bool, gap: Option<f32>, ctx: &egui::Context) -> usize {
    let mut items = boxes(doc, sel, ctx);
    let lo = |r: &Rect| if horizontal { r.min.x } else { r.min.y };
    let hi = |r: &Rect| if horizontal { r.max.x } else { r.max.y };
    let mid = |r: &Rect| if horizontal { r.center().x } else { r.center().y };
    items.sort_by(|a, b| mid(&a.1).partial_cmp(&mid(&b.1)).unwrap_or(std::cmp::Ordering::Equal));
    let n = items.len();
    if n < 2 || (gap.is_none() && n < 3) {
        return n;
    }
    let gap = gap.unwrap_or_else(|| {
        let span = hi(&items[n - 1].1) - lo(&items[0].1);
        let sizes: f32 = items.iter().map(|(_, r)| hi(r) - lo(r)).sum();
        (span - sizes) / (n - 1) as f32
    });
    let mut next = hi(&items[0].1) + gap;
    for (o, r) in items.iter().skip(1) {
        let d = next - lo(r);
        doc.translate(o, if horizontal { vec2(d, 0.0) } else { vec2(0.0, d) });
        next += hi(r) - lo(r) + gap;
    }
    n
}

/// The point a selection turns around: the atom it hangs from when it is one branch of a
/// larger structure, otherwise the middle of the selection.
pub fn rotation_pivot(doc: &Doc, sel: &Selection) -> Option<Pos2> {
    let mut outside: BTreeSet<usize> = BTreeSet::new();
    for b in &doc.bonds {
        let (ia, ib) = (sel.atoms.contains(&b.a), sel.atoms.contains(&b.b));
        if ia && !ib {
            outside.insert(b.b);
        } else if ib && !ia {
            outside.insert(b.a);
        }
    }
    if outside.len() == 1 {
        let o = *outside.iter().next().unwrap();
        return Some(doc.atoms[o].pos);
    }
    doc.bounds_of(Some(sel)).map(|b| b.center())
}

// ---------------------------------------------------------------------------
// Rulers and grid

pub const RULER_W: f32 = 26.0;
pub const RULER_H: f32 = 20.0;

/// Centimetres between numbered marks so that labels stay readable.
fn label_step_cm(px_per_cm: f32) -> f32 {
    for s in [1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0] {
        if s * px_per_cm >= 40.0 {
            return s;
        }
    }
    200.0
}

/// Grid lines every centimetre (or wider when zoomed far out), with the origin lines darker.
/// `to_screen` maps drawing units to screen points; `px_per_unit` is the zoom.
pub fn draw_grid(painter: &egui::Painter, area: Rect, origin: Pos2, px_per_unit: f32, upm: f32) {
    let px_per_cm = px_per_unit * upm * 10.0;
    if px_per_cm <= 0.0 {
        return;
    }
    let mut step_cm = 1.0;
    while step_cm * px_per_cm < 14.0 {
        step_cm *= 2.0;
    }
    let step = step_cm * px_per_cm;
    let thin = Stroke::new(1.0, Color32::from_gray(222));
    let k0 = ((area.min.x - origin.x) / step).floor() as i64;
    let k1 = ((area.max.x - origin.x) / step).ceil() as i64;
    for k in k0..=k1 {
        let x = (origin.x + k as f32 * step).round() + 0.5;
        painter.line_segment([pos2(x, area.min.y), pos2(x, area.max.y)], thin);
    }
    let k0 = ((area.min.y - origin.y) / step).floor() as i64;
    let k1 = ((area.max.y - origin.y) / step).ceil() as i64;
    for k in k0..=k1 {
        let y = (origin.y + k as f32 * step).round() + 0.5;
        painter.line_segment([pos2(area.min.x, y), pos2(area.max.x, y)], thin);
    }
}

/// Show Crosshair: a pair of black lines through the page origin, used to line things up.
pub fn draw_crosshair(painter: &egui::Painter, area: Rect, origin: Pos2) {
    let ink = Stroke::new(1.0, Color32::from_gray(40));
    let x = origin.x.round() + 0.5;
    let y = origin.y.round() + 0.5;
    if x >= area.min.x && x <= area.max.x {
        painter.line_segment([pos2(x, area.min.y), pos2(x, area.max.y)], ink);
    }
    if y >= area.min.y && y <= area.max.y {
        painter.line_segment([pos2(area.min.x, y), pos2(area.max.x, y)], ink);
    }
}

/// Rulers along the top and left of the sheet, in centimetres with millimetre ticks.
/// `pointer` marks where the mouse is.
pub fn draw_rulers(painter: &egui::Painter, top: Rect, left: Rect, origin: Pos2, px_per_unit: f32, upm: f32, pointer: Option<Pos2>) {
    let bg = Color32::from_gray(246);
    let ink = Color32::from_gray(90);
    let tick = Stroke::new(1.0, Color32::from_gray(130));
    painter.rect_filled(top, 0.0, bg);
    painter.rect_filled(left, 0.0, bg);
    painter.rect_filled(Rect::from_min_max(pos2(left.min.x, top.min.y), pos2(left.max.x, top.max.y)), 0.0, bg);
    painter.line_segment([pos2(top.min.x, top.max.y - 0.5), pos2(top.max.x, top.max.y - 0.5)], Stroke::new(1.0, Color32::from_gray(200)));
    painter.line_segment([pos2(left.max.x - 0.5, left.min.y), pos2(left.max.x - 0.5, left.max.y)], Stroke::new(1.0, Color32::from_gray(200)));
    let px_per_mm = px_per_unit * upm;
    if px_per_mm <= 0.0 {
        return;
    }
    let label_cm = label_step_cm(px_per_mm * 10.0);
    // Smallest tick: 1 mm, 5 mm or 1 cm, whichever is at least 4 px apart.
    let minor_mm = [1.0f32, 5.0, 10.0, 50.0, 100.0].into_iter().find(|m| m * px_per_mm >= 4.0).unwrap_or(500.0);
    let font = egui::FontId::proportional(10.0);
    let painter_top = painter.with_clip_rect(top);
    let painter_left = painter.with_clip_rect(left);

    // Horizontal ruler.
    let m0 = ((top.min.x - origin.x) / px_per_mm / minor_mm).floor() as i64;
    let m1 = ((top.max.x - origin.x) / px_per_mm / minor_mm).ceil() as i64;
    for k in m0..=m1 {
        let mm = k as f32 * minor_mm;
        let x = (origin.x + mm * px_per_mm).round() + 0.5;
        let len = tick_len(mm, label_cm, RULER_H);
        painter_top.line_segment([pos2(x, top.max.y - len), pos2(x, top.max.y)], tick);
        if is_multiple(mm, label_cm * 10.0) {
            painter_top.text(pos2(x + 2.0, top.min.y + 1.0), egui::Align2::LEFT_TOP, fmt_cm(mm), font.clone(), ink);
        }
    }
    // Vertical ruler.
    let m0 = ((left.min.y - origin.y) / px_per_mm / minor_mm).floor() as i64;
    let m1 = ((left.max.y - origin.y) / px_per_mm / minor_mm).ceil() as i64;
    for k in m0..=m1 {
        let mm = k as f32 * minor_mm;
        let y = (origin.y + mm * px_per_mm).round() + 0.5;
        let len = tick_len(mm, label_cm, RULER_W);
        painter_left.line_segment([pos2(left.max.x - len, y), pos2(left.max.x, y)], tick);
        if is_multiple(mm, label_cm * 10.0) {
            painter_left.text(pos2(left.min.x + 2.0, y + 1.0), egui::Align2::LEFT_TOP, fmt_cm(mm), font.clone(), ink);
        }
    }
    if let Some(p) = pointer {
        let mark = Stroke::new(1.0, Color32::from_rgb(40, 110, 220));
        if p.x >= top.min.x && p.x <= top.max.x {
            painter_top.line_segment([pos2(p.x, top.min.y), pos2(p.x, top.max.y)], mark);
        }
        if p.y >= left.min.y && p.y <= left.max.y {
            painter_left.line_segment([pos2(left.min.x, p.y), pos2(left.max.x, p.y)], mark);
        }
    }
}

fn is_multiple(v: f32, of: f32) -> bool {
    let q = v / of;
    (q - q.round()).abs() < 1e-3
}

fn tick_len(mm: f32, label_cm: f32, full: f32) -> f32 {
    if is_multiple(mm, label_cm * 10.0) {
        full * 0.9
    } else if is_multiple(mm, 10.0) {
        full * 0.45
    } else if is_multiple(mm, 5.0) {
        full * 0.32
    } else {
        full * 0.2
    }
}

fn fmt_cm(mm: f32) -> String {
    format!("{}", (mm / 10.0).round() as i64)
}

/// Rotate a selection by `deg` degrees (clockwise on screen) around `pivot`.
pub fn rotate(doc: &mut Doc, sel: &Selection, pivot: Pos2, deg: f32) {
    let r = deg.to_radians();
    doc.transform(sel, |p| pivot + rot(p - pivot, r));
}

