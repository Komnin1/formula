//! Open chemistry formats: SMILES, MDL molfile (V2000/V3000), SDfile,
//! reaction molfile (RXN V2000/V3000) and Chemical Markup Language (CML).

use crate::chem;
use crate::doc::*;
use crate::elements;
use crate::xml;
use egui::{Vec2, pos2, vec2};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// SMILES

/// Neighbours of an atom in the order they are written, for its @ / @@ mark.
#[derive(Clone, Copy, PartialEq)]
enum Slot {
    Atom(usize),
    H,
    Ring(u32),
}

pub fn parse_smiles(s: &str) -> Result<Doc, String> {
    parse_smiles_oriented(s, &|_| {})
}

/// Read SMILES, let `orient` turn or mirror the laid-out drawing, then draw @ / @@ centres
/// with a wedged or hashed bond so the drawing has the written handedness.
pub fn parse_smiles_oriented(s: &str, orient: &dyn Fn(&mut Doc)) -> Result<Doc, String> {
    let s = s.trim();
    let s = s.split_whitespace().next().unwrap_or("");
    if s.is_empty() {
        return Err("empty SMILES".into());
    }
    let mut d = Doc::default();
    let mut arom: Vec<bool> = Vec::new();
    let mut bond_arom: Vec<bool> = Vec::new();
    let mut explicit_h: Vec<Option<i32>> = Vec::new();
    let mut stack: Vec<usize> = Vec::new();
    let mut prev: Option<usize> = None;
    let mut pending: Option<(u8, Stereo)> = None;
    let mut rings: HashMap<u32, (usize, Option<u8>)> = HashMap::new();
    let mut chir: Vec<u8> = Vec::new();
    let mut slots: Vec<Vec<Slot>> = Vec::new();
    let ch: Vec<char> = s.chars().collect();
    let mut i = 0;
    let connect = |d: &mut Doc, bond_arom: &mut Vec<bool>, arom: &Vec<bool>, a: usize, b: usize, ord: Option<u8>| {
        let o = ord.unwrap_or(1);
        if d.add_bond(a, b, o, Stereo::None).is_some() && d.bonds.len() > bond_arom.len() {
            bond_arom.push(ord.is_none() && arom[a] && arom[b]);
        }
    };
    while i < ch.len() {
        let c = ch[i];
        match c {
            '(' => {
                stack.push(prev.ok_or("branch before atom")?);
                i += 1;
            }
            ')' => {
                prev = Some(stack.pop().ok_or("unbalanced )")?);
                i += 1;
            }
            '-' => { pending = Some((1, Stereo::None)); i += 1; }
            '=' => { pending = Some((2, Stereo::None)); i += 1; }
            '#' => { pending = Some((3, Stereo::None)); i += 1; }
            '$' => { pending = Some((3, Stereo::None)); i += 1; }
            ':' => { pending = None; i += 1; }
            '/' | '\\' => { pending = Some((1, Stereo::None)); i += 1; }
            '.' => { prev = None; pending = None; i += 1; }
            '%' | '0'..='9' => {
                let num: u32 = if c == '%' {
                    let n: String = ch[i + 1..(i + 3).min(ch.len())].iter().collect();
                    i += 3;
                    n.parse().map_err(|_| "bad ring number")?
                } else {
                    i += 1;
                    c.to_digit(10).unwrap()
                };
                let cur = prev.ok_or("ring bond before atom")?;
                let ord = pending.take().map(|p| p.0);
                if let Some((other, o2)) = rings.remove(&num) {
                    connect(&mut d, &mut bond_arom, &arom, other, cur, ord.or(o2));
                    slots[cur].push(Slot::Atom(other));
                    if let Some(sl) = slots[other].iter_mut().find(|x| **x == Slot::Ring(num)) {
                        *sl = Slot::Atom(cur);
                    }
                } else {
                    rings.insert(num, (cur, ord));
                    slots[cur].push(Slot::Ring(num));
                }
            }
            '[' => {
                let j = ch[i..].iter().position(|&x| x == ']').ok_or("unclosed [")? + i;
                let inner: String = ch[i + 1..j].iter().collect();
                i = j + 1;
                let (z, is_arom, hcount, charge, iso, chirality) = parse_bracket(&inner)?;
                let idx = d.add_atom(pos2(0.0, 0.0), z);
                d.atoms[idx].charge = charge as i8;
                d.atoms[idx].isotope = iso;
                arom.push(is_arom);
                explicit_h.push(Some(hcount));
                chir.push(chirality);
                slots.push(Vec::new());
                if let Some(p) = prev {
                    let ord = pending.take().map(|x| x.0);
                    connect(&mut d, &mut bond_arom, &arom, p, idx, ord);
                    slots[idx].push(Slot::Atom(p));
                    slots[p].push(Slot::Atom(idx));
                }
                if hcount == 1 {
                    slots[idx].push(Slot::H);
                }
                prev = Some(idx);
            }
            '*' => {
                let idx = d.add_atom(pos2(0.0, 0.0), 0);
                d.atoms[idx].label = Some("R".into());
                arom.push(false);
                explicit_h.push(None);
                chir.push(0);
                slots.push(Vec::new());
                if let Some(p) = prev {
                    let ord = pending.take().map(|x| x.0);
                    connect(&mut d, &mut bond_arom, &arom, p, idx, ord);
                    slots[idx].push(Slot::Atom(p));
                    slots[p].push(Slot::Atom(idx));
                }
                prev = Some(idx);
                i += 1;
            }
            _ if c.is_ascii_alphabetic() => {
                let two: String = ch[i..(i + 2).min(ch.len())].iter().collect();
                let (sym, len, is_arom) = if two == "Cl" || two == "Br" {
                    (two.clone(), 2, false)
                } else if "BCNOPSFI".contains(c) {
                    (c.to_string(), 1, false)
                } else if "bcnops".contains(c) {
                    (c.to_ascii_uppercase().to_string(), 1, true)
                } else {
                    return Err(format!("unexpected '{}'", c));
                };
                let z = elements::by_symbol(&sym).ok_or("unknown element")?;
                let idx = d.add_atom(pos2(0.0, 0.0), z);
                arom.push(is_arom);
                explicit_h.push(None);
                chir.push(0);
                slots.push(Vec::new());
                if let Some(p) = prev {
                    let ord = pending.take().map(|x| x.0);
                    connect(&mut d, &mut bond_arom, &arom, p, idx, ord);
                    slots[idx].push(Slot::Atom(p));
                    slots[p].push(Slot::Atom(idx));
                }
                prev = Some(idx);
                i += len;
            }
            _ => return Err(format!("unexpected '{}'", c)),
        }
    }
    if !rings.is_empty() {
        return Err("unclosed ring".into());
    }
    kekulize(&mut d, &arom, &bond_arom, &explicit_h)?;
    // Bracket atoms with unusual hydrogen counts: show as radicals so the count matches.
    for (k, h) in explicit_h.iter().enumerate() {
        if let Some(h) = h {
            let imp = d.implicit_h(k);
            if imp > *h && d.atoms[k].label.is_none() {
                let diff = imp - h;
                if d.atoms[k].el == elements::C || d.atoms[k].el == elements::N || d.atoms[k].el == elements::O {
                    d.atoms[k].radical = diff.min(2) as u8;
                }
            }
        }
    }
    chem::layout(&mut d);
    orient(&mut d);
    assign_wedges(&mut d, &chir, &slots);
    Ok(d)
}

