//! Windows Enhanced Metafile (EMF) pictures: the vector picture Windows programs such as Word
//! take from the clipboard. The structure itself rides along inside the picture as a comment
//! record (CDXML text), so a picture copied back out of a document can be edited again.

use crate::doc::*;
use crate::render::{self, Canvas, Face};
use egui::{Color32, Pos2, Rect, Vec2, pos2, vec2};

/// Logical units per point (1/20 pt keeps lines smooth with whole-number coordinates).
const LPP: f32 = 20.0;
/// Marker at the start of the comment that carries the structure.
pub const COMMENT_TAG: &[u8] = b"FMLA";

const EMR_HEADER: u32 = 1;
const EMR_POLYGON: u32 = 3;
const EMR_POLYLINE: u32 = 4;
const EMR_SETWINDOWEXTEX: u32 = 9;
const EMR_SETWINDOWORGEX: u32 = 10;
const EMR_SETVIEWPORTEXTEX: u32 = 11;
const EMR_SETVIEWPORTORGEX: u32 = 12;
const EMR_EOF: u32 = 14;
const EMR_SETMAPMODE: u32 = 17;
const EMR_SETBKMODE: u32 = 18;
const EMR_SETTEXTALIGN: u32 = 22;
const EMR_SETTEXTCOLOR: u32 = 24;
const EMR_SELECTOBJECT: u32 = 37;
const EMR_CREATEBRUSHINDIRECT: u32 = 39;
const EMR_DELETEOBJECT: u32 = 40;
const EMR_GDICOMMENT: u32 = 70;
const EMR_EXTCREATEFONTINDIRECTW: u32 = 82;
const EMR_EXTTEXTOUTW: u32 = 84;
const EMR_EXTCREATEPEN: u32 = 95;

const NULL_BRUSH: u32 = 0x8000_0005;
const NULL_PEN: u32 = 0x8000_0008;
const DEFAULT_GUI_FONT: u32 = 0x8000_0011;
const H_PEN: u32 = 1;
const H_BRUSH: u32 = 2;
const H_FONT: u32 = 3;

struct EmfCanvas<'a> {
    out: Vec<u8>,
    records: u32,
    ctx: &'a egui::Context,
    offset: Vec2,
    /// Points per drawing unit.
    k: f32,
}

fn le(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn lei(out: &mut Vec<u8>, v: i32) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn colorref(c: Color32) -> u32 {
    c.r() as u32 | (c.g() as u32) << 8 | (c.b() as u32) << 16
}

impl EmfCanvas<'_> {
    fn rec(&mut self, ty: u32, body: &[u8]) {
        let size = (8 + body.len()).div_ceil(4) * 4;
        le(&mut self.out, ty);
        le(&mut self.out, size as u32);
        self.out.extend_from_slice(body);
        self.out.resize(self.out.len() + size - 8 - body.len(), 0);
        self.records += 1;
    }
    fn rec_u32s(&mut self, ty: u32, vals: &[u32]) {
        let mut b = Vec::new();
        for v in vals {
            le(&mut b, *v);
        }
        self.rec(ty, &b);
    }
    fn l(&self, p: Pos2) -> (i32, i32) {
        let q = (p + self.offset) * self.k * LPP;
        (q.x.round() as i32, q.y.round() as i32)
    }
    fn pen(&mut self, w: f32, c: Color32) {
        // Geometric solid pen with round caps and joins.
        let mut b = Vec::new();
        for v in [H_PEN, 0, 0, 0, 0, 0x0001_0000, ((w * self.k * LPP).round() as u32).max(1), 0, colorref(c), 0, 0] {
            le(&mut b, v);
        }
        self.rec(EMR_EXTCREATEPEN, &b);
        self.rec_u32s(EMR_SELECTOBJECT, &[H_PEN]);
        self.rec_u32s(EMR_SELECTOBJECT, &[NULL_BRUSH]);
    }
    fn drop_pen(&mut self) {
        self.rec_u32s(EMR_SELECTOBJECT, &[NULL_PEN]);
        self.rec_u32s(EMR_DELETEOBJECT, &[H_PEN]);
    }
    fn points(&mut self, ty: u32, pts: &[Pos2]) {
        if pts.len() < 2 {
            return;
        }
        let ls: Vec<(i32, i32)> = pts.iter().map(|p| self.l(*p)).collect();
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for &(x, y) in &ls {
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        let mut b = Vec::new();
        // Bounds in device pixels (96 per inch).
        let px = |v: i32| (v as f32 / LPP * 96.0 / 72.0).round() as i32;
        for v in [px(x0), px(y0), px(x1), px(y1)] {
            lei(&mut b, v);
        }
        le(&mut b, ls.len() as u32);
        for (x, y) in ls {
            lei(&mut b, x);
            lei(&mut b, y);
        }
        self.rec(ty, &b);
    }
    fn stroke_path(&mut self, pts: &[Pos2], w: f32, c: Color32) {
        self.pen(w, c);
        self.points(EMR_POLYLINE, pts);
        self.drop_pen();
    }
}

fn ellipse_points(r: Rect, n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|k| {
            let a = std::f32::consts::TAU * k as f32 / n as f32;
            r.center() + vec2(a.cos() * r.width() / 2.0, a.sin() * r.height() / 2.0)
        })
        .collect()
}

