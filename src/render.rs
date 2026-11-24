//! Drawing a document, either on screen (egui) or into an SVG file.

use crate::chem;
use crate::doc::*;
use crate::elements;
use egui::{Color32, Pos2, Rect, Vec2, pos2, vec2};
use std::collections::HashSet;

#[derive(Clone, Copy, PartialEq)]
pub enum Face {
    Regular,
    Bold,
    Italic,
    Serif,
    SerifBold,
    /// Fixed width, for sequences.
    Mono,
}

/// Face for atom labels in a style.
pub fn label_face(st: &Style) -> Face {
    match (st.label_font, st.label_bold) {
        (LabelFont::Sans, false) => Face::Regular,
        (LabelFont::Sans, true) => Face::Bold,
        (LabelFont::Serif, false) => Face::Serif,
        (LabelFont::Serif, true) => Face::SerifBold,
    }
}

pub trait Canvas {
    fn line(&mut self, a: Pos2, b: Pos2, w: f32, c: Color32);
    fn polyline(&mut self, pts: &[Pos2], w: f32, c: Color32);
    fn poly(&mut self, pts: &[Pos2], fill: Color32);
    fn circle(&mut self, c: Pos2, r: f32, fill: Option<Color32>, stroke: Option<(f32, Color32)>);
    fn rect(&mut self, r: Rect, w: f32, c: Color32, rounding: f32);
    fn ellipse(&mut self, r: Rect, w: f32, c: Color32);
    /// Text with its top-left corner at `pos`.
    fn text(&mut self, pos: Pos2, s: &str, size: f32, c: Color32, face: Face);
    fn measure(&mut self, s: &str, size: f32, face: Face) -> Vec2;

    fn dashed(&mut self, a: Pos2, b: Pos2, w: f32, c: Color32, dash: f32, gap: f32) {
        let d = b - a;
        let len = d.length();
        if len < 1e-3 {
            return;
        }
        let dir = d / len;
        let mut t = 0.0;
        while t < len {
            let e = (t + dash).min(len);
            self.line(a + dir * t, a + dir * e, w, c);
            t = e + gap;
        }
    }
}

pub fn rgb(c: Option<Rgb>, default: Color32) -> Color32 {
    match c {
        // Black follows the drawing colour, so it stays visible on a dark background.
        Some([0, 0, 0]) => default,
        Some(c) => Color32::from_rgb(c[0], c[1], c[2]),
        None => default,
    }
}

