//! Monosaccharides in every common drawing: Fischer projection (open chain), Haworth
//! projection, and ring conformers — chair, boat, skew-boat, half-chair and envelope for
//! pyranoses, envelope and twist for furanoses.
//!
//! A sugar is described by its Fischer projection: for each stereocentre, top to bottom, "R"
//! when the OH points right and "L" when it points left ("D" marks a deoxy CH2). Rings, α/β
//! and the conformers follow from that. Conformer rings are built in 3D with Cremer–Pople
//! puckering, the ring substituents placed tetrahedrally (so axial and equatorial come out on
//! their own), and the result is projected onto the page.

use crate::chem;
use crate::doc::*;
use egui::{Pos2, pos2};

#[derive(Clone, Copy)]
pub struct Sugar {
    pub name: &'static str,
    /// A ketose (C=O at C2) rather than an aldose (CHO at C1).
    pub keto: bool,
    /// The D form's stereocentres, top to bottom.
    pub config: &'static str,
    /// The last carbon: "CH2OH", or "CH3" for 6-deoxy sugars.
    pub bottom: &'static str,
    /// The form found in nature is L (as for fucose and rhamnose).
    pub natural_l: bool,
}

const fn s(name: &'static str, keto: bool, config: &'static str) -> Sugar {
    Sugar { name, keto, config, bottom: "CH2OH", natural_l: false }
}

pub const HEXOSES: &[Sugar] = &[
    s("Glucose", false, "RLRR"),
    s("Galactose", false, "RLLR"),
    s("Mannose", false, "LLRR"),
    s("Allose", false, "RRRR"),
    s("Altrose", false, "LRRR"),
    s("Gulose", false, "RRLR"),
    s("Idose", false, "LRLR"),
    s("Talose", false, "LLLR"),
    s("Fructose", true, "LRR"),
    s("Psicose", true, "RRR"),
    s("Sorbose", true, "RLR"),
    s("Tagatose", true, "LLR"),
    Sugar { name: "Fucose (6-deoxygalactose)", keto: false, config: "RLLR", bottom: "CH3", natural_l: true },
    Sugar { name: "Rhamnose (6-deoxymannose)", keto: false, config: "LLRR", bottom: "CH3", natural_l: true },
];

pub const PENTOSES: &[Sugar] = &[
    s("Ribose", false, "RRR"),
    s("2-Deoxyribose", false, "DRR"),
    s("Arabinose", false, "LRR"),
    s("Xylose", false, "RLR"),
    s("Lyxose", false, "LLR"),
    s("Ribulose", true, "RR"),
    s("Xylulose", true, "LR"),
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Ring {
    Pyranose,
    Furanose,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Drawing {
    Fischer,
    Haworth,
    Chair,
    Boat,
    SkewBoat,
    HalfChair,
    Envelope,
    Twist,
}

impl Drawing {
    pub fn name(self) -> &'static str {
        match self {
            Drawing::Fischer => "Fischer projection",
            Drawing::Haworth => "Haworth projection",
            Drawing::Chair => "Chair",
            Drawing::Boat => "Boat",
            Drawing::SkewBoat => "Skew-boat",
            Drawing::HalfChair => "Half-chair",
            Drawing::Envelope => "Envelope",
            Drawing::Twist => "Twist",
        }
    }
    /// The drawings that make sense for a ring size (None = open chain).
    pub fn for_ring(r: Option<Ring>) -> &'static [Drawing] {
        match r {
            None => &[Drawing::Fischer],
            Some(Ring::Pyranose) => &[Drawing::Haworth, Drawing::Chair, Drawing::Boat, Drawing::SkewBoat, Drawing::HalfChair, Drawing::Envelope],
            Some(Ring::Furanose) => &[Drawing::Haworth, Drawing::Envelope, Drawing::Twist],
        }
    }
}

/// What to draw: the sugar, D or L, open chain or ring, α or β, and the drawing style.
#[derive(Clone, Copy)]
pub struct Choice {
    pub sugar: Sugar,
    pub l_form: bool,
    pub ring: Option<Ring>,
    pub beta: bool,
    pub drawing: Drawing,
}

