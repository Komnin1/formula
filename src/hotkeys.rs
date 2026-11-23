//! Hot keys: type a key while pointing at an atom or a bond (or with a "hotspot" atom set)
//! to change it or grow the structure from it. The keys follow the common chemistry
//! editor cheat sheet (1 = bond, 2 = carbonyl, o = OH, 6 = six-membered ring, ...).

use crate::cdx;
use crate::doc::*;
use egui::{Pos2, Vec2, pos2, vec2};

/// What a hot key did.
pub struct Done {
    /// Where the hotspot goes next (None: stays where it was).
    pub hotspot: Option<usize>,
    pub status: String,
}

fn done(hotspot: Option<usize>, status: &str) -> Option<Done> {
    Some(Done { hotspot, status: status.to_string() })
}

/// Label hot keys for atoms.
pub const ATOM_LABELS: &[(&str, &str)] = &[
    ("o", "OH"), ("q", "OH"), ("O", "OMe"), ("n", "NH2"), ("w", "NH2"), ("N", "NO2"),
    ("s", "SH"), ("S", "SiH3"), ("p", "PH2"), ("P", "Ph"), ("f", "F"), ("F", "CF3"),
    ("l", "Cl"), ("C", "Cl"), ("L", "Li"), ("b", "Br"), ("B", "BH2"), ("h", "H"),
    ("H", "Cbz"), ("y", "Boc"), ("Q", "Fmoc"), ("m", "Me"), ("M", "MgBr"), ("c", "CH3"),
    ("i", "I"), ("d", "D"), ("A", "Ac"), ("e", "Et"), ("E", "CO2Me"), ("Z", "N3"),
];

/// Every hot key with a short description, for the help panel.
pub const HELP_ATOM: &[(&str, &str)] = &[
    ("1", "single bond (hotspot moves to the new atom)"),
    ("0", "single bond, other zig-zag direction; straight out from a chain atom"),
    ("2", "C=O; at a chain end the chain goes on from the carbonyl"),
    ("3 / a", "ring: benzene"),
    ("4 / 5", "two bonds, one wedged / hashed; the chain goes on along the plain one"),
    ("6 / 7 / u / v", "ring: six / five / four / three (spiro on a chain atom); hotspot to the far ring atom"),
    ("8", "vinyl (C=C)"),
    ("9", "two bonds (branch or gem-dimethyl)"),
    ("K", "three bonds (tert-butyl); on a chain atom: two bonds, wedge and hash"),
    ("z", "alkyne (C≡C)"),
    ("k", "sulfonyl (SO2); at a chain end the chain goes on from the S"),
    ("j / J", "cyclohexane chair, two orientations"),
    ("+ / −", "raise / lower the charge"),
    ("Enter", "edit the label"),
];

pub const HELP_BOND: &[(&str, &str)] = &[
    ("1 / 2 / 3", "single / double / triple"),
    ("w", "wedge"),
    ("h / W", "hashed wedge"),
    ("H", "hashed (even width)"),
    ("b", "bold"),
    ("B", "bold double"),
    ("d", "dashed"),
    ("D", "double with one dashed line"),
    ("y", "wavy"),
    ("a", "fuse a benzene ring"),
    ("z", "fuse a five-membered ring with two double bonds"),
    ("v / 4 / 5 / 6 / 7 / 8", "fuse a ring of 3 / 4 / 5 / 6 / 7 / 8 atoms"),
    ("9 / 0", "fuse a cyclohexane chair, two orientations"),
];

/// Set a label on an atom the way typing it would.
pub fn set_label(doc: &mut Doc, i: usize, text: &str) {
    let a = &mut doc.atoms[i];
    match text {
        "CH3" => {
            // Shown as "CH3" while terminal; becomes CH2, CH… as bonds are added.
            a.el = crate::elements::C;
            a.label = None;
            a.show = true;
        }
        "D" => {
            a.el = crate::elements::H;
            a.label = Some("D".into());
            a.show = false;
        }
        _ => {
            let (z, label) = cdx::interpret_label(text);
            a.el = z;
            a.show = label.is_none() && text.trim() != "C";
            a.label = label;
        }
    }
}

/// Direction from atom `i` to its only neighbour.
fn into_dir(doc: &Doc, i: usize) -> Option<Vec2> {
    let n = doc.neighbors(i);
    (n.len() == 1).then(|| (doc.atoms[n[0]].pos - doc.atoms[i].pos).normalized())
}

