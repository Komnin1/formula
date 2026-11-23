//! Chemistry: label formulas, molecular formula, masses, isotope pattern, rings,
//! aromaticity, 2D layout and structure clean-up.

use crate::doc::{BOND_LEN, Doc, rot};
use crate::elements;
use egui::{Pos2, Vec2, pos2, vec2};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};

pub type Formula = BTreeMap<u8, i32>;

/// Group abbreviations: (label, group formula, SMILES with the attachment atom first).
pub const ABBREVIATIONS: &[(&str, &str, &str)] = &[
    ("Me", "CH3", "C"),
    ("Et", "C2H5", "CC"),
    ("Pr", "C3H7", "CCC"),
    ("nPr", "C3H7", "CCC"),
    ("iPr", "C3H7", "C(C)C"),
    ("Bu", "C4H9", "CCCC"),
    ("nBu", "C4H9", "CCCC"),
    ("iBu", "C4H9", "CC(C)C"),
    ("sBu", "C4H9", "C(C)CC"),
    ("tBu", "C4H9", "C(C)(C)C"),
    ("Ph", "C6H5", "c1ccccc1"),
    ("PPh3", "C18H15P", "P(c1ccccc1)(c1ccccc1)c1ccccc1"),
    ("P(Ph)3", "C18H15P", "P(c1ccccc1)(c1ccccc1)c1ccccc1"),
    ("PPh2", "C12H10P", "P(c1ccccc1)c1ccccc1"),
    ("P(Ph)2", "C12H10P", "P(c1ccccc1)c1ccccc1"),
    ("Bn", "C7H7", "Cc1ccccc1"),
    ("Cy", "C6H11", "C1CCCCC1"),
    ("Ac", "C2H3O", "C(C)=O"),
    ("Bz", "C7H5O", "C(=O)c1ccccc1"),
    ("Boc", "C5H9O2", "C(=O)OC(C)(C)C"),
    ("Cbz", "C8H7O2", "C(=O)OCc1ccccc1"),
    ("Ts", "C7H7O2S", "S(=O)(=O)c1ccc(C)cc1"),
    ("Ms", "CH3O2S", "S(C)(=O)=O"),
    ("Tf", "CF3O2S", "S(=O)(=O)C(F)(F)F"),
    ("TMS", "C3H9Si", "[Si](C)(C)C"),
    ("TBS", "C6H15Si", "[Si](C)(C)C(C)(C)C"),
    ("TBDMS", "C6H15Si", "[Si](C)(C)C(C)(C)C"),
];

/// Common written groups that can be expanded into atoms.
pub const GROUP_SMILES: &[(&str, &str)] = &[
    ("OH", "O"),
    ("NH2", "N"),
    ("SH", "S"),
    ("CH3", "C"),
    ("OMe", "OC"),
    ("OEt", "OCC"),
    ("OAc", "OC(C)=O"),
    ("OBn", "OCc1ccccc1"),
    ("OPh", "Oc1ccccc1"),
    ("OTf", "OS(=O)(=O)C(F)(F)F"),
    ("OTs", "OS(=O)(=O)c1ccc(C)cc1"),
    ("OCF3", "OC(F)(F)F"),
    ("NHAc", "NC(C)=O"),
    ("NHBoc", "NC(=O)OC(C)(C)C"),
    ("NMe2", "N(C)C"),
    ("NEt2", "N(CC)CC"),
    ("NHMe", "NC"),
    ("CO2H", "C(=O)O"),
    ("COOH", "C(=O)O"),
    ("CO2Me", "C(=O)OC"),
    ("COOMe", "C(=O)OC"),
    ("CO2Et", "C(=O)OCC"),
    ("COOEt", "C(=O)OCC"),
    ("CONH2", "C(N)=O"),
    ("COMe", "C(C)=O"),
    ("CHO", "C=O"),
    ("CN", "C#N"),
    ("NO2", "[N+](=O)[O-]"),
    ("CF3", "C(F)(F)F"),
    ("CCl3", "C(Cl)(Cl)Cl"),
    ("SO3H", "S(=O)(=O)O"),
    ("SO2Me", "S(C)(=O)=O"),
    ("SMe", "SC"),
    ("CH2OH", "CO"),
];

pub fn expansion_smiles(label: &str) -> Option<&'static str> {
    ABBREVIATIONS
        .iter()
        .find(|a| a.0 == label)
        .map(|a| a.2)
        .or_else(|| GROUP_SMILES.iter().find(|g| g.0 == label).map(|g| g.1))
}

/// One piece of a written label: text and how many times it repeats.
#[derive(Clone, Debug)]
pub struct LabelToken {
    pub text: String,
    pub count: u32,
}