impl Choice {
    pub fn title(&self) -> String {
        let dl = if self.l_form { "L" } else { "D" };
        let base = self.sugar.name.split(' ').next().unwrap_or(self.sugar.name);
        // "2-Deoxyribose" is named 2-deoxy-D-ribose, 2-deoxy-β-D-ribofuranose.
        let (pre, core) = match base.strip_prefix("2-Deoxy") {
            Some(rest) => ("2-Deoxy-", rest.to_lowercase()),
            None => ("", base.to_string()),
        };
        match self.ring {
            None => format!("{}{}-{}", pre, dl, core),
            Some(r) => {
                let stem = core.trim_end_matches("ose").to_lowercase();
                let ring = if r == Ring::Pyranose { "pyranose" } else { "furanose" };
                let anomer = if self.beta { "β" } else { "α" };
                if pre.is_empty() {
                    let mut cap = stem.clone();
                    if let Some(f) = cap.get_mut(0..1) {
                        f.make_ascii_uppercase();
                    }
                    format!("{}-{}-{}o{}", anomer, dl, cap, ring)
                } else {
                    format!("{}{}-{}-{}o{}", pre, anomer, dl, stem, ring)
                }
            }
        }
    }
}

fn config(c: &Choice) -> String {
    if c.l_form {
        c.sugar.config.chars().map(|x| match x {
            'R' => 'L',
            'L' => 'R',
            o => o,
        }).collect()
    } else {
        c.sugar.config.to_string()
    }
}

/// Number of carbons.
fn carbons(sg: &Sugar) -> usize {
    sg.config.len() + if sg.keto { 3 } else { 2 }
}

/// The ring can form: the closing carbon exists and still has its OH.
pub fn ring_possible(sg: &Sugar, r: Ring) -> bool {
    let anomeric = if sg.keto { 2 } else { 1 };
    let closing = anomeric + if r == Ring::Pyranose { 4 } else { 3 };
    let n = carbons(sg);
    closing < n || (closing == n && sg.bottom.contains("OH"))
}

const L: f32 = BOND_LEN;

fn label_atom(d: &mut Doc, p: Pos2, text: &str) -> usize {
    match text {
        "OH" => d.add_atom(p, 8),
        "H" => d.add_atom(p, 1),
        t => {
            let i = d.add_atom(p, chem::label_main_element(t));
            d.atoms[i].label = Some(t.to_string());
            // "CH3" stays a carbon whose hydrogens follow its bonds.
            d.atoms[i].simplify_label();
            i
        }
    }
}

fn fischer(c: &Choice) -> Doc {
    let cfg = config(c);
    let mut d = Doc::default();
    let top = if c.sugar.keto { "CH2OH" } else { "CHO" };
    let mut prev = label_atom(&mut d, pos2(0.0, 0.0), top);
    let mut y = 0.0;
    let centres: String = if c.sugar.keto { format!("K{}", cfg) } else { cfg };
    for ch in centres.chars() {
        y += L;
        let ci = d.add_atom(pos2(0.0, y), 6);
        d.add_bond(prev, ci, 1, Stereo::None);
        if ch == 'K' {
            let o = d.add_atom(pos2(L, y), 8);
            d.add_bond(ci, o, 2, Stereo::None);
        } else {
            let (left, right) = match ch {
                'R' => ("H", "OH"),
                'L' => ("OH", "H"),
                _ => ("H", "H"),
            };
            for (x, t) in [(-L, left), (L, right)] {
                let a = label_atom(&mut d, pos2(x, y), t);
                d.add_bond(ci, a, 1, Stereo::None);
            }
        }
        prev = ci;
    }
    let b = label_atom(&mut d, pos2(0.0, y + L), c.sugar.bottom);
    d.add_bond(prev, b, 1, Stereo::None);
    d
}

