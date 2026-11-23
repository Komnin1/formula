//! Estimated physical and chemical properties of one structure, computed offline:
//! - Joback group contributions: boiling and melting point, critical temperature, pressure
//!   and volume, heat and Gibbs energy of formation (ideal gas, 298 K);
//! - topological polar surface area (Ertl, N and O contributions);
//! - Log P by Moriguchi's method, and molar refractivity from atom and bond refractions.

use crate::chem;
use crate::doc::*;
use std::collections::{HashMap, HashSet};

/// The structure as a plain graph with explicit hydrogen counts.
struct Mol {
    el: Vec<u8>,
    h: Vec<i32>,
    charge: Vec<i32>,
    /// Neighbours with bond order.
    adj: Vec<Vec<(usize, u8)>>,
    ring: Vec<bool>,
    arom: Vec<bool>,
    /// False when some label could not be turned into atoms (values are then partial).
    complete: bool,
}

impl Mol {
    fn build(doc: &Doc, atoms: &[usize]) -> Mol {
        let mut sub = doc.extract(&Selection { atoms: atoms.iter().copied().collect(), ..Default::default() });
        // Expand group labels (OMe, CO2H, Ph, ...) into atoms.
        for i in (0..sub.atoms.len()).rev() {
            if sub.atoms[i].label.is_some() {
                crate::formats::expand_atom(&mut sub, i);
            }
        }
        let n = sub.atoms.len();
        let mut complete = true;
        let mut el = vec![0u8; n];
        let mut h = vec![0i32; n];
        let mut charge = vec![0i32; n];
        for i in 0..n {
            let a = &sub.atoms[i];
            charge[i] = a.charge as i32;
            match &a.label {
                None => {
                    el[i] = a.el;
                    h[i] = sub.implicit_h(i);
                }
                Some(l) => {
                    // A label with one heavy atom (OH, NH2, SH, HO, Cl...).
                    let f = chem::label_formula(l);
                    let heavy: Vec<(u8, i32)> = f.iter().flat_map(|f| f.iter().filter(|(z, _)| **z != 1).map(|(z, c)| (*z, *c))).collect();
                    if heavy.len() == 1 && heavy[0].1 == 1 {
                        el[i] = heavy[0].0;
                        h[i] = f.as_ref().and_then(|f| f.get(&1).copied()).unwrap_or(0);
                    } else {
                        complete = false;
                        el[i] = chem::label_main_element(l);
                    }
                }
            }
        }
        let mut adj = vec![Vec::new(); n];
        for b in &sub.bonds {
            adj[b.a].push((b.b, b.order.max(1)));
            adj[b.b].push((b.a, b.order.max(1)));
        }
        // Explicit hydrogen atoms count as hydrogens of their neighbour.
        let mut keep = vec![true; n];
        for i in 0..n {
            if el[i] == 1 && adj[i].len() == 1 {
                keep[i] = false;
                h[adj[i][0].0] += 1;
            }
        }
        let rings = chem::sssr(&sub);
        let rb = chem::ring_bonds(&sub, &rings);
        let ab = chem::aromatic_bonds(&sub);
        let mut ring = vec![false; n];
        let mut arom = vec![false; n];
        for (bi, b) in sub.bonds.iter().enumerate() {
            if rb.contains(&bi) {
                ring[b.a] = true;
                ring[b.b] = true;
            }
            if ab.contains(&bi) {
                arom[b.a] = true;
                arom[b.b] = true;
            }
        }
        for i in 0..n {
            if !keep[i] {
                el[i] = 0;
                adj[i].clear();
            }
            adj[i].retain(|(j, _)| keep[*j]);
        }
        Mol { el, h, charge, adj, ring, arom, complete }
    }

    fn heavy(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.el.len()).filter(|&i| self.el[i] > 1)
    }
    fn doubles_to(&self, i: usize, z: u8) -> Vec<usize> {
        self.adj[i].iter().filter(|(j, o)| *o == 2 && self.el[*j] == z).map(|(j, _)| *j).collect()
    }
    fn n_order(&self, i: usize, o: u8) -> usize {
        self.adj[i].iter().filter(|(_, b)| *b == o).count()
    }
}

