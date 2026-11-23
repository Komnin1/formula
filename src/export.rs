//! Vector exports (PDF, Encapsulated PostScript) and raster pictures (PNG, JPEG, GIF, BMP, TIFF).

use crate::doc::*;
use crate::render::{self, Canvas, Face};
use egui::{Color32, Pos2, Rect, Vec2, pos2, vec2};

/// Shared drawing for PDF and PostScript (both have y pointing up).
struct PageCanvas<'a> {
    ops: String,
    ctx: &'a egui::Context,
    height: f32,
    offset: Vec2,
    ps: bool,
    /// Points per drawing unit (from the style's bond length).
    k: f32,
}

impl PageCanvas<'_> {
    fn p(&self, p: Pos2) -> (f32, f32) {
        let q = (p + self.offset) * self.k;
        (q.x, self.height - q.y)
    }
    fn color(&mut self, c: Color32, stroke: bool) {
        let (r, g, b) = (c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0);
        if self.ps {
            self.ops.push_str(&format!("{:.3} {:.3} {:.3} setrgbcolor\n", r, g, b));
        } else if stroke {
            self.ops.push_str(&format!("{:.3} {:.3} {:.3} RG\n", r, g, b));
        } else {
            self.ops.push_str(&format!("{:.3} {:.3} {:.3} rg\n", r, g, b));
        }
    }
    fn path(&mut self, pts: &[Pos2], close: bool) {
        for (k, p) in pts.iter().enumerate() {
            let (x, y) = self.p(*p);
            let op = match (self.ps, k == 0) {
                (true, true) => "moveto",
                (true, false) => "lineto",
                (false, true) => "m",
                (false, false) => "l",
            };
            self.ops.push_str(&format!("{:.2} {:.2} {}\n", x, y, op));
        }
        if close {
            self.ops.push_str(if self.ps { "closepath\n" } else { "h\n" });
        }
    }
    fn stroke(&mut self, w: f32) {
        let w = w * self.k;
        if self.ps {
            self.ops.push_str(&format!("{:.2} setlinewidth 1 setlinecap 1 setlinejoin stroke\n", w));
        } else {
            self.ops.push_str(&format!("{:.2} w 1 J 1 j S\n", w));
        }
    }
    fn fill(&mut self) {
        self.ops.push_str(if self.ps { "fill\n" } else { "f\n" });
    }
}

fn ps_escape(s: &str) -> String {
    let mut o = String::new();
    for ch in s.chars() {
        match ch {
            '(' | ')' | '\\' => {
                o.push('\\');
                o.push(ch);
            }
            '−' => o.push('-'),
            '•' => o.push_str("\\267"),
            c if (c as u32) < 128 => o.push(c),
            c if (c as u32) < 256 => o.push_str(&format!("\\{:03o}", c as u32)),
            _ => o.push('?'),
        }
    }
    o
}

fn ellipse_points(r: Rect, n: usize) -> Vec<Pos2> {
    (0..n)
        .map(|k| {
            let a = std::f32::consts::TAU * k as f32 / n as f32;
            r.center() + vec2(a.cos() * r.width() / 2.0, a.sin() * r.height() / 2.0)
        })
        .collect()
}