pub fn element_color(z: u8) -> Color32 {
    match z {
        7 => Color32::from_rgb(30, 60, 220),
        8 => Color32::from_rgb(220, 20, 20),
        9 | 17 => Color32::from_rgb(20, 150, 40),
        35 => Color32::from_rgb(150, 40, 30),
        53 => Color32::from_rgb(120, 20, 160),
        15 => Color32::from_rgb(230, 120, 0),
        16 => Color32::from_rgb(190, 160, 0),
        5 => Color32::from_rgb(200, 110, 90),
        14 => Color32::from_rgb(140, 120, 90),
        _ => Color32::BLACK,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum PartKind {
    Normal,
    Sub,
    Sup,
}

struct Part {
    text: String,
    kind: PartKind,
}

pub struct RenderOpts {
    pub color_atoms: bool,
    pub warnings: bool,
}

/// Measured label for an atom: where its parts go and how big the box around the symbol is.
pub struct LabelLayout {
    pieces: Vec<(Pos2, String, f32)>, // top-left, text, size
    pub symbol_box: Rect,
    pub full_box: Rect,
}

pub fn shows_label(doc: &Doc, i: usize) -> bool {
    let a = &doc.atoms[i];
    a.label.is_some()
        || a.show
        || a.el != elements::C
        || a.isotope != 0
        || doc.degree(i) == 0
        || doc.style.show_carbons
        || (doc.style.show_terminal_carbons && doc.degree(i) == 1)
}

fn label_parts(doc: &Doc, i: usize) -> (Vec<Part>, usize, usize) {
    // (parts, anchor start, anchor end exclusive)
    let a = &doc.atoms[i];
    let p = a.pos;
    let dx: f32 = doc.neighbors(i).iter().map(|&n| doc.atoms[n].pos.x - p.x).sum();
    let attach_right = dx > 1.0;
    let mut parts = Vec::new();
    let (s, e);
    if let Some(l) = &a.label {
        let text = if attach_right { chem::reversed_label(l) } else { l.clone() };
        let toks = chem::tokenize_label(&text);
        match toks {
            Some(toks) if !toks.is_empty() => {
                let n = toks.len();
                let anchor_tok = if attach_right { n - 1 } else { 0 };
                let mut anchor = (0, 0);
                for (k, t) in toks.iter().enumerate() {
                    if k == anchor_tok {
                        anchor.0 = parts.len();
                    }
                    parts.push(Part { text: t.text.clone(), kind: PartKind::Normal });
                    if k == anchor_tok {
                        anchor.1 = parts.len();
                    }
                    if t.count > 1 || (t.text == ")" && t.count > 1) {
                        parts.push(Part { text: t.count.to_string(), kind: PartKind::Sub });
                    }
                }
                s = anchor.0;
                e = anchor.1;
            }
            _ => {
                parts.push(Part { text: text.clone(), kind: PartKind::Normal });
                s = 0;
                e = 1;
            }
        }
    } else {
        let sym = elements::symbol(a.el).to_string();
        let h = if doc.style.hide_implicit_h { 0 } else { doc.implicit_h(i) };
        let h_left = attach_right && doc.degree(i) > 0;
        if a.isotope != 0 && !h_left {
            parts.push(Part { text: a.isotope.to_string(), kind: PartKind::Sup });
        }
        if h_left && h > 0 {
            parts.push(Part { text: "H".into(), kind: PartKind::Normal });
            if h > 1 {
                parts.push(Part { text: h.to_string(), kind: PartKind::Sub });
            }
        }
        if a.isotope != 0 && h_left {
            parts.push(Part { text: a.isotope.to_string(), kind: PartKind::Sup });
        }
        s = parts.len();
        parts.push(Part { text: sym, kind: PartKind::Normal });
        e = parts.len();
        if !h_left && h > 0 {
            parts.push(Part { text: "H".into(), kind: PartKind::Normal });
            if h > 1 {
                parts.push(Part { text: h.to_string(), kind: PartKind::Sub });
            }
        }
    }
    if a.charge != 0 {
        let mut t = String::new();
        if a.charge.abs() > 1 {
            t.push_str(&a.charge.abs().to_string());
        }
        t.push(if a.charge > 0 { '+' } else { '−' });
        parts.push(Part { text: t, kind: PartKind::Sup });
    }
    // Radical dots are drawn apart from the text (see `draw_radical`).
    (parts, s, e)
}

/// Where an atom's radical dot goes: straight out from the atom on the side away from its
/// bonds (as the original places it: N• of N=O sits to the left of the N), clear of any H or
/// charge written next to the symbol. Returns the centre of the dot (or of the pair).
pub fn radical_spot(doc: &Doc, i: usize, lab: Option<&LabelLayout>) -> (Pos2, Vec2) {
    let a = &doc.atoms[i];
    let size = doc.style.label_size * a.size;
    let bonds: Vec<Vec2> = doc.neighbors(i).iter().map(|&n| (doc.atoms[n].pos - a.pos).normalized()).filter(|v| v.length() > 0.5).collect();
    let mut cands: Vec<Vec2> = Vec::new();
    let sum: Vec2 = bonds.iter().copied().fold(Vec2::ZERO, |s, v| s + v);
    if sum.length() > 0.3 {
        cands.push(-sum.normalized());
    }
    cands.extend([vec2(0.0, -1.0), vec2(0.0, 1.0), vec2(-1.0, 0.0), vec2(1.0, 0.0), vec2(0.71, -0.71), vec2(-0.71, -0.71)]);
    let gap = size * 0.25;
    let place = |d: Vec2| -> Pos2 {
        match lab {
            Some(l) => {
                // Just beyond the symbol's box in that direction.
                let hw = l.symbol_box.width() / 2.0;
                let hh = l.symbol_box.height() / 2.0;
                let tx = if d.x.abs() > 1e-4 { hw / d.x.abs() } else { f32::MAX };
                let ty = if d.y.abs() > 1e-4 { hh / d.y.abs() } else { f32::MAX };
                a.pos + d * (tx.min(ty) + gap)
            }
            None => a.pos + d * size * 0.45,
        }
    };
    for d in &cands {
        // Keep away from bonds…
        if bonds.iter().any(|b| b.dot(*d) > 0.64) {
            continue;
        }
        let p = place(*d);
        // …and from H, counts and charges written beside the symbol.
        if let Some(l) = lab {
            let others = l.full_box.width() > l.symbol_box.width() + 1.0;
            if others && l.full_box.expand(size * 0.08).contains(p) && !l.symbol_box.expand(gap * 0.9).contains(p) {
                continue;
            }
        }
        return (p, *d);
    }
    (place(vec2(0.0, -1.0)), vec2(0.0, -1.0))
}

/// Draw one dot (doublet) or two side by side (triplet / diradical).
fn draw_radical(doc: &Doc, i: usize, lab: Option<&LabelLayout>, cv: &mut dyn Canvas, col: Color32) {
    let n = doc.atoms[i].radical;
    if n == 0 {
        return;
    }
    let (c, d) = radical_spot(doc, i, lab);
    let r = doc.style.label_size * doc.atoms[i].size * 0.09;
    if n == 1 {
        cv.circle(c, r, Some(col), None);
    } else {
        let across = vec2(-d.y, d.x) * r * 1.6;
        cv.circle(c + across, r, Some(col), None);
        cv.circle(c - across, r, Some(col), None);
    }
}

pub fn layout_label(doc: &Doc, i: usize, cv: &mut dyn Canvas) -> LabelLayout {
    // Scaled structures carry their own size (Object ▸ Scale, the selection's corners).
    let size = doc.style.label_size * doc.atoms[i].size;
    let face = label_face(&doc.style);
    let (parts, s, e) = label_parts(doc, i);
    let small = size * 0.68;
    let widths: Vec<f32> = parts
        .iter()
        .map(|p| cv.measure(&p.text, if p.kind == PartKind::Normal { size } else { small }, face).x)
        .collect();
    let h = cv.measure("C", size, face).y;
    let pre: f32 = widths[..s].iter().sum();
    let anchor_w: f32 = widths[s..e].iter().sum();
    let pos = doc.atoms[i].pos;
    let mut x = pos.x - pre - anchor_w / 2.0;
    let top = pos.y - h * 0.5;
    let mut pieces = Vec::new();
    let mut full = Rect::NOTHING;
    for (k, p) in parts.iter().enumerate() {
        let (y, sz) = match p.kind {
            PartKind::Normal => (top, size),
            PartKind::Sub => (top + h * 0.42, small),
            PartKind::Sup => (top - h * 0.22, small),
        };
        pieces.push((pos2(x, y), p.text.clone(), sz));
        let hh = if p.kind == PartKind::Normal { h } else { h * 0.7 };
        full.extend_with(pos2(x, y + hh * 0.12));
        full.extend_with(pos2(x + widths[k], y + hh * 0.92));
        x += widths[k];
    }
    let cap = h * 0.36;
    let symbol_box = Rect::from_center_size(pos, vec2(anchor_w.max(size * 0.55), cap * 2.0));
    LabelLayout { pieces, symbol_box, full_box: full }
}

/// How far from the atom centre a bond should stop in direction `dir`.
fn clip_dist(lab: &Option<LabelLayout>, dir: Vec2, margin: f32) -> f32 {
    match lab {
        None => 0.0,
        Some(l) => {
            let hw = l.symbol_box.width() / 2.0 + margin;
            let hh = l.symbol_box.height() / 2.0 + margin;
            let tx = if dir.x.abs() > 1e-4 { hw / dir.x.abs() } else { f32::MAX };
            let ty = if dir.y.abs() > 1e-4 { hh / dir.y.abs() } else { f32::MAX };
            tx.min(ty)
        }
    }
}

// ---------------------------------------------------------------------------
// Biopolymer monomers: coloured shapes with their codes, as the original's HELM editor
// draws them (amino acids round, sugars heptagons, phosphates diamonds, bases trapezoids).

/// Shape radius for the monomers of a drawing: 0.4 of the usual distance between linked
/// monomers.
pub fn monomer_radius(doc: &Doc) -> f32 {
    let mut d: Vec<f32> = doc
        .bonds
        .iter()
        .filter(|b| is_unit(doc, b.a) && is_unit(doc, b.b))
        .map(|b| (doc.atoms[b.a].pos - doc.atoms[b.b].pos).length())
        .filter(|l| *l > 1.0)
        .collect();
    d.sort_by(|a, b| a.partial_cmp(b).unwrap());
    d.get(d.len() / 2).copied().unwrap_or(BOND_LEN) * 0.38
}

fn is_unit(doc: &Doc, i: usize) -> bool {
    doc.atoms[i].monomer.as_ref().is_some_and(|m| m.role == MonomerRole::Unit)
}

/// The cap on an unused side attachment: not drawn (the original hides it too).
pub fn is_hidden_cap(doc: &Doc, i: usize) -> bool {
    doc.atoms[i].monomer.as_ref().is_some_and(|m| m.role == MonomerRole::Branch)
}

/// Colour of a monomer, by its natural analog (the original's palette, read off its drawings).
pub fn monomer_color(m: &Monomer) -> Color32 {
    let key = if m.analog.is_empty() { m.symbol.as_str() } else { m.analog.as_str() };
    let hex = |v: u32| Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8);
    match m.polymer {
        Polymer::Peptide => hex(match key {
            "A" | "G" => 0xcdb8d9,
            "R" => 0xb08cbd,
            "K" => 0x8f9a92,
            "H" => 0x6fa6c2,
            "I" | "V" => 0x80b4ae,
            "L" => 0x8cbfa2,
            "M" => 0xa7c582,
            "F" => 0xcbc86c,
            "W" => 0xe29a55,
            "Y" => 0xe0844c,
            "S" => 0xe2b45c,
            "T" => 0xe3a055,
            "C" => 0xe6d070,
            "P" => 0xd8c070,
            "N" => 0xe6b080,
            "D" => 0xe08a60,
            "Q" => 0x7189c9,
            "E" => 0x6f97cd,
            "U" => 0x9ec991,
            "O" => 0x777777,
            _ => 0xbdbdbd,
        }),
        Polymer::Nucleic if m.branch => hex(match key {
            "A" => 0x9a6fad,
            "G" => 0xc9b8d9,
            "C" => 0x6f8fcf,
            "T" | "U" => 0xd5bd55,
            "I" => 0xb39acb,
            _ => 0x8a8a8a,
        }),
        Polymer::Nucleic if m.is_phosphate() => hex(0xb2dca8),
        Polymer::Nucleic => hex(0xc2c09e),
        Polymer::Chem => hex(0xd9d0c0),
    }
}

/// Outline of a monomer's shape (not for circles).
fn monomer_outline(shape: MonomerShape, c: Pos2, r: f32) -> Vec<Pos2> {
    match shape {
        MonomerShape::Heptagon => (0..7)
            .map(|k| {
                let a = -std::f32::consts::FRAC_PI_2 + k as f32 * std::f32::consts::TAU / 7.0;
                c + vec2(a.cos(), a.sin()) * r
            })
            .collect(),
        MonomerShape::Diamond => vec![c + vec2(0.0, -r), c + vec2(r, 0.0), c + vec2(0.0, r), c + vec2(-r, 0.0)],
        MonomerShape::Trapezoid => vec![c + vec2(-0.55 * r, -0.75 * r), c + vec2(0.55 * r, -0.75 * r), c + vec2(r, 0.75 * r), c + vec2(-r, 0.75 * r)],
        MonomerShape::Rect => vec![c + vec2(-1.2 * r, -0.7 * r), c + vec2(1.2 * r, -0.7 * r), c + vec2(1.2 * r, 0.7 * r), c + vec2(-1.2 * r, 0.7 * r)],
        MonomerShape::Circle => vec![],
    }
}

/// The box bonds stop at.
fn monomer_box(m: &Monomer, c: Pos2, r: f32) -> Rect {
    match m.shape() {
        MonomerShape::Trapezoid => Rect::from_center_size(c, vec2(2.0 * r, 1.5 * r)),
        MonomerShape::Rect => Rect::from_center_size(c, vec2(2.4 * r, 1.4 * r)),
        _ => Rect::from_center_size(c, vec2(2.0 * r, 2.0 * r)),
    }
}

fn draw_monomer(m: &Monomer, c: Pos2, r: f32, label_size: f32, cv: &mut dyn Canvas, face: Face) {
    let fill = monomer_color(m);
    let shape = m.shape();
    if shape == MonomerShape::Circle {
        cv.circle(c, r, Some(fill), None);
    } else {
        cv.poly(&monomer_outline(shape, c, r), fill);
    }
    // The code, shrunk to fit the shape and cut short with "…" when even that is too long.
    let room = match shape {
        MonomerShape::Diamond => r * 1.1,
        MonomerShape::Trapezoid => r * 1.5,
        MonomerShape::Rect => r * 2.2,
        _ => r * 1.75,
    };
    let full = m.symbol.as_str();
    let base = (label_size * 1.1).min(r * 0.8);
    let mut size = base;
    let mut text = full.to_string();
    let w = cv.measure(&text, size, face).x;
    if w > room {
        size = (base * room / w).max(base * 0.3);
        let mut n = text.chars().count();
        while cv.measure(&text, size, face).x > room && n > 2 {
            n -= 1;
            text = format!("{}…", full.chars().take(n).collect::<String>());
        }
    }
    let dark = {
        let [r8, g8, b8, _] = fill.to_array();
        0.299 * r8 as f32 + 0.587 * g8 as f32 + 0.114 * b8 as f32 <= 120.0
    };
    let col = if dark { Color32::WHITE } else { Color32::BLACK };
    let ms = cv.measure(&text, size, face);
    cv.text(c - ms / 2.0, &text, size, col, face);
}

pub fn draw_doc(doc: &Doc, cv: &mut dyn Canvas, opts: &RenderOpts) {
    let st = &doc.style;
    let lw = st.line_width;
    let ink = st.ink();
    let face = label_face(st);
    // Shapes go under everything.
    for s in &doc.shapes {
        draw_shape(s, cv, lw, ink, st.label_size);
    }
    let has_monomers = doc.atoms.iter().any(|a| a.monomer.is_some());
    let mr = if has_monomers { monomer_radius(doc) } else { 0.0 };
    let labels: Vec<Option<LabelLayout>> = (0..doc.atoms.len())
        .map(|i| match doc.atoms[i].monomer.as_ref().filter(|m| m.role == MonomerRole::Unit) {
            // Bonds stop at a monomer's shape.
            Some(m) => {
                let bx = monomer_box(m, doc.atoms[i].pos, mr);
                Some(LabelLayout { pieces: vec![], symbol_box: bx, full_box: bx })
            }
            None if shows_label(doc, i) => Some(layout_label(doc, i, cv)),
            None => None,
        })
        .collect();
    let rings = chem::sssr(doc);
    let margin = st.margin;
    for (bi, b) in doc.bonds.iter().enumerate() {
        if has_monomers && (is_hidden_cap(doc, b.a) || is_hidden_cap(doc, b.b)) {
            continue;
        }
        draw_bond(doc, bi, b, &labels, &rings, cv, margin, opts);
    }
    for (i, a) in doc.atoms.iter().enumerate() {
        if let Some(m) = &a.monomer {
            match m.role {
                MonomerRole::Branch => continue,
                MonomerRole::Unit => {
                    draw_monomer(m, a.pos, mr, st.label_size, cv, face);
                    continue;
                }
                MonomerRole::Terminus => {}
            }
        }
        let col = if let Some(c) = a.color {
            Color32::from_rgb(c[0], c[1], c[2])
        } else if opts.color_atoms {
            let c = element_color(if a.label.is_some() { chem::label_main_element(a.label.as_ref().unwrap()) } else { a.el });
            if c == Color32::BLACK { ink } else { c }
        } else {
            ink
        };
        let col = if col == Color32::BLACK { ink } else { col };
        draw_radical(doc, i, labels[i].as_ref(), cv, col);
        if let Some(l) = &labels[i] {
            for (p, t, sz) in &l.pieces {
                cv.text(*p, t, *sz, col, face);
            }
        } else if a.charge != 0 {
            // Charge on a plain carbon: small symbol next to the corner.
            let mut d = -doc.free_direction(i);
            if doc.degree(i) > 0 {
                d = doc.free_direction(i);
            }
            let c = a.pos + d * st.label_size * a.size * 0.55;
            let mut t = String::new();
            if a.charge.abs() > 1 {
                t.push_str(&a.charge.abs().to_string());
            }
            t.push(if a.charge > 0 { '+' } else { '−' });
            let sz = st.label_size * a.size * 0.75;
            let m = cv.measure(&t, sz, face);
            cv.text(c - m / 2.0, &t, sz, col, face);
        }
        if opts.warnings && st.show_warnings && doc.valence_error(i) {
            let r = labels[i].as_ref().map(|l| l.full_box.expand(2.0)).unwrap_or(Rect::from_center_size(a.pos, vec2(10.0, 10.0)));
            cv.rect(r, 1.0, Color32::from_rgb(230, 0, 0), 0.0);
        }
    }
    for a in &doc.arrows {
        draw_arrow(a, cv, lw, ink);
    }
    for c in &doc.captions {
        draw_caption(c, cv, ink);
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_bond(
    doc: &Doc,
    bi: usize,
    b: &Bond,
    labels: &[Option<LabelLayout>],
    rings: &[Vec<usize>],
    cv: &mut dyn Canvas,
    margin: f32,
    _opts: &RenderOpts,
) {
    // A scaled structure: line widths, wedges, hashes, double-bond gaps and margins follow its
    // size, so it keeps its proportions.
    let k = (doc.atoms[b.a].size + doc.atoms[b.b].size) * 0.5;
    let scaled;
    let st = if (k - 1.0).abs() > 1e-4 {
        let mut s = doc.style.clone();
        s.line_width *= k;
        s.bold_width *= k;
        s.hash_spacing *= k;
        s.wedge_width *= k;
        s.margin *= k;
        scaled = s;
        &scaled
    } else {
        &doc.style
    };
    let margin = margin * k;
    let lw = st.line_width;
    let col = rgb(b.color, st.ink());
    let pa0 = doc.atoms[b.a].pos;
    let pb0 = doc.atoms[b.b].pos;
    let d = pb0 - pa0;
    let len = d.length();
    if len < 1e-3 {
        return;
    }
    let dir = d / len;
    let ca = clip_dist(&labels[b.a], dir, margin);
    let cb = clip_dist(&labels[b.b], -dir, margin);
    if ca + cb >= len {
        return;
    }
    let pa = pa0 + dir * ca;
    let pb = pb0 - dir * cb;
    let nrm = vec2(-dir.y, dir.x);
    let spacing = st.dbl_spacing * BOND_LEN * k;
    match b.order {
        2 => {
            let side = double_side(doc, bi, rings);
            if side == 0.0 {
                // A bold or dashed double bond keeps one line plain.
                let (p1, p2) = (pa + nrm * spacing * 0.5, pb + nrm * spacing * 0.5);
                match b.stereo {
                    Stereo::Bold => cv.line(p1, p2, st.bold_width, col),
                    Stereo::Dashed => cv.dashed(p1, p2, lw, col, 3.0, 2.5),
                    _ => cv.line(p1, p2, lw, col),
                }
                cv.line(pa - nrm * spacing * 0.5, pb - nrm * spacing * 0.5, lw, col);
            } else {
                // Dashed double: the main line stays solid, the inner one is dashed.
                if b.stereo == Stereo::Dashed {
                    cv.line(pa, pb, lw, col);
                } else {
                    draw_single(b, pa, pb, cv, st, col);
                }
                let off = nrm * spacing * side;
                let short = BOND_LEN * 0.12;
                let sa = if labels[b.a].is_some() { 0.0 } else { short };
                let sb = if labels[b.b].is_some() { 0.0 } else { short };
                let p1 = pa + off + dir * sa;
                let p2 = pb + off - dir * sb;
                if (p2 - p1).dot(dir) > 0.0 {
                    if b.stereo == Stereo::Dashed {
                        cv.dashed(p1, p2, lw, col, 3.0, 2.5);
                    } else {
                        cv.line(p1, p2, lw, col);
                    }
                }
            }
        }
        3 => {
            cv.line(pa, pb, lw, col);
            cv.line(pa + nrm * spacing, pb + nrm * spacing, lw, col);
            cv.line(pa - nrm * spacing, pb - nrm * spacing, lw, col);
        }
        _ => draw_single(b, pa, pb, cv, st, col),
    }
}

fn draw_single(b: &Bond, pa: Pos2, pb: Pos2, cv: &mut dyn Canvas, st: &Style, col: Color32) {
    let (lw, bw) = (st.line_width, st.bold_width);
    let d = pb - pa;
    let len = d.length();
    let dir = d / len.max(1e-3);
    let nrm = vec2(-dir.y, dir.x);
    let wide = st.wedge_width * 0.5;
    match b.stereo {
        Stereo::Wedge => {
            cv.poly(&[pa + nrm * (lw * 0.5), pb + nrm * wide, pb - nrm * wide, pa - nrm * (lw * 0.5)], col);
        }
        Stereo::Hash => {
            let n = ((len / (st.hash_spacing + lw).max(0.5)).floor() as usize).max(3);
            for k in 0..=n {
                let t = k as f32 / n as f32;
                let w = lw * 0.5 + (wide - lw * 0.5) * t;
                let c = pa + d * t;
                cv.line(c + nrm * w, c - nrm * w, lw, col);
            }
        }
        Stereo::Hashed => {
            let n = ((len / (st.hash_spacing + lw).max(0.5)).floor() as usize).max(3);
            let w = (bw * 0.5).max(lw);
            for k in 0..=n {
                let c = pa + d * (k as f32 / n as f32);
                cv.line(c + nrm * w, c - nrm * w, lw, col);
            }
        }
        Stereo::Bold => cv.line(pa, pb, bw, col),
        Stereo::Dashed => cv.dashed(pa, pb, lw, col, 3.0, 2.5),
        Stereo::Wavy => {
            let n = ((len / 2.0) as usize).max(8);
            let waves = (len / 6.0).max(2.0);
            let pts: Vec<Pos2> = (0..=n)
                .map(|k| {
                    let t = k as f32 / n as f32;
                    pa + d * t + nrm * (t * waves * std::f32::consts::TAU).sin() * 2.2
                })
                .collect();
            cv.polyline(&pts, lw, col);
        }
        Stereo::None => cv.line(pa, pb, lw, col),
    }
}

/// Which side the second line of a double bond goes: +1, -1 or 0 (centred).
pub fn double_side(doc: &Doc, bi: usize, rings: &[Vec<usize>]) -> f32 {
    let b = &doc.bonds[bi];
    match b.dbl {
        DblPos::Center => return 0.0,
        DblPos::Left => return -1.0,
        DblPos::Right => return 1.0,
        DblPos::Auto => {}
    }
    let pa = doc.atoms[b.a].pos;
    let pb = doc.atoms[b.b].pos;
    let d = (pb - pa).normalized();
    let nrm = vec2(-d.y, d.x);
    let mid = pa + (pb - pa) * 0.5;
    // In a ring: towards the ring centre (prefer rings with more double bonds).
    let mut best: Option<(i32, usize, Pos2)> = None;
    for r in rings {
        let n = r.len();
        let has = (0..n).any(|k| {
            let (x, y) = (r[k], r[(k + 1) % n]);
            (x == b.a && y == b.b) || (x == b.b && y == b.a)
        });
        if !has {
            continue;
        }
        let dbls = (0..n)
            .filter(|&k| doc.find_bond(r[k], r[(k + 1) % n]).map(|x| doc.bonds[x].order == 2).unwrap_or(false))
            .count() as i32;
        let c = chem::centroid(doc, r);
        let score = dbls * 10 - n as i32;
        if best.map(|x| score > x.0).unwrap_or(true) {
            best = Some((score, n, c));
        }
    }
    if let Some((_, _, c)) = best {
        return if (c - mid).dot(nrm) >= 0.0 { 1.0 } else { -1.0 };
    }
    let da = doc.degree(b.a);
    let db = doc.degree(b.b);
    // Cumulated double bonds (azide N=N+=N−, allene C=C=C, O=C=O, ketene) run in a straight
    // line through the middle atom and are all drawn centred, so they line up.
    let multiple = |i: usize| doc.bonds.iter().filter(|x| (x.a == i || x.b == i) && x.order >= 2).count();
    if multiple(b.a) >= 2 || multiple(b.b) >= 2 {
        return 0.0;
    }
    // Terminal double bonds (C=O, C=CH2 on a branch point) are centred.
    if (da == 1 && db != 2) || (db == 1 && da != 2) || (da == 1 && db == 1) {
        return 0.0;
    }
    if (da == 1 && shows_label(doc, b.a)) || (db == 1 && shows_label(doc, b.b)) {
        return 0.0;
    }
    let mut side = 0.0;
    for (i, p) in [(b.a, pa), (b.b, pb)] {
        for n in doc.neighbors(i) {
            if n == b.a || n == b.b {
                continue;
            }
            side += (doc.atoms[n].pos - p).normalized().dot(nrm);
        }
    }
    if side.abs() < 1e-3 {
        // Trans chain: follow the first atom's neighbour.
        if let Some(n) = doc.neighbors(b.a).into_iter().find(|&n| n != b.b) {
            return if (doc.atoms[n].pos - pa).dot(nrm) >= 0.0 { 1.0 } else { -1.0 };
        }
        return 0.0;
    }
    side.signum()
}

pub fn draw_arrow(a: &Arrow, cv: &mut dyn Canvas, lw: f32, ink: Color32) {
    let col = rgb(a.color, ink);
    let d = a.to - a.from;
    let len = d.length();
    if len < 1.0 {
        return;
    }
    let dir = d / len;
    let nrm = vec2(-dir.y, dir.x);
    let st = a.style;
    let k = st.size.scale();
    let (hl, hw) = if st.wide { (BOND_LEN * 0.5 * k, BOND_LEN * 0.22 * k) } else { (BOND_LEN * 0.36 * k, BOND_LEN * 0.13 * k) };
    // `nrm` points to the right of the direction of travel on screen (y grows downwards).
    let head = |cv: &mut dyn Canvas, tip: Pos2, dir: Vec2, half: Half| {
        let nrm = vec2(-dir.y, dir.x);
        let base = tip - dir * hl;
        let inner = tip - dir * (hl * 0.7);
        match half {
            Half::Full => cv.poly(&[tip, base + nrm * hw, inner, base - nrm * hw], col),
            Half::Left => cv.poly(&[tip, base - nrm * hw, inner], col),
            Half::Right => cv.poly(&[tip, base + nrm * hw, inner], col),
        }
    };
    let back = hl * 0.6;
    match a.kind {
        ArrowKind::Reaction => {
            cv.line(a.from, a.to - dir * back, lw, col);
            head(cv, a.to, dir, st.half);
        }
        ArrowKind::Line => cv.line(a.from, a.to, lw, col),
        ArrowKind::Dashed => {
            cv.dashed(a.from, a.to - dir * back, lw, col, 5.0 * k.max(0.7), 3.5 * k.max(0.7));
            head(cv, a.to, dir, st.half);
        }
        ArrowKind::NoGo | ArrowKind::NoGoHash => {
            cv.line(a.from, a.to - dir * back, lw, col);
            head(cv, a.to, dir, st.half);
            let m = a.from + (d - dir * hl) * 0.45;
            let s = BOND_LEN * 0.2 * k.max(0.7);
            if a.kind == ArrowKind::NoGo {
                cv.line(m + (dir + nrm) * s, m - (dir + nrm) * s, lw * 1.3, col);
                cv.line(m + (dir - nrm) * s, m - (dir - nrm) * s, lw * 1.3, col);
            } else {
                // Two parallel slashes.
                for off in [-0.45f32, 0.45] {
                    let c = m + dir * (s * off);
                    cv.line(c + (nrm * -1.0 + dir * 0.55) * s, c - (nrm * -1.0 + dir * 0.55) * s, lw * 1.3, col);
                }
            }
        }
        ArrowKind::Resonance => {
            cv.line(a.from + dir * back, a.to - dir * back, lw, col);
            head(cv, a.to, dir, Half::Full);
            head(cv, a.from, -dir, Half::Full);
        }
        ArrowKind::Equilibrium | ArrowKind::Unbalanced => {
            let o = nrm * (BOND_LEN * 0.09 * k.max(0.75));
            // Upper: forward with the upper barb; lower: backward with the lower barb.
            let t1 = a.from - o;
            let t2 = a.to - o;
            cv.line(t1, t2 - dir * back * 0.3, lw, col);
            head(cv, t2, dir, Half::Left);
            // An unbalanced equilibrium has a shorter reverse arrow, centred under the forward one.
            let shrink = if a.kind == ArrowKind::Unbalanced { len * 0.22 } else { 0.0 };
            let b1 = a.to + o - dir * shrink;
            let b2 = a.from + o + dir * shrink;
            cv.line(b1, b2 + dir * back * 0.3, lw, col);
            head(cv, b2, -dir, Half::Left);
        }
        ArrowKind::Retro => {
            let o = nrm * (BOND_LEN * 0.09 * k.max(0.75));
            let end = a.to - dir * (hl * 0.55);
            cv.line(a.from + o, end + o, lw, col);
            cv.line(a.from - o, end - o, lw, col);
            let w = BOND_LEN * 0.22 * k;
            cv.polyline(&[a.to - dir * hl * 0.9 + nrm * w, a.to, a.to - dir * hl * 0.9 - nrm * w], lw, col);
        }
        ArrowKind::Hollow => {
            // An outlined block arrow.
            let sw = BOND_LEN * 0.12 * k;
            let bw = BOND_LEN * 0.3 * k;
            let bl = (BOND_LEN * 0.5 * k).min(len * 0.6);
            let base = a.to - dir * bl;
            let pts = [
                a.from + nrm * sw,
                base + nrm * sw,
                base + nrm * bw,
                a.to,
                base - nrm * bw,
                base - nrm * sw,
                a.from - nrm * sw,
                a.from + nrm * sw,
            ];
            cv.polyline(&pts, lw, col);
        }
        ArrowKind::Curved | ArrowKind::Rotation => {
            let pts = a.path(48);
            let n = pts.len();
            if n < 3 {
                return;
            }
            let tip = pts[n - 1];
            let tdir = (pts[n - 1] - pts[n - 3]).normalized();
            let tail = pts[0];
            let sdir = (pts[0] - pts[2]).normalized();
            // Trim the line under the heads.
            let mut body: Vec<Pos2> = pts.iter().copied().filter(|p| (tip - *p).length() > back && (!st.both || (tail - *p).length() > back)).collect();
            if st.both {
                body.insert(0, tail - sdir * back);
            }
            body.push(tip - tdir * back);
            cv.polyline(&body, lw, col);
            head(cv, tip, tdir, st.half);
            if st.both {
                let other = match st.half {
                    Half::Left => Half::Right,
                    Half::Right => Half::Left,
                    Half::Full => Half::Full,
                };
                head(cv, tail, sdir, other);
            }
        }
    }
}

/// Position (top-left) and size of a bracket's index text.
pub fn bracket_label_box(s: &Shape, cv: &mut dyn Canvas, text_size: f32) -> Option<(Pos2, f32, Vec2)> {
    let label = s.label.as_deref().filter(|l| !l.trim().is_empty() && s.is_bracket())?;
    let r = Rect::from_two_pos(s.min, s.max);
    let sz = text_size * 0.75;
    let m = cv.measure(label, sz, Face::Regular);
    Some((pos2(r.max.x + sz * 0.3, r.max.y - m.y * 0.45), sz, m))
}

pub fn draw_shape(s: &Shape, cv: &mut dyn Canvas, lw: f32, ink: Color32, text_size: f32) {
    let col = rgb(s.color, ink);
    let r = Rect::from_two_pos(s.min, s.max);
    if let Some((p, sz, _)) = bracket_label_box(s, cv, text_size) {
        cv.text(p, s.label.as_deref().unwrap_or(""), sz, col, Face::Regular);
    }
    match s.kind {
        ShapeKind::Rect => cv.rect(r, lw, col, 0.0),
        ShapeKind::RoundRect => cv.rect(r, lw, col, (r.width().min(r.height()) * 0.15).min(12.0)),
        ShapeKind::Ellipse => cv.ellipse(r, lw, col),
        ShapeKind::Brackets => {
            let t = (r.height() * 0.08).clamp(4.0, 10.0);
            cv.polyline(&[pos2(r.min.x + t, r.min.y), r.min, pos2(r.min.x, r.max.y), pos2(r.min.x + t, r.max.y)], lw, col);
            cv.polyline(&[pos2(r.max.x - t, r.min.y), pos2(r.max.x, r.min.y), r.max, pos2(r.max.x - t, r.max.y)], lw, col);
        }
        ShapeKind::Parens => {
            let bulge = (r.height() * 0.12).clamp(4.0, 14.0);
            for (x, s) in [(r.min.x, -1.0f32), (r.max.x, 1.0)] {
                let pts: Vec<Pos2> = (0..=20)
                    .map(|k| {
                        let t = k as f32 / 20.0;
                        let y = r.min.y + r.height() * t;
                        let off = (t * std::f32::consts::PI).sin() * bulge * s;
                        pos2(x + off - s * bulge * 0.0, y)
                    })
                    .collect();
                cv.polyline(&pts, lw, col);
            }
        }
    }
}

/// Split caption text into runs (text, subscript?) – digits after letters become subscripts.
pub fn caption_runs(text: &str, formula: bool) -> Vec<(String, bool)> {
    if !formula {
        return vec![(text.to_string(), false)];
    }
    let mut out: Vec<(String, bool)> = Vec::new();
    let mut prev_letter = false;
    for ch in text.chars() {
        let sub = ch.is_ascii_digit() && (prev_letter || out.last().map(|x| x.1).unwrap_or(false));
        match out.last_mut() {
            Some(last) if last.1 == sub => last.0.push(ch),
            _ => out.push((ch.to_string(), sub)),
        }
        prev_letter = ch.is_alphabetic() || ch == ')';
    }
    out
}

pub fn caption_face(c: &Caption) -> Face {
    match (c.serif, c.bold, c.italic) {
        (true, true, _) => Face::SerifBold,
        (true, false, false) => Face::Serif,
        (_, true, _) => Face::Bold,
        (_, _, true) => Face::Italic,
        _ => Face::Regular,
    }
}

pub fn draw_caption(c: &Caption, cv: &mut dyn Canvas, ink: Color32) {
    let col = rgb(c.color, ink);
    if c.seq.is_some() {
        // A sequence: bold header, then numbered lines of 50 letters in blocks of 10.
        let h = cv.measure("Hg", c.size, Face::Mono).y * 1.1;
        let mut y = c.pos.y;
        for l in crate::sequence::shown_lines(&c.text) {
            if l.header {
                cv.text(pos2(c.pos.x, y), &l.text, c.size, col, Face::Bold);
            } else {
                cv.text(pos2(c.pos.x, y), &l.number, c.size, col.gamma_multiply(0.5), Face::Mono);
                let w = cv.measure(&l.number, c.size, Face::Mono).x;
                cv.text(pos2(c.pos.x + w, y), &l.text, c.size, col, Face::Mono);
            }
            y += h;
        }
        return;
    }
    let face = caption_face(c);
    let mut y = c.pos.y;
    for line in c.text.split('\n') {
        let mut x = c.pos.x;
        let h = cv.measure("Hg", c.size, face).y;
        for (run, sub) in caption_runs(line, c.formula) {
            let sz = if sub { c.size * 0.7 } else { c.size };
            let yy = if sub { y + h * 0.4 } else { y };
            cv.text(pos2(x, yy), &run, sz, col, face);
            x += cv.measure(&run, sz, face).x;
        }
        y += h;
    }
}

pub fn caption_size(c: &Caption, cv: &mut dyn Canvas) -> Vec2 {
    if c.seq.is_some() {
        let lines = crate::sequence::shown_lines(&c.text);
        let h = cv.measure("Hg", c.size, Face::Mono).y * 1.1;
        let w = lines
            .iter()
            .map(|l| if l.header { cv.measure(&l.text, c.size, Face::Bold).x } else { cv.measure(&format!("{}{}", l.number, l.text), c.size, Face::Mono).x })
            .fold(4.0f32, f32::max);
        return vec2(w, h * lines.len().max(1) as f32);
    }
    let face = caption_face(c);
    let mut w: f32 = 0.0;
    let mut h = 0.0;
    for line in c.text.split('\n') {
        let mut x = 0.0;
        for (run, sub) in caption_runs(line, c.formula) {
            x += cv.measure(&run, if sub { c.size * 0.7 } else { c.size }, face).x;
        }
        w = w.max(x);
        h += cv.measure("Hg", c.size, face).y;
    }
    vec2(w.max(4.0), h)
}

// ---------------------------------------------------------------------------
// On-screen canvas

pub struct EguiCanvas<'a> {
    pub painter: &'a egui::Painter,
    pub origin: Pos2,
    pub zoom: f32,
}

pub fn font_family(face: Face) -> egui::FontFamily {
    match face {
        Face::Regular => egui::FontFamily::Name("chem".into()),
        Face::Bold => egui::FontFamily::Name("chem-bold".into()),
        Face::Italic => egui::FontFamily::Name("chem-italic".into()),
        Face::Serif => egui::FontFamily::Name("chem-serif".into()),
        Face::SerifBold => egui::FontFamily::Name("chem-serif-bold".into()),
        Face::Mono => egui::FontFamily::Monospace,
    }
}

impl EguiCanvas<'_> {
    pub fn to_screen(&self, p: Pos2) -> Pos2 {
        self.origin + p.to_vec2() * self.zoom
    }
}

impl Canvas for EguiCanvas<'_> {
    fn line(&mut self, a: Pos2, b: Pos2, w: f32, c: Color32) {
        self.painter.line_segment([self.to_screen(a), self.to_screen(b)], (w * self.zoom, c));
    }
    fn polyline(&mut self, pts: &[Pos2], w: f32, c: Color32) {
        let p: Vec<Pos2> = pts.iter().map(|p| self.to_screen(*p)).collect();
        self.painter.add(egui::Shape::line(p, egui::Stroke::new(w * self.zoom, c)));
    }
    fn poly(&mut self, pts: &[Pos2], fill: Color32) {
        let p: Vec<Pos2> = pts.iter().map(|p| self.to_screen(*p)).collect();
        // Triangulate as a fan (shapes here are convex or nearly).
        let mut mesh = egui::Mesh::default();
        for q in &p {
            mesh.colored_vertex(*q, fill);
        }
        for k in 1..p.len().saturating_sub(1) {
            mesh.add_triangle(0, k as u32, k as u32 + 1);
        }
        self.painter.add(egui::Shape::mesh(mesh));
    }
    fn circle(&mut self, c: Pos2, r: f32, fill: Option<Color32>, stroke: Option<(f32, Color32)>) {
        let c = self.to_screen(c);
        if let Some(f) = fill {
            self.painter.circle_filled(c, r * self.zoom, f);
        }
        if let Some((w, col)) = stroke {
            self.painter.circle_stroke(c, r * self.zoom, (w * self.zoom, col));
        }
    }
    fn rect(&mut self, r: Rect, w: f32, c: Color32, rounding: f32) {
        let r = Rect::from_two_pos(self.to_screen(r.min), self.to_screen(r.max));
        self.painter.rect_stroke(r, rounding * self.zoom, (w * self.zoom, c), egui::StrokeKind::Middle);
    }
    fn ellipse(&mut self, r: Rect, w: f32, c: Color32) {
        let r = Rect::from_two_pos(self.to_screen(r.min), self.to_screen(r.max));
        self.painter.add(egui::Shape::ellipse_stroke(r.center(), r.size() / 2.0, (w * self.zoom, c)));
    }
    fn text(&mut self, pos: Pos2, s: &str, size: f32, c: Color32, face: Face) {
        if size * self.zoom < 1.0 {
            return;
        }
        let g = self.painter.layout_no_wrap(s.to_string(), egui::FontId::new(size * self.zoom, font_family(face)), c);
        self.painter.galley(self.to_screen(pos), g, c);
    }
    fn measure(&mut self, s: &str, size: f32, face: Face) -> Vec2 {
        let z = self.zoom.max(0.05);
        let g = self.painter.layout_no_wrap(s.to_string(), egui::FontId::new(size * z, font_family(face)), Color32::BLACK);
        g.size() / z
    }
}