/// Values for one structure. `None` where the method can't give one.
#[derive(Clone, Debug, Default)]
pub struct ChemProps {
    pub boiling_k: Option<f64>,
    pub melting_k: Option<f64>,
    pub crit_temp_k: Option<f64>,
    pub crit_pres_bar: Option<f64>,
    pub crit_vol: Option<f64>,
    pub gibbs_kj: Option<f64>,
    pub heat_form_kj: Option<f64>,
    pub log_p: Option<f64>,
    pub mr: Option<f64>,
    pub tpsa: Option<f64>,
    /// Groups the Joback method has no value for (results are then left out).
    pub unknown_groups: Vec<String>,
    pub complete: bool,
}

// Joback increments: Tc, Pc, Vc, Tb, Tm, Hf, Gf.
type Inc = (f64, f64, f64, f64, f64, f64, f64);
fn joback(group: &str) -> Option<Inc> {
    Some(match group {
        "-CH3" => (0.0141, -0.0012, 65.0, 23.58, -5.10, -76.45, -43.96),
        "-CH2-" => (0.0189, 0.0, 56.0, 22.88, 11.27, -20.64, 8.42),
        ">CH-" => (0.0164, 0.0020, 41.0, 21.74, 12.64, 29.89, 58.36),
        ">C<" => (0.0067, 0.0043, 27.0, 18.25, 46.43, 82.23, 116.02),
        "=CH2" => (0.0113, -0.0028, 56.0, 18.18, -4.32, -9.63, 3.77),
        "=CH-" => (0.0129, -0.0006, 46.0, 24.96, 8.73, 37.97, 48.53),
        "=C<" => (0.0117, 0.0011, 38.0, 24.14, 11.14, 83.99, 92.36),
        "=C=" => (0.0026, 0.0028, 36.0, 26.15, 17.78, 142.14, 136.70),
        "#CH" => (0.0027, -0.0008, 46.0, 9.20, -11.18, 79.30, 77.71),
        "#C-" => (0.0020, 0.0016, 37.0, 27.38, 64.32, 115.51, 109.82),
        "ring -CH2-" => (0.0100, 0.0025, 48.0, 27.15, 7.75, -26.80, -3.68),
        "ring >CH-" => (0.0122, 0.0004, 38.0, 21.78, 19.88, 8.67, 40.99),
        "ring >C<" => (0.0042, 0.0061, 27.0, 21.32, 60.15, 79.72, 87.88),
        "ring =CH-" => (0.0082, 0.0011, 41.0, 26.73, 8.13, 2.09, 11.30),
        "ring =C<" => (0.0143, 0.0008, 32.0, 31.01, 37.02, 46.43, 54.05),
        "-F" => (0.0111, -0.0057, 27.0, -0.03, -15.78, -251.92, -247.19),
        "-Cl" => (0.0105, -0.0049, 58.0, 38.13, 13.55, -71.55, -64.31),
        "-Br" => (0.0133, 0.0057, 71.0, 66.86, 43.43, -29.48, -38.06),
        "-I" => (0.0068, -0.0034, 97.0, 93.84, 41.69, 21.06, 5.74),
        "-OH (alcohol)" => (0.0741, 0.0112, 28.0, 92.88, 44.45, -208.04, -189.20),
        "-OH (phenol)" => (0.0240, 0.0184, -25.0, 76.34, 82.83, -221.65, -197.37),
        "-O- (nonring)" => (0.0168, 0.0015, 18.0, 22.42, 22.23, -132.22, -105.00),
        "-O- (ring)" => (0.0098, 0.0048, 13.0, 31.22, 23.05, -138.16, -98.22),
        ">C=O (nonring)" => (0.0380, 0.0031, 62.0, 76.75, 61.20, -133.22, -120.50),
        ">C=O (ring)" => (0.0284, 0.0028, 55.0, 94.97, 75.97, -164.50, -126.27),
        "O=CH- (aldehyde)" => (0.0379, 0.0030, 82.0, 72.24, 36.90, -162.03, -143.48),
        "-COOH (acid)" => (0.0791, 0.0077, 89.0, 169.09, 155.50, -426.72, -387.87),
        "-COO- (ester)" => (0.0481, 0.0005, 82.0, 81.10, 53.60, -337.92, -301.95),
        "=O (other)" => (0.0143, 0.0101, 36.0, -10.50, 2.08, -247.61, -250.83),
        "-NH2" => (0.0243, 0.0109, 38.0, 73.23, 66.89, -22.02, 14.07),
        ">NH (nonring)" => (0.0295, 0.0077, 35.0, 50.17, 52.66, 53.47, 89.39),
        ">NH (ring)" => (0.0130, 0.0114, 29.0, 52.82, 101.51, 31.65, 75.61),
        ">N- (nonring)" => (0.0169, 0.0074, 9.0, 11.74, 48.84, 123.34, 163.16),
        "-N= (ring)" => (0.0085, 0.0076, 34.0, 57.55, 68.40, 55.52, 79.93),
        "-CN" => (0.0496, -0.0101, 91.0, 125.66, 59.89, 88.43, 89.22),
        "-NO2" => (0.0437, 0.0064, 91.0, 152.54, 127.24, -66.57, -16.83),
        "-SH" => (0.0031, 0.0084, 63.0, 63.56, 20.09, -17.33, -22.99),
        "-S- (nonring)" => (0.0119, 0.0049, 54.0, 68.78, 34.40, 41.87, 33.12),
        "-S- (ring)" => (0.0019, 0.0051, 38.0, 52.10, 79.93, 39.10, 27.76),
        _ => return None,
    })
}