impl Canvas for PageCanvas<'_> {
    fn line(&mut self, a: Pos2, b: Pos2, w: f32, c: Color32) {
        self.color(c, true);
        self.path(&[a, b], false);
        self.stroke(w);
    }
    fn polyline(&mut self, pts: &[Pos2], w: f32, c: Color32) {
        self.color(c, true);
        self.path(pts, false);
        self.stroke(w);
    }
    fn poly(&mut self, pts: &[Pos2], fill: Color32) {
        self.color(fill, false);
        self.path(pts, true);
        self.fill();
    }
    fn circle(&mut self, c: Pos2, r: f32, fill: Option<Color32>, stroke: Option<(f32, Color32)>) {
        let pts = ellipse_points(Rect::from_center_size(c, vec2(r * 2.0, r * 2.0)), 32);
        if let Some(f) = fill {
            self.poly(&pts, f);
        }
        if let Some((w, col)) = stroke {
            self.color(col, true);
            self.path(&pts, true);
            self.stroke(w);
        }
    }
    fn rect(&mut self, r: Rect, w: f32, c: Color32, _rounding: f32) {
        self.color(c, true);
        self.path(&[r.min, pos2(r.max.x, r.min.y), r.max, pos2(r.min.x, r.max.y)], true);
        self.stroke(w);
    }
    fn ellipse(&mut self, r: Rect, w: f32, c: Color32) {
        self.color(c, true);
        self.path(&ellipse_points(r, 48), true);
        self.stroke(w);
    }
    fn text(&mut self, pos: Pos2, s: &str, size: f32, c: Color32, face: Face) {
        let h = self.measure("C", size, face).y;
        let (x, y) = self.p(pos2(pos.x, pos.y + h * 0.78));
        let font = match face {
            Face::Regular => "F1",
            Face::Bold => "F2",
            Face::Italic => "F3",
            Face::Serif => "F4",
            Face::SerifBold => "F5",
            Face::Mono => "F6",
        };
        let size = size * self.k;
        self.color(c, false);
        if self.ps {
            let ps_font = match face {
                Face::Regular => "/Helvetica",
                Face::Bold => "/Helvetica-Bold",
                Face::Italic => "/Helvetica-Oblique",
                Face::Serif => "/Times-Roman",
                Face::SerifBold => "/Times-Bold",
                Face::Mono => "/Courier",
            };
            self.ops.push_str(&format!("{} findfont {:.2} scalefont setfont {:.2} {:.2} moveto ({}) show\n", ps_font, size, x, y, ps_escape(s)));
        } else {
            self.ops.push_str(&format!("BT /{} {:.2} Tf {:.2} {:.2} Td ({}) Tj ET\n", font, size, x, y, ps_escape(s)));
        }
    }
    fn measure(&mut self, s: &str, size: f32, face: Face) -> Vec2 {
        self.ctx.fonts_mut(|f| f.layout_no_wrap(s.to_string(), egui::FontId::new(size, render::font_family(face)), Color32::BLACK).size())
    }
}

pub(crate) fn page_bounds(doc: &Doc) -> Rect {
    doc.bounds_of(None).unwrap_or(Rect::from_min_size(Pos2::ZERO, vec2(100.0, 100.0))).expand(BOND_LEN * 0.8)
}

/// Drawing coordinates are scaled to points so a standard bond has the style's printed length.
fn draw_ops(doc: &Doc, ctx: &egui::Context, ps: bool, color_atoms: bool) -> (String, f32, f32) {
    let b = page_bounds(doc);
    let k = doc.style.bond_length.max(1.0) / BOND_LEN;
    let (w, h) = (b.width() * k, b.height() * k);
    let mut cv = PageCanvas { ops: String::new(), ctx, height: h, offset: -b.min.to_vec2(), ps, k };
    if doc.style.dark {
        // Background first.
        cv.poly(&[b.min, pos2(b.max.x, b.min.y), b.max, pos2(b.min.x, b.max.y)], doc.style.paper());
    }
    render::draw_doc(doc, &mut cv, &render::RenderOpts { color_atoms, warnings: false });
    (cv.ops, w, h)
}

pub fn to_pdf(doc: &Doc, ctx: &egui::Context, color_atoms: bool) -> Vec<u8> {
    to_pdf_with_data(doc, ctx, color_atoms, None)
}

/// Vendor tag on the metadata stream that carries the structure. Chemistry drawing programs on
/// the Mac use this tag for the CDXML copy of a pasted picture, so the same picture can be
/// turned back into an editable structure by them and by this app.
pub const PDF_STRUCTURE_VENDOR: &str = "com.cambridgesoft.chemdraw.12";