/// The ring carbons from the anomeric one round to the ring-closing one, each with its
/// (upper-face, lower-face) groups in the standard orientation (ring O at the back right).
pub fn ring_groups(c: &Choice) -> Option<Vec<(String, String)>> {
    let r = c.ring?;
    if !ring_possible(&c.sugar, r) {
        return None;
    }
    let cfg: Vec<char> = config(c).chars().collect();
    let n = carbons(&c.sugar);
    let anomeric = if c.sugar.keto { 2 } else { 1 };
    let closing = anomeric + if r == Ring::Pyranose { 4 } else { 3 };
    // Fischer side of carbon i (1-based): config starts at C2 (aldose) or C3 (ketose).
    let first = anomeric + 1;
    let side = |i: usize| cfg.get(i - first).copied().unwrap_or('D');
    // D or L is read from the last stereocentre.
    let d_series = cfg.iter().rev().find(|x| **x == 'R' || **x == 'L').copied().unwrap_or('R') == 'R';
    let mut out = Vec::new();
    // The anomeric carbon: β puts the OH on the same face as the tail of a D sugar (up).
    let other = if c.sugar.keto { "CH2OH" } else { "H" };
    let oh_up = c.beta == d_series;
    out.push(if oh_up { ("OH".to_string(), other.to_string()) } else { (other.to_string(), "OH".to_string()) });
    for i in anomeric + 1..=closing {
        let pair = if i == closing {
            let tail = n - i;
            let tail_label = match tail {
                0 => None,
                1 => Some(c.sugar.bottom.to_string()),
                2 => Some(if c.sugar.bottom == "CH3" { "CH(OH)CH3".to_string() } else { "CH(OH)CH2OH".to_string() }),
                _ => Some("C3H6O3".to_string()),
            };
            match tail_label {
                None => ("H".to_string(), "H".to_string()),
                // The O that closes the ring was on the right for D: the tail then points up.
                Some(t) => if side(i) == 'R' { (t, "H".to_string()) } else { ("H".to_string(), t) },
            }
        } else {
            match side(i) {
                'R' => ("H".to_string(), "OH".to_string()),
                'L' => ("OH".to_string(), "H".to_string()),
                _ => ("H".to_string(), "H".to_string()),
            }
        };
        out.push(pair);
    }
    Some(out)
}

/// The edge nearest the viewer, as in the usual hand-drawn style: a wedge widening from
/// `a` to `b`, a bold bond `b`–`c`, and a wedge narrowing from `c` to `e`.
fn front_edge(d: &mut Doc, a: usize, b: usize, c: usize, e: usize) {
    for (x, y, s) in [(a, b, Stereo::Wedge), (b, c, Stereo::Bold), (e, c, Stereo::Wedge)] {
        match d.find_bond(x, y) {
            Some(bi) => {
                d.bonds[bi].a = x;
                d.bonds[bi].b = y;
                d.bonds[bi].stereo = s;
            }
            None => {
                d.add_bond(x, y, 1, s);
            }
        }
    }
}