/// Joback groups of a structure.
fn joback_groups(m: &Mol) -> Vec<String> {
    let mut g: Vec<String> = Vec::new();
    let mut used: HashSet<usize> = HashSet::new();
    // Multi-atom groups first.
    for i in m.heavy() {
        if m.el[i] == 6 {
            let dbl_o: Vec<usize> = m.doubles_to(i, 8).into_iter().filter(|&o| m.adj[o].len() == 1).collect();
            if dbl_o.len() == 1 {
                let o = dbl_o[0];
                let single_o: Vec<usize> = m.adj[i].iter().filter(|(j, b)| *b == 1 && m.el[*j] == 8).map(|(j, _)| *j).collect();
                if let Some(&oh) = single_o.iter().find(|&&x| m.h[x] == 1 && m.adj[x].len() == 1) {
                    g.push("-COOH (acid)".into());
                    used.extend([i, o, oh]);
                } else if let Some(&oe) = single_o.iter().find(|&&x| m.adj[x].len() == 2 && m.h[x] == 0) {
                    g.push("-COO- (ester)".into());
                    used.extend([i, o, oe]);
                } else if m.h[i] == 1 {
                    g.push("O=CH- (aldehyde)".into());
                    used.extend([i, o]);
                } else {
                    g.push(if m.ring[i] { ">C=O (ring)" } else { ">C=O (nonring)" }.into());
                    used.extend([i, o]);
                }
            }
            if let Some(&(nn, _)) = m.adj[i].iter().find(|(j, b)| *b == 3 && m.el[*j] == 7) {
                g.push("-CN".into());
                used.extend([i, nn]);
            }
        }
        if m.el[i] == 7 && !used.contains(&i) {
            let os: Vec<usize> = m.adj[i].iter().filter(|(j, _)| m.el[*j] == 8 && m.adj[*j].len() == 1).map(|(j, _)| *j).collect();
            if os.len() == 2 {
                g.push("-NO2".into());
                used.insert(i);
                used.extend(os);
            }
        }
    }
    for i in m.heavy() {
        if used.contains(&i) {
            continue;
        }
        let h = m.h[i];
        let ring = m.ring[i];
        let deg = m.adj[i].len() as i32;
        let name: String = match m.el[i] {
            6 => {
                if m.n_order(i, 3) > 0 {
                    if h == 1 { "#CH" } else { "#C-" }.into()
                } else if m.n_order(i, 2) >= 2 {
                    "=C=".into()
                } else if m.n_order(i, 2) == 1 {
                    match (h, ring) {
                        (2, _) => "=CH2".into(),
                        (1, true) => "ring =CH-".into(),
                        (1, false) => "=CH-".into(),
                        (_, true) => "ring =C<".into(),
                        _ => "=C<".into(),
                    }
                } else {
                    match (h, ring) {
                        (3.., _) => "-CH3".into(),
                        (2, true) => "ring -CH2-".into(),
                        (2, false) => "-CH2-".into(),
                        (1, true) => "ring >CH-".into(),
                        (1, false) => ">CH-".into(),
                        (_, true) => "ring >C<".into(),
                        _ => ">C<".into(),
                    }
                }
            }
            8 => {
                if m.n_order(i, 2) > 0 {
                    "=O (other)".into()
                } else if h >= 1 {
                    let phenol = m.adj[i].iter().any(|(j, _)| m.arom[*j]);
                    if phenol { "-OH (phenol)" } else { "-OH (alcohol)" }.into()
                } else if ring {
                    "-O- (ring)".into()
                } else {
                    "-O- (nonring)".into()
                }
            }
            7 => {
                if m.n_order(i, 2) > 0 || (m.arom[i] && h == 0 && deg == 2) {
                    if h >= 1 {
                        "=NH".into()
                    } else if ring {
                        "-N= (ring)".into()
                    } else {
                        "-N= (nonring)".into()
                    }
                } else {
                    match (h, ring) {
                        (2.., _) => "-NH2".into(),
                        (1, true) => ">NH (ring)".into(),
                        (1, false) => ">NH (nonring)".into(),
                        // Joback has no ring value for a tertiary N; the open-chain one is used.
                        _ => ">N- (nonring)".into(),
                    }
                }
            }
            16 => {
                if h >= 1 {
                    "-SH".into()
                } else if ring {
                    "-S- (ring)".into()
                } else {
                    "-S- (nonring)".into()
                }
            }
            9 => "-F".into(),
            17 => "-Cl".into(),
            35 => "-Br".into(),
            53 => "-I".into(),
            z => format!("{} atom", crate::elements::symbol(z)),
        };
        g.push(name);
    }
    g
}