fn grow(doc: &mut Doc, i: usize, dir: Vec2, order: u8, stereo: Stereo, el: u8) -> usize {
    let q = doc.atoms[i].pos + dir * BOND_LEN;
    let j = match doc.atom_at(q, BOND_LEN * 0.2, None) {
        Some(j) => j,
        None => doc.add_atom(q, el),
    };
    doc.add_bond(i, j, order, stereo);
    j
}

/// Two new bond directions from atom `i` (a branch on a chain end, or gem pair on a chain atom).
fn two_dirs(doc: &Doc, i: usize) -> (Vec2, Vec2) {
    match doc.degree(i) {
        0 => {
            let d = vec2(30f32.to_radians().cos(), -30f32.to_radians().sin());
            (d, rot(d, 120f32.to_radians()))
        }
        1 => {
            let d = into_dir(doc, i).unwrap_or(vec2(-1.0, 0.0));
            (rot(d, 120f32.to_radians()), rot(d, -120f32.to_radians()))
        }
        _ => {
            let f = doc.free_direction(i);
            (rot(f, 35f32.to_radians()), rot(f, -35f32.to_radians()))
        }
    }
}

/// At a chain end: (side direction, chain direction). The chain goes where "1" would put
/// it; the side group takes the other zig-zag position.
fn branch_and_chain(doc: &Doc, i: usize) -> (Vec2, Vec2) {
    let on = doc.free_direction(i);
    let side = match into_dir(doc, i) {
        Some(d) => {
            let c1 = rot(d, 120f32.to_radians());
            let c2 = rot(d, -120f32.to_radians());
            if (c1 - on).length() < (c2 - on).length() { c2 } else { c1 }
        }
        None => rot(on, -120f32.to_radians()),
    };
    (side, on)
}

/// Where `p` of a template goes when template points `s1`, `s2` are put on `d1`, `d2`.
fn map_point(s1: Pos2, s2: Pos2, d1: Pos2, d2: Pos2, p: Pos2) -> Pos2 {
    let sv = s2 - s1;
    let dv = d2 - d1;
    let k = dv.length() / sv.length().max(1e-6);
    let a = dv.y.atan2(dv.x) - sv.y.atan2(sv.x);
    d1 + rot((p - s1) * k, a)
}

/// The flat chair template in drawing units, optionally mirrored.
fn chair_points(mirror: bool) -> Vec<Pos2> {
    crate::sugars::CHAIR.iter().map(|p| pos2(p[0] * BOND_LEN, if mirror { -p[1] } else { p[1] } * BOND_LEN)).collect()
}

/// Add a chair whose ring atoms `k1`, `k2` are the existing atoms `a1`, `a2`.
fn place_chair(doc: &mut Doc, pts: &[Pos2], k1: usize, s2: Pos2, a1: usize, d2: Pos2, fixed: &[(usize, usize)]) {
    let s1 = pts[k1];
    let d1 = doc.atoms[a1].pos;
    let mut ids = Vec::with_capacity(6);
    for (k, p) in pts.iter().enumerate() {
        let id = match fixed.iter().find(|f| f.0 == k) {
            Some(f) => f.1,
            None => {
                let q = map_point(s1, s2, d1, d2, *p);
                match doc.atom_at(q, BOND_LEN * 0.2, None) {
                    Some(j) => j,
                    None => doc.add_atom(q, crate::elements::C),
                }
            }
        };
        ids.push(id);
    }
    for k in 0..6 {
        let (a, b) = (ids[k], ids[(k + 1) % 6]);
        if doc.find_bond(a, b).is_none() {
            doc.add_bond(a, b, 1, Stereo::None);
        }
    }
}

/// A cyclohexane chair hanging from atom `q` (reached along `dir`) by one of its tips.
fn chair_on_atom(doc: &mut Doc, q: usize, dir: Vec2, mirror: bool) {
    let pts = chair_points(mirror);
    let c = pts.iter().fold(Vec2::ZERO, |s, p| s + p.to_vec2()) / 6.0;
    let tip = 4;
    let reach = (c - pts[tip].to_vec2()).length();
    let to = doc.atoms[q].pos + dir * reach;
    place_chair(doc, &pts, tip, c.to_pos2(), q, to, &[(tip, q)]);
}