impl Canvas for EmfCanvas<'_> {
    fn line(&mut self, a: Pos2, b: Pos2, w: f32, c: Color32) {
        self.stroke_path(&[a, b], w, c);
    }
    fn polyline(&mut self, pts: &[Pos2], w: f32, c: Color32) {
        self.stroke_path(pts, w, c);
    }
    fn poly(&mut self, pts: &[Pos2], fill: Color32) {
        self.rec_u32s(EMR_SELECTOBJECT, &[NULL_PEN]);
        self.rec_u32s(EMR_CREATEBRUSHINDIRECT, &[H_BRUSH, 0, colorref(fill), 0]);
        self.rec_u32s(EMR_SELECTOBJECT, &[H_BRUSH]);
        self.points(EMR_POLYGON, pts);
        self.rec_u32s(EMR_SELECTOBJECT, &[NULL_BRUSH]);
        self.rec_u32s(EMR_DELETEOBJECT, &[H_BRUSH]);
    }
    fn circle(&mut self, c: Pos2, r: f32, fill: Option<Color32>, stroke: Option<(f32, Color32)>) {
        let pts = ellipse_points(Rect::from_center_size(c, vec2(r * 2.0, r * 2.0)), 32);
        if let Some(f) = fill {
            self.poly(&pts, f);
        }
        if let Some((w, col)) = stroke {
            self.stroke_path(&pts, w, col);
        }
    }
    fn rect(&mut self, r: Rect, w: f32, c: Color32, _rounding: f32) {
        self.stroke_path(&[r.min, pos2(r.max.x, r.min.y), r.max, pos2(r.min.x, r.max.y), r.min], w, c);
    }
    fn ellipse(&mut self, r: Rect, w: f32, c: Color32) {
        self.stroke_path(&ellipse_points(r, 48), w, c);
    }
    fn text(&mut self, pos: Pos2, s: &str, size: f32, c: Color32, face: Face) {
        let h = self.measure("C", size, face).y;
        let (x, y) = self.l(pos2(pos.x, pos.y + h * 0.78));
        // Font (LOGFONTW).
        let mut b = Vec::new();
        le(&mut b, H_FONT);
        lei(&mut b, -((size * self.k * LPP).round() as i32));
        for v in [0i32, 0, 0] {
            lei(&mut b, v);
        }
        let bold = matches!(face, Face::Bold | Face::SerifBold);
        lei(&mut b, if bold { 700 } else { 400 });
        b.extend_from_slice(&[matches!(face, Face::Italic) as u8, 0, 0, 1, 0, 0, 4, 0]);
        let name = match face {
            Face::Serif | Face::SerifBold => "Times New Roman",
            Face::Mono => "Courier New",
            _ => "Arial",
        };
        let mut wide: Vec<u16> = name.encode_utf16().collect();
        wide.resize(32, 0);
        for u in wide {
            b.extend_from_slice(&u.to_le_bytes());
        }
        self.rec(EMR_EXTCREATEFONTINDIRECTW, &b);
        self.rec_u32s(EMR_SELECTOBJECT, &[H_FONT]);
        self.rec_u32s(EMR_SETTEXTCOLOR, &[colorref(c)]);
        // Text with explicit advances so it lines up like on screen.
        let chars: Vec<char> = s.chars().map(|ch| if (ch as u32) > 0xFFFF { '?' } else { ch }).collect();
        let units: Vec<u16> = chars.iter().map(|&ch| if ch == '−' { '-' as u16 } else { ch as u16 }).collect();
        let dx: Vec<u32> = chars.iter().map(|ch| (self.measure(&ch.to_string(), size, face).x * self.k * LPP).round().max(0.0) as u32).collect();
        let n = units.len() as u32;
        let off_string = 76u32;
        let str_bytes = (units.len() * 2).div_ceil(4) * 4;
        let off_dx = off_string + str_bytes as u32;
        let mut t = Vec::new();
        let w: i32 = dx.iter().sum::<u32>() as i32;
        let px = |v: i32| (v as f32 / LPP * 96.0 / 72.0).round() as i32;
        for v in [px(x), px(y) - px((size * self.k * LPP) as i32), px(x + w), px(y)] {
            lei(&mut t, v);
        }
        le(&mut t, 1); // GM_COMPATIBLE
        // Page units to 0.01 mm.
        let scale = 2540.0f32 / 72.0 / LPP;
        t.extend_from_slice(&scale.to_le_bytes());
        t.extend_from_slice(&scale.to_le_bytes());
        lei(&mut t, x);
        lei(&mut t, y);
        le(&mut t, n);
        le(&mut t, off_string);
        le(&mut t, 0); // options
        for _ in 0..4 {
            lei(&mut t, 0);
        }
        le(&mut t, off_dx);
        for u in &units {
            t.extend_from_slice(&u.to_le_bytes());
        }
        t.resize((off_string - 8) as usize + str_bytes, 0);
        for d in dx {
            le(&mut t, d);
        }
        self.rec(EMR_EXTTEXTOUTW, &t);
        self.rec_u32s(EMR_SELECTOBJECT, &[DEFAULT_GUI_FONT]);
        self.rec_u32s(EMR_DELETEOBJECT, &[H_FONT]);
    }
    fn measure(&mut self, s: &str, size: f32, face: Face) -> Vec2 {
        self.ctx.fonts_mut(|f| f.layout_no_wrap(s.to_string(), egui::FontId::new(size, render::font_family(face)), Color32::BLACK).size())
    }
}