/// Topological polar surface area from N and O atoms (Ertl et al. 2000).
fn tpsa(m: &Mol) -> f64 {
    let mut t = 0.0;
    for i in m.heavy() {
        let (h, q, deg) = (m.h[i], m.charge[i], m.adj[i].len());
        let singles = m.n_order(i, 1);
        let doubles = m.n_order(i, 2);
        let triples = m.n_order(i, 3);
        let ar = m.arom[i];
        let in3ring = false;
        t += match m.el[i] {
            7 if ar => match (h, q, deg) {
                (1, 0, _) => 15.79,
                (_, 0, 2) => 12.89,
                (_, 0, 3) => 4.41,
                (1, 1, _) => 14.14,
                (_, 1, _) => 4.10,
                _ => 0.0,
            },
            7 => match (q, h) {
                (0, 0) if triples == 1 => 23.79,
                (0, 0) if doubles == 2 => 11.68,
                (0, 0) if doubles == 1 && singles == 1 => 12.36,
                (0, 0) if singles == 3 => {
                    if in3ring {
                        3.01
                    } else {
                        3.24
                    }
                }
                (0, 1) if doubles == 1 => 23.85,
                (0, 1) => 12.03,
                (0, 2) => 26.02,
                (1, 0) if doubles == 1 && singles == 2 => 3.01,
                (1, 0) if triples == 1 => 4.36,
                (1, 0) => 0.0,
                (1, 1) if doubles == 1 => 13.97,
                (1, 1) => 4.44,
                (1, 2) if doubles == 1 => 25.59,
                (1, 2) => 16.61,
                (1, 3) => 27.64,
                _ => 0.0,
            },
            8 if ar => 13.14,
            8 => match (q, h) {
                (-1, _) => 23.06,
                (0, 1) => 20.23,
                (0, 0) if doubles == 1 => 17.07,
                (0, 0) => 9.23,
                _ => 0.0,
            },
            _ => 0.0,
        };
    }
    t
}