// ---------------------------------------------------------------------------
// SVG canvas

pub struct SvgCanvas<'a> {
    pub out: String,
    pub ctx: &'a egui::Context,
    pub offset: Vec2,
}

fn hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

impl SvgCanvas<'_> {
    fn p(&self, p: Pos2) -> Pos2 {
        p + self.offset
    }
}

impl Canvas for SvgCanvas<'_> {
    fn line(&mut self, a: Pos2, b: Pos2, w: f32, c: Color32) {
        let (a, b) = (self.p(a), self.p(b));
        self.out.push_str(&format!(
            "<line x1=\"{:.2}\" y1=\"{:.2}\" x2=\"{:.2}\" y2=\"{:.2}\" stroke=\"{}\" stroke-width=\"{:.2}\" stroke-linecap=\"round\"/>\n",
            a.x, a.y, b.x, b.y, hex(c), w
        ));
    }
    fn polyline(&mut self, pts: &[Pos2], w: f32, c: Color32) {
        let s: Vec<String> = pts.iter().map(|p| { let p = self.p(*p); format!("{:.2},{:.2}", p.x, p.y) }).collect();
        self.out.push_str(&format!(
            "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.2}\" stroke-linejoin=\"round\"/>\n",
            s.join(" "), hex(c), w
        ));
    }
    fn poly(&mut self, pts: &[Pos2], fill: Color32) {
        let s: Vec<String> = pts.iter().map(|p| { let p = self.p(*p); format!("{:.2},{:.2}", p.x, p.y) }).collect();
        self.out.push_str(&format!("<polygon points=\"{}\" fill=\"{}\"/>\n", s.join(" "), hex(fill)));
    }
    fn circle(&mut self, c: Pos2, r: f32, fill: Option<Color32>, stroke: Option<(f32, Color32)>) {
        let c = self.p(c);
        let f = fill.map(hex).unwrap_or("none".into());
        let (sw, sc) = stroke.map(|(w, c)| (w, hex(c))).unwrap_or((0.0, "none".into()));
        self.out.push_str(&format!("<circle cx=\"{:.2}\" cy=\"{:.2}\" r=\"{:.2}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{:.2}\"/>\n", c.x, c.y, r, f, sc, sw));
    }
    fn rect(&mut self, r: Rect, w: f32, c: Color32, rounding: f32) {
        let m = self.p(r.min);
        self.out.push_str(&format!(
            "<rect x=\"{:.2}\" y=\"{:.2}\" width=\"{:.2}\" height=\"{:.2}\" rx=\"{:.2}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.2}\"/>\n",
            m.x, m.y, r.width(), r.height(), rounding, hex(c), w
        ));
    }
    fn ellipse(&mut self, r: Rect, w: f32, c: Color32) {
        let ce = self.p(r.center());
        self.out.push_str(&format!(
            "<ellipse cx=\"{:.2}\" cy=\"{:.2}\" rx=\"{:.2}\" ry=\"{:.2}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{:.2}\"/>\n",
            ce.x, ce.y, r.width() / 2.0, r.height() / 2.0, hex(c), w
        ));
    }
    fn text(&mut self, pos: Pos2, s: &str, size: f32, c: Color32, face: Face) {
        let p = self.p(pos);
        let h = self.measure("C", size, face).y;
        let weight = if matches!(face, Face::Bold | Face::SerifBold) { " font-weight=\"bold\"" } else { "" };
        let style = if face == Face::Italic { " font-style=\"italic\"" } else { "" };
        let family = match face {
            Face::Serif | Face::SerifBold => "Times New Roman, Times, serif",
            Face::Mono => "Courier New, Courier, monospace",
            _ => "Arial, Helvetica, sans-serif",
        };
        self.out.push_str(&format!(
            "<text x=\"{:.2}\" y=\"{:.2}\" font-family=\"{}\" font-size=\"{:.2}\" fill=\"{}\"{}{}>{}</text>\n",
            p.x, p.y + h * 0.78, family, size, hex(c), weight, style, esc(s)
        ));
    }
    fn measure(&mut self, s: &str, size: f32, face: Face) -> Vec2 {
        self.ctx.fonts_mut(|f| f.layout_no_wrap(s.to_string(), egui::FontId::new(size, font_family(face)), Color32::BLACK).size())
    }
}

