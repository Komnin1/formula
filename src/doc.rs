//! The drawing: atoms, bonds, arrows, captions and shapes, plus editing helpers.

use egui::{Pos2, Vec2, pos2, vec2};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashSet};

pub const BOND_LEN: f32 = 30.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Stereo {
    #[default]
    None,
    Wedge,
    Hash,
    Wavy,
    Bold,
    Dashed,
    /// Hashed bond of even width (not a wedge).
    Hashed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum DblPos {
    #[default]
    Auto,
    Left,
    Right,
    Center,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArrowKind {
    Reaction,
    Line,
    Equilibrium,
    Retro,
    Resonance,
    Curved,
    Dashed,
    NoGo,
    /// An outlined block arrow.
    Hollow,
    /// Equilibrium lying to one side: the reverse arrow is shorter.
    Unbalanced,
    /// A reaction arrow crossed by two slashes.
    NoGoHash,
    /// Rotation about an axis: an elliptical loop with a head.
    Rotation,
}

/// How big an arrowhead is (the three columns of the arrow palette).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HeadSize {
    #[default]
    Large,
    Medium,
    Small,
}

impl HeadSize {
    pub fn scale(self) -> f32 {
        match self {
            HeadSize::Large => 1.0,
            HeadSize::Medium => 0.78,
            HeadSize::Small => 0.58,
        }
    }
}

/// Which side of the arrowhead is drawn: both (full) or one barb (a half head, as for
/// one-electron "fishhook" arrows).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Half {
    #[default]
    Full,
    /// The barb on the left of the direction of travel (above a left-to-right arrow).
    Left,
    Right,
}

/// Extra look of an arrow beyond its kind. All default values give the plain arrow.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct ArrowStyle {
    #[serde(default)]
    pub size: HeadSize,
    /// A long, wide head.
    #[serde(default)]
    pub wide: bool,
    #[serde(default)]
    pub half: Half,
    /// Curved arrows: the angle the arc turns through, in degrees; the sign picks the side it
    /// bulges to. 0 is the gentle default curve. Rotation arrows: the sign is the direction.
    #[serde(default)]
    pub arc: f32,
    /// A head at the tail end as well (curved arrows).
    #[serde(default)]
    pub both: bool,
}