/// A cyclohexane chair fused on bond `bi`, on the side away from the other atoms.
fn chair_on_bond(doc: &mut Doc, bi: usize, other_edge: bool) {
    let (ia, ib) = (doc.bonds[bi].a, doc.bonds[bi].b);
    let (pa, pb) = (doc.atoms[ia].pos, doc.atoms[ib].pos);
    let mid = pa + (pb - pa) * 0.5;
    let nrm = rot((pb - pa).normalized(), std::f32::consts::FRAC_PI_2);
    let crowd: f32 = doc.neighbors(ia).into_iter().chain(doc.neighbors(ib)).filter(|k| *k != ia && *k != ib).map(|k| (doc.atoms[k].pos - mid).dot(nrm)).sum();
    // Ring edge 4–3 (steep) or 4–5 (shallow) goes on the bond.
    let (k1, k2) = if other_edge { (4, 5) } else { (4, 3) };
    for mirror in [false, true] {
        let pts = chair_points(mirror);
        let c = pts.iter().fold(Vec2::ZERO, |s, p| s + p.to_vec2()) / 6.0;
        let at = map_point(pts[k1], pts[k2], pa, pb, c.to_pos2());
        let away = (at - mid).dot(nrm);
        if away * crowd <= 0.0 || mirror {
            place_chair(doc, &pts, k1, pts[k2], ia, pb, &[(k1, ia), (k2, ib)]);
            return;
        }
    }
}

/// Hot key on an atom. None if the key isn't an atom hot key.
pub fn atom_key(doc: &mut Doc, i: usize, key: &str) -> Option<Done> {
    if let Some((_, l)) = ATOM_LABELS.iter().find(|(k, _)| *k == key) {
        set_label(doc, i, l);
        return done(None, &format!("Label {}", l));
    }
    let chain = doc.degree(i) >= 2;
    match key {
        "1" => {
            let j = grow(doc, i, doc.free_direction(i), 1, Stereo::None, crate::elements::C);
            done(Some(j), "Added a bond")
        }
        // On a chain atom: one bond straight out.
        "0" if chain => {
            let j = grow(doc, i, doc.free_direction(i), 1, Stereo::None, crate::elements::C);
            done(Some(j), "Added a bond")
        }
        "0" => {
            let d = match into_dir(doc, i) {
                Some(d) => {
                    // The other of the two zig-zag positions.
                    let f = doc.free_direction(i);
                    let c1 = rot(d, 120f32.to_radians());
                    let c2 = rot(d, -120f32.to_radians());
                    if (c1 - f).length() < 0.1 { c2 } else { c1 }
                }
                None => rot(doc.free_direction(i), 60f32.to_radians()),
            };
            let j = grow(doc, i, d, 1, Stereo::None, crate::elements::C);
            done(Some(j), "Added a bond")
        }
        // On a chain atom: only the =O.
        "2" if chain => {
            grow(doc, i, doc.free_direction(i), 2, Stereo::None, crate::elements::O);
            done(None, "Added C=O")
        }
        // At a chain end the atom becomes a carbonyl and the chain goes on from it.
        "2" => {
            let (side, on) = branch_and_chain(doc, i);
            grow(doc, i, side, 2, Stereo::None, crate::elements::O);
            let j = grow(doc, i, on, 1, Stereo::None, crate::elements::C);
            done(Some(j), "Added C=O")
        }
        "3" | "a" => {
            let ring = doc.add_ring(6, true, RingAnchor::Atom(i));
            done(ring.get(3).copied(), "Added a benzene ring")
        }
        // The hotspot goes to the ring atom farthest from the attachment.
        "6" | "7" | "u" | "v" => {
            let n = match key {
                "6" => 6,
                "7" => 5,
                "u" => 4,
                _ => 3,
            };
            let ring = doc.add_ring(n, false, RingAnchor::Atom(i));
            done(ring.get(n / 2).copied(), &format!("Added a {}-membered ring", n))
        }
        "j" | "J" => {
            let d = doc.free_direction(i);
            let q = grow(doc, i, d, 1, Stereo::None, crate::elements::C);
            chair_on_atom(doc, q, d, key == "J");
            done(None, "Added a cyclohexane chair")
        }
        // At a chain end: the wedge, hash or branch goes to the side, the chain goes on.
        "4" | "5" | "9" if !chain => {
            let st = match key {
                "4" => Stereo::Wedge,
                "5" => Stereo::Hash,
                _ => Stereo::None,
            };
            let (side, on) = branch_and_chain(doc, i);
            grow(doc, i, side, 1, st, crate::elements::C);
            let j = grow(doc, i, on, 1, Stereo::None, crate::elements::C);
            done(Some(j), "Added two bonds")
        }
        "4" | "5" | "9" => {
            let (d1, d2) = two_dirs(doc, i);
            let st = match key {
                "4" => Stereo::Wedge,
                "5" => Stereo::Hash,
                _ => Stereo::None,
            };
            grow(doc, i, d1, 1, Stereo::None, crate::elements::C);
            let j = grow(doc, i, d2, 1, st, crate::elements::C);
            done(Some(j), "Added two bonds")
        }
        "K" => {
            if chain {
                let (d1, d2) = two_dirs(doc, i);
                grow(doc, i, d1, 1, Stereo::Wedge, crate::elements::C);
                grow(doc, i, d2, 1, Stereo::Hash, crate::elements::C);
                done(None, "Added a wedged and a hashed bond")
            } else {
                let d = into_dir(doc, i).unwrap_or(vec2(-1.0, 0.0));
                let mut last = i;
                for a in [90f32, 180.0, 270.0] {
                    last = grow(doc, i, rot(d, a.to_radians()), 1, Stereo::None, crate::elements::C);
                }
                done(Some(last), "Added a tert-butyl group")
            }
        }
        "8" => {
            let j = grow(doc, i, doc.free_direction(i), 2, Stereo::None, crate::elements::C);
            done(Some(j), "Added C=C")
        }
        "z" => {
            let d = into_dir(doc, i).map(|d| -d).unwrap_or(doc.free_direction(i));
            let j = grow(doc, i, d, 3, Stereo::None, crate::elements::C);
            done(Some(j), "Added C≡C")
        }
        // The atom becomes a sulfonyl S; at a chain end the chain goes on from it.
        "k" => {
            set_label(doc, i, "S");
            let next = (!chain).then(|| grow(doc, i, doc.free_direction(i), 1, Stereo::None, crate::elements::C));
            let (d1, d2) = two_dirs(doc, i);
            grow(doc, i, d1, 2, Stereo::None, crate::elements::O);
            grow(doc, i, d2, 2, Stereo::None, crate::elements::O);
            done(next, "Added SO2")
        }
        "+" | "=" => {
            doc.atoms[i].charge = (doc.atoms[i].charge + 1).min(9);
            done(None, "Charge raised")
        }
        "-" | "−" => {
            doc.atoms[i].charge = (doc.atoms[i].charge - 1).max(-9);
            done(None, "Charge lowered")
        }
        _ => None,
    }
}