/// Draw each @ / @@ centre with one wedged or hashed bond. The bond goes to an outside
/// substituent (not a ring bond, preferably a small end group) and is wedged or hashed so the
/// drawing has the handedness written in the SMILES.
fn assign_wedges(d: &mut Doc, chir: &[u8], slots: &[Vec<Slot>]) {
    let ring_bonds = chem::ring_bonds(d, &chem::sssr(d));
    for c in 0..d.atoms.len() {
        let want = chir.get(c).copied().unwrap_or(0);
        let order = &slots[c];
        if want == 0 || order.len() != 4 || order.iter().any(|s| matches!(s, Slot::Ring(_))) {
            continue;
        }
        let cp = d.atoms[c].pos;
        let unit = |v: Vec2| if v.length() > 1e-4 { v / v.length() } else { v };
        let atoms: Vec<usize> = order.iter().filter_map(|s| if let Slot::Atom(j) = s { Some(*j) } else { None }).collect();
        let sum: Vec2 = atoms.iter().map(|&j| unit(d.atoms[j].pos - cp)).fold(Vec2::ZERO, |a, b| a + b);
        let h2d = if sum.length() > 0.2 { -unit(sum) } else { vec2(0.0, 1.0) };
        // Candidates: single, not yet drawn as wedges, outside rings, not ending at another centre.
        let mut best: Option<(usize, (u8, u8, usize, usize))> = None;
        for &j in &atoms {
            let Some(bi) = d.find_bond(c, j) else { continue };
            let b = &d.bonds[bi];
            if b.order != 1 || b.stereo != Stereo::None {
                continue;
            }
            let in_ring = ring_bonds.contains(&bi) as u8;
            let other_centre = (chir.get(j).copied().unwrap_or(0) != 0) as u8;
            let rank = (in_ring, other_centre, d.neighbors(j).len(), j);
            if best.as_ref().is_none_or(|(_, r)| rank < *r) {
                best = Some((j, rank));
            }
        }
        let Some((w, _)) = best else { continue };
        let det_for = |zw: f32| {
            let v: Vec<[f32; 3]> = order
                .iter()
                .map(|s| match s {
                    Slot::Atom(j) => {
                        let u = unit(d.atoms[*j].pos - cp);
                        [u.x, u.y, if *j == w { zw } else { 0.0 }]
                    }
                    _ => [h2d.x, h2d.y, -zw],
                })
                .collect();
            let r = |k: usize| [v[k][0] - v[0][0], v[k][1] - v[0][1], v[k][2] - v[0][2]];
            let (a, b, cc) = (r(1), r(2), r(3));
            a[0] * (b[1] * cc[2] - b[2] * cc[1]) - a[1] * (b[0] * cc[2] - b[2] * cc[0]) + a[2] * (b[0] * cc[1] - b[1] * cc[0])
        };
        // In drawing coordinates (y down, z toward the viewer) "@" gives a positive volume.
        let ok = |det: f32| if want == 1 { det > 0.0 } else { det < 0.0 };
        let stereo = if ok(det_for(1.0)) {
            Stereo::Wedge
        } else if ok(det_for(-1.0)) {
            Stereo::Hash
        } else {
            continue;
        };
        if let Some(bi) = d.find_bond(c, w) {
            let b = &mut d.bonds[bi];
            if b.a != c {
                std::mem::swap(&mut b.a, &mut b.b);
            }
            b.stereo = stereo;
        }
    }
}