/// The drawing as an EMF picture at its printed size. `data` (the structure as CDXML) is stored
/// in a comment record so the picture can be turned back into a structure.
pub fn to_emf(doc: &Doc, ctx: &egui::Context, color_atoms: bool, data: Option<&[u8]>) -> Vec<u8> {
    let b = crate::export::page_bounds(doc);
    let k = doc.style.bond_length.max(1.0) / BOND_LEN;
    let (wpt, hpt) = (b.width() * k, b.height() * k);
    let mut cv = EmfCanvas { out: Vec::new(), records: 0, ctx, offset: -b.min.to_vec2(), k };
    let (wl, hl) = ((wpt * LPP).round() as u32, (hpt * LPP).round() as u32);
    let (wpx, hpx) = ((wpt * 96.0 / 72.0).round() as u32, (hpt * 96.0 / 72.0).round() as u32);
    cv.rec_u32s(EMR_SETMAPMODE, &[8]); // MM_ANISOTROPIC
    cv.rec_u32s(EMR_SETWINDOWORGEX, &[0, 0]);
    cv.rec_u32s(EMR_SETWINDOWEXTEX, &[wl.max(1), hl.max(1)]);
    cv.rec_u32s(EMR_SETVIEWPORTORGEX, &[0, 0]);
    cv.rec_u32s(EMR_SETVIEWPORTEXTEX, &[wpx.max(1), hpx.max(1)]);
    cv.rec_u32s(EMR_SETBKMODE, &[1]); // transparent
    cv.rec_u32s(EMR_SETTEXTALIGN, &[24]); // baseline, left
    if let Some(d) = data {
        let mut c = Vec::new();
        le(&mut c, (COMMENT_TAG.len() + d.len()) as u32);
        c.extend_from_slice(COMMENT_TAG);
        c.extend_from_slice(d);
        cv.rec(EMR_GDICOMMENT, &c);
    }
    if doc.style.dark {
        cv.poly(&[b.min, pos2(b.max.x, b.min.y), b.max, pos2(b.min.x, b.max.y)], doc.style.paper());
    }
    render::draw_doc(doc, &mut cv, &render::RenderOpts { color_atoms, warnings: false });
    cv.rec_u32s(EMR_EOF, &[0, 16, 20]);
    let body = std::mem::take(&mut cv.out);
    let records = cv.records + 1;

    let mut h = Vec::new();
    le(&mut h, EMR_HEADER);
    le(&mut h, 108);
    for v in [0i32, 0, wpx as i32 - 1, hpx as i32 - 1] {
        lei(&mut h, v); // bounds, device pixels
    }
    let hmm = |pt: f32| (pt * 2540.0 / 72.0).round() as i32;
    for v in [0, 0, hmm(wpt) - 1, hmm(hpt) - 1] {
        lei(&mut h, v); // frame, 0.01 mm
    }
    le(&mut h, 0x464D_4520); // " EMF"
    le(&mut h, 0x0001_0000);
    le(&mut h, (108 + body.len()) as u32);
    le(&mut h, records);
    h.extend_from_slice(&4u16.to_le_bytes()); // handles (incl. slot 0)
    h.extend_from_slice(&0u16.to_le_bytes());
    for v in [0u32, 0, 0] {
        le(&mut h, v); // description, palette
    }
    for v in [1920u32, 1080, 508, 286] {
        le(&mut h, v); // reference device: pixels and millimetres (96 dpi)
    }
    for v in [0u32, 0, 0] {
        le(&mut h, v); // pixel format, OpenGL
    }
    for v in [508_000u32, 285_750] {
        le(&mut h, v);
    }
    debug_assert_eq!(h.len(), 108);
    h.extend_from_slice(&body);
    h
}

/// Every comment record's data in an EMF (used to find a structure stored in a picture).
pub fn comments(emf: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    if emf.len() < 8 || u32::from_le_bytes([emf[0], emf[1], emf[2], emf[3]]) != EMR_HEADER {
        return out;
    }
    let mut off = 0usize;
    while off + 8 <= emf.len() {
        let ty = u32::from_le_bytes(emf[off..off + 4].try_into().unwrap());
        let size = u32::from_le_bytes(emf[off + 4..off + 8].try_into().unwrap()) as usize;
        if size < 8 || off + size > emf.len() {
            break;
        }
        if ty == EMR_GDICOMMENT && size >= 12 {
            let n = u32::from_le_bytes(emf[off + 8..off + 12].try_into().unwrap()) as usize;
            let end = (off + 12 + n).min(off + size);
            out.push(&emf[off + 12..end]);
        }
        if ty == EMR_EOF {
            break;
        }
        off += size;
    }
    out
}