/// A group on a ring atom, drawn skeletally: OH as an O, CH2OH as a zig-zag C–O, CH3 as a
/// bare line end, a two-carbon tail as a zig-zag with its OH. `dir` points from the ring
/// atom to the group. Returns false for a hydrogen that was left implied.
fn add_group(d: &mut Doc, ring_atom: usize, dir: egui::Vec2, text: &str, show_h: bool, next: Option<egui::Vec2>) -> bool {
    let p0 = d.atoms[ring_atom].pos;
    let dir = dir.normalized();
    let p1 = p0 + dir * L;
    // The next bond of a zig-zag turns 60° whichever way stays clear of what's drawn.
    let turn = |d: &Doc, from: Pos2, along: egui::Vec2| -> egui::Vec2 {
        if let Some(n) = next {
            return n.normalized();
        }
        let cands = [rot(along, 60f32.to_radians()), rot(along, -60f32.to_radians())];
        let clear = |v: egui::Vec2| d.atoms.iter().map(|a| (a.pos - (from + v * L)).length()).fold(f32::MAX, f32::min);
        if clear(cands[0]) >= clear(cands[1]) { cands[0] } else { cands[1] }
    };
    match text {
        "H" if !show_h => false,
        "OH" | "H" => {
            let a = label_atom(d, p1, text);
            d.add_bond(ring_atom, a, 1, Stereo::None);
            true
        }
        "CH3" => {
            let c = d.add_atom(p1, 6);
            d.add_bond(ring_atom, c, 1, Stereo::None);
            true
        }
        "CH2OH" => {
            let c = d.add_atom(p1, 6);
            d.add_bond(ring_atom, c, 1, Stereo::None);
            let v = turn(d, p1, dir);
            let o = d.add_atom(p1 + v * L, 8);
            d.add_bond(c, o, 1, Stereo::None);
            true
        }
        "CH(OH)CH2OH" | "CH(OH)CH3" => {
            let c5 = d.add_atom(p1, 6);
            d.add_bond(ring_atom, c5, 1, Stereo::None);
            let v = turn(d, p1, dir);
            let c6 = d.add_atom(p1 + v * L, 6);
            d.add_bond(c5, c6, 1, Stereo::None);
            // The OH sits in the gap between the ring bond and the chain.
            let w = (dir - v).normalized();
            let o5 = d.add_atom(p1 + w * L, 8);
            d.add_bond(c5, o5, 1, Stereo::None);
            if text == "CH(OH)CH2OH" {
                let u = rot(v, if (rot(v, 60f32.to_radians()) - dir).length() > (rot(v, -60f32.to_radians()) - dir).length() { 60f32.to_radians() } else { -60f32.to_radians() });
                let o6 = d.add_atom(p1 + v * L + u * L, 8);
                d.add_bond(c6, o6, 1, Stereo::None);
            }
            true
        }
        t => {
            let a = label_atom(d, p1, t);
            d.add_bond(ring_atom, a, 1, Stereo::None);
            true
        }
    }
}

/// Haworth projection: the ring seen edge-on from slightly above, ring O at the back right,
/// the front edge drawn wedge–bold–wedge, every group straight up or down. The ring is tall
/// enough that groups pointing into it from the front and back edges keep clear of each other.
fn haworth(furanose: bool, subs: &[(String, String)]) -> Doc {
    let mut d = Doc::default();
    // Ring carbons from the anomeric one round to the ring-closing one, then the ring O.
    let (carbons, o_pos): (Vec<[f32; 2]>, [f32; 2]) = if furanose {
        (vec![[1.35, 0.0], [0.7, 0.75], [-0.7, 0.75], [-1.35, 0.0]], [0.0, -0.55])
    } else {
        (vec![[1.5, 0.05], [0.85, 0.8], [-0.85, 0.8], [-1.5, 0.05], [-0.6, -0.65]], [0.6, -0.65])
    };
    let idx: Vec<usize> = carbons.iter().map(|p| d.add_atom(pos2(p[0] * L, p[1] * L), 6)).collect();
    let o = d.add_atom(pos2(o_pos[0] * L, o_pos[1] * L), 8);
    let last = idx[idx.len() - 1];
    for k in 0..idx.len() - 1 {
        d.add_bond(idx[k], idx[k + 1], 1, Stereo::None);
    }
    d.add_bond(last, o, 1, Stereo::None);
    d.add_bond(o, idx[0], 1, Stereo::None);
    front_edge(&mut d, idx[0], idx[1], idx[2], idx[3]);
    let ys: Vec<f32> = carbons.iter().map(|p| p[1]).collect();
    let (front, back) = (ys.iter().cloned().fold(f32::MIN, f32::max), ys.iter().cloned().fold(f32::MAX, f32::min).min(o_pos[1]));
    for (k, (up, down)) in subs.iter().enumerate() {
        let Some(&c) = idx.get(k) else { break };
        let y = carbons[k][1];
        // Groups pointing into the ring are shorter.
        let inner = 0.45 * L;
        let outer = 0.7 * L;
        let up_len = if (y - front).abs() < 0.01 { inner } else { outer };
        // Groups hanging below the side atoms clear the front wedges.
        let down_len = if (y - back).abs() < 0.01 { inner } else if (y - front).abs() < 0.01 { outer } else { 0.92 * L };
        let p = d.atoms[c].pos;
        for (dy, t) in [(-up_len, up), (down_len, down)] {
            let a = label_atom(&mut d, p + egui::vec2(0.0, dy), t);
            d.add_bond(c, a, 1, Stereo::None);
        }
    }
    d
}