fn parse_bracket(s: &str) -> Result<(u8, bool, i32, i32, u16, u8), String> {
    let ch: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut iso = String::new();
    while i < ch.len() && ch[i].is_ascii_digit() {
        iso.push(ch[i]);
        i += 1;
    }
    if i >= ch.len() {
        return Err("empty bracket atom".into());
    }
    let (z, arom) = if ch[i].is_ascii_lowercase() {
        let two: String = ch[i..(i + 2).min(ch.len())].iter().collect();
        if two == "se" || two == "as" {
            i += 2;
            (elements::by_symbol_loose(&two).unwrap(), true)
        } else {
            let z = elements::by_symbol(&ch[i].to_ascii_uppercase().to_string()).ok_or("bad aromatic atom")?;
            i += 1;
            (z, true)
        }
    } else if ch[i] == '*' {
        i += 1;
        (0, false)
    } else {
        let mut sym = ch[i].to_string();
        if i + 1 < ch.len() && ch[i + 1].is_ascii_lowercase() && elements::by_symbol(&format!("{}{}", ch[i], ch[i + 1])).is_some() {
            sym.push(ch[i + 1]);
            i += 2;
        } else {
            i += 1;
        }
        (elements::by_symbol(&sym).ok_or_else(|| format!("unknown element {}", sym))?, false)
    };
    let mut chirality = 0u8;
    while i < ch.len() && ch[i] == '@' {
        chirality += 1;
        i += 1;
    }
    let mut h = 0;
    if i < ch.len() && ch[i] == 'H' {
        i += 1;
        h = 1;
        let mut n = String::new();
        while i < ch.len() && ch[i].is_ascii_digit() {
            n.push(ch[i]);
            i += 1;
        }
        if !n.is_empty() {
            h = n.parse().unwrap();
        }
    }
    let mut charge = 0;
    while i < ch.len() && (ch[i] == '+' || ch[i] == '-') {
        let sign = if ch[i] == '+' { 1 } else { -1 };
        i += 1;
        let mut n = String::new();
        while i < ch.len() && ch[i].is_ascii_digit() {
            n.push(ch[i]);
            i += 1;
        }
        charge += sign * n.parse::<i32>().unwrap_or(1);
    }
    Ok((z, arom, h, charge, iso.parse().unwrap_or(0), chirality.min(2)))
}

fn kekulize(d: &mut Doc, arom: &[bool], bond_arom: &[bool], explicit_h: &[Option<i32>]) -> Result<(), String> {
    let n = d.atoms.len();
    // Atoms that need a double bond.
    let mut need = vec![false; n];
    for i in 0..n {
        if !arom[i] {
            continue;
        }
        let a = &d.atoms[i];
        let has_double = d.bonds.iter().any(|b| (b.a == i || b.b == i) && b.order == 2);
        if has_double {
            continue;
        }
        let deg = d.degree(i) as i32;
        let h = explicit_h[i].unwrap_or(0);
        need[i] = match a.el {
            6 => a.charge == 0 || (deg + h) < 3,
            7 | 15 => (a.charge == 0 && deg + h == 2) || (a.charge == 1 && deg + h == 3),
            5 => false,
            8 | 16 | 34 => a.charge == 1 && deg == 2,
            _ => false,
        };
        // Aromatic carbon written in brackets with an H already has its 3 connections.
        if a.el == 6 && explicit_h[i].is_some() && a.charge != 0 {
            need[i] = false;
        }
    }
    let cand: Vec<usize> = (0..d.bonds.len()).filter(|&k| bond_arom.get(k).copied().unwrap_or(false)).collect();
    let mut matched = vec![false; n];
    fn solve(d: &Doc, need: &[bool], matched: &mut Vec<bool>, cand: &[usize], chosen: &mut Vec<usize>, steps: &mut usize) -> bool {
        *steps += 1;
        if *steps > 200_000 {
            return false;
        }
        // Pick the unmatched needing atom with fewest options.
        let mut best: Option<(usize, Vec<usize>)> = None;
        for i in 0..need.len() {
            if !need[i] || matched[i] {
                continue;
            }
            let opts: Vec<usize> = cand
                .iter()
                .copied()
                .filter(|&k| {
                    let b = &d.bonds[k];
                    (b.a == i || b.b == i) && need[b.other(i)] && !matched[b.other(i)]
                })
                .collect();
            if best.as_ref().map(|x| opts.len() < x.1.len()).unwrap_or(true) {
                best = Some((i, opts));
            }
        }
        let Some((i, opts)) = best else { return true };
        if opts.is_empty() {
            return false;
        }
        for k in opts {
            let j = d.bonds[k].other(i);
            matched[i] = true;
            matched[j] = true;
            chosen.push(k);
            if solve(d, need, matched, cand, chosen, steps) {
                return true;
            }
            chosen.pop();
            matched[i] = false;
            matched[j] = false;
        }
        false
    }
    let mut chosen = Vec::new();
    let mut steps = 0;
    if !solve(d, &need, &mut matched, &cand, &mut chosen, &mut steps) {
        // Retry allowing pyrrole-type nitrogens to stay single-bonded.
        for i in 0..n {
            if need[i] && d.atoms[i].el == 7 {
                need[i] = false;
            }
        }
        matched = vec![false; n];
        chosen.clear();
        steps = 0;
        if !solve(d, &need, &mut matched, &cand, &mut chosen, &mut steps) {
            return Err("could not assign aromatic double bonds".into());
        }
    }
    for k in chosen {
        d.bonds[k].order = 2;
    }
    Ok(())
}

/// Replace group labels by real atoms (used before writing chemistry formats).
pub fn expand_labels(d: &Doc) -> Doc {
    let mut out = d.clone();
    loop {
        let Some(i) = out.atoms.iter().position(|a| a.label.as_ref().map(|l| chem::expansion_smiles(l).is_some()).unwrap_or(false)) else {
            break;
        };
        expand_atom(&mut out, i);
    }
    out
}