impl ArrowStyle {
    pub fn is_default(&self) -> bool {
        *self == ArrowStyle::default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ShapeKind {
    #[default]
    Rect,
    RoundRect,
    Ellipse,
    Brackets,
    Parens,
}

pub type Rgb = [u8; 3];

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Atom {
    pub pos: Pos2,
    /// Atomic number; 0 means a free-text label (see `label`).
    pub el: u8,
    #[serde(default)]
    pub charge: i8,
    #[serde(default)]
    pub isotope: u16,
    #[serde(default)]
    pub radical: u8,
    /// Group label such as "OMe" or "CO2H" (formula comes from the label).
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub color: Option<Rgb>,
    /// Data from the opened file that this app does not edit, written back on save.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<Box<Extra>>,
    /// Show this atom's symbol with its hydrogens (a carbon written "CH3"), the count
    /// following its bonds: CH4 → CH3 → CH2 → CH as bonds are added.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub show: bool,
    /// A unit of a biopolymer sequence (amino acid, sugar, base, phosphate, linker), or one
    /// of the sequence's end caps: drawn as a coloured shape with its code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monomer: Option<Box<Monomer>>,
    /// Size relative to the document style (1: as set): a structure scaled with Object ▸ Scale
    /// or the selection's corners draws its labels, bond widths and spacing this much larger.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub size: f32,
}

fn one() -> f32 {
    1.0
}

fn is_one(v: &f32) -> bool {
    (*v - 1.0).abs() < 1e-6
}

/// What a monomer of a biopolymer is, as the original's HELM editor stores it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Monomer {
    pub role: MonomerRole,
    /// Polymer type: peptide (amino acids), nucleic acid (sugars, bases, phosphates), or a
    /// chemical linker.
    pub polymer: Polymer,
    /// Backbone unit (amino acid, sugar, phosphate) or branch (nucleobase).
    #[serde(default)]
    pub branch: bool,
    pub symbol: String,
    #[serde(default)]
    pub name: String,
    /// The natural monomer it is a variant of ("A" for N-methylalanine); picks the colour.
    #[serde(default)]
    pub analog: String,
    /// Formula of the residue without its attachment caps ("C3H5NO" for alanine); empty when
    /// it has none (ambiguity codes such as X).
    #[serde(default)]
    pub formula: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonomerRole {
    #[default]
    Unit,
    /// The cap at either end of a chain ("H", "OH").
    Terminus,
    /// The cap on an unused side attachment (hidden, as the original does).
    Branch,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Polymer {
    #[default]
    Peptide,
    Nucleic,
    Chem,
}

impl Monomer {
    /// The shape it is drawn as.
    pub fn shape(&self) -> MonomerShape {
        match (self.polymer, self.branch) {
            (Polymer::Peptide, _) => MonomerShape::Circle,
            (Polymer::Nucleic, true) => MonomerShape::Trapezoid,
            (Polymer::Nucleic, false) if self.is_phosphate() => MonomerShape::Diamond,
            (Polymer::Nucleic, false) => MonomerShape::Heptagon,
            (Polymer::Chem, _) => MonomerShape::Rect,
        }
    }
    /// Phosphates and other linkers of the nucleic-acid backbone.
    pub fn is_phosphate(&self) -> bool {
        let n = self.name.to_ascii_lowercase();
        self.symbol == "P" || self.symbol == "sP" || n.contains("phosph") || n.contains("linker")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MonomerShape {
    Circle,
    Heptagon,
    Diamond,
    Trapezoid,
    Rect,
}

impl Atom {
    pub fn new(pos: Pos2, el: u8) -> Self {
        Atom { pos, el, charge: 0, isotope: 0, radical: 0, label: None, color: None, extra: None, show: false, monomer: None, size: 1.0 }
    }

    /// A label that only names one element and its hydrogens ("CH3", "H3C", "CH2", "NH2",
    /// "OH") becomes that element, shown, so the number of hydrogens follows the bonds
    /// instead of staying as written.
    pub fn simplify_label(&mut self) {
        let Some(mut l) = self.label.as_deref().map(|l| l.trim().to_string()) else { return };
        // Monomer codes ("I" is isoleucine, not iodine) and sequence caps stay as they are.
        if l == "D" || l == "T" || self.monomer.is_some() {
            return;
        }
        // A charge written at the end: "NH3+", "PH4+", "O-", "Fe3+" (a digit right after an
        // H counts hydrogens, so "NH3+" is +1, not +3).
        let mut charge = 0i8;
        if l.chars().count() > 1 && l.ends_with(['+', '-', '−']) {
            let sign: i8 = if l.ends_with('+') { 1 } else { -1 };
            l.pop();
            let mut digits = String::new();
            while l.len() > 1 && l.ends_with(|c: char| c.is_ascii_digit()) && !l[..l.len() - 1].ends_with('H') {
                digits.insert(0, l.pop().unwrap());
            }
            charge = sign * digits.parse::<i8>().unwrap_or(1);
        }
        let (z, rest) = crate::cdx::interpret_label(&l);
        if rest.is_none() && z > 0 {
            self.el = z;
            self.label = None;
            self.show = l != "C" || self.show;
            if charge != 0 && self.charge == 0 {
                self.charge = charge;
            }
        }
    }
}

/// Pass-through data kept from an opened CDX or CDXML document.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Extra {
    pub id: u32,
    /// Fragment the atom belonged to in the file.
    pub frag: u32,
    pub cdx_props: Vec<(u16, Vec<u8>)>,
    pub cdx_children: Vec<Vec<u8>>,
    pub xml_attrs: Vec<(String, String)>,
    pub xml_children: Vec<String>,
    /// Label (or arrow kind) at load time; nested data is only reused if it is unchanged.
    pub source_label: Option<String>,
    /// Id of the atom's label text in the file, so saving again keeps the same ids.
    #[serde(default)]
    pub text_id: u32,
}

/// File-level pass-through data.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct FileExtra {
    /// Our coordinates = file coordinates * scale.
    pub scale: f32,
    pub colors: Vec<Rgb>,
    pub cdx_doc_props: Vec<(u16, Vec<u8>)>,
    pub cdx_doc_children: Vec<Vec<u8>>,
    pub cdx_page_props: Vec<(u16, Vec<u8>)>,
    pub cdx_page_children: Vec<Vec<u8>>,
    pub cdx_frags: Vec<(u32, Vec<(u16, Vec<u8>)>, Vec<Vec<u8>>)>,
    pub xml_doc_attrs: Vec<(String, String)>,
    pub xml_doc_children: Vec<String>,
    pub xml_page_attrs: Vec<(String, String)>,
    pub xml_page_children: Vec<String>,
    pub xml_frags: Vec<(u32, Vec<(String, String)>, Vec<String>)>,
    pub max_id: u32,
    #[serde(default)]
    pub page_id: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Bond {
    pub a: usize,
    pub b: usize,
    pub order: u8,
    #[serde(default)]
    pub stereo: Stereo,
    #[serde(default)]
    pub dbl: DblPos,
    #[serde(default)]
    pub color: Option<Rgb>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<Box<Extra>>,
}

impl Bond {
    pub fn other(&self, i: usize) -> usize {
        if self.a == i { self.b } else { self.a }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Arrow {
    pub from: Pos2,
    pub to: Pos2,
    pub kind: ArrowKind,
    #[serde(default)]
    pub color: Option<Rgb>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<Box<Extra>>,
    #[serde(default, skip_serializing_if = "ArrowStyle::is_default")]
    pub style: ArrowStyle,
}

impl Arrow {
    pub fn new(from: Pos2, to: Pos2, kind: ArrowKind, style: ArrowStyle) -> Arrow {
        Arrow { from, to, kind, color: None, extra: None, style }
    }

    /// Points along the arrow's line (its centre line for curved and rotation arrows).
    pub fn path(&self, n: usize) -> Vec<Pos2> {
        match self.kind {
            ArrowKind::Curved if self.style.arc.abs() > 1.0 => arc_points(self.from, self.to, self.style.arc, n),
            ArrowKind::Curved => curved_points(self.from, self.to, n),
            ArrowKind::Rotation => rotation_points(self.from, self.to, self.style.arc < 0.0, n),
            _ => vec![self.from, self.to],
        }
    }
}

/// A circular arc from `from` to `to` turning through `deg` degrees (sign: which side it bulges).
pub fn arc_points(from: Pos2, to: Pos2, deg: f32, n: usize) -> Vec<Pos2> {
    let d = to - from;
    let c = d.length();
    let th = deg.abs().clamp(5.0, 340.0).to_radians();
    if c < 1e-3 {
        return vec![from, to];
    }
    // Bulge on the same side as the default curve for positive angles.
    let side = rot(d, -std::f32::consts::FRAC_PI_2).normalized() * deg.signum();
    let r = c / (2.0 * (th / 2.0).sin());
    let centre = from + d * 0.5 - side * (r * (th / 2.0).cos());
    let v0 = from - centre;
    // Turn whichever way passes through the bulge: halfway round, the arc points straight along `side`.
    let sgn = if rot(v0, th / 2.0).dot(side) >= rot(v0, -th / 2.0).dot(side) { 1.0 } else { -1.0 };
    (0..=n).map(|k| centre + rot(v0, sgn * th * k as f32 / n as f32)).collect()
}

/// A rotation arrow: most of a narrow ellipse whose long axis runs from `from` to `to`.
pub fn rotation_points(from: Pos2, to: Pos2, reverse: bool, n: usize) -> Vec<Pos2> {
    let d = to - from;
    let a = d.length() / 2.0;
    if a < 1e-3 {
        return vec![from, to];
    }
    let u = d / (2.0 * a);
    let w = vec2(-u.y, u.x);
    let b = a * 0.42;
    let c = from + d * 0.5;
    let (t0, t1) = (35f32.to_radians(), 325f32.to_radians());
    (0..=n)
        .map(|k| {
            let mut t = t0 + (t1 - t0) * k as f32 / n as f32;
            if reverse {
                t = -t;
            }
            // Start near the `from` end and go round.
            c - u * (a * t.cos()) + w * (b * t.sin())
        })
        .collect()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Caption {
    pub pos: Pos2,
    pub text: String,
    pub size: f32,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub italic: bool,
    /// Times New Roman instead of Arial.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub serif: bool,
    /// Digits after letters drawn as subscripts.
    #[serde(default)]
    pub formula: bool,
    #[serde(default)]
    pub color: Option<Rgb>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<Box<Extra>>,
    /// A protein, DNA or RNA sequence: `text` is then FASTA and is shown in numbered blocks.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seq: Option<crate::sequence::SeqKind>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Shape {
    pub min: Pos2,
    pub max: Pos2,
    pub kind: ShapeKind,
    #[serde(default)]
    pub color: Option<Rgb>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<Box<Extra>>,
    /// Brackets: index shown at the lower right ("n" for a repeating unit, "2" for a repeat count).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Brackets read from a file as two one-sided brackets: file data of the right-hand one
    /// (the left-hand one is in `extra`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pair: Option<Box<Extra>>,
    /// The file's bracketed-group record for these brackets (which atoms repeat).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<Box<Extra>>,
}

impl Shape {
    pub fn new(min: Pos2, max: Pos2, kind: ShapeKind) -> Self {
        Shape { min, max, kind, color: None, extra: None, label: None, pair: None, group: None }
    }
    pub fn is_bracket(&self) -> bool {
        matches!(self.kind, ShapeKind::Brackets | ShapeKind::Parens)
    }
}

/// Font used for atom labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum LabelFont {
    #[default]
    Sans,
    Serif,
}

/// The printed page, in centimetres: paper size (already turned for landscape) and margins.
/// The drawing area of one page is the paper less the margins.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Page {
    pub paper_w: f32,
    pub paper_h: f32,
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
}

pub const LETTER: (f32, f32) = (21.59, 27.94);
pub const A4: (f32, f32) = (21.0, 29.7);

impl Default for Page {
    fn default() -> Self {
        Page { paper_w: LETTER.0, paper_h: LETTER.1, left: 1.27, right: 1.27, top: 1.27, bottom: 1.27 }
    }
}

impl Page {
    /// Drawing area of one page (cm), never smaller than 1 cm.
    pub fn area_cm(&self) -> (f32, f32) {
        ((self.paper_w - self.left - self.right).max(1.0), (self.paper_h - self.top - self.bottom).max(1.0))
    }
    pub fn landscape(&self) -> bool {
        self.paper_w > self.paper_h
    }
    /// "Letter", "A4" or "Custom", whichever way round.
    pub fn paper_name(&self) -> &'static str {
        let (a, b) = (self.paper_w.min(self.paper_h), self.paper_w.max(self.paper_h));
        let near = |s: (f32, f32)| (a - s.0).abs() < 0.05 && (b - s.1).abs() < 0.05;
        if near(LETTER) {
            "Letter"
        } else if near(A4) {
            "A4"
        } else {
            "Custom"
        }
    }
}

/// How a drawing looks. Sizes are in drawing units, where a standard bond is
/// `BOND_LEN` long; `bond_length` says how long that bond is on paper (points).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Style {
    pub line_width: f32,
    pub bold_width: f32,
    pub dbl_spacing: f32, // fraction of bond length
    pub label_size: f32,
    pub caption_size: f32,
    pub show_carbons: bool,
    pub show_warnings: bool,
    /// Name of the settings set this style came from.
    pub name: String,
    /// Printed bond length in points.
    pub bond_length: f32,
    pub hash_spacing: f32,
    /// Gap between a bond end and an atom label.
    pub margin: f32,
    /// Width of the wide end of a wedge.
    pub wedge_width: f32,
    pub label_font: LabelFont,
    pub label_bold: bool,
    pub show_terminal_carbons: bool,
    /// Light drawing on a dark background.
    pub dark: bool,
    /// Font of new captions.
    pub caption_font: LabelFont,
    /// Atom labels without their implicit hydrogens ("N" rather than "NH").
    pub hide_implicit_h: bool,
    pub page: Page,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            line_width: 1.0,
            bold_width: 4.0,
            dbl_spacing: 0.18,
            label_size: 10.0,
            caption_size: 12.0,
            show_carbons: false,
            show_warnings: true,
            name: "New Document".into(),
            bond_length: BOND_LEN,
            hash_spacing: 2.7,
            margin: 2.0,
            wedge_width: 6.0,
            label_font: LabelFont::Sans,
            label_bold: false,
            show_terminal_carbons: false,
            dark: false,
            caption_font: LabelFont::Sans,
            hide_implicit_h: false,
            page: Page::default(),
        }
    }
}

impl Style {
    /// Drawing colour for lines and text without their own colour.
    pub fn ink(&self) -> egui::Color32 {
        if self.dark { egui::Color32::from_rgb(235, 235, 235) } else { egui::Color32::BLACK }
    }
    pub fn paper(&self) -> egui::Color32 {
        if self.dark { egui::Color32::from_rgb(30, 30, 34) } else { egui::Color32::WHITE }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct Doc {
    pub atoms: Vec<Atom>,
    pub bonds: Vec<Bond>,
    #[serde(default)]
    pub arrows: Vec<Arrow>,
    #[serde(default)]
    pub captions: Vec<Caption>,
    #[serde(default)]
    pub shapes: Vec<Shape>,
    #[serde(default)]
    pub style: Style,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<Box<FileExtra>>,
}

/// What is selected (bonds are selected when both ends are).
#[derive(Clone, Debug, Default)]
pub struct Selection {
    pub atoms: BTreeSet<usize>,
    pub arrows: BTreeSet<usize>,
    pub captions: BTreeSet<usize>,
    pub shapes: BTreeSet<usize>,
}

impl Selection {
    pub fn is_empty(&self) -> bool {
        self.atoms.is_empty() && self.arrows.is_empty() && self.captions.is_empty() && self.shapes.is_empty()
    }
    pub fn clear(&mut self) {
        *self = Selection::default();
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    Atom(usize),
    Bond(usize),
    Arrow(usize),
    Caption(usize),
    Shape(usize),
}

pub fn rot(v: Vec2, ang: f32) -> Vec2 {
    let (s, c) = ang.sin_cos();
    vec2(v.x * c - v.y * s, v.x * s + v.y * c)
}

pub fn dist_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let l2 = ab.length_sq();
    if l2 < 1e-6 {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

pub fn angle_of(v: Vec2) -> f32 {
    v.y.atan2(v.x)
}

pub fn norm_angle(mut a: f32) -> f32 {
    use std::f32::consts::TAU;
    while a < 0.0 {
        a += TAU;
    }
    while a >= TAU {
        a -= TAU;
    }
    a
}

impl Doc {
    pub fn is_empty(&self) -> bool {
        self.atoms.is_empty() && self.arrows.is_empty() && self.captions.is_empty() && self.shapes.is_empty()
    }

    pub fn neighbors(&self, i: usize) -> Vec<usize> {
        self.bonds.iter().filter(|b| b.a == i || b.b == i).map(|b| b.other(i)).collect()
    }

    pub fn adjacency(&self) -> Vec<Vec<(usize, usize)>> {
        let mut adj = vec![Vec::new(); self.atoms.len()];
        for (bi, b) in self.bonds.iter().enumerate() {
            adj[b.a].push((b.b, bi));
            adj[b.b].push((b.a, bi));
        }
        adj
    }

    pub fn find_bond(&self, a: usize, b: usize) -> Option<usize> {
        self.bonds.iter().position(|x| (x.a == a && x.b == b) || (x.a == b && x.b == a))
    }

    pub fn bond_order_sum(&self, i: usize) -> i32 {
        self.bonds.iter().filter(|b| b.a == i || b.b == i).map(|b| b.order as i32).sum()
    }

    pub fn degree(&self, i: usize) -> usize {
        self.bonds.iter().filter(|b| b.a == i || b.b == i).count()
    }

    /// Implicit hydrogens on an atom (0 for group labels and metals).
    pub fn implicit_h(&self, i: usize) -> i32 {
        let a = &self.atoms[i];
        if a.label.is_some() || a.el == 0 {
            return 0;
        }
        let used = self.bond_order_sum(i) + a.radical as i32;
        let vals = crate::elements::charged_valences(a.el, a.charge as i32);
        for v in vals {
            if v >= used {
                return v - used;
            }
        }
        0
    }

    /// True when an atom has more bonds than any normal valence allows.
    pub fn valence_error(&self, i: usize) -> bool {
        let a = &self.atoms[i];
        if a.label.is_some() || a.el == 0 {
            return false;
        }
        let vals = crate::elements::charged_valences(a.el, a.charge as i32);
        if vals.is_empty() {
            return false;
        }
        let used = self.bond_order_sum(i) + a.radical as i32;
        used > *vals.iter().max().unwrap()
    }

    pub fn atom_at(&self, p: Pos2, tol: f32, skip: Option<usize>) -> Option<usize> {
        let mut best = None;
        let mut bd = tol;
        for (i, a) in self.atoms.iter().enumerate() {
            if Some(i) == skip {
                continue;
            }
            let d = (a.pos - p).length();
            if d < bd {
                bd = d;
                best = Some(i);
            }
        }
        best
    }

    pub fn hit(&self, p: Pos2, tol: f32) -> Option<Hit> {
        if let Some(i) = self.atom_at(p, tol * 1.4, None) {
            return Some(Hit::Atom(i));
        }
        let mut best = None;
        let mut bd = tol;
        for (i, b) in self.bonds.iter().enumerate() {
            let d = dist_to_segment(p, self.atoms[b.a].pos, self.atoms[b.b].pos);
            if d < bd {
                bd = d;
                best = Some(Hit::Bond(i));
            }
        }
        if best.is_some() {
            return best;
        }
        for (i, c) in self.captions.iter().enumerate() {
            if caption_rect(c).expand(3.0).contains(p) {
                return Some(Hit::Caption(i));
            }
        }
        for (i, a) in self.arrows.iter().enumerate() {
            let d = a.path(32).windows(2).map(|w| dist_to_segment(p, w[0], w[1])).fold(f32::MAX, f32::min);
            if d < tol {
                return Some(Hit::Arrow(i));
            }
        }
        for (i, s) in self.shapes.iter().enumerate() {
            let r = egui::Rect::from_two_pos(s.min, s.max);
            if r.expand(tol).contains(p) && !r.shrink(tol).contains(p) {
                return Some(Hit::Shape(i));
            }
        }
        None
    }

    pub fn add_atom(&mut self, pos: Pos2, el: u8) -> usize {
        self.atoms.push(Atom::new(pos, el));
        self.atoms.len() - 1
    }

    /// Reuse an atom already at `pos`, otherwise add a carbon there.
    pub fn atom_or_new(&mut self, pos: Pos2) -> usize {
        if let Some(i) = self.atom_at(pos, BOND_LEN * 0.2, None) {
            i
        } else {
            self.add_atom(pos, crate::elements::C)
        }
    }

    pub fn add_bond(&mut self, a: usize, b: usize, order: u8, stereo: Stereo) -> Option<usize> {
        if a == b {
            return None;
        }
        if let Some(i) = self.find_bond(a, b) {
            return Some(i);
        }
        self.bonds.push(Bond { a, b, order, stereo, dbl: DblPos::Auto, color: None, extra: None });
        Some(self.bonds.len() - 1)
    }

    /// Remove atoms (and their bonds), keeping indices of the rest consistent.
    pub fn remove_atoms(&mut self, set: &HashSet<usize>) {
        if set.is_empty() {
            return;
        }
        let mut map = vec![usize::MAX; self.atoms.len()];
        let mut new_atoms = Vec::new();
        for (i, a) in self.atoms.iter().enumerate() {
            if !set.contains(&i) {
                map[i] = new_atoms.len();
                new_atoms.push(a.clone());
            }
        }
        self.atoms = new_atoms;
        self.bonds.retain(|b| map[b.a] != usize::MAX && map[b.b] != usize::MAX);
        for b in &mut self.bonds {
            b.a = map[b.a];
            b.b = map[b.b];
        }
    }

    pub fn remove_bond(&mut self, bi: usize) {
        let b = self.bonds.remove(bi);
        // Remove atoms left alone, unless they carry a label.
        let mut lonely = HashSet::new();
        for i in [b.a, b.b] {
            if self.degree(i) == 0 && self.atoms[i].el == crate::elements::C && self.atoms[i].label.is_none() {
                lonely.insert(i);
            }
        }
        self.remove_atoms(&lonely);
    }

    pub fn delete_selection(&mut self, sel: &Selection) {
        let set: HashSet<usize> = sel.atoms.iter().copied().collect();
        self.remove_atoms(&set);
        for i in sel.arrows.iter().rev() {
            self.arrows.remove(*i);
        }
        for i in sel.captions.iter().rev() {
            self.captions.remove(*i);
        }
        for i in sel.shapes.iter().rev() {
            self.shapes.remove(*i);
        }
    }

    /// Connected pieces of the structure, as lists of atom indices.
    pub fn fragments(&self) -> Vec<Vec<usize>> {
        let adj = self.adjacency();
        let mut seen = vec![false; self.atoms.len()];
        let mut out = Vec::new();
        for s in 0..self.atoms.len() {
            if seen[s] {
                continue;
            }
            let mut comp = vec![];
            let mut stack = vec![s];
            seen[s] = true;
            while let Some(u) = stack.pop() {
                comp.push(u);
                for &(v, _) in &adj[u] {
                    if !seen[v] {
                        seen[v] = true;
                        stack.push(v);
                    }
                }
            }
            comp.sort();
            out.push(comp);
        }
        out
    }

    pub fn fragment_of(&self, i: usize) -> Vec<usize> {
        self.fragments().into_iter().find(|f| f.contains(&i)).unwrap_or_default()
    }

    /// Copy of the selected part as a separate drawing.
    pub fn extract(&self, sel: &Selection) -> Doc {
        let mut d = Doc { style: self.style.clone(), ..Default::default() };
        let mut map = std::collections::HashMap::new();
        for &i in &sel.atoms {
            map.insert(i, d.atoms.len());
            d.atoms.push(self.atoms[i].clone());
        }
        for b in &self.bonds {
            if let (Some(&a), Some(&bb)) = (map.get(&b.a), map.get(&b.b)) {
                let mut nb = b.clone();
                nb.a = a;
                nb.b = bb;
                d.bonds.push(nb);
            }
        }
        for &i in &sel.arrows {
            d.arrows.push(self.arrows[i].clone());
        }
        for &i in &sel.captions {
            d.captions.push(self.captions[i].clone());
        }
        for &i in &sel.shapes {
            d.shapes.push(self.shapes[i].clone());
        }
        d
    }

    /// Add another drawing into this one; returns a selection of what was added.
    pub fn merge(&mut self, other: &Doc, offset: Vec2) -> Selection {
        let mut sel = Selection::default();
        let base = self.atoms.len();
        for a in &other.atoms {
            let mut a = a.clone();
            a.extra = None;
            a.pos += offset;
            sel.atoms.insert(self.atoms.len());
            self.atoms.push(a);
        }
        for b in &other.bonds {
            let mut b = b.clone();
            b.extra = None;
            b.a += base;
            b.b += base;
            self.bonds.push(b);
        }
        for a in &other.arrows {
            let mut a = a.clone();
            a.extra = None;
            a.from += offset;
            a.to += offset;
            sel.arrows.insert(self.arrows.len());
            self.arrows.push(a);
        }
        for c in &other.captions {
            let mut c = c.clone();
            c.extra = None;
            c.pos += offset;
            sel.captions.insert(self.captions.len());
            self.captions.push(c);
        }
        for s in &other.shapes {
            let mut s = s.clone();
            s.extra = None;
            s.min += offset;
            s.max += offset;
            sel.shapes.insert(self.shapes.len());
            self.shapes.push(s);
        }
        sel
    }

    pub fn bounds_of(&self, sel: Option<&Selection>) -> Option<egui::Rect> {
        let mut r = egui::Rect::NOTHING;
        let inc_atom = |i: usize| sel.map(|s| s.atoms.contains(&i)).unwrap_or(true);
        for (i, a) in self.atoms.iter().enumerate() {
            if inc_atom(i) {
                r.extend_with(a.pos);
            }
        }
        for (i, a) in self.arrows.iter().enumerate() {
            if sel.map(|s| s.arrows.contains(&i)).unwrap_or(true) {
                r.extend_with(a.from);
                r.extend_with(a.to);
                for p in a.path(16) {
                    r.extend_with(p);
                }
            }
        }
        for (i, c) in self.captions.iter().enumerate() {
            if sel.map(|s| s.captions.contains(&i)).unwrap_or(true) {
                let cr = caption_rect(c);
                r.extend_with(cr.min);
                r.extend_with(cr.max);
            }
        }
        for (i, s) in self.shapes.iter().enumerate() {
            if sel.map(|x| x.shapes.contains(&i)).unwrap_or(true) {
                r.extend_with(s.min);
                r.extend_with(s.max);
            }
        }
        if r.is_positive() || (r.min.x <= r.max.x && r.min.y <= r.max.y) { Some(r) } else { None }
    }

    pub fn translate(&mut self, sel: &Selection, d: Vec2) {
        for &i in &sel.atoms {
            self.atoms[i].pos += d;
        }
        for &i in &sel.arrows {
            self.arrows[i].from += d;
            self.arrows[i].to += d;
        }
        for &i in &sel.captions {
            self.captions[i].pos += d;
        }
        for &i in &sel.shapes {
            self.shapes[i].min += d;
            self.shapes[i].max += d;
        }
    }

    /// Apply a point transform to the selection.
    pub fn transform(&mut self, sel: &Selection, f: impl Fn(Pos2) -> Pos2) {
        for &i in &sel.atoms {
            self.atoms[i].pos = f(self.atoms[i].pos);
        }
        // A mirror image turns curves and one-sided heads the other way.
        let (o, ex, ey) = (f(Pos2::ZERO), f(pos2(1.0, 0.0)), f(pos2(0.0, 1.0)));
        let mirrored = (ex - o).x * (ey - o).y - (ex - o).y * (ey - o).x < 0.0;
        for &i in &sel.arrows {
            let a = &mut self.arrows[i];
            a.from = f(a.from);
            a.to = f(a.to);
            if mirrored {
                a.style.arc = -a.style.arc;
                a.style.half = match a.style.half {
                    Half::Left => Half::Right,
                    Half::Right => Half::Left,
                    h => h,
                };
            }
        }
        for &i in &sel.captions {
            let c = &mut self.captions[i];
            let r = caption_rect(c);
            let nc = f(r.center());
            c.pos += nc - r.center();
        }
        for &i in &sel.shapes {
            let a = f(self.shapes[i].min);
            let b = f(self.shapes[i].max);
            self.shapes[i].min = pos2(a.x.min(b.x), a.y.min(b.y));
            self.shapes[i].max = pos2(a.x.max(b.x), a.y.max(b.y));
        }
    }

    pub fn select_all(&self) -> Selection {
        Selection {
            atoms: (0..self.atoms.len()).collect(),
            arrows: (0..self.arrows.len()).collect(),
            captions: (0..self.captions.len()).collect(),
            shapes: (0..self.shapes.len()).collect(),
        }
    }

    /// Best direction for a new bond leaving atom `i`.
    pub fn free_direction(&self, i: usize) -> Vec2 {
        let p = self.atoms[i].pos;
        let nbrs = self.neighbors(i);
        let dirs: Vec<Vec2> = nbrs.iter().map(|&n| (self.atoms[n].pos - p).normalized()).collect();
        let deg30 = 30f32.to_radians();
        match dirs.len() {
            0 => vec2(deg30.cos(), -deg30.sin()),
            1 => {
                let d = dirs[0];
                let n = nbrs[0];
                // Straight line after a triple bond or a cumulated double.
                if let Some(bi) = self.find_bond(i, n) {
                    let o = self.bonds[bi].order;
                    if o == 3 || (o == 2 && self.bonds.iter().any(|b| (b.a == n || b.b == n) && b.order == 2 && b.other(n) != i)) {
                        return -d;
                    }
                }
                let c1 = rot(d, 120f32.to_radians());
                let c2 = rot(d, -120f32.to_radians());
                // Zig-zag: go away from the neighbour's other neighbours.
                let others: Vec<Pos2> = self.neighbors(n).into_iter().filter(|&x| x != i).map(|x| self.atoms[x].pos).collect();
                if others.is_empty() {
                    if c1.x > c2.x || (c1.x == c2.x && c1.y < c2.y) { c1 } else { c2 }
                } else {
                    let score = |c: Vec2| {
                        let q = p + c * BOND_LEN;
                        others.iter().map(|o| (*o - q).length()).sum::<f32>()
                    };
                    if score(c1) >= score(c2) { c1 } else { c2 }
                }
            }
            _ => {
                let mut angs: Vec<f32> = dirs.iter().map(|d| norm_angle(angle_of(*d))).collect();
                angs.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let mut best_gap = -1.0;
                let mut best_mid = 0.0;
                for k in 0..angs.len() {
                    let a = angs[k];
                    let b = if k + 1 < angs.len() { angs[k + 1] } else { angs[0] + std::f32::consts::TAU };
                    if b - a > best_gap {
                        best_gap = b - a;
                        best_mid = (a + b) / 2.0;
                    }
                }
                vec2(best_mid.cos(), best_mid.sin())
            }
        }
    }

    /// Add a ring of `n` atoms (aromatic for benzene-like templates) attached by a click.
    pub fn add_ring(&mut self, n: usize, aromatic: bool, at: RingAnchor) -> Vec<usize> {
        use std::f32::consts::PI;
        let l = BOND_LEN;
        let r = l / (2.0 * (PI / n as f32).sin());
        let apothem = l / (2.0 * (PI / n as f32).tan());
        let step = 2.0 * PI / n as f32;
        let mut pts = Vec::with_capacity(n);
        match at {
            RingAnchor::Point(c) => {
                let start = -PI / 2.0 + if n % 2 == 1 { 0.0 } else { 0.0 };
                for k in 0..n {
                    let a = start + step * k as f32;
                    pts.push(c + vec2(a.cos(), a.sin()) * r);
                }
                // Even rings: flat sides left/right (pointy top) like the usual hexagon.
            }
            RingAnchor::Atom(i) => {
                let p = self.atoms[i].pos;
                let nbrs = self.neighbors(i);
                let dir = match nbrs.len() {
                    0 => vec2(0.0, -1.0),
                    // At the end of a bond the ring continues straight on, so the line
                    // through the bond cuts the ring into two mirror halves.
                    1 => (p - self.atoms[nbrs[0]].pos).normalized(),
                    _ => self.free_direction(i),
                };
                let c = p + dir * r;
                let a0 = angle_of(p - c);
                for k in 0..n {
                    let a = a0 + step * k as f32;
                    pts.push(c + vec2(a.cos(), a.sin()) * r);
                }
                pts[0] = p;
            }
            RingAnchor::Bond(bi) => {
                let (ia, ib) = (self.bonds[bi].a, self.bonds[bi].b);
                let pa = self.atoms[ia].pos;
                let pb = self.atoms[ib].pos;
                let mid = pa + (pb - pa) * 0.5;
                let mut nrm = rot((pb - pa).normalized(), PI / 2.0);
                // Put the ring on the side with fewer atoms.
                let mut side = 0.0;
                for k in self.neighbors(ia).into_iter().chain(self.neighbors(ib)) {
                    if k != ia && k != ib {
                        side += (self.atoms[k].pos - mid).dot(nrm);
                    }
                }
                if side > 0.0 {
                    nrm = -nrm;
                }
                let c = mid + nrm * apothem;
                let a0 = angle_of(pa - c);
                let a1 = angle_of(pb - c);
                let mut dirn = norm_angle(a1 - a0);
                dirn = if dirn < PI { 1.0 } else { -1.0 };
                for k in 0..n {
                    let a = a0 + dirn * step * k as f32;
                    pts.push(c + vec2(a.cos(), a.sin()) * r);
                }
                pts[0] = pa;
                pts[1] = pb;
            }
        }
        let idx: Vec<usize> = pts.iter().map(|p| self.atom_or_new(*p)).collect();
        let mut ring_bonds = Vec::new();
        for k in 0..n {
            let a = idx[k];
            let b = idx[(k + 1) % n];
            if let Some(bi) = self.add_bond(a, b, 1, Stereo::None) {
                ring_bonds.push(bi);
            }
        }
        if aromatic && ring_bonds.len() == n {
            let has_double = |d: &Doc, i: usize, except: usize| {
                d.bonds.iter().enumerate().any(|(k, b)| k != except && (b.a == i || b.b == i) && b.order >= 2)
            };
            // Try both alternations and use the one that fits existing double bonds best.
            let mut best = (0usize, i32::MIN);
            for start in 0..2 {
                let mut score = 0;
                for k in (start..n).step_by(2) {
                    let bi = ring_bonds[k];
                    let b = &self.bonds[bi];
                    if b.order == 2 {
                        score += 2;
                    } else if has_double(self, b.a, bi) || has_double(self, b.b, bi) {
                        score -= 3;
                    }
                }
                if score > best.1 {
                    best = (start, score);
                }
            }
            for k in (best.0..n).step_by(2) {
                if n % 2 == 1 && k == n - 1 {
                    break;
                }
                let bi = ring_bonds[k];
                let (a, b) = (self.bonds[bi].a, self.bonds[bi].b);
                if self.bonds[bi].order == 1 && !has_double(self, a, bi) && !has_double(self, b, bi) {
                    self.bonds[bi].order = 2;
                }
            }
        }
        idx
    }

    /// Zig-zag chain points from `start` towards `toward`.
    pub fn chain_points(start: Pos2, toward: Pos2) -> Vec<Pos2> {
        let d = toward - start;
        let len = d.length();
        let step = BOND_LEN * (30f32.to_radians()).cos();
        let n = ((len / step).round() as usize).max(1);
        let dir = if len > 1e-3 { d / len } else { vec2(1.0, 0.0) };
        let mut perp = rot(dir, -std::f32::consts::FRAC_PI_2);
        if perp.y > 0.0 {
            perp = -perp;
        }
        let mut pts = vec![start];
        for k in 1..=n {
            let off = if k % 2 == 1 { perp * (BOND_LEN * 0.5) } else { Vec2::ZERO };
            pts.push(start + dir * (step * k as f32) + off);
        }
        pts
    }
}

#[derive(Clone, Copy, Debug)]
pub enum RingAnchor {
    Point(Pos2),
    Atom(usize),
    Bond(usize),
}

/// Rough box of a caption (exact size comes from the renderer, this is for hit tests).
pub fn caption_rect(c: &Caption) -> egui::Rect {
    if c.seq.is_some() {
        let lines = crate::sequence::shown_lines(&c.text);
        let w = lines.iter().map(|l| l.number.chars().count() + l.text.chars().count()).max().unwrap_or(1).max(1) as f32 * c.size * 0.6;
        let h = lines.len().max(1) as f32 * c.size * 1.25;
        return egui::Rect::from_min_size(c.pos, vec2(w, h));
    }
    let lines: Vec<&str> = c.text.split('\n').collect();
    let w = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1).max(1) as f32 * c.size * 0.55;
    let h = lines.len().max(1) as f32 * c.size * 1.2;
    egui::Rect::from_min_size(c.pos, vec2(w, h))
}

pub fn curved_points(from: Pos2, to: Pos2, n: usize) -> Vec<Pos2> {
    let d = to - from;
    let ctrl = from + d * 0.5 + rot(d, -std::f32::consts::FRAC_PI_2) * 0.45;
    (0..=n)
        .map(|k| {
            let t = k as f32 / n as f32;
            let a = from.to_vec2() * (1.0 - t) * (1.0 - t) + ctrl.to_vec2() * 2.0 * (1.0 - t) * t + to.to_vec2() * t * t;
            a.to_pos2()
        })
        .collect()
}