/// The ⁴C₁ chair in the usual flat template (ring O at the back right): steep bonds at 60°,
/// shallow ones at 15°, positions in bond lengths. Order: ring O, C1 (anomeric) … C5.
pub(crate) const CHAIR: [[f32; 2]; 6] = [[1.9275, 0.0], [2.4273, 0.8662], [1.4662, 0.6125], [0.4993, 0.8662], [0.0, 0.0], [0.9611, 0.253]];
/// Equatorial directions (each parallel to a ring bond, pointing out), C1 … C5.
const CHAIR_EQ: [[f32; 2]; 5] = [[0.9669, -0.2537], [0.4995, 0.8663], [-0.995, -0.099], [-0.9669, 0.2537], [-0.4993, -0.8662]];

/// A pyranose chair drawn on the flat template: axial groups straight up or down, equatorial
/// ones parallel to the ring bonds, ring hydrogens implied.
fn chair(subs: &[(String, String)]) -> Doc {
    let mut d = Doc::default();
    let idx: Vec<usize> = CHAIR.iter().enumerate().map(|(j, p)| d.add_atom(pos2(p[0] * L, p[1] * L), if j == 0 { 8 } else { 6 })).collect();
    for j in 0..6 {
        d.add_bond(idx[j], idx[(j + 1) % 6], 1, Stereo::None);
    }
    // Front edge C4–C3–C2–C1.
    front_edge(&mut d, idx[4], idx[3], idx[2], idx[1]);
    for (k, (up, down)) in subs.iter().enumerate().take(5) {
        let j = k + 1;
        // Axial bonds point up at C2 and C4, down at C1, C3 and C5.
        let axial_up = j % 2 == 0;
        let eq = egui::vec2(CHAIR_EQ[k][0], CHAIR_EQ[k][1]);
        let (up_dir, down_dir) = if axial_up { (egui::vec2(0.0, -1.0), eq) } else { (eq, egui::vec2(0.0, 1.0)) };
        for (dir, t) in [(up_dir, up), (down_dir, down)] {
            // An equatorial CH2OH at C5 runs on parallel to the shallow ring bonds.
            let next = (j == 5 && dir == eq).then(|| egui::vec2(0.9669, -0.2537));
            add_group(&mut d, idx[j], dir, t, false, next);
        }
    }
    d
}

/// Mirror a drawing left to right (D ↔ L).
fn mirrored(mut d: Doc) -> Doc {
    for a in &mut d.atoms {
        a.pos.x = -a.pos.x;
    }
    d
}