/// Expand one labelled atom in place. Returns false if the label isn't known.
pub fn expand_atom(d: &mut Doc, i: usize) -> bool {
    let Some(label) = d.atoms[i].label.clone() else { return false };
    let Some(smi) = chem::expansion_smiles(&label) else { return false };
    let Ok(mut frag) = parse_smiles(smi) else { return false };
    let p = d.atoms[i].pos;
    let dir = if d.degree(i) == 1 {
        let n = d.neighbors(i)[0];
        (p - d.atoms[n].pos).normalized()
    } else {
        vec2(1.0, 0.0)
    };
    // Orient the group: first atom at p, its own neighbours pointing away.
    let first = frag.atoms[0].pos;
    let c = chem::centroid(&frag, &(0..frag.atoms.len()).collect::<Vec<_>>());
    let fdir = if (c - first).length() > 1e-3 { (c - first).normalized() } else { vec2(1.0, 0.0) };
    let ang = dir.y.atan2(dir.x) - fdir.y.atan2(fdir.x);
    for a in &mut frag.atoms {
        let v = rot(a.pos - first, ang);
        a.pos = p + v;
    }
    let charge = d.atoms[i].charge;
    // Replace atom i by frag atom 0.
    d.atoms[i].el = frag.atoms[0].el;
    d.atoms[i].label = None;
    d.atoms[i].charge = frag.atoms[0].charge + charge;
    let base = d.atoms.len();
    for a in frag.atoms.iter().skip(1) {
        d.atoms.push(a.clone());
    }
    let map = |k: usize| if k == 0 { i } else { base + k - 1 };
    for b in &frag.bonds {
        let mut nb = b.clone();
        nb.a = map(b.a);
        nb.b = map(b.b);
        d.bonds.push(nb);
    }
    true
}

pub fn write_smiles(d0: &Doc) -> String {
    let d = expand_labels(d0);
    let adj = d.adjacency();
    let mut parts = Vec::new();
    for frag in d.fragments() {
        let start = frag.iter().copied().min_by_key(|&i| adj[i].len()).unwrap();
        let mut visited = vec![false; d.atoms.len()];
        let mut tree_bond = vec![false; d.bonds.len()];
        // DFS to find tree edges.
        fn dfs(u: usize, adj: &[Vec<(usize, usize)>], visited: &mut Vec<bool>, tree: &mut Vec<bool>) {
            visited[u] = true;
            for &(v, b) in &adj[u] {
                if !visited[v] {
                    tree[b] = true;
                    dfs(v, adj, visited, tree);
                }
            }
        }
        dfs(start, &adj, &mut visited, &mut tree_bond);
        let mut out = String::new();
        let mut open: HashMap<usize, u32> = HashMap::new();
        let mut used_digits: Vec<bool> = vec![false; 100];
        let mut done = vec![false; d.atoms.len()];
        fn bond_sym(o: u8) -> &'static str {
            match o {
                2 => "=",
                3 => "#",
                _ => "",
            }
        }
        #[allow(clippy::too_many_arguments)]
        fn write(
            u: usize,
            from_bond: Option<usize>,
            d: &Doc,
            adj: &[Vec<(usize, usize)>],
            tree: &[bool],
            done: &mut Vec<bool>,
            open: &mut HashMap<usize, u32>,
            used: &mut Vec<bool>,
            out: &mut String,
        ) {
            done[u] = true;
            out.push_str(&atom_smiles(d, u));
            // Ring closures.
            for &(_, b) in &adj[u] {
                if tree[b] || Some(b) == from_bond {
                    continue;
                }
                if let Some(dg) = open.remove(&b) {
                    out.push_str(bond_sym(d.bonds[b].order));
                    out.push_str(&ring_label(dg));
                    used[dg as usize] = false;
                } else {
                    let dg = (1..100).find(|&k| !used[k]).unwrap_or(99) as u32;
                    used[dg as usize] = true;
                    open.insert(b, dg);
                    out.push_str(bond_sym(d.bonds[b].order));
                    out.push_str(&ring_label(dg));
                }
            }
            let kids: Vec<(usize, usize)> = adj[u].iter().copied().filter(|&(v, b)| tree[b] && !done[v]).collect();
            for (k, (v, b)) in kids.iter().enumerate() {
                let last = k + 1 == kids.len();
                if !last {
                    out.push('(');
                }
                out.push_str(bond_sym(d.bonds[*b].order));
                write(*v, Some(*b), d, adj, tree, done, open, used, out);
                if !last {
                    out.push(')');
                }
            }
        }
        write(start, None, &d, &adj, &tree_bond, &mut done, &mut open, &mut used_digits, &mut out);
        parts.push(out);
    }
    parts.join(".")
}

fn ring_label(d: u32) -> String {
    if d < 10 { d.to_string() } else { format!("%{}", d) }
}

fn atom_smiles(d: &Doc, i: usize) -> String {
    let a = &d.atoms[i];
    if let Some(l) = &a.label {
        let _ = l;
        return "*".into();
    }
    if a.el == 0 {
        return "*".into();
    }
    let sym = elements::symbol(a.el);
    let organic = matches!(a.el, 5 | 6 | 7 | 8 | 9 | 15 | 16 | 17 | 35 | 53);
    if organic && a.charge == 0 && a.isotope == 0 && a.radical == 0 {
        return sym.to_string();
    }
    let h = d.implicit_h(i);
    let mut s = String::from("[");
    if a.isotope != 0 {
        s.push_str(&a.isotope.to_string());
    }
    s.push_str(sym);
    if h == 1 {
        s.push('H');
    } else if h > 1 {
        s.push_str(&format!("H{}", h));
    }
    if a.charge != 0 {
        s.push(if a.charge > 0 { '+' } else { '-' });
        if a.charge.abs() > 1 {
            s.push_str(&a.charge.abs().to_string());
        }
    }
    s.push(']');
    s
}

// ---------------------------------------------------------------------------
// Molfiles

const MOL_SCALE: f32 = 1.5 / BOND_LEN; // file Å per drawing unit

fn mol_symbol(d: &Doc, i: usize) -> String {
    let a = &d.atoms[i];
    if let Some(l) = &a.label {
        return if l.len() <= 3 { l.clone() } else { "R".into() };
    }
    if a.el == 0 { "R".into() } else { elements::symbol(a.el).into() }
}

fn stereo_code(b: &Bond) -> u8 {
    match b.stereo {
        Stereo::Wedge => 1,
        Stereo::Hash => 6,
        Stereo::Wavy => 4,
        _ => 0,
    }
}