/// Split a label such as "CO2Me" into tokens [C][O2][Me]. None if it can't be read.
pub fn tokenize_label(s: &str) -> Option<Vec<LabelToken>> {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        if c == '(' || c == ')' {
            // Keep parentheses as their own tokens.
            let mut j = i + 1;
            let mut num = String::new();
            if c == ')' {
                while j < chars.len() && chars[j].is_ascii_digit() {
                    num.push(chars[j]);
                    j += 1;
                }
            }
            out.push(LabelToken { text: c.to_string(), count: num.parse().unwrap_or(1) });
            i = j;
            continue;
        }
        let rest: String = chars[i..].iter().collect();
        let mut matched = None;
        // Longest abbreviation first.
        let mut abbrs: Vec<&str> = ABBREVIATIONS.iter().map(|a| a.0).collect();
        abbrs.sort_by_key(|a| std::cmp::Reverse(a.len()));
        for a in abbrs {
            if rest.starts_with(a) {
                // Don't read "Pr" in "Pr" element context etc. – abbreviations win.
                matched = Some(a.to_string());
                break;
            }
        }
        if matched.is_none() {
            if c.is_ascii_uppercase() {
                if i + 1 < chars.len() && chars[i + 1].is_ascii_lowercase() {
                    let two: String = chars[i..i + 2].iter().collect();
                    if elements::by_symbol(&two).is_some() {
                        matched = Some(two);
                    }
                }
                if matched.is_none() {
                    let one = c.to_string();
                    // D and T: deuterium and tritium.
                    if elements::by_symbol(&one).is_some() || c == 'D' || c == 'T' {
                        matched = Some(one);
                    }
                }
            }
        }
        let m = matched?;
        i += m.chars().count();
        let mut num = String::new();
        while i < chars.len() && chars[i].is_ascii_digit() {
            num.push(chars[i]);
            i += 1;
        }
        out.push(LabelToken { text: m, count: num.parse().unwrap_or(1) });
    }
    if out.is_empty() { None } else { Some(out) }
}

/// Formula of a written label (hydrogens as written; D and T count as hydrogen).
pub fn label_formula(s: &str) -> Option<Formula> {
    label_formula_iso(s, false)
}

/// Same, with D and T kept as the isotopes ²H and ³H (`iso`), for masses.
pub fn label_formula_iso(s: &str, iso: bool) -> Option<Formula> {
    let toks = tokenize_label(s)?;
    let mut stack: Vec<Formula> = vec![Formula::new()];
    for t in toks {
        match t.text.as_str() {
            "(" => stack.push(Formula::new()),
            ")" => {
                let g = stack.pop()?;
                let top = stack.last_mut()?;
                for (k, v) in g {
                    *top.entry(k).or_default() += v * t.count as i32;
                }
            }
            x => {
                let f = if let Some(a) = ABBREVIATIONS.iter().find(|a| a.0 == x) {
                    label_formula_iso(a.1, iso)?
                } else if x == "D" || x == "T" {
                    let mass = if x == "D" { 2 } else { 3 };
                    let k = if iso { elements::iso_key(elements::H, mass).unwrap_or(elements::H) } else { elements::H };
                    let mut f = Formula::new();
                    f.insert(k, 1);
                    f
                } else {
                    let z = elements::by_symbol(x)?;
                    let mut f = Formula::new();
                    f.insert(z, 1);
                    f
                };
                let top = stack.last_mut()?;
                for (k, v) in f {
                    *top.entry(k).or_default() += v * t.count as i32;
                }
            }
        }
    }
    if stack.len() != 1 {
        return None;
    }
    stack.pop()
}

/// The label written the other way round, for groups attached on their right ("CO2H" -> "HO2C").
pub fn reversed_label(s: &str) -> String {
    match tokenize_label(s) {
        Some(t) if !t.iter().any(|x| x.text == "(" || x.text == ")") => t
            .iter()
            .rev()
            .map(|x| if x.count > 1 { format!("{}{}", x.text, x.count) } else { x.text.clone() })
            .collect(),
        _ => s.to_string(),
    }
}

/// Main element of a label (for colour and file export), e.g. "OMe" -> O.
pub fn label_main_element(s: &str) -> u8 {
    tokenize_label(s)
        .and_then(|t| t.first().and_then(|x| elements::by_symbol(&x.text)))
        .unwrap_or(0)
}