/// Log P by Moriguchi's method (MLOGP).
fn mlogp(m: &Mol) -> f64 {
    let mut cx: f64 = 0.0;
    let mut no: f64 = 0.0;
    for i in m.heavy() {
        cx += match m.el[i] {
            6 => 1.0,
            9 => 0.5,
            17 => 1.0,
            35 => 1.5,
            53 => 2.0,
            _ => 0.0,
        };
        if m.el[i] == 7 || m.el[i] == 8 {
            no += 1.0;
        }
    }
    let is_no = |i: usize| m.el[i] == 7 || m.el[i] == 8;
    // Proximity of N and O atoms: 2 for each bonded pair, 1 for each pair one atom apart.
    let mut prx = 0.0;
    for i in m.heavy().filter(|&i| is_no(i)) {
        for &(j, _) in &m.adj[i] {
            if j > i && is_no(j) {
                prx += 2.0;
            }
            for &(k, _) in &m.adj[j] {
                if k > i && k != i && is_no(k) {
                    prx += 1.0;
                }
            }
        }
    }
    // Carboxamide: one less.
    for i in m.heavy().filter(|&i| m.el[i] == 6) {
        if !m.doubles_to(i, 8).is_empty() && m.adj[i].iter().any(|(j, b)| *b == 1 && m.el[*j] == 7) {
            prx -= 1.0;
        }
    }
    let nitro: Vec<usize> = m
        .heavy()
        .filter(|&i| m.el[i] == 7 && m.adj[i].iter().filter(|(j, _)| m.el[*j] == 8 && m.adj[*j].len() == 1).count() == 2)
        .collect();
    let mut ub: f64 = 0.0;
    for i in m.heavy() {
        for &(j, o) in &m.adj[i] {
            if j > i && o >= 2 && !(nitro.contains(&i) || nitro.contains(&j)) {
                ub += 1.0;
            }
        }
    }
    // Substituents on aromatic rings (up to 4).
    let mut pol = 0.0;
    for i in m.heavy().filter(|&i| m.arom[i]) {
        for &(j, _) in &m.adj[i] {
            if !m.arom[j] && m.el[j] > 1 {
                pol += 1.0;
            }
        }
    }
    let pol: f64 = f64::min(pol, 4.0);
    let has_acid = m.heavy().any(|i| m.el[i] == 6 && !m.doubles_to(i, 8).is_empty() && m.adj[i].iter().any(|(j, b)| *b == 1 && m.el[*j] == 8 && m.h[*j] == 1));
    let has_amine = m.heavy().any(|i| m.el[i] == 7 && !m.arom[i] && m.n_order(i, 2) == 0 && m.n_order(i, 3) == 0 && m.adj[i].iter().all(|(j, _)| m.el[*j] == 6 && m.doubles_to(*j, 8).is_empty()));
    let amp = if has_acid && has_amine { 1.0 } else { 0.0 };
    let only_ch = m.heavy().all(|i| m.el[i] == 6);
    let alk = if only_ch && ub <= 1.0 && !m.heavy().any(|i| m.arom[i]) { 1.0 } else { 0.0 };
    let rng = if m.heavy().any(|i| m.ring[i] && !m.arom[i]) { 1.0 } else { 0.0 };
    let qn = m.heavy().filter(|&i| m.el[i] == 7 && m.charge[i] == 1 && m.adj[i].len() == 4).count() as f64;
    let ncs = m.heavy().filter(|&i| m.el[i] == 7 && m.adj[i].iter().any(|(j, b)| *b == 2 && m.el[*j] == 6 && m.doubles_to(*j, 16).len() == 1)).count() as f64;
    -1.041 + 1.244 * cx.powf(0.6) - 1.017 * no.powf(0.9) + 0.406 * prx - 0.145 * ub.powf(0.8) + 0.268 * pol - 2.215 * amp + 0.912 * alk - 0.392 * rng
        - 3.684 * qn
        + 0.474 * nitro.len() as f64
        + 1.582 * ncs
}