pub fn write_molfile_v2000(d0: &Doc, title: &str) -> String {
    let d = expand_labels(d0);
    let mut s = String::new();
    s.push_str(title);
    s.push('\n');
    s.push_str("  Bondwrite 2D\n\n");
    s.push_str(&format!("{:>3}{:>3}  0  0  0  0  0  0  0  0999 V2000\n", d.atoms.len(), d.bonds.len()));
    let c = mol_center(&d);
    for (i, a) in d.atoms.iter().enumerate() {
        let x = (a.pos.x - c.x) * MOL_SCALE;
        let y = -(a.pos.y - c.y) * MOL_SCALE;
        s.push_str(&format!("{:>10.4}{:>10.4}{:>10.4} {:<3} 0  0  0  0  0  0  0  0  0  0  0  0\n", x, y, 0.0, mol_symbol(&d, i)));
    }
    for b in &d.bonds {
        s.push_str(&format!("{:>3}{:>3}{:>3}{:>3}\n", b.a + 1, b.b + 1, b.order, stereo_code(b)));
    }
    let charged: Vec<(usize, i8)> = d.atoms.iter().enumerate().filter(|(_, a)| a.charge != 0).map(|(i, a)| (i, a.charge)).collect();
    for chunk in charged.chunks(8) {
        s.push_str(&format!("M  CHG{:>3}", chunk.len()));
        for (i, c) in chunk {
            s.push_str(&format!(" {:>3} {:>3}", i + 1, c));
        }
        s.push('\n');
    }
    let iso: Vec<(usize, u16)> = d.atoms.iter().enumerate().filter(|(_, a)| a.isotope != 0).map(|(i, a)| (i, a.isotope)).collect();
    for chunk in iso.chunks(8) {
        s.push_str(&format!("M  ISO{:>3}", chunk.len()));
        for (i, m) in chunk {
            s.push_str(&format!(" {:>3} {:>3}", i + 1, m));
        }
        s.push('\n');
    }
    let rad: Vec<(usize, u8)> = d.atoms.iter().enumerate().filter(|(_, a)| a.radical != 0).map(|(i, a)| (i, if a.radical == 1 { 2 } else { 1 })).collect();
    for chunk in rad.chunks(8) {
        s.push_str(&format!("M  RAD{:>3}", chunk.len()));
        for (i, m) in chunk {
            s.push_str(&format!(" {:>3} {:>3}", i + 1, m));
        }
        s.push('\n');
    }
    s.push_str("M  END\n");
    s
}

pub fn write_molfile_v3000(d0: &Doc, title: &str) -> String {
    let d = expand_labels(d0);
    let mut s = String::new();
    s.push_str(title);
    s.push_str("\n  Bondwrite 2D\n\n  0  0  0     0  0            999 V3000\n");
    s.push_str(&v3000_ctab(&d));
    s.push_str("M  END\n");
    s
}

fn v3000_ctab(d: &Doc) -> String {
    let c = mol_center(d);
    let mut s = String::new();
    s.push_str("M  V30 BEGIN CTAB\n");
    s.push_str(&format!("M  V30 COUNTS {} {} 0 0 0\n", d.atoms.len(), d.bonds.len()));
    s.push_str("M  V30 BEGIN ATOM\n");
    for (i, a) in d.atoms.iter().enumerate() {
        let x = (a.pos.x - c.x) * MOL_SCALE;
        let y = -(a.pos.y - c.y) * MOL_SCALE;
        let mut line = format!("M  V30 {} {} {:.4} {:.4} 0 0", i + 1, mol_symbol(d, i), x, y);
        if a.charge != 0 {
            line.push_str(&format!(" CHG={}", a.charge));
        }
        if a.isotope != 0 {
            line.push_str(&format!(" MASS={}", a.isotope));
        }
        if a.radical != 0 {
            line.push_str(&format!(" RAD={}", if a.radical == 1 { 2 } else { 1 }));
        }
        s.push_str(&line);
        s.push('\n');
    }
    s.push_str("M  V30 END ATOM\nM  V30 BEGIN BOND\n");
    for (k, b) in d.bonds.iter().enumerate() {
        let cfg = match b.stereo {
            Stereo::Wedge => " CFG=1",
            Stereo::Wavy => " CFG=2",
            Stereo::Hash => " CFG=3",
            _ => "",
        };
        s.push_str(&format!("M  V30 {} {} {} {}{}\n", k + 1, b.order, b.a + 1, b.b + 1, cfg));
    }
    s.push_str("M  V30 END BOND\nM  V30 END CTAB\n");
    s
}

fn mol_center(d: &Doc) -> egui::Pos2 {
    d.bounds_of(None).map(|r| r.center()).unwrap_or_default()
}

fn atom_from_symbol(sym: &str, pos: egui::Pos2) -> Atom {
    let mut a = Atom::new(pos, elements::C);
    match elements::by_symbol(sym) {
        Some(z) => a.el = z,
        None => {
            a.el = 0;
            a.label = Some(sym.to_string());
        }
    }
    a
}