pub fn atoms_formula(doc: &Doc, atoms: &[usize]) -> (Formula, i32) {
    let mut f = Formula::new();
    let mut charge = 0;
    for &i in atoms {
        let a = &doc.atoms[i];
        charge += a.charge as i32;
        // A monomer of a biopolymer: its residue formula (the caps are atoms of their own).
        if let Some(m) = a.monomer.as_ref().filter(|m| m.role == crate::doc::MonomerRole::Unit) {
            if let Some(lf) = label_formula(&m.formula) {
                for (k, v) in lf {
                    *f.entry(k).or_default() += v;
                }
            }
            continue;
        }
        if let Some(l) = &a.label {
            if let Some(mut lf) = label_formula_iso(l, true) {
                // An isotope set on a label applies to its first element ("¹³CH3").
                if a.isotope != 0 {
                    let z = label_main_element(l);
                    if let (Some(k), Some(n)) = (elements::iso_key(z, a.isotope), lf.get(&z).copied()) {
                        if n > 0 {
                            *lf.entry(z).or_default() -= 1;
                            *lf.entry(k).or_default() += 1;
                        }
                    }
                }
                for (k, v) in lf {
                    *f.entry(k).or_default() += v;
                }
            }
            continue;
        }
        if a.el == 0 {
            continue;
        }
        // A labelled atom (¹³C, ¹⁵N, ²H...) is counted as its isotope.
        let key = if a.isotope != 0 { elements::iso_key(a.el, a.isotope).unwrap_or(a.el) } else { a.el };
        *f.entry(key).or_default() += 1;
        let h = doc.implicit_h(i);
        if h > 0 {
            *f.entry(elements::H).or_default() += h;
        }
    }
    f.retain(|_, v| *v != 0);
    (f, charge)
}

/// Hill order: C, H, then alphabetical (alphabetical throughout without carbon). A labelled
/// isotope follows its element ("C₈¹³CH₁₂").
pub fn hill_order(f: &Formula) -> Vec<(u8, i32)> {
    let mut v: Vec<(u8, i32)> = f.iter().map(|(k, v)| (*k, *v)).collect();
    let has_c = f.keys().any(|k| elements::key_element(*k) == elements::C);
    v.sort_by(|a, b| {
        let key = |k: u8| -> (u8, &'static str, u16) {
            let z = elements::key_element(k);
            let iso = elements::iso_of_key(k).map(|x| x.1).unwrap_or(0);
            if has_c && z == elements::C {
                (0, "", iso)
            } else if has_c && z == elements::H {
                (1, "", iso)
            } else {
                (2, elements::symbol(z), iso)
            }
        };
        key(a.0).cmp(&key(b.0))
    });
    v
}

pub fn formula_string(f: &Formula, charge: i32) -> String {
    let mut s = String::new();
    for (z, n) in hill_order(f) {
        s.push_str(&elements::key_symbol(z));
        if n != 1 {
            s.push_str(&n.to_string());
        }
    }
    if charge != 0 {
        if charge.abs() > 1 {
            s.push_str(&charge.abs().to_string());
        }
        s.push(if charge > 0 { '+' } else { '-' });
    }
    s
}

pub const ELECTRON: f64 = 0.000548579909;

pub fn mol_weight(f: &Formula) -> f64 {
    f.iter().map(|(z, n)| elements::key_mass(*z) * *n as f64).sum()
}

pub fn exact_mass(f: &Formula, charge: i32) -> f64 {
    f.iter().map(|(z, n)| elements::key_mono(*z) * *n as f64).sum::<f64>() - charge as f64 * ELECTRON
}

/// Isotope pattern: (m/z, relative intensity %) for the strongest peaks.
pub fn isotope_pattern(f: &Formula, charge: i32) -> Vec<(f64, f64)> {
    isotope_peaks(f, charge, 0.5, 6)
}

/// The ion a mass spectrometer measures: a charged molecule as it is (m/z = mass / |z|), a
/// neutral one with a proton added ([M+H]⁺, positive electrospray).
pub struct Ion {
    pub label: String,
    pub formula: Formula,
    pub charge: i32,
}