/// Hot key on a bond. None if the key isn't a bond hot key.
pub fn bond_key(doc: &mut Doc, bi: usize, key: &str) -> Option<Done> {
    let set = |doc: &mut Doc, order: u8, stereo: Stereo, reverse: bool| {
        let b = &mut doc.bonds[bi];
        b.order = order;
        b.stereo = stereo;
        if reverse {
            std::mem::swap(&mut b.a, &mut b.b);
        }
    };
    let ring = |doc: &mut Doc, n: usize, aromatic: bool| {
        doc.add_ring(n, aromatic, RingAnchor::Bond(bi));
    };
    match key {
        "1" => set(doc, 1, Stereo::None, false),
        "2" => set(doc, 2, Stereo::None, false),
        "3" => set(doc, 3, Stereo::None, false),
        "w" => set(doc, 1, Stereo::Wedge, false),
        "h" | "W" => set(doc, 1, Stereo::Hash, false),
        "H" => set(doc, 1, Stereo::Hashed, false),
        "b" => set(doc, 1, Stereo::Bold, false),
        "B" => set(doc, 2, Stereo::Bold, false),
        "d" => set(doc, 1, Stereo::Dashed, false),
        "D" => set(doc, 2, Stereo::Dashed, false),
        "y" => set(doc, 1, Stereo::Wavy, false),
        "a" => ring(doc, 6, true),
        "z" => ring(doc, 5, true),
        "v" => ring(doc, 3, false),
        "4" => ring(doc, 4, false),
        "5" => ring(doc, 5, false),
        "6" => ring(doc, 6, false),
        "9" | "0" => chair_on_bond(doc, bi, key == "0"),
        "7" => ring(doc, 7, false),
        "8" => ring(doc, 8, false),
        _ => return None,
    }
    done(None, "Bond changed")
}

/// The neighbour of `i` lying most in direction `dir` (for moving the hotspot with arrow keys).
pub fn neighbor_towards(doc: &Doc, i: usize, dir: Vec2) -> Option<usize> {
    let p = doc.atoms[i].pos;
    doc.neighbors(i)
        .into_iter()
        .map(|n| (n, (doc.atoms[n].pos - p).normalized().dot(dir)))
        .filter(|(_, d)| *d > 0.3)
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .map(|(n, _)| n)
}