/// Read a molfile (V2000 or V3000). Coordinates are rescaled to the drawing bond length.
pub fn read_molfile(text: &str) -> Result<Doc, String> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 4 {
        return Err("molfile too short".into());
    }
    let counts = lines[3];
    let mut d = Doc::default();
    let mut raw: Vec<(f32, f32)> = Vec::new();
    if counts.contains("V3000") {
        let mut in_atoms = false;
        let mut in_bonds = false;
        let mut joined: Vec<String> = Vec::new();
        let mut cont = String::new();
        for l in &lines[4..] {
            if let Some(rest) = l.strip_prefix("M  V30 ") {
                if let Some(r) = rest.strip_suffix('-') {
                    cont.push_str(r);
                    continue;
                }
                cont.push_str(rest);
                joined.push(std::mem::take(&mut cont));
            }
        }
        for l in joined {
            let t = l.trim();
            if t.starts_with("BEGIN ATOM") { in_atoms = true; continue; }
            if t.starts_with("END ATOM") { in_atoms = false; continue; }
            if t.starts_with("BEGIN BOND") { in_bonds = true; continue; }
            if t.starts_with("END BOND") { in_bonds = false; continue; }
            let f: Vec<&str> = t.split_whitespace().collect();
            if in_atoms && f.len() >= 5 {
                let x: f32 = f[2].parse().unwrap_or(0.0);
                let y: f32 = f[3].parse().unwrap_or(0.0);
                raw.push((x, y));
                let mut a = atom_from_symbol(f[1], pos2(0.0, 0.0));
                for kv in &f[5..] {
                    if let Some(v) = kv.strip_prefix("CHG=") { a.charge = v.parse().unwrap_or(0); }
                    if let Some(v) = kv.strip_prefix("MASS=") { a.isotope = v.parse().unwrap_or(0); }
                    if let Some(v) = kv.strip_prefix("RAD=") { a.radical = if v == "2" { 1 } else { 2 }; }
                }
                d.atoms.push(a);
            } else if in_bonds && f.len() >= 4 {
                let order: u8 = f[1].parse().unwrap_or(1);
                let a: usize = f[2].parse::<usize>().map_err(|_| "bad bond")? - 1;
                let b: usize = f[3].parse::<usize>().map_err(|_| "bad bond")? - 1;
                let mut st = Stereo::None;
                for kv in &f[4..] {
                    if let Some(v) = kv.strip_prefix("CFG=") {
                        st = match v { "1" => Stereo::Wedge, "2" => Stereo::Wavy, "3" => Stereo::Hash, _ => Stereo::None };
                    }
                }
                if a < d.atoms.len() && b < d.atoms.len() {
                    d.bonds.push(Bond { a, b, order: order.clamp(1, 3), stereo: st, dbl: DblPos::Auto, color: None, extra: None });
                }
            }
        }
    } else {
        let na: usize = counts.get(0..3).unwrap_or("0").trim().parse().map_err(|_| "bad counts line")?;
        let nb: usize = counts.get(3..6).unwrap_or("0").trim().parse().map_err(|_| "bad counts line")?;
        for k in 0..na {
            let l = lines.get(4 + k).ok_or("missing atom line")?;
            let x: f32 = l.get(0..10).unwrap_or("0").trim().parse().unwrap_or(0.0);
            let y: f32 = l.get(10..20).unwrap_or("0").trim().parse().unwrap_or(0.0);
            let sym = l.get(31..34).unwrap_or("C").trim();
            raw.push((x, y));
            let mut a = atom_from_symbol(sym, pos2(0.0, 0.0));
            let cc: i32 = l.get(36..39).unwrap_or("0").trim().parse().unwrap_or(0);
            if cc != 0 && cc != 4 {
                a.charge = (4 - cc) as i8;
            }
            d.atoms.push(a);
        }
        for k in 0..nb {
            let l = lines.get(4 + na + k).ok_or("missing bond line")?;
            let a: usize = l.get(0..3).unwrap_or("0").trim().parse::<usize>().map_err(|_| "bad bond")?;
            let b: usize = l.get(3..6).unwrap_or("0").trim().parse::<usize>().map_err(|_| "bad bond")?;
            let order: u8 = l.get(6..9).unwrap_or("1").trim().parse().unwrap_or(1);
            let st: u8 = l.get(9..12).unwrap_or("0").trim().parse().unwrap_or(0);
            if a == 0 || b == 0 || a > na || b > na {
                continue;
            }
            let stereo = match st { 1 => Stereo::Wedge, 6 => Stereo::Hash, 4 => Stereo::Wavy, _ => Stereo::None };
            d.bonds.push(Bond { a: a - 1, b: b - 1, order: if order == 4 { 1 } else { order.clamp(1, 3) }, stereo, dbl: DblPos::Auto, color: None, extra: None });
        }
        let mut charges_reset = false;
        for l in &lines[4 + na + nb..] {
            if l.starts_with("M  END") {
                break;
            }
            let key = l.get(0..6).unwrap_or("");
            if key == "M  CHG" || key == "M  ISO" || key == "M  RAD" {
                if key == "M  CHG" && !charges_reset {
                    for a in &mut d.atoms {
                        a.charge = 0;
                    }
                    charges_reset = true;
                }
                let f: Vec<&str> = l[6..].split_whitespace().collect();
                let n: usize = f.first().and_then(|x| x.parse().ok()).unwrap_or(0);
                for k in 0..n {
                    let idx: usize = f.get(1 + 2 * k).and_then(|x| x.parse().ok()).unwrap_or(0);
                    let v: i32 = f.get(2 + 2 * k).and_then(|x| x.parse().ok()).unwrap_or(0);
                    if idx == 0 || idx > d.atoms.len() {
                        continue;
                    }
                    let a = &mut d.atoms[idx - 1];
                    match key {
                        "M  CHG" => a.charge = v as i8,
                        "M  ISO" => a.isotope = v as u16,
                        _ => a.radical = if v == 2 { 1 } else if v > 0 { 2 } else { 0 },
                    }
                }
            }
        }
    }
    place_raw(&mut d, &raw);
    Ok(d)
}