pub fn to_svg(doc: &Doc, ctx: &egui::Context, color_atoms: bool) -> String {
    let bounds = doc.bounds_of(None).unwrap_or(Rect::from_min_size(Pos2::ZERO, vec2(100.0, 100.0))).expand(BOND_LEN);
    let mut cv = SvgCanvas { out: String::new(), ctx, offset: -bounds.min.to_vec2() };
    draw_doc(doc, &mut cv, &RenderOpts { color_atoms, warnings: false });
    // Printed size follows the style's bond length.
    let k = doc.style.bond_length / BOND_LEN;
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{:.1}pt\" height=\"{:.1}pt\" viewBox=\"0 0 {:.2} {:.2}\">\n<rect width=\"100%\" height=\"100%\" fill=\"{}\"/>\n{}</svg>\n",
        bounds.width() * k, bounds.height() * k, bounds.width(), bounds.height(), hex(doc.style.paper()), cv.out
    )
}

#[allow(dead_code)]
pub fn bond_set(doc: &Doc, sel: &Selection) -> HashSet<usize> {
    doc.bonds
        .iter()
        .enumerate()
        .filter(|(_, b)| sel.atoms.contains(&b.a) && sel.atoms.contains(&b.b))
        .map(|(i, _)| i)
        .collect()
}