fn superscript(n: i32) -> String {
    const SUP: [char; 10] = ['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
    let digits = if n.abs() > 1 { n.abs().to_string().chars().map(|c| SUP[c as usize - '0' as usize]).collect() } else { String::new() };
    format!("{}{}", digits, if n > 0 { '⁺' } else { '⁻' })
}

pub fn ms_ion(f: &Formula, charge: i32) -> Ion {
    if charge != 0 {
        return Ion { label: format!("M{}", superscript(charge)), formula: f.clone(), charge };
    }
    let mut g = f.clone();
    *g.entry(elements::H).or_default() += 1;
    Ion { label: "[M+H]⁺".into(), formula: g, charge: 1 }
}

impl Ion {
    /// Monoisotopic m/z.
    pub fn mz(&self) -> f64 {
        exact_mass(&self.formula, self.charge) / self.charge.abs().max(1) as f64
    }
    /// Predicted spectrum: the isotope peaks down to 0.1 % of the strongest.
    pub fn spectrum(&self) -> Vec<(f64, f64)> {
        isotope_peaks(&self.formula, self.charge, 0.1, 16)
    }
}

/// Isotope peaks (m/z, % of the strongest) at least `min_pct` high, the `max` strongest, by m/z.
pub fn isotope_peaks(f: &Formula, charge: i32, min_pct: f64, max: usize) -> Vec<(f64, f64)> {
    // Distribution by nominal mass offset: (probability, weighted mass sum).
    let mut dist: Vec<(f64, f64)> = vec![(1.0, 0.0)];
    for (&z, &n) in f {
        if n <= 0 {
            continue;
        }
        // A labelled atom is taken as fully enriched in its isotope.
        let iso = match elements::iso_of_key(z) {
            Some(x) => vec![(x.2, 100.0)],
            None => elements::isotopes(z),
        };
        let base = iso[0].0.round();
        let el_dist: Vec<(usize, f64, f64)> = iso
            .iter()
            .map(|(m, ab)| (((m.round() - base).max(0.0)) as usize, ab / 100.0, *m))
            .collect();
        // Room for the heaviest isotope (selenium's span 8 mass units).
        let span = el_dist.iter().map(|x| x.0).max().unwrap_or(0) + 1;
        for _ in 0..n {
            let mut next: Vec<(f64, f64)> = vec![(0.0, 0.0); dist.len() + span];
            for (k, (p, ms)) in dist.iter().enumerate() {
                if *p < 1e-12 {
                    continue;
                }
                let mean = if *p > 0.0 { ms / p } else { 0.0 };
                for (off, pa, ma) in &el_dist {
                    let np = p * pa;
                    next[k + off].0 += np;
                    next[k + off].1 += np * (mean + ma);
                }
            }
            while next.len() > 1 && next.last().unwrap().0 < 1e-9 {
                next.pop();
            }
            dist = next;
        }
    }
    let maxp = dist.iter().map(|d| d.0).fold(0.0, f64::max);
    if maxp <= 0.0 {
        return vec![];
    }
    let q = charge.abs().max(1) as f64;
    let mut out: Vec<(f64, f64)> = dist
        .iter()
        .filter(|d| d.0 / maxp >= min_pct / 100.0)
        .map(|(p, ms)| ((ms / p - charge as f64 * ELECTRON) / q, p / maxp * 100.0))
        .collect();
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    out.truncate(max);
    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    out
}

pub fn elemental_analysis(f: &Formula) -> Vec<(u8, f64)> {
    let mw = mol_weight(f);
    if mw <= 0.0 {
        return vec![];
    }
    // By element: a labelled isotope counts towards its element.
    let mut out: Vec<(u8, f64)> = Vec::new();
    for (k, n) in hill_order(f) {
        let z = elements::key_element(k);
        let pct = elements::key_mass(k) * n as f64 / mw * 100.0;
        match out.iter_mut().find(|x| x.0 == z) {
            Some(x) => x.1 += pct,
            None => out.push((z, pct)),
        }
    }
    out
}

/// Smallest set of smallest rings, as lists of atoms in ring order.
pub fn sssr(doc: &Doc) -> Vec<Vec<usize>> {
    let adj = doc.adjacency();
    let nb = doc.bonds.len();
    let comps = doc.fragments().len();
    let target = (nb + comps).saturating_sub(doc.atoms.len());
    if target == 0 {
        return vec![];
    }
    let mut cands: Vec<(Vec<usize>, Vec<usize>)> = Vec::new(); // (atoms, bonds)
    let mut seen: HashSet<Vec<usize>> = HashSet::new();
    for (ei, e) in doc.bonds.iter().enumerate() {
        // Shortest path from e.a to e.b avoiding e.
        let mut prev: HashMap<usize, (usize, usize)> = HashMap::new();
        let mut q = VecDeque::new();
        q.push_back(e.a);
        prev.insert(e.a, (usize::MAX, usize::MAX));
        while let Some(u) = q.pop_front() {
            if u == e.b {
                break;
            }
            for &(v, bi) in &adj[u] {
                if bi == ei || prev.contains_key(&v) {
                    continue;
                }
                prev.insert(v, (u, bi));
                q.push_back(v);
            }
        }
        if !prev.contains_key(&e.b) {
            continue;
        }
        let mut atoms = vec![e.b];
        let mut bonds = vec![ei];
        let mut cur = e.b;
        while cur != e.a {
            let (p, bi) = prev[&cur];
            bonds.push(bi);
            atoms.push(p);
            cur = p;
        }
        let mut key = bonds.clone();
        key.sort();
        if seen.insert(key) {
            cands.push((atoms, bonds));
        }
    }
    cands.sort_by_key(|c| c.1.len());
    let words = nb.div_ceil(64).max(1);
    let mut basis: Vec<Vec<u64>> = Vec::new();
    let mut out = Vec::new();
    for (atoms, bonds) in cands {
        let mut v = vec![0u64; words];
        for b in &bonds {
            v[b / 64] ^= 1 << (b % 64);
        }
        // Reduce against basis (kept in echelon form by leading bit).
        let mut r = v.clone();
        for bv in &basis {
            let lead = leading_bit(bv);
            if let Some(l) = lead {
                if r[l / 64] >> (l % 64) & 1 == 1 {
                    for k in 0..words {
                        r[k] ^= bv[k];
                    }
                }
            }
        }
        if r.iter().any(|x| *x != 0) {
            basis.push(r);
            basis.sort_by_key(|b| std::cmp::Reverse(leading_bit(b)));
            out.push(atoms);
            if out.len() >= target {
                break;
            }
        }
    }
    out
}

fn leading_bit(v: &[u64]) -> Option<usize> {
    for k in (0..v.len()).rev() {
        if v[k] != 0 {
            return Some(k * 64 + 63 - v[k].leading_zeros() as usize);
        }
    }
    None
}

pub fn ring_bonds(doc: &Doc, rings: &[Vec<usize>]) -> HashSet<usize> {
    let mut s = HashSet::new();
    for r in rings {
        for k in 0..r.len() {
            if let Some(b) = doc.find_bond(r[k], r[(k + 1) % r.len()]) {
                s.insert(b);
            }
        }
    }
    s
}

/// Bonds that are part of aromatic rings (simple Hückel-style check on the drawn
/// alternating double bonds, plus 5-rings with a lone-pair heteroatom).
pub fn aromatic_bonds(doc: &Doc) -> HashSet<usize> {
    let rings = sssr(doc);
    let mut arom_rings: Vec<bool> = vec![false; rings.len()];
    let in_ring_double = |i: usize| -> bool {
        doc.bonds.iter().any(|b| (b.a == i || b.b == i) && b.order == 2 && {
            let o = b.other(i);
            rings.iter().any(|r| r.contains(&o) && r.contains(&i))
        })
    };
    for (k, r) in rings.iter().enumerate() {
        let n = r.len();
        let all_sp2 = r.iter().all(|&i| in_ring_double(i));
        if n == 6 && all_sp2 {
            arom_rings[k] = true;
        } else if n == 5 {
            let non: Vec<usize> = r.iter().copied().filter(|&i| !in_ring_double(i)).collect();
            if non.len() == 1 && matches!(doc.atoms[non[0]].el, 7 | 8 | 16 | 34) {
                arom_rings[k] = true;
            }
        }
    }
    let mut out = HashSet::new();
    for (k, r) in rings.iter().enumerate() {
        if arom_rings[k] {
            for j in 0..r.len() {
                if let Some(b) = doc.find_bond(r[j], r[(j + 1) % r.len()]) {
                    out.insert(b);
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Layout and clean-up

/// Give a structure (with no useful coordinates) a 2D drawing.
pub fn layout(doc: &mut Doc) {
    let n = doc.atoms.len();
    if n == 0 {
        return;
    }
    let rings = sssr(doc);
    let adj = doc.adjacency();
    let mut placed = vec![false; n];
    let mut ring_done = vec![false; rings.len()];
    let mut x_offset = 0.0f32;
    for comp in doc.fragments() {
        let start = comp
            .iter()
            .copied()
            .find(|i| rings.iter().any(|r| r.contains(i)))
            .unwrap_or(comp[0]);
        // Place the starting point (a whole ring if it is in one).
        let mut queue: VecDeque<usize> = VecDeque::new();
        if let Some(ri) = rings.iter().position(|r| r.contains(&start)) {
            place_polygon(doc, &rings[ri], None, &mut placed, pos2(x_offset, 0.0));
            ring_done[ri] = true;
            for &a in &rings[ri] {
                queue.push_back(a);
            }
        } else {
            doc.atoms[start].pos = pos2(x_offset, 0.0);
            placed[start] = true;
            queue.push_back(start);
        }
        let mut flip: HashMap<usize, bool> = HashMap::new();
        while let Some(u) = queue.pop_front() {
            // Rings through u not yet placed.
            for (ri, r) in rings.iter().enumerate() {
                if ring_done[ri] || !r.contains(&u) {
                    continue;
                }
                ring_done[ri] = true;
                place_ring_from(doc, r, &mut placed);
                for &a in r {
                    queue.push_back(a);
                }
            }
            // Chain neighbours.
            let todo: Vec<usize> = adj[u].iter().map(|x| x.0).filter(|v| !placed[*v]).collect();
            if todo.is_empty() {
                continue;
            }
            let p = doc.atoms[u].pos;
            let pdirs: Vec<Vec2> = adj[u]
                .iter()
                .filter(|x| placed[x.0])
                .map(|x| (doc.atoms[x.0].pos - p).normalized())
                .collect();
            let dirs = spread_dirs(doc, u, &pdirs, todo.len(), &mut flip);
            for (k, v) in todo.iter().enumerate() {
                doc.atoms[*v].pos = p + dirs[k] * BOND_LEN;
                placed[*v] = true;
                // Remember zig-zag parity.
                let f = *flip.get(&u).unwrap_or(&false);
                flip.insert(*v, !f);
                queue.push_back(*v);
            }
        }
        let comp_bounds = comp.iter().fold(egui::Rect::NOTHING, |mut r, &i| {
            r.extend_with(doc.atoms[i].pos);
            r
        });
        // Shift the component so it sits right of the previous one.
        let dx = x_offset - comp_bounds.min.x;
        for &i in &comp {
            doc.atoms[i].pos.x += dx;
        }
        x_offset += comp_bounds.width() + BOND_LEN * 2.0;
    }
    let all: Vec<usize> = (0..n).collect();
    if has_overlaps(doc, &all) {
        relax(doc, &all, 300, false);
    }
}

fn spread_dirs(doc: &Doc, u: usize, pdirs: &[Vec2], k: usize, flip: &mut HashMap<usize, bool>) -> Vec<Vec2> {
    use std::f32::consts::{PI, TAU};
    let linear = doc.bonds.iter().filter(|b| (b.a == u || b.b == u) && b.order == 3).count() > 0
        || doc.bonds.iter().filter(|b| (b.a == u || b.b == u) && b.order == 2).count() >= 2;
    if pdirs.is_empty() {
        // Two neighbours: the usual 120° zig-zag angle, not a straight line (which hides the
        // atom, as in the CH2 of cysteine's side chain).
        if k == 2 && !linear {
            return vec![vec2((-PI / 6.0).cos(), (-PI / 6.0).sin()), vec2((PI + PI / 6.0).cos(), (PI + PI / 6.0).sin())];
        }
        let base = -PI / 6.0;
        return (0..k).map(|j| { let a = base + TAU * j as f32 / k as f32; vec2(a.cos(), a.sin()) }).collect();
    }
    if pdirs.len() == 1 {
        let d = pdirs[0];
        if k == 1 {
            if linear {
                return vec![-d];
            }
            let f = *flip.get(&u).unwrap_or(&false);
            let ang = if f { 120f32 } else { -120f32 };
            return vec![rot(d, ang.to_radians())];
        }
        if k == 2 {
            return vec![rot(d, 120f32.to_radians()), rot(d, -120f32.to_radians())];
        }
        return (1..=k).map(|j| rot(d, TAU * j as f32 / (k + 1) as f32)).collect();
    }
    let mut angs: Vec<f32> = pdirs.iter().map(|d| crate::doc::norm_angle(d.y.atan2(d.x))).collect();
    angs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut best = (0.0, 0.0);
    for j in 0..angs.len() {
        let a = angs[j];
        let b = if j + 1 < angs.len() { angs[j + 1] } else { angs[0] + TAU };
        if b - a > best.1 - best.0 {
            best = (a, b);
        }
    }
    let gap = best.1 - best.0;
    (1..=k)
        .map(|j| {
            let a = best.0 + gap * j as f32 / (k + 1) as f32;
            vec2(a.cos(), a.sin())
        })
        .collect()
}

fn place_polygon(doc: &mut Doc, ring: &[usize], _: Option<()>, placed: &mut [bool], center: Pos2) {
    use std::f32::consts::{PI, TAU};
    let n = ring.len();
    let r = BOND_LEN / (2.0 * (PI / n as f32).sin());
    for (k, &a) in ring.iter().enumerate() {
        let ang = -PI / 2.0 + TAU * k as f32 / n as f32;
        doc.atoms[a].pos = center + vec2(ang.cos(), ang.sin()) * r;
        placed[a] = true;
    }
}

/// Place a ring that shares one or more already-placed atoms.
fn place_ring_from(doc: &mut Doc, ring: &[usize], placed: &mut [bool]) {
    use std::f32::consts::{PI, TAU};
    let n = ring.len();
    let r = BOND_LEN / (2.0 * (PI / n as f32).sin());
    let apothem = BOND_LEN / (2.0 * (PI / n as f32).tan());
    let placed_idx: Vec<usize> = (0..n).filter(|&k| placed[ring[k]]).collect();
    if placed_idx.len() == n {
        return;
    }
    // Look for a placed edge (k, k+1).
    let edge = (0..n).find(|&k| placed[ring[k]] && placed[ring[(k + 1) % n]]);
    if let Some(k) = edge {
        let a = ring[k];
        let b = ring[(k + 1) % n];
        let pa = doc.atoms[a].pos;
        let pb = doc.atoms[b].pos;
        let mid = pa + (pb - pa) * 0.5;
        let mut nrm = rot((pb - pa).normalized(), PI / 2.0);
        // Away from the other placed neighbours.
        let mut side = 0.0;
        for x in doc.neighbors(a).into_iter().chain(doc.neighbors(b)) {
            if x != a && x != b && placed[x] {
                side += (doc.atoms[x].pos - mid).dot(nrm);
            }
        }
        if side > 0.0 {
            nrm = -nrm;
        }
        let c = mid + nrm * apothem;
        let a0 = (pa - c).y.atan2((pa - c).x);
        let a1 = (pb - c).y.atan2((pb - c).x);
        let s = if crate::doc::norm_angle(a1 - a0) < PI { 1.0 } else { -1.0 };
        // Walk backwards from a so that ring order is respected.
        for j in 0..n {
            let idx = (k + n - j) % n;
            let atom = ring[idx];
            if !placed[atom] {
                let ang = a0 - s * TAU * j as f32 / n as f32;
                doc.atoms[atom].pos = c + vec2(ang.cos(), ang.sin()) * r;
                placed[atom] = true;
            }
        }
        return;
    }
    // Spiro: one shared atom.
    let k = placed_idx[0];
    let a = ring[k];
    let p = doc.atoms[a].pos;
    let others: Vec<Vec2> = doc
        .neighbors(a)
        .into_iter()
        .filter(|x| placed[*x])
        .map(|x| (doc.atoms[x].pos - p).normalized())
        .collect();
    let dir = if others.is_empty() {
        vec2(0.0, -1.0)
    } else {
        let s: Vec2 = others.iter().fold(Vec2::ZERO, |acc, d| acc + *d);
        if s.length() < 1e-3 { rot(others[0], PI / 2.0) } else { -s.normalized() }
    };
    let c = p + dir * r;
    let a0 = (p - c).y.atan2((p - c).x);
    for j in 1..n {
        let atom = ring[(k + j) % n];
        if !placed[atom] {
            let ang = a0 + TAU * j as f32 / n as f32;
            doc.atoms[atom].pos = c + vec2(ang.cos(), ang.sin()) * r;
            placed[atom] = true;
        }
    }
}

fn has_overlaps(doc: &Doc, atoms: &[usize]) -> bool {
    for (x, &i) in atoms.iter().enumerate() {
        for &j in &atoms[x + 1..] {
            if (doc.atoms[i].pos - doc.atoms[j].pos).length() < BOND_LEN * 0.5 {
                return true;
            }
        }
    }
    false
}

/// Tidy up bond lengths and angles of the given atoms (others stay where they are).
pub fn relax(doc: &mut Doc, atoms: &[usize], iters: usize, keep_center: bool) {
    use std::f32::consts::PI;
    let set: HashSet<usize> = atoms.iter().copied().collect();
    let rings = sssr(doc);
    let l = BOND_LEN;
    let mut cons: Vec<(usize, usize, f32, f32)> = Vec::new(); // i, j, target, weight
    let mut pairs: HashSet<(usize, usize)> = HashSet::new();
    let key = |a: usize, b: usize| if a < b { (a, b) } else { (b, a) };
    for b in &doc.bonds {
        if set.contains(&b.a) || set.contains(&b.b) {
            cons.push((b.a, b.b, l, 1.0));
            pairs.insert(key(b.a, b.b));
        }
    }
    // Ring shape: all pairs in the ring at regular polygon distances.
    for r in &rings {
        if !r.iter().any(|a| set.contains(a)) {
            continue;
        }
        let n = r.len();
        let rad = l / (2.0 * (PI / n as f32).sin());
        for x in 0..n {
            for y in x + 1..n {
                let k = (y - x).min(n - (y - x));
                if k == 1 {
                    continue;
                }
                let t = 2.0 * rad * (PI * k as f32 / n as f32).sin();
                if pairs.insert(key(r[x], r[y])) {
                    cons.push((r[x], r[y], t, 0.8));
                }
            }
        }
    }
    // Angles at each atom.
    let adj = doc.adjacency();
    for &c in atoms {
        let nb: Vec<usize> = adj[c].iter().map(|x| x.0).collect();
        let deg = nb.len();
        let linear = adj[c].iter().any(|x| doc.bonds[x.1].order == 3)
            || adj[c].iter().filter(|x| doc.bonds[x.1].order == 2).count() >= 2;
        for x in 0..deg {
            for y in x + 1..deg {
                let (i, j) = (nb[x], nb[y]);
                if pairs.contains(&key(i, j)) {
                    continue;
                }
                let ang = if linear && deg == 2 {
                    PI
                } else if deg <= 3 {
                    2.0 * PI / 3.0
                } else {
                    // Four or more: spread evenly, allowing ring angles to win.
                    2.0 * PI / deg as f32
                };
                let t = 2.0 * l * (ang / 2.0).sin();
                pairs.insert(key(i, j));
                cons.push((i, j, t, 0.5));
            }
        }
    }
    let center_before = centroid(doc, atoms);
    for it in 0..iters {
        let k = 0.5;
        for &(i, j, t, w) in &cons {
            let d = doc.atoms[j].pos - doc.atoms[i].pos;
            let len = d.length().max(1e-3);
            let corr = d * ((len - t) / len) * k * w;
            let mi = set.contains(&i);
            let mj = set.contains(&j);
            match (mi, mj) {
                (true, true) => {
                    doc.atoms[i].pos += corr * 0.5;
                    doc.atoms[j].pos -= corr * 0.5;
                }
                (true, false) => doc.atoms[i].pos += corr,
                (false, true) => doc.atoms[j].pos -= corr,
                _ => {}
            }
        }
        // Push apart atoms that are too close.
        if it % 2 == 0 {
            for (x, &i) in atoms.iter().enumerate() {
                for &j in &atoms[x + 1..] {
                    if pairs.contains(&key(i, j)) {
                        continue;
                    }
                    let d = doc.atoms[j].pos - doc.atoms[i].pos;
                    let len = d.length();
                    let min = l * 1.2;
                    if len < min {
                        let dir = if len < 1e-3 { vec2(((i * 7 + j) % 5) as f32 - 2.0, 1.0).normalized() } else { d / len };
                        let push = dir * (min - len) * 0.25;
                        doc.atoms[i].pos -= push;
                        doc.atoms[j].pos += push;
                    }
                }
            }
        }
    }
    if keep_center {
        let c = centroid(doc, atoms);
        let shift = center_before - c;
        for &i in atoms {
            doc.atoms[i].pos += shift;
        }
    }
}

pub fn centroid(doc: &Doc, atoms: &[usize]) -> Pos2 {
    if atoms.is_empty() {
        return Pos2::ZERO;
    }
    let s = atoms.iter().fold(Vec2::ZERO, |acc, &i| acc + doc.atoms[i].pos.to_vec2());
    (s / atoms.len() as f32).to_pos2()
}

/// Clean up a structure: regularise bond lengths and angles.
pub fn clean(doc: &mut Doc, atoms: &[usize]) {
    let frags = doc.fragments();
    for f in frags {
        let part: Vec<usize> = f.iter().copied().filter(|i| atoms.contains(i)).collect();
        if part.len() < 2 {
            continue;
        }
        relax(doc, &part, 500, true);
    }
}

#[derive(Default, Clone)]
pub struct Properties {
    pub heavy_atoms: usize,
    pub rings: usize,
    pub donors: usize,
    pub acceptors: usize,
    pub rotatable: usize,
}

pub fn properties(doc: &Doc, atoms: &[usize]) -> Properties {
    let set: HashSet<usize> = atoms.iter().copied().collect();
    let rings = sssr(doc);
    let rb = ring_bonds(doc, &rings);
    let mut p = Properties::default();
    for &i in atoms {
        let a = &doc.atoms[i];
        if let Some(l) = &a.label {
            if let Some(f) = label_formula(l) {
                p.heavy_atoms += f.iter().filter(|(z, _)| **z != 1).map(|(_, n)| *n as usize).sum::<usize>();
                p.acceptors += f.get(&7).copied().unwrap_or(0) as usize + f.get(&8).copied().unwrap_or(0) as usize;
                if l == "OH" || l == "NH2" || l == "NH" || l == "CO2H" || l == "COOH" || l == "SH" {
                    p.donors += 1;
                }
            }
            continue;
        }
        if a.el != 1 {
            p.heavy_atoms += 1;
        }
        if a.el == 7 || a.el == 8 {
            p.acceptors += 1;
            if doc.implicit_h(i) > 0 {
                p.donors += 1;
            }
        }
    }
    p.rings = rings.iter().filter(|r| r.iter().all(|a| set.contains(a))).count();
    for (bi, b) in doc.bonds.iter().enumerate() {
        if !set.contains(&b.a) || !set.contains(&b.b) || b.order != 1 || rb.contains(&bi) {
            continue;
        }
        let heavy_deg = |i: usize| doc.neighbors(i).iter().filter(|&&x| doc.atoms[x].el != 1).count()
            + if doc.atoms[i].label.is_some() { 1 } else { 0 };
        if heavy_deg(b.a) > 1 && heavy_deg(b.b) > 1 {
            p.rotatable += 1;
        }
    }
    p
}

/// Bounding box helper for fresh structures from text: centre them at `at`.
pub fn center_at(doc: &mut Doc, at: Pos2) {
    if let Some(r) = doc.bounds_of(None) {
        let d = at - r.center();
        let all = doc.select_all();
        doc.translate(&all, d);
    }
}