/// Molar refractivity (cm³/mol) from atom and bond refractions (Lorentz–Lorenz, sodium D line).
fn molar_refractivity(m: &Mol) -> f64 {
    let mut r = 0.0;
    for i in m.heavy() {
        r += m.h[i] as f64 * 1.100;
        r += match m.el[i] {
            6 => 2.418,
            1 => 1.100,
            8 => {
                if m.n_order(i, 2) > 0 {
                    2.211
                } else if m.h[i] > 0 {
                    1.525
                } else {
                    1.643
                }
            }
            7 => {
                if m.n_order(i, 3) > 0 {
                    3.118
                } else {
                    match m.h[i] {
                        2 => 2.322,
                        1 => 2.502,
                        _ => 2.840,
                    }
                }
            }
            9 => 0.95,
            17 => 5.967,
            35 => 8.865,
            53 => 13.900,
            16 => 7.69,
            15 => 6.8,
            _ => 0.0,
        };
        for &(j, o) in &m.adj[i] {
            if j > i && m.el[i] != 8 && m.el[j] != 8 {
                // C≡N is already in the nitrile N value.
                if o == 2 {
                    r += 1.733;
                } else if o == 3 && m.el[i] != 7 && m.el[j] != 7 {
                    r += 2.398;
                }
            }
        }
    }
    r
}

pub fn compute(doc: &Doc, atoms: &[usize]) -> ChemProps {
    let m = Mol::build(doc, atoms);
    let groups = joback_groups(&m);
    let mut counts: HashMap<String, usize> = HashMap::new();
    for g in &groups {
        *counts.entry(g.clone()).or_default() += 1;
    }
    let mut unknown: Vec<String> = counts.keys().filter(|g| joback(g).is_none()).cloned().collect();
    unknown.sort();
    let mut p = ChemProps { complete: m.complete, ..Default::default() };
    let heavy = m.heavy().count();
    if heavy == 0 {
        return p;
    }
    if unknown.is_empty() {
        let s = groups.iter().filter_map(|g| joback(g)).fold((0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0), |a, x| {
            (a.0 + x.0, a.1 + x.1, a.2 + x.2, a.3 + x.3, a.4 + x.4, a.5 + x.5, a.6 + x.6)
        });
        let n_atoms = (heavy as i32 + m.heavy().map(|i| m.h[i]).sum::<i32>()) as f64;
        let tb = 198.0 + s.3;
        p.boiling_k = Some(tb);
        p.melting_k = Some(122.5 + s.4);
        p.crit_temp_k = Some(tb / (0.584 + 0.965 * s.0 - s.0 * s.0));
        p.crit_pres_bar = Some(1.0 / (0.113 + 0.0032 * n_atoms - s.1).powi(2));
        p.crit_vol = Some(17.5 + s.2);
        p.heat_form_kj = Some(68.29 + s.5);
        p.gibbs_kj = Some(53.88 + s.6);
    }
    p.unknown_groups = unknown;
    p.tpsa = Some(tpsa(&m));
    p.log_p = Some(mlogp(&m));
    p.mr = Some(molar_refractivity(&m));
    p
}

/// Rows shown in the Chemical Properties panel: name, value text, unit.
pub fn rows(p: &ChemProps) -> Vec<(&'static str, Option<String>, &'static str)> {
    let f2 = |v: Option<f64>| v.map(|v| format!("{:.2}", v));
    vec![
        ("Boiling Point", f2(p.boiling_k), "K"),
        ("Melting Point", f2(p.melting_k), "K"),
        ("Critical Temp", f2(p.crit_temp_k), "K"),
        ("Critical Pres", f2(p.crit_pres_bar), "Bar"),
        ("Critical Vol", f2(p.crit_vol), "cm³/mol"),
        ("Gibbs Energy", f2(p.gibbs_kj), "kJ/mol"),
        ("Log P", f2(p.log_p), ""),
        ("MR", f2(p.mr), "cm³/mol"),
        ("Heat of Form", f2(p.heat_form_kj), "kJ/mol"),
        ("tPSA", f2(p.tpsa), "Å²"),
    ]
}