/// PDF picture; with `cdxml` the structure is stored in the document's metadata stream.
pub fn to_pdf_with_data(doc: &Doc, ctx: &egui::Context, color_atoms: bool, cdxml: Option<&str>) -> Vec<u8> {
    let (ops, w, h) = draw_ops(doc, ctx, false, color_atoms);
    let mut objs: Vec<String> = Vec::new();
    objs.push(if cdxml.is_some() { "<< /Type /Catalog /Pages 2 0 R /Metadata 10 0 R >>".into() } else { "<< /Type /Catalog /Pages 2 0 R >>".into() });
    objs.push("<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into());
    // The fixed-width font comes last, after the optional metadata (object 10).
    let mono = if cdxml.is_some() { 11 } else { 10 };
    objs.push(format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] /Resources << /Font << /F1 5 0 R /F2 6 0 R /F3 7 0 R /F4 8 0 R /F5 9 0 R /F6 {} 0 R >> >> /Contents 4 0 R >>",
        w, h, mono
    ));
    objs.push(format!("<< /Length {} >>\nstream\n{}endstream", ops.len(), ops));
    for f in ["Helvetica", "Helvetica-Bold", "Helvetica-Oblique", "Times-Roman", "Times-Bold"] {
        objs.push(format!("<< /Type /Font /Subtype /Type1 /BaseFont /{} /Encoding /WinAnsiEncoding >>", f));
    }
    if let Some(x) = cdxml {
        objs.push(format!("<< /Type /Metadata /Subtype /XML /Vendor /{} /Length {} >>\nstream\n{}\nendstream", PDF_STRUCTURE_VENDOR, x.len(), x));
    }
    objs.push("<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding >>".into());
    let mut out = String::from("%PDF-1.4\n%\u{00e2}\u{00e3}\n");
    let mut offsets = Vec::new();
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n");
    out.clear();
    for (k, o) in objs.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", k + 1, o).as_bytes());
    }
    let xref = bytes.len();
    let mut x = format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1);
    for o in offsets {
        x.push_str(&format!("{:010} 00000 n \n", o));
    }
    x.push_str(&format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n", objs.len() + 1, xref));
    bytes.extend_from_slice(x.as_bytes());
    bytes
}

pub fn to_eps(doc: &Doc, ctx: &egui::Context, color_atoms: bool) -> String {
    let (ops, w, h) = draw_ops(doc, ctx, true, color_atoms);
    format!(
        "%!PS-Adobe-3.0 EPSF-3.0\n%%BoundingBox: 0 0 {} {}\n%%HiResBoundingBox: 0 0 {:.2} {:.2}\n%%Creator: Formula\n%%EndComments\ngsave\n{}grestore\nshowpage\n%%EOF\n",
        w.ceil() as i32, h.ceil() as i32, w, h, ops
    )
}

/// Save a captured picture in the format given by the file extension.
pub fn save_image(img: &egui::ColorImage, crop: Option<Rect>, path: &std::path::Path) -> Result<(), String> {
    let [w, h] = img.size;
    let (x0, y0, x1, y1) = match crop {
        Some(r) => (
            (r.min.x.max(0.0) as usize).min(w),
            (r.min.y.max(0.0) as usize).min(h),
            (r.max.x.max(0.0) as usize).min(w),
            (r.max.y.max(0.0) as usize).min(h),
        ),
        None => (0, 0, w, h),
    };
    if x1 <= x0 || y1 <= y0 {
        return Err("nothing to save".into());
    }
    let mut buf = image::RgbaImage::new((x1 - x0) as u32, (y1 - y0) as u32);
    for y in y0..y1 {
        for x in x0..x1 {
            let c = img.pixels[y * w + x];
            buf.put_pixel((x - x0) as u32, (y - y0) as u32, image::Rgba([c.r(), c.g(), c.b(), 255]));
        }
    }
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("png").to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" | "bmp" => image::DynamicImage::ImageRgba8(buf).to_rgb8().save(path).map_err(|e| e.to_string()),
        _ => buf.save(path).map_err(|e| e.to_string()),
    }
}