type V3 = [f32; 3];
fn sub3(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add3(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn mul3(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn unit(a: V3) -> V3 {
    let l = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt().max(1e-6);
    mul3(a, 1.0 / l)
}

/// Heights of the ring atoms above the mean plane (Cremer–Pople), ring O first.
/// Pyranose: θ = 0 chair, 90° boat / skew-boat, 50.8° half-chair, 54.7° envelope.
/// Furanose: φ in steps of 36° gives envelopes, half-way between them twists.
fn puckering(drawing: Drawing, furanose: bool, l_form: bool) -> Vec<f32> {
    let deg = |x: f32| x.to_radians();
    if furanose {
        let q = 0.42 * L;
        // Envelope with C3 out of the plane; twist turns φ half a step further.
        let phi = match drawing {
            Drawing::Twist => -72.0 + 18.0,
            _ => -72.0,
        };
        let sign = if l_form { -1.0 } else { 1.0 };
        (0..5).map(|j| sign * q * (2.0f32 / 5.0).sqrt() * (deg(phi + 144.0 * j as f32)).cos()).collect()
    } else {
        let q = 0.62 * L;
        let (theta, phi): (f32, f32) = match drawing {
            Drawing::Boat => (90.0, -120.0),
            Drawing::SkewBoat => (90.0, -90.0),
            Drawing::HalfChair => (50.8, 30.0),
            Drawing::Envelope => (54.7, 0.0),
            _ => (0.0, 0.0),
        };
        // ⁴C₁ for D sugars, ¹C₄ for L.
        let theta = if l_form { 180.0 - theta } else { theta };
        (0..6)
            .map(|j| {
                let q2 = q * deg(theta).sin();
                let q3 = q * deg(theta).cos();
                q2 * (1.0f32 / 3.0).sqrt() * deg(phi + 120.0 * j as f32).cos() + q3 * (1.0f32 / 6.0).sqrt() * if j % 2 == 0 { 1.0 } else { -1.0 }
            })
            .collect()
    }
}

/// A ring conformer: ring built in 3D, substituents placed tetrahedrally, then projected as
/// seen from slightly above the ring.
fn conformer(furanose: bool, subs: &[(String, String)], drawing: Drawing, l_form: bool) -> Doc {
    let n = if furanose { 5 } else { 6 };
    let angles: &[f32] = if furanose { &[270.0, 342.0, 54.0, 126.0, 198.0] } else { &[300.0, 0.0, 60.0, 120.0, 180.0, 240.0] };
    let radius = if furanose { L / (2.0 * 36f32.to_radians().sin()) } else { L };
    let z = puckering(drawing, furanose, l_form);
    let p3: Vec<V3> = (0..n).map(|j| {
        let a = angles[j].to_radians();
        [radius * a.cos(), z[j], radius * a.sin()]
    }).collect();
    let elev = 22f32.to_radians();
    let project = |p: V3| pos2(p[0], -p[1] * elev.cos() + p[2] * elev.sin());
    let mut d = Doc::default();
    let idx: Vec<usize> = (0..n).map(|j| d.add_atom(project(p3[j]), if j == 0 { 8 } else { 6 })).collect();
    for j in 0..n {
        d.add_bond(idx[j], idx[(j + 1) % n], 1, Stereo::None);
    }
    // The three ring bonds nearest the viewer make the wedge–bold–wedge front edge.
    let depth = |k: usize| p3[k][2] + p3[(k + 1) % n][2];
    let s = (0..n).max_by(|&x, &y| (depth(x) + depth((x + 1) % n) + depth((x + 2) % n)).total_cmp(&(depth(y) + depth((y + 1) % n) + depth((y + 2) % n)))).unwrap_or(0);
    front_edge(&mut d, idx[s], idx[(s + 1) % n], idx[(s + 2) % n], idx[(s + 3) % n]);
    let half = 54.75f32.to_radians();
    for (k, (up, down)) in subs.iter().enumerate() {
        let j = k + 1;
        if j >= n {
            break;
        }
        let (p, a, b) = (p3[j], p3[j - 1], p3[(j + 1) % n]);
        let out = unit(add3(sub3(p, a), sub3(p, b)));
        let mut nrm = unit(cross(sub3(a, p), sub3(b, p)));
        if nrm[1] < 0.0 {
            nrm = mul3(nrm, -1.0);
        }
        for (sign, t) in [(1.0, up), (-1.0, down)] {
            let dir = unit(add3(mul3(out, half.cos()), mul3(nrm, sign * half.sin())));
            let q = add3(p, mul3(dir, L));
            let v = project(q) - project(p);
            // Ring hydrogens are left implied, as in the usual conformer drawings.
            add_group(&mut d, idx[j], v, t, false, None);
        }
    }
    d
}

/// The drawing for a choice (None when that ring can't form for this sugar).
pub fn draw(c: &Choice) -> Option<Doc> {
    match (c.ring, c.drawing) {
        (None, _) | (_, Drawing::Fischer) => Some(fischer(&Choice { ring: None, ..*c })),
        (Some(r), Drawing::Haworth) => Some(haworth(r == Ring::Furanose, &ring_groups(c)?)),
        // An L sugar's ¹C₄ chair is the mirror image of its D enantiomer's ⁴C₁ chair.
        (Some(Ring::Pyranose), Drawing::Chair) if c.l_form => Some(mirrored(chair(&ring_groups(&Choice { l_form: false, ..*c })?))),
        (Some(Ring::Pyranose), Drawing::Chair) => Some(chair(&ring_groups(c)?)),
        (Some(r), dr) => {
            if !Drawing::for_ring(Some(r)).contains(&dr) {
                return None;
            }
            Some(conformer(r == Ring::Furanose, &ring_groups(c)?, dr, c.l_form))
        }
    }
}