/// Turn file coordinates (y up) into drawing coordinates with the usual bond length.
fn place_raw(d: &mut Doc, raw: &[(f32, f32)]) {
    let mut lens: Vec<f32> = d
        .bonds
        .iter()
        .map(|b| {
            let (x1, y1) = raw[b.a];
            let (x2, y2) = raw[b.b];
            ((x1 - x2).powi(2) + (y1 - y2).powi(2)).sqrt()
        })
        .filter(|l| *l > 1e-4)
        .collect();
    lens.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let all_zero = raw.iter().all(|(x, y)| x.abs() < 1e-4 && y.abs() < 1e-4);
    let med = lens.get(lens.len() / 2).copied().unwrap_or(1.5);
    let s = BOND_LEN / med;
    for (k, a) in d.atoms.iter_mut().enumerate() {
        a.pos = pos2(raw[k].0 * s, -raw[k].1 * s);
    }
    if all_zero && d.atoms.len() > 1 {
        chem::layout(d);
    }
}

/// Read an SD file: every record, placed side by side.
pub fn read_sdf(text: &str) -> Result<Doc, String> {
    let mut out = Doc::default();
    let mut x = 0.0;
    for rec in text.split("$$$$") {
        if rec.trim().is_empty() {
            continue;
        }
        let rec = rec.trim_start_matches(['\r', '\n']);
        let d = read_molfile(rec)?;
        let b = d.bounds_of(None).unwrap_or(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(0.0, 0.0)));
        out.merge(&d, vec2(x - b.min.x, -b.center().y));
        x += b.width() + BOND_LEN * 2.0;
    }
    if out.atoms.is_empty() {
        return Err("no structures found".into());
    }
    Ok(out)
}

pub fn write_sdf(d: &Doc) -> String {
    let mut s = String::new();
    for f in d.fragments() {
        let sel = Selection { atoms: f.into_iter().collect(), ..Default::default() };
        let part = d.extract(&sel);
        s.push_str(&write_molfile_v2000(&part, ""));
        s.push_str("$$$$\n");
    }
    s
}

// ---------------------------------------------------------------------------
// Reactions: fragments left of the first arrow are reactants, right are products.

pub fn split_reaction(d: &Doc) -> (Vec<Doc>, Vec<Doc>) {
    let frags = d.fragments();
    let arrow = d.arrows.iter().find(|a| a.kind != ArrowKind::Line && a.kind != ArrowKind::Curved);
    let mut re = Vec::new();
    let mut pr = Vec::new();
    for f in frags {
        let c = chem::centroid(d, &f);
        let sel = Selection { atoms: f.into_iter().collect(), ..Default::default() };
        let part = d.extract(&sel);
        let is_product = match arrow {
            Some(a) => {
                let dir = (a.to - a.from).normalized();
                let mid = a.from + (a.to - a.from) * 0.5;
                (c - mid).dot(dir) > 0.0
            }
            None => false,
        };
        if is_product { pr.push(part) } else { re.push(part) }
    }
    (re, pr)
}

pub fn write_rxn_v2000(d: &Doc) -> String {
    let (re, pr) = split_reaction(d);
    let mut s = String::from("$RXN\n\n  Bondwrite\n\n");
    s.push_str(&format!("{:>3}{:>3}\n", re.len(), pr.len()));
    for m in re.iter().chain(pr.iter()) {
        s.push_str("$MOL\n");
        s.push_str(&write_molfile_v2000(m, ""));
    }
    s
}

pub fn write_rxn_v3000(d: &Doc) -> String {
    let (re, pr) = split_reaction(d);
    let mut s = String::from("$RXN V3000\n\n  Bondwrite\n\n");
    s.push_str(&format!("M  V30 COUNTS {} {}\n", re.len(), pr.len()));
    s.push_str("M  V30 BEGIN REACTANT\n");
    for m in &re {
        s.push_str(&v3000_ctab(&expand_labels(m)));
    }
    s.push_str("M  V30 END REACTANT\nM  V30 BEGIN PRODUCT\n");
    for m in &pr {
        s.push_str(&v3000_ctab(&expand_labels(m)));
    }
    s.push_str("M  V30 END PRODUCT\nM  END\n");
    s
}

fn lay_out_reaction(re: Vec<Doc>, pr: Vec<Doc>) -> Doc {
    let mut out = Doc::default();
    let mut x = 0.0;
    let place = |out: &mut Doc, m: &Doc, x: &mut f32| {
        let b = m.bounds_of(None).unwrap_or(egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(0.0, 0.0)));
        out.merge(m, vec2(*x - b.min.x, -b.center().y));
        *x += b.width() + BOND_LEN;
    };
    for (k, m) in re.iter().enumerate() {
        if k > 0 {
            out.captions.push(Caption { pos: pos2(x - BOND_LEN * 0.75, -8.0), text: "+".into(), size: 14.0, bold: false, italic: false, serif: false, formula: false, color: None, extra: None, seq: None });
        }
        place(&mut out, m, &mut x);
    }
    out.arrows.push(Arrow { from: pos2(x, 0.0), to: pos2(x + BOND_LEN * 2.5, 0.0), kind: ArrowKind::Reaction, color: None, extra: None, style: Default::default() });
    x += BOND_LEN * 3.5;
    for (k, m) in pr.iter().enumerate() {
        if k > 0 {
            out.captions.push(Caption { pos: pos2(x - BOND_LEN * 0.75, -8.0), text: "+".into(), size: 14.0, bold: false, italic: false, serif: false, formula: false, color: None, extra: None, seq: None });
        }
        place(&mut out, m, &mut x);
    }
    out
}

pub fn read_rxn(text: &str) -> Result<Doc, String> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() || !lines[0].starts_with("$RXN") {
        return Err("not a reaction file".into());
    }
    if lines[0].contains("V3000") {
        // Split into CTAB blocks.
        let mut re = Vec::new();
        let mut pr = Vec::new();
        let mut section = 0;
        let mut block: Vec<String> = Vec::new();
        for l in &lines {
            let t = l.trim();
            if t.contains("BEGIN REACTANT") { section = 1; continue; }
            if t.contains("BEGIN PRODUCT") { section = 2; continue; }
            if t.contains("BEGIN AGENT") { section = 3; continue; }
            if t.contains("BEGIN CTAB") { block.clear(); }
            block.push(l.to_string());
            if t.contains("END CTAB") {
                let molfile = format!("\n\n\n  0  0  0     0  0            999 V3000\n{}\nM  END\n", block.join("\n"));
                let m = read_molfile(&molfile)?;
                match section { 1 => re.push(m), 2 => pr.push(m), _ => {} }
                block.clear();
            }
        }
        return Ok(lay_out_reaction(re, pr));
    }
    let counts = lines.get(4).ok_or("missing counts")?;
    let nr: usize = counts.get(0..3).unwrap_or("0").trim().parse().unwrap_or(0);
    let np: usize = counts.get(3..6).unwrap_or("0").trim().parse().unwrap_or(0);
    let mols: Vec<&str> = text.split("$MOL").skip(1).collect();
    let mut re = Vec::new();
    let mut pr = Vec::new();
    for (k, m) in mols.iter().enumerate() {
        let m = m.strip_prefix('\n').or_else(|| m.strip_prefix("\r\n")).unwrap_or(m);
        let d = read_molfile(m)?;
        if k < nr { re.push(d) } else if k < nr + np { pr.push(d) }
    }
    Ok(lay_out_reaction(re, pr))
}

// ---------------------------------------------------------------------------
// CML

pub fn write_cml(d0: &Doc) -> String {
    let d = expand_labels(d0);
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<cml xmlns=\"http://www.xml-cml.org/schema\">\n");
    for (fi, f) in d.fragments().iter().enumerate() {
        s.push_str(&format!(" <molecule id=\"m{}\">\n  <atomArray>\n", fi + 1));
        for &i in f {
            let a = &d.atoms[i];
            let sym = if a.el == 0 { "R" } else { elements::symbol(a.el) };
            let mut extra = String::new();
            if a.charge != 0 {
                extra.push_str(&format!(" formalCharge=\"{}\"", a.charge));
            }
            if a.isotope != 0 {
                extra.push_str(&format!(" isotopeNumber=\"{}\"", a.isotope));
            }
            extra.push_str(&format!(" hydrogenCount=\"{}\"", d.implicit_h(i)));
            s.push_str(&format!(
                "   <atom id=\"a{}\" elementType=\"{}\" x2=\"{:.4}\" y2=\"{:.4}\"{}/>\n",
                i + 1, sym, a.pos.x * MOL_SCALE, -a.pos.y * MOL_SCALE, extra
            ));
        }
        s.push_str("  </atomArray>\n  <bondArray>\n");
        for (bi, b) in d.bonds.iter().enumerate() {
            if !f.contains(&b.a) {
                continue;
            }
            let st = match b.stereo { Stereo::Wedge => "<bondStereo>W</bondStereo>", Stereo::Hash => "<bondStereo>H</bondStereo>", _ => "" };
            if st.is_empty() {
                s.push_str(&format!("   <bond id=\"b{}\" atomRefs2=\"a{} a{}\" order=\"{}\"/>\n", bi + 1, b.a + 1, b.b + 1, b.order));
            } else {
                s.push_str(&format!("   <bond id=\"b{}\" atomRefs2=\"a{} a{}\" order=\"{}\">{}</bond>\n", bi + 1, b.a + 1, b.b + 1, b.order, st));
            }
        }
        s.push_str("  </bondArray>\n </molecule>\n");
    }
    s.push_str("</cml>\n");
    s
}

pub fn read_cml(text: &str) -> Result<Doc, String> {
    let root = xml::parse(text)?;
    let mut d = Doc::default();
    let mut raw = Vec::new();
    let mut ids: HashMap<String, usize> = HashMap::new();
    fn walk<'a>(n: &'a xml::Node, out: &mut Vec<&'a xml::Node>, name: &str) {
        if n.local() == name {
            out.push(n);
        }
        for c in n.elements() {
            walk(c, out, name);
        }
    }
    let mut atoms = Vec::new();
    walk(&root, &mut atoms, "atom");
    for a in atoms {
        let sym = a.attr("elementType").unwrap_or("C");
        let x: f32 = a.attr("x2").or(a.attr("x3")).and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let y: f32 = a.attr("y2").or(a.attr("y3")).and_then(|v| v.parse().ok()).unwrap_or(0.0);
        let mut at = atom_from_symbol(sym, pos2(0.0, 0.0));
        at.charge = a.attr("formalCharge").and_then(|v| v.parse().ok()).unwrap_or(0);
        at.isotope = a.attr("isotopeNumber").and_then(|v| v.parse().ok()).unwrap_or(0);
        if let Some(id) = a.attr("id") {
            ids.insert(id.to_string(), d.atoms.len());
        }
        raw.push((x, y));
        d.atoms.push(at);
    }
    let mut bonds = Vec::new();
    walk(&root, &mut bonds, "bond");
    for b in bonds {
        let refs: Vec<&str> = b.attr("atomRefs2").unwrap_or("").split_whitespace().collect();
        if refs.len() != 2 {
            continue;
        }
        let (Some(&x), Some(&y)) = (ids.get(refs[0]), ids.get(refs[1])) else { continue };
        let order = match b.attr("order").unwrap_or("1") {
            "2" | "D" | "d" => 2,
            "3" | "T" | "t" => 3,
            _ => 1,
        };
        let st = b.elements().find(|c| c.local() == "bondStereo").map(|c| c.text());
        let stereo = match st.as_deref() { Some("W") => Stereo::Wedge, Some("H") => Stereo::Hash, _ => Stereo::None };
        d.bonds.push(Bond { a: x, b: y, order, stereo, dbl: DblPos::Auto, color: None, extra: None });
    }
    if d.atoms.is_empty() {
        return Err("no atoms in CML".into());
    }
    place_raw(&mut d, &raw);
    Ok(d)
}
