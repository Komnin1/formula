//! Predicted ¹H and ¹³C NMR spectra, computed offline from additive increments:
//! - ¹H: Shoolery/Pretsch increments for sp³ C–H, benzene ortho/meta/para increments,
//!   Pascual–Meier–Simon increments for alkene H, base values for heteroaromatics, aldehydes,
//!   acids, alcohols and amines; multiplicities by the n+1 rule with typical J values;
//! - ¹³C: Grant–Paul for sp³ carbons with substituent α/β/γ increments, benzene
//!   ipso/ortho/meta/para increments, alkene increments, carbonyl and nitrile base values.
//! Equivalent atoms are found from the structure's symmetry (topology) and share one peak.
//! Typical accuracy is about ±0.3 ppm for ¹H and ±5 ppm for ¹³C.

use crate::chem;
use crate::doc::*;
use std::collections::{HashMap, HashSet, VecDeque};

/// One signal of a predicted spectrum.
#[derive(Clone, Debug)]
pub struct Peak {
    pub shift: f64,
    /// Atoms of the drawing it comes from (a group label counts as one atom).
    pub atoms: Vec<usize>,
    /// Number of nuclei (¹H: integral; ¹³C: equivalent carbons).
    pub count: usize,
    /// "s", "d", "t", "dd", "m", "br s"...
    pub mult: String,
    /// Coupling constants (Hz), largest first.
    pub j: Vec<f64>,
    /// The couplings as (number of partners, J) for drawing the multiplet.
    pub pattern: Vec<(usize, f64)>,
    /// What the signal is ("CH3", "ArH", "CHO", "C=O"...).
    pub what: String,
    /// Share of the molecules giving this signal (partly enriched labels: the labelled and
    /// unlabelled isotopomers give separate peaks).
    pub frac: f64,
    /// Shift taken from measured spectra of this many compounds with the same surroundings (0: from the rules).
    pub learned: u16,
}

#[derive(Clone, Debug, Default)]
pub struct Prediction {
    pub h: Vec<Peak>,
    pub c: Vec<Peak>,
    /// ¹⁹F (ppm from CFCl₃) and ³¹P (ppm from 85% H₃PO₄, proton-decoupled).
    pub f: Vec<Peak>,
    pub p: Vec<Peak>,
    /// ¹⁵N (ppm from liquid NH₃, proton-decoupled) of ¹⁵N-labelled nitrogens only.
    pub n: Vec<Peak>,
    /// Labels that couldn't be turned into atoms (their nuclei are missing).
    pub skipped: Vec<String>,
}

/// The structure as a graph with hydrogen counts, ring and symmetry information.
struct Mol {
    el: Vec<u8>,
    h: Vec<usize>,
    /// Mass number of a labelled atom (0: natural), and D/T atoms bonded to each atom.
    iso: Vec<u16>,
    dn: Vec<usize>,
    charge: Vec<i8>,
    pos: Vec<egui::Pos2>,
    /// Neighbours with bond order (1, 2, 3).
    adj: Vec<Vec<(usize, u8)>>,
    arom: Vec<bool>,
    /// Bonds that are aromatic, as sorted atom pairs.
    arom_bond: HashSet<(usize, usize)>,
    /// Smallest rings (atom lists).
    rings: Vec<Vec<usize>>,
    /// Aromatic rings.
    arings: Vec<Vec<usize>>,
    /// Atom of the drawing each atom belongs to.
    owner: Vec<usize>,
    /// Symmetry class.
    class: Vec<u64>,
    alive: Vec<bool>,
    /// Solvent exchanges OH/NH protons (D2O, CD3OD): they and their couplings vanish.
    exchange: bool,
    /// Enrichment of labelled atoms (0–1): the rest of the molecules are unlabelled.
    enrich: f64,
    /// Measured shifts by surroundings.
    db: Option<std::sync::Arc<ShiftDb>>,
}

impl Mol {
    fn build(doc: &Doc, keep: &HashSet<usize>, skipped: &mut Vec<String>) -> Mol {
        let mut d = doc.clone();
        let mut owner: Vec<usize> = (0..d.atoms.len()).collect();
        // Expand group labels (OMe, CO2Et, Ph...) into atoms, remembering whose they are.
        let mut guard = 0;
        while let Some(i) = d.atoms.iter().enumerate().position(|(k, a)| keep.contains(&owner[k]) && a.label.as_ref().is_some_and(|l| chem::expansion_smiles(l).is_some())) {
            let before = d.atoms.len();
            if !crate::formats::expand_atom(&mut d, i) {
                break;
            }
            for _ in before..d.atoms.len() {
                owner.push(owner[i]);
            }
            guard += 1;
            if guard > 500 {
                break;
            }
        }
        // Formula labels of a central atom with halogens ("PF6", "BF4", "SF6", "PCl5"): the
        // central atom with its halogens around it.
        for i in 0..d.atoms.len() {
            if !keep.contains(&owner[i]) {
                continue;
            }
            let Some(l) = d.atoms[i].label.clone() else { continue };
            let Some(toks) = chem::tokenize_label(&l) else { continue };
            let (Some(first), rest) = (toks.first(), &toks[1.min(toks.len())..]) else { continue };
            let Some(z) = crate::elements::by_symbol(&first.text) else { continue };
            let halo: Vec<(u8, u32)> = rest.iter().filter_map(|t| crate::elements::by_symbol(&t.text).map(|x| (x, t.count))).collect();
            if first.count != 1 || rest.is_empty() || halo.len() != rest.len() || !halo.iter().all(|(x, _)| matches!(x, 9 | 17 | 35 | 53)) {
                continue;
            }
            d.atoms[i].el = z;
            d.atoms[i].label = None;
            let total: u32 = halo.iter().map(|x| x.1).sum();
            let p = d.atoms[i].pos;
            let mut k = 0;
            for (x, c) in halo {
                for _ in 0..c {
                    let ang = std::f32::consts::TAU * k as f32 / total.max(1) as f32;
                    let j = d.add_atom(p + egui::vec2(ang.cos(), ang.sin()) * BOND_LEN, x);
                    d.add_bond(i, j, 1, Stereo::None);
                    owner.push(owner[i]);
                    k += 1;
                }
            }
        }
        let n = d.atoms.len();
        let mut el = vec![0u8; n];
        let mut h = vec![0usize; n];
        let mut alive = vec![false; n];
        // Mass numbers of labelled atoms, and deuterium/tritium on each atom.
        let mut iso = vec![0u16; n];
        let mut dn = vec![0usize; n];
        for i in 0..n {
            if !keep.contains(&owner[i]) {
                continue;
            }
            let a = &d.atoms[i];
            alive[i] = true;
            iso[i] = a.isotope;
            match &a.label {
                None => {
                    el[i] = a.el;
                    h[i] = d.implicit_h(i).max(0) as usize;
                }
                Some(l) if l == "D" || l == "T" => {
                    el[i] = 1;
                    iso[i] = if l == "D" { 2 } else { 3 };
                }
                Some(l) => {
                    // D and T in a label (CD3, OD) are not protons.
                    let f = chem::label_formula_iso(l, true);
                    let heavy: Vec<(u8, i32)> = f.iter().flat_map(|f| f.iter().filter(|(z, _)| crate::elements::key_element(**z) != 1).map(|(z, c)| (*z, *c))).collect();
                    if heavy.len() == 1 && heavy[0].1 == 1 {
                        el[i] = crate::elements::key_element(heavy[0].0);
                        if let Some((_, a, _)) = crate::elements::iso_of_key(heavy[0].0) {
                            iso[i] = a;
                        }
                        h[i] = f.as_ref().and_then(|f| f.get(&1).copied()).unwrap_or(0).max(0) as usize;
                        dn[i] = f.as_ref().map(|f| f.iter().filter(|(k, _)| **k >= 128 && crate::elements::key_element(**k) == 1).map(|(_, c)| *c).sum::<i32>()).unwrap_or(0).max(0) as usize;
                    } else {
                        el[i] = chem::label_main_element(l);
                        if !skipped.contains(l) {
                            skipped.push(l.clone());
                        }
                    }
                }
            }
        }
        let mut adj = vec![Vec::new(); n];
        for b in &d.bonds {
            if alive[b.a] && alive[b.b] {
                adj[b.a].push((b.b, b.order.clamp(1, 3)));
                adj[b.b].push((b.a, b.order.clamp(1, 3)));
            }
        }
        // Drawn hydrogen atoms count as hydrogens of their neighbour.
        for i in 0..n {
            if alive[i] && el[i] == 1 && adj[i].len() == 1 {
                let j = adj[i][0].0;
                // Deuterium and tritium don't show in ¹H; they split carbon in ¹³C.
                if iso[i] >= 2 {
                    dn[j] += 1;
                } else {
                    h[j] += 1;
                }
                alive[i] = false;
            }
        }
        for i in 0..n {
            if !alive[i] {
                adj[i].clear();
            }
        }
        for i in 0..n {
            let a = alive.clone();
            adj[i].retain(|(j, _)| a[*j]);
        }
        let ab = chem::aromatic_bonds(&d);
        let mut arom = vec![false; n];
        let mut arom_bond = HashSet::new();
        for bi in ab {
            let b = &d.bonds[bi];
            if alive[b.a] && alive[b.b] {
                arom[b.a] = true;
                arom[b.b] = true;
                arom_bond.insert((b.a.min(b.b), b.a.max(b.b)));
            }
        }
        let rings: Vec<Vec<usize>> = chem::sssr(&d).into_iter().filter(|r| r.iter().all(|&a| alive[a])).collect();
        let arings: Vec<Vec<usize>> = rings
            .iter()
            .filter(|r| {
                (0..r.len()).all(|k| {
                    let (x, y) = (r[k], r[(k + 1) % r.len()]);
                    arom_bond.contains(&(x.min(y), x.max(y)))
                })
            })
            .cloned()
            .collect();
        let pos = d.atoms.iter().map(|a| a.pos).collect();
        let charge = d.atoms.iter().map(|a| a.charge).collect();
        let mut m = Mol { el, h, iso, dn, charge, pos, adj, arom, arom_bond, rings, arings, owner, class: vec![0; n], alive, exchange: false, enrich: 1.0, db: None };
        m.class = m.symmetry();
        m
    }

    /// Symmetry classes by iterative refinement of atom invariants.
    fn symmetry(&self) -> Vec<u64> {
        use std::hash::{Hash, Hasher};
        let hash = |v: &dyn Fn(&mut std::collections::hash_map::DefaultHasher)| {
            let mut s = std::collections::hash_map::DefaultHasher::new();
            v(&mut s);
            s.finish()
        };
        let n = self.el.len();
        // Aromatic bonds count alike whichever Kekulé form was drawn.
        let code = |i: usize, j: usize, o: u8| if self.is_arom_bond(i, j) { 4u8 } else { o };
        let mut c: Vec<u64> = (0..n)
            .map(|i| hash(&|s| (self.el[i], self.h[i], self.iso[i], self.dn[i], self.adj[i].len(), self.arom[i], self.adj[i].iter().map(|x| code(i, x.0, x.1) as usize).sum::<usize>()).hash(s)))
            .collect();
        let count = |c: &[u64]| c.iter().enumerate().filter(|(i, _)| self.alive[*i]).map(|x| x.1).collect::<HashSet<_>>().len();
        let mut k = count(&c);
        for _ in 0..n.max(1) {
            let next: Vec<u64> = (0..n)
                .map(|i| {
                    let mut nb: Vec<(u64, u8)> = self.adj[i].iter().map(|(j, o)| (c[*j], code(i, *j, *o))).collect();
                    nb.sort();
                    hash(&|s| (c[i], &nb).hash(s))
                })
                .collect();
            let k2 = count(&next);
            c = next;
            if k2 == k {
                break;
            }
            k = k2;
        }
        c
    }

    fn order(&self, a: usize, b: usize) -> u8 {
        self.adj[a].iter().find(|x| x.0 == b).map(|x| x.1).unwrap_or(0)
    }
    fn is_arom_bond(&self, a: usize, b: usize) -> bool {
        self.arom_bond.contains(&(a.min(b), a.max(b)))
    }
    fn nbrs(&self, a: usize) -> impl Iterator<Item = usize> + '_ {
        self.adj[a].iter().map(|x| x.0)
    }
    /// sp3 carbon (no multiple bonds, not aromatic).
    fn sp3(&self, a: usize) -> bool {
        self.el[a] == 6 && !self.arom[a] && self.adj[a].iter().all(|x| x.1 == 1)
    }
    /// Atom double-bonded to O (a carbonyl carbon).
    fn carbonyl(&self, a: usize) -> bool {
        self.el[a] == 6 && self.adj[a].iter().any(|(j, o)| *o == 2 && self.el[*j] == 8)
    }
    /// Smallest ring containing the atom.
    fn ring_of(&self, a: usize) -> Option<&Vec<usize>> {
        self.rings.iter().filter(|r| r.contains(&a)).min_by_key(|r| r.len())
    }
    /// The aromatic ring an atom is read in: six-membered first, then the smallest.
    fn aring_of(&self, a: usize) -> Option<&Vec<usize>> {
        self.arings.iter().filter(|r| r.contains(&a)).min_by_key(|r| (r.len() != 6, r.len()))
    }
    /// Shortest distance between two atoms going round ring `r` only.
    fn ring_dist(&self, r: &[usize], a: usize, b: usize) -> usize {
        let (Some(i), Some(j)) = (r.iter().position(|x| *x == a), r.iter().position(|x| *x == b)) else { return 99 };
        let d = i.abs_diff(j);
        d.min(r.len() - d)
    }
    /// Atoms of aromatic rings fused to `r` (sharing a bond), including `r`.
    fn fused_with(&self, r: &[usize]) -> HashSet<usize> {
        let mut s: HashSet<usize> = r.iter().copied().collect();
        for q in &self.arings {
            if q.iter().filter(|a| r.contains(a)).count() >= 2 {
                s.extend(q.iter().copied());
            }
        }
        s
    }
    fn hetero(&self, a: usize) -> bool {
        !matches!(self.el[a], 1 | 6)
    }
}

/// A substituent as seen from the atom it is attached to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Sub {
    Alkyl,
    Cf3,
    Vinyl,
    Alkynyl,
    Aryl,
    Cho,
    Cor,
    Cooh,
    Coor,
    Conr,
    Cocl,
    Cn,
    Imine,
    Oh,
    Or,
    Oar,
    Ocor,
    Nr,
    Nar,
    Nhcor,
    No2,
    F,
    Cl,
    Br,
    I,
    Sr,
    So2r,
    /// Phosphine P(III), P=O / P=S group, phosphonium P+.
    P3,
    P5,
    Pplus,
    Other,
}

/// Increments for one substituent type.
struct Inc {
    /// ¹H: δ of CH3–X; Shoolery-type Z for CH2/CH; β effect on H two bonds further.
    ch3: f64,
    z: f64,
    beta_h: f64,
    /// ¹H on benzene: ortho, meta, para.
    ar_h: [f64; 3],
    /// ¹³C on benzene: ipso, ortho, meta, para.
    ar_c: [f64; 4],
    /// ¹H on an alkene: geminal, cis, trans.
    al_h: [f64; 3],
    /// ¹³C on an alkene: the carbon carrying it, the other carbon.
    al_c: [f64; 2],
    /// ¹³C on an sp3 carbon: α, β, γ.
    c: [f64; 3],
}

const fn inc(ch3: f64, z: f64, beta_h: f64, ar_h: [f64; 3], ar_c: [f64; 4], al_h: [f64; 3], al_c: [f64; 2], c: [f64; 3]) -> Inc {
    Inc { ch3, z, beta_h, ar_h, ar_c, al_h, al_c, c }
}

/// Values from the standard increment tables (Pretsch, Silverstein); rounded.
fn incr(s: Sub) -> Inc {
    use Sub::*;
    match s {
        Alkyl => inc(0.86, 0.0, 0.0, [-0.18, -0.11, -0.21], [9.3, 0.7, -0.1, -2.9], [0.45, -0.22, -0.28], [10.6, -7.9], [9.1, 9.4, -2.5]),
        Cf3 => inc(1.2, 0.6, 0.2, [0.32, 0.14, 0.20], [2.6, -3.1, 0.4, 3.4], [0.66, 0.61, 0.32], [4.0, 3.0], [10.0, 1.0, -2.0]),
        Vinyl => inc(1.71, 0.75, 0.1, [0.04, -0.04, -0.12], [9.5, -2.0, 0.2, -0.5], [1.00, -0.09, -0.23], [13.6, -7.0], [20.0, 6.0, -0.5]),
        Alkynyl => inc(1.80, 0.90, 0.15, [0.15, -0.02, -0.01], [-6.1, 3.8, 0.4, -0.2], [0.47, 0.38, 0.12], [-14.0, 7.0], [4.5, 5.5, -3.5]),
        Aryl => inc(2.34, 1.45, 0.3, [0.23, 0.07, -0.02], [13.0, -1.1, 0.5, -1.0], [1.38, 0.36, -0.07], [12.5, -11.0], [23.0, 9.0, -2.0]),
        Cho => inc(2.20, 1.10, 0.2, [0.53, 0.18, 0.28], [8.2, 1.2, 0.5, 5.8], [1.02, 0.95, 1.17], [15.3, 14.5], [31.0, 0.0, -2.0]),
        Cor => inc(2.10, 1.20, 0.2, [0.60, 0.10, 0.20], [8.9, 0.1, -0.1, 4.4], [1.10, 1.12, 0.87], [13.8, 4.7], [30.0, 1.0, -2.0]),
        Cooh => inc(2.08, 1.00, 0.2, [0.77, 0.11, 0.25], [2.1, 1.6, -0.1, 5.2], [0.97, 1.41, 0.71], [5.0, 9.8], [21.0, 3.0, -2.0]),
        Coor => inc(2.01, 0.95, 0.2, [0.68, 0.08, 0.19], [2.0, 1.2, -0.1, 4.3], [0.80, 1.18, 0.55], [6.3, 7.0], [20.0, 3.0, -2.0]),
        Conr => inc(2.02, 1.00, 0.2, [0.46, 0.09, 0.17], [5.0, -1.2, 0.1, 3.4], [1.37, 0.98, 0.46], [9.0, 4.0], [22.0, 2.5, -2.0]),
        Cocl => inc(2.66, 1.60, 0.3, [0.84, 0.22, 0.36], [4.7, 2.7, 0.3, 6.6], [1.11, 1.46, 1.01], [8.0, 14.0], [33.0, 2.0, -3.0]),
        Cn => inc(1.98, 1.20, 0.4, [0.29, 0.12, 0.25], [-15.7, 3.6, 0.7, 4.3], [0.27, 0.75, 0.55], [-15.1, 14.2], [4.0, 3.0, -3.0]),
        Imine => inc(2.0, 1.1, 0.2, [0.4, 0.1, 0.2], [6.0, 0.0, 0.0, 2.0], [1.0, 0.8, 0.6], [10.0, 5.0], [25.0, 2.0, -2.0]),
        Oh => inc(3.39, 2.30, 0.25, [-0.53, -0.14, -0.43], [26.9, -12.8, 1.4, -7.4], [1.22, -1.07, -1.21], [29.0, -39.0], [48.0, 10.0, -5.0]),
        Or => inc(3.30, 2.15, 0.25, [-0.45, -0.07, -0.41], [31.4, -14.4, 1.0, -7.7], [1.22, -1.07, -1.21], [29.0, -39.0], [58.0, 8.0, -4.0]),
        Oar => inc(3.73, 2.75, 0.35, [-0.36, -0.04, -0.28], [27.6, -11.2, -0.3, -6.9], [1.21, -0.60, -1.00], [28.0, -37.0], [57.0, 7.0, -4.0]),
        Ocor => inc(3.67, 2.85, 0.35, [-0.27, 0.02, -0.13], [22.4, -7.1, 0.4, -3.2], [2.11, -0.35, -0.64], [18.4, -26.7], [52.0, 6.5, -6.0]),
        Nr => inc(2.30, 1.30, 0.15, [-0.71, -0.22, -0.62], [18.2, -13.4, 0.8, -10.0], [0.80, -1.26, -1.21], [28.0, -32.0], [29.0, 11.0, -5.0]),
        Nar => inc(3.70, 2.40, 0.25, [-0.20, -0.05, -0.20], [10.0, -5.0, 0.5, -4.0], [1.0, -0.5, -0.7], [15.0, -15.0], [33.0, 8.0, -4.0]),
        Nhcor => inc(2.80, 1.95, 0.2, [0.14, -0.07, -0.27], [9.7, -8.1, 0.2, -4.4], [2.08, -0.57, -0.72], [20.0, -25.0], [26.0, 8.0, -4.0]),
        No2 => inc(4.29, 3.10, 0.6, [0.95, 0.26, 0.38], [19.9, -4.9, 0.9, 6.1], [1.87, 1.32, 0.62], [22.3, -0.9], [63.0, 4.0, -5.0]),
        F => inc(4.26, 3.10, 0.3, [-0.26, 0.00, -0.20], [34.8, -13.0, 1.6, -4.4], [1.54, -0.40, -1.02], [24.9, -34.3], [68.0, 9.0, -4.0]),
        Cl => inc(3.05, 2.30, 0.45, [0.03, -0.02, -0.09], [6.4, 0.2, 1.0, -2.0], [1.08, 0.18, 0.13], [2.8, -6.1], [31.0, 11.0, -4.0]),
        Br => inc(2.68, 2.10, 0.55, [0.18, -0.08, -0.04], [-5.4, 3.4, 2.2, -1.0], [1.07, 0.45, 0.55], [-8.6, -0.9], [20.0, 11.0, -3.0]),
        I => inc(2.16, 1.90, 0.6, [0.39, -0.21, 0.00], [-32.2, 9.9, 2.6, -7.3], [1.14, 0.81, 0.88], [-38.1, 7.0], [-6.0, 11.0, -1.0]),
        Sr => inc(2.09, 1.30, 0.2, [-0.08, -0.10, -0.24], [10.0, -1.9, 0.2, -3.6], [1.11, -0.29, -0.13], [18.0, -16.0], [20.0, 7.0, -3.0]),
        So2r => inc(2.90, 1.75, 0.4, [0.71, 0.30, 0.38], [12.3, -1.4, 0.8, 5.1], [1.55, 1.16, 0.93], [12.0, 6.0], [40.0, 1.0, -3.0]),
        P3 => inc(1.0, 0.4, 0.1, [0.06, -0.04, 0.0], [9.0, 5.2, 0.0, 0.2], [0.6, 0.2, 0.0], [12.0, 2.0], [18.0, 1.0, -1.0]),
        P5 => inc(2.0, 1.0, 0.2, [0.41, 0.20, 0.27], [4.0, 3.6, 0.0, 3.4], [1.1, 0.9, 0.6], [8.0, 10.0], [17.0, 1.0, -1.0]),
        Pplus => inc(3.2, 2.2, 0.4, [0.48, 0.41, 0.58], [-10.0, 6.0, 2.1, 7.0], [1.4, 1.2, 0.9], [-5.0, 25.0], [12.0, 1.0, -1.0]),
        Other => inc(1.0, 0.2, 0.0, [0.0, 0.0, 0.0], [5.0, 0.0, 0.0, 0.0], [0.5, 0.0, 0.0], [5.0, 0.0], [5.0, 3.0, -2.0]),
    }
}

impl Mol {
    /// What atom `x` is, as a substituent of atom `from`.
    fn sub(&self, from: usize, x: usize) -> Sub {
        let others: Vec<usize> = self.nbrs(x).filter(|y| *y != from).collect();
        match self.el[x] {
            6 => {
                if self.arom[x] {
                    return Sub::Aryl;
                }
                if self.adj[x].iter().any(|(y, o)| *o == 3 && self.el[*y] == 7) {
                    return Sub::Cn;
                }
                if self.carbonyl(x) {
                    let rest: Vec<usize> = others.iter().copied().filter(|y| !(self.el[*y] == 8 && self.order(x, *y) == 2)).collect();
                    if rest.iter().any(|y| self.el[*y] == 8) {
                        let o = rest.iter().find(|y| self.el[**y] == 8).copied().unwrap();
                        return if self.h[o] > 0 || self.adj[o].len() == 1 { Sub::Cooh } else { Sub::Coor };
                    }
                    if rest.iter().any(|y| self.el[*y] == 7) {
                        return Sub::Conr;
                    }
                    if rest.iter().any(|y| self.el[*y] == 17) {
                        return Sub::Cocl;
                    }
                    if rest.is_empty() {
                        return Sub::Cho;
                    }
                    return Sub::Cor;
                }
                if self.adj[x].iter().any(|(y, o)| *o == 2 && self.el[*y] == 7) {
                    return Sub::Imine;
                }
                if self.adj[x].iter().any(|(_, o)| *o == 3) {
                    return Sub::Alkynyl;
                }
                if self.adj[x].iter().any(|(_, o)| *o == 2) {
                    return Sub::Vinyl;
                }
                if others.iter().filter(|y| self.el[**y] == 9).count() >= 3 {
                    return Sub::Cf3;
                }
                Sub::Alkyl
            }
            8 => {
                if self.h[x] > 0 || others.is_empty() {
                    return Sub::Oh;
                }
                let y = others[0];
                if self.carbonyl(y) {
                    Sub::Ocor
                } else if self.arom[y] || (self.el[y] == 6 && self.adj[y].iter().any(|(_, o)| *o == 2)) {
                    Sub::Oar
                } else {
                    Sub::Or
                }
            }
            7 => {
                let oxy = self.nbrs(x).filter(|y| self.el[*y] == 8).count();
                if oxy >= 2 {
                    return Sub::No2;
                }
                if others.iter().any(|y| self.carbonyl(*y) || (self.el[*y] == 16 && self.adj[*y].iter().any(|(z, o)| *o == 2 && self.el[*z] == 8))) {
                    return Sub::Nhcor;
                }
                if self.arom[x] {
                    return Sub::Nar;
                }
                if self.adj[x].iter().any(|(_, o)| *o >= 2) {
                    return Sub::Imine;
                }
                Sub::Nr
            }
            9 => Sub::F,
            17 => Sub::Cl,
            35 => Sub::Br,
            53 => Sub::I,
            16 => {
                if self.adj[x].iter().any(|(y, o)| *o == 2 && self.el[*y] == 8) {
                    Sub::So2r
                } else {
                    Sub::Sr
                }
            }
            15 => {
                if self.adj[x].iter().any(|(y, o)| *o >= 2 && matches!(self.el[*y], 8 | 16)) {
                    Sub::P5
                } else if self.charge[x] > 0 {
                    Sub::Pplus
                } else {
                    Sub::P3
                }
            }
            14 | 5 => Sub::Alkyl,
            _ => Sub::Other,
        }
    }
}

/// Hydrogens of one atom that share a shift (an alkene CH2 has two sites).
#[derive(Clone, Debug)]
struct HSite {
    atom: usize,
    n: usize,
    shift: f64,
    what: &'static str,
    /// OH, NH, SH: broad, no resolved coupling.
    exch: bool,
    /// Where the hydrogen points (alkenes), for cis/trans.
    hpos: Option<egui::Pos2>,
    /// ¹J(¹⁵N,H) if the hydrogen sits on a ¹⁵N-labelled nitrogen.
    jn: f64,
}

fn rot(v: egui::Vec2, deg: f32) -> egui::Vec2 {
    let (s, c) = deg.to_radians().sin_cos();
    egui::vec2(v.x * c - v.y * s, v.x * s + v.y * c)
}

fn cross(a: egui::Vec2, b: egui::Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

impl Mol {
    /// The acyl carbon (or sulfonyl S) of an amide nitrogen.
    fn amide_c(&self, n: usize) -> Option<usize> {
        if self.el[n] != 7 || self.arom[n] {
            return None;
        }
        self.nbrs(n).find(|y| self.carbonyl(*y) || (self.el[*y] == 16 && self.adj[*y].iter().any(|(z, o)| *o == 2 && self.el[*z] == 8)))
    }

    /// An acceptor ortho to an aroyl group that the amide N–H can hydrogen-bond to
    /// (P=O, ester or ketone C=O, NO2, OR): a six- or seven-membered ring.
    fn ortho_acceptor(&self, c: usize) -> bool {
        let Some(r) = self.nbrs(c).find(|y| self.arom[*y]) else { return false };
        for q in self.nbrs(r).filter(|q| *q != c && self.arom[*q]) {
            for y in self.nbrs(q).filter(|y| !self.arom[*y] && *y != r) {
                let acceptor = match self.el[y] {
                    15 => self.adj[y].iter().any(|(z, o)| *o == 2 && self.el[*z] == 8),
                    6 => self.carbonyl(y),
                    7 => self.nbrs(y).filter(|z| self.el[*z] == 8).count() >= 2,
                    8 => true,
                    16 => self.adj[y].iter().any(|(z, o)| *o == 2 && self.el[*z] == 8),
                    _ => false,
                };
                if acceptor {
                    return true;
                }
            }
        }
        false
    }

    /// A phosphonium or quaternary ammonium cation in the structure: its counter-anion
    /// hydrogen-bonds to amide N–H in CDCl3 / CD2Cl2 and moves it about 2.3 ppm downfield
    /// (N-(3-triphenylphosphoniopropyl)benzamides: 8.7 ppm).
    fn has_cation(&self) -> bool {
        (0..self.el.len()).any(|i| self.alive[i] && self.charge[i] > 0 && matches!(self.el[i], 7 | 15) && self.h[i] == 0)
    }

    /// Amide N–H. Primary amides give two signals: the H-bonded one (to an ortho acceptor)
    /// far downfield, the other near 5.7 ppm (2-(diphenylphosphoryl)benzamide: 8.69, 5.67).
    /// Secondary amides couple to N–CH (J ≈ 5.8 Hz) in non-exchanging solvents.
    fn amide_h(&self, n: usize) -> Vec<HSite> {
        let Some(c) = self.amide_c(n) else { return vec![] };
        let labelled = self.iso[n] == 15;
        let aroyl = self.nbrs(c).any(|y| self.arom[y]);
        let hb = self.ortho_acceptor(c);
        let salt = if self.has_cation() { 2.3 } else { 0.0 };
        let site = |shift: f64, nh: usize, jn: f64| HSite { atom: n, n: nh, shift, what: "NH (amide)", exch: false, hpos: None, jn: if labelled { jn } else { 0.0 } };
        if self.h[n] >= 2 {
            // ¹J(¹⁵N,H): 90.0 Hz for the H-bonded proton, 86.8 Hz for the free one.
            let free = 5.7 + if aroyl { 0.0 } else { -0.2 } + salt * 0.5;
            let bonded = if hb { 8.7 } else { 6.2 + salt * 0.5 };
            vec![site(bonded, 1, if hb { 90.0 } else { 88.5 }), site(free, 1, 86.8)]
        } else {
            let n_aryl = self.nbrs(n).any(|y| y != c && self.arom[y]);
            let mut s = if n_aryl { 7.6 } else { 6.2 + if aroyl { 0.2 } else { 0.0 } };
            s += salt;
            if hb {
                s += 0.9;
            }
            vec![site(s, 1, 92.0)]
        }
    }
}

impl Mol {
    fn h_sites(&self) -> Vec<HSite> {
        let mut out = Vec::new();
        for a in 0..self.el.len() {
            if !self.alive[a] || self.h[a] == 0 {
                continue;
            }
            let site = |shift: f64, what: &'static str, exch: bool| HSite { atom: a, n: self.h[a], shift, what, exch, hpos: None, jn: 0.0 };
            match self.el[a] {
                6 => {
                    if self.arom[a] {
                        out.push(site(self.aromatic_h(a), "ArH", false));
                    } else if self.carbonyl(a) {
                        let rest: Vec<usize> = self.nbrs(a).filter(|y| !(self.el[*y] == 8 && self.order(a, *y) == 2)).collect();
                        let s = if rest.iter().any(|y| self.el[*y] == 8) {
                            8.05
                        } else if rest.iter().any(|y| self.el[*y] == 7) {
                            8.15
                        } else if rest.iter().any(|y| self.arom[*y] || self.adj[*y].iter().any(|(_, o)| *o == 2)) {
                            9.95
                        } else {
                            9.75
                        };
                        out.push(site(s, "CHO", false));
                    } else if self.adj[a].iter().any(|(_, o)| *o == 3) {
                        let conj = self.nbrs(a).any(|y| self.nbrs(y).any(|z| z != a && (self.arom[z] || self.carbonyl(z))));
                        out.push(site(if conj { 3.05 } else { 1.95 }, "≡CH", false));
                    } else if let Some(b) = self.adj[a].iter().find(|(y, o)| *o == 2 && self.el[*y] == 6).map(|x| x.0) {
                        out.extend(self.alkene_h(a, b));
                    } else if self.adj[a].iter().any(|(y, o)| *o == 2 && self.el[*y] == 7) {
                        out.push(site(8.1, "CH=N", false));
                    } else if self.sp3(a) {
                        let (s, w) = self.sp3_h(a);
                        out.push(site(s, w, false));
                    }
                }
                // In D2O or CD3OD, OH/NH/SH protons are replaced by deuterium.
                7 | 8 | 16 if self.exchange => {}
                7 if self.amide_c(a).is_some() => out.extend(self.amide_h(a)),
                8 => {
                    let nb: Vec<usize> = self.nbrs(a).collect();
                    let s = match nb.first() {
                        Some(&y) if self.carbonyl(y) => 11.5,
                        Some(&y) if self.arom[y] || self.adj[y].iter().any(|(_, o)| *o == 2) => 5.5,
                        Some(_) => 2.0,
                        None => 1.56,
                    };
                    out.push(site(s, if s > 10.0 { "COOH" } else { "OH" }, true));
                }
                7 => {
                    let nb: Vec<usize> = self.nbrs(a).collect();
                    let acyl = nb.iter().any(|y| self.carbonyl(*y) || (self.el[*y] == 16 && self.adj[*y].iter().any(|(z, o)| *o == 2 && self.el[*z] == 8)));
                    let s = if acyl {
                        if self.h[a] >= 2 {
                            6.0
                        } else if nb.iter().any(|y| self.arom[*y]) {
                            7.6
                        } else {
                            6.2
                        }
                    } else if self.arom[a] {
                        let more_n = self.aring_of(a).is_some_and(|r| r.iter().filter(|x| self.el[**x] == 7).count() > 1);
                        if more_n { 10.5 } else { 8.1 }
                    } else if nb.iter().any(|y| self.arom[*y]) {
                        3.8
                    } else {
                        1.4
                    };
                    // ¹J(¹⁵N,H): pyrrole-type ≈ 97, arylamine ≈ 85, alkylamine ≈ 75 Hz.
                    let jn = if self.iso[a] != 15 {
                        0.0
                    } else if self.arom[a] {
                        97.0
                    } else if nb.iter().any(|y| self.arom[*y]) {
                        85.0
                    } else {
                        75.0
                    };
                    out.push(HSite { jn, ..site(s, "NH", true) });
                }
                16 => {
                    let ar = self.nbrs(a).any(|y| self.arom[y]);
                    out.push(site(if ar { 3.4 } else { 1.4 }, "SH", true));
                }
                15 => {
                    let s = match self.p_kind(a) {
                        PKind::P3 => if self.nbrs(a).any(|y| self.arom[y]) { 4.0 } else { 2.8 },
                        PKind::Ester => 6.8,
                        PKind::Plus => 7.5,
                        _ => 8.0,
                    };
                    out.push(site(s, "PH", false));
                }
                _ => {}
            }
        }
        out
    }

    /// sp3 C–H: CH3–X values, CH2/CH from a base plus substituent Z values, β effects and ring
    /// corrections.
    fn sp3_h(&self, a: usize) -> (f64, &'static str) {
        let subs: Vec<(usize, Sub)> = self.nbrs(a).filter(|x| self.el[*x] > 1).map(|x| (x, self.sub(a, x))).collect();
        let what = match self.h[a] {
            4 => "CH4",
            3 => "CH3",
            2 => "CH2",
            _ => "CH",
        };
        if subs.is_empty() {
            return (0.23, what);
        }
        let mut s = match self.h[a] {
            3 => {
                let (x, k) = subs[0];
                // A methyl ketone on an aromatic ring (acetophenone 2.6).
                let ar_ketone = k == Sub::Cor && self.nbrs(x).any(|y| y != a && self.arom[y]);
                incr(k).ch3 + if ar_ketone { 0.45 } else { 0.0 }
            }
            n => {
                let mut z: f64 = subs.iter().map(|x| incr(x.1).z).sum();
                if z > 3.0 {
                    z = 3.0 + 0.6 * (z - 3.0);
                }
                (if n == 2 { 1.25 } else { 1.50 }) + z
            }
        };
        // β effects of groups on neighbouring sp3 carbons.
        let mut beta = 0.0;
        for &(x, k) in &subs {
            if k != Sub::Alkyl {
                continue;
            }
            for y in self.nbrs(x).filter(|y| *y != a && self.el[*y] > 1) {
                beta += incr(self.sub(x, y)).beta_h;
            }
        }
        s += beta.min(1.0);
        if let Some(r) = self.ring_of(a) {
            s += match r.len() {
                3 => -1.1,
                4 => 0.7,
                5 => 0.25,
                6 => 0.18,
                _ => 0.1,
            };
        }
        (s.clamp(-0.5, 7.5), what)
    }

    /// Aromatic C–H: base value for the ring (benzene, fused, heteroaromatic) plus the
    /// ortho/meta/para increments of the ring's substituents.
    fn aromatic_h(&self, a: usize) -> f64 {
        let Some(r) = self.aring_of(a).cloned() else { return 7.3 };
        let s = self.fused_with(&r);
        let fused = s.len() > r.len();
        let het: Vec<usize> = r.iter().copied().filter(|x| self.hetero(*x)).collect();
        let junction = |x: usize| self.arings.iter().filter(|q| q.contains(&x)).count() > 1;
        let mut base = if r.len() == 6 {
            if het.is_empty() {
                if fused {
                    if self.nbrs(a).any(junction) { 7.80 } else { 7.45 }
                } else {
                    7.27
                }
            } else {
                let near = het.iter().filter(|x| self.ring_dist(&r, a, **x) == 1).count();
                let d = het.iter().map(|x| self.ring_dist(&r, a, *x)).min().unwrap_or(3);
                (match (near, d) {
                    (2.., _) => 9.2,
                    (_, 1) => 8.6,
                    (_, 2) => 7.3,
                    _ => 7.7,
                }) + if fused { 0.1 } else { 0.0 }
            }
        } else {
            let adj_het: Vec<usize> = het.iter().copied().filter(|x| self.ring_dist(&r, a, *x) == 1).collect();
            let kind = |x: usize| match self.el[x] {
                8 => 0,
                16 => 1,
                7 if self.adj[x].len() >= 3 || self.h[x] > 0 => 2,
                7 => 3,
                _ => 4,
            };
            (match adj_het.len() {
                2.. => {
                    if adj_het.iter().any(|x| self.el[*x] == 16) {
                        8.8
                    } else {
                        7.9
                    }
                }
                1 => [7.4, 7.3, 6.7, 7.5, 7.0][kind(adj_het[0])],
                _ => het.first().map(|x| [6.4, 7.1, 6.2, 7.1, 6.8][kind(*x)]).unwrap_or(6.5),
            }) + if fused { 0.2 } else { 0.0 }
        };
        for &q in &r {
            let d = self.ring_dist(&r, a, q);
            if !(1..=3).contains(&d) {
                continue;
            }
            for x in self.nbrs(q) {
                if s.contains(&x) || self.el[x] <= 1 {
                    continue;
                }
                base += incr(self.sub(q, x)).ar_h[d - 1];
            }
        }
        base
    }

    /// Alkene H (Pascual–Meier–Simon): 5.25 + geminal, cis and trans increments.
    fn alkene_h(&self, a: usize, b: usize) -> Vec<HSite> {
        let pa = self.pos[a];
        let axis = self.pos[b] - pa;
        let len = axis.length().max(1e-3);
        let heavy: Vec<usize> = self.nbrs(a).filter(|x| self.el[*x] > 1).collect();
        let hdirs: Vec<egui::Vec2> = if self.h[a] >= 2 {
            vec![rot(axis, 120.0), rot(axis, -120.0)]
        } else {
            let s: egui::Vec2 = heavy.iter().map(|x| (self.pos[*x] - pa).normalized()).fold(egui::Vec2::ZERO, |u, v| u + v);
            vec![if s.length() > 1e-3 { -s.normalized() * len } else { rot(axis, 120.0) }]
        };
        let vals = |from: usize, x: usize| -> [f64; 3] {
            let k = self.sub(from, x);
            if k == Sub::Alkyl {
                // CH2–O, CH2–N or CH2–halogen behave differently from plain alkyl.
                if self.nbrs(x).any(|y| y != from && matches!(self.el[y], 8 | 7)) {
                    return [0.64, -0.01, -0.02];
                }
                if self.nbrs(x).any(|y| y != from && matches!(self.el[y], 9 | 17 | 35 | 53)) {
                    return [0.70, 0.11, -0.04];
                }
            }
            if k == Sub::Vinyl {
                return [1.24, 0.02, -0.05];
            }
            incr(k).al_h
        };
        let n_each = if hdirs.len() == 2 { 1 } else { self.h[a] };
        hdirs
            .iter()
            .map(|hd| {
                let hp = pa + *hd;
                let side_h = cross(axis, hp - pa).signum();
                let mut s = 5.25;
                for &x in heavy.iter().filter(|x| **x != b) {
                    s += vals(a, x)[0];
                }
                for x in self.nbrs(b).filter(|x| *x != a && self.el[*x] > 1) {
                    let cis = cross(axis, self.pos[x] - pa).signum() == side_h;
                    s += vals(b, x)[if cis { 1 } else { 2 }];
                }
                HSite { atom: a, n: n_each, shift: s.clamp(3.5, 9.0), what: if self.h[a] >= 2 { "=CH2" } else { "=CH" }, exch: false, hpos: Some(hp), jn: 0.0 }
            })
            .collect()
    }
}

/// Multiplet name from the couplings (partners, J), largest J first: "d", "dt", "q", "m".
fn mult_name(groups: &[(usize, f64)]) -> String {
    let letter = |n: usize| match n {
        1 => Some("d"),
        2 => Some("t"),
        3 => Some("q"),
        4 => Some("quint"),
        5 => Some("sext"),
        6 => Some("sept"),
        _ => None,
    };
    match groups.len() {
        0 => "s".into(),
        1 => letter(groups[0].0).unwrap_or("m").into(),
        2 => match (letter(groups[0].0), letter(groups[1].0)) {
            (Some(x), Some(y)) if x.len() == 1 && y.len() == 1 => format!("{}{}", x, y),
            _ => "m".into(),
        },
        _ => "m".into(),
    }
}

impl Mol {
    /// Couplings of one site with the hydrogens on neighbouring atoms, merged by J.
    fn couplings(&self, s: &HSite, sites: &[HSite]) -> Vec<(usize, f64)> {
        let a = s.atom;
        // Exchanging OH/NH: no resolved coupling (¹⁵N splitting is added by the caller).
        if s.exch {
            return vec![];
        }
        let mut by_j: Vec<(usize, f64)> = Vec::new();
        for b in self.nbrs(a) {
            // Carbon neighbours, and amide N–H, which couples to N–CH (J ≈ 5.8 Hz).
            if self.el[b] != 6 && !(self.el[b] == 7 && self.el[a] == 6) && !(self.el[a] == 7 && self.el[b] == 6) {
                continue;
            }
            for t in sites.iter().filter(|t| t.atom == b && !t.exch) {
                // Equivalent hydrogens don't split each other.
                if self.class[b] == self.class[a] && (t.shift - s.shift).abs() < 0.01 {
                    continue;
                }
                let j = if self.el[a] == 7 || self.el[b] == 7 {
                    if self.carbonyl(a) || self.carbonyl(b) { 1.5 } else { 5.8 }
                } else if self.arom[a] && self.arom[b] && self.is_arom_bond(a, b) {
                    if self.aring_of(a).is_some_and(|r| r.len() == 5) { 3.5 } else { 7.8 }
                } else if self.order(a, b) == 2 {
                    match (s.hpos, t.hpos) {
                        (Some(p), Some(q)) => {
                            let axis = self.pos[b] - self.pos[a];
                            let same = cross(axis, p - self.pos[a]).signum() == cross(axis, q - self.pos[a]).signum();
                            if same { 10.5 } else { 16.5 }
                        }
                        _ => 12.0,
                    }
                } else if self.carbonyl(a) || self.carbonyl(b) {
                    2.0
                } else if self.sp3(a) && self.sp3(b) {
                    7.0
                } else {
                    6.8
                };
                // Couplings within about 1 Hz look alike (N–CH2–CH2: apparent quartet).
                match by_j.iter_mut().find(|g| (g.1 - j).abs() < 1.1) {
                    Some(g) => g.0 += t.n,
                    None => by_j.push((t.n, j)),
                }
            }
        }
        // Phosphorus: ¹J for H on P itself, ²J and ³J through one or two bonds.
        let mut add = |n: usize, j: f64| match by_j.iter_mut().find(|g| (g.1 - j).abs() < 0.6) {
            Some(g) => g.0 += n,
            None => by_j.push((n, j)),
        };
        // H on a ¹³C-labelled carbon: ¹J(C,H), larger for sp2 and sp carbons.
        if self.el[a] == 6 && self.iso[a] == 13 {
            add(1, if self.carbonyl(a) {
                172.0
            } else if self.adj[a].iter().any(|(_, o)| *o == 3) {
                250.0
            } else if self.arom[a] || self.adj[a].iter().any(|(_, o)| *o == 2) {
                160.0
            } else {
                127.0
            });
        }
        if self.el[a] == 15 {
            add(1, match self.p_kind(a) {
                PKind::P3 => 200.0,
                PKind::Ester => 690.0,
                _ => 480.0,
            });
        }
        for (p, d) in self.p_near(a) {
            if let Some(j) = self.j_ph(p, d, a) {
                add(1, j);
            }
        }
        by_j.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap());
        by_j
    }

    fn h_peaks(&self) -> Vec<Peak> {
        let mut sites = self.h_sites();
        // Measured shifts for C–H with the same surroundings (not for the two different H of an
        // alkene CH2, nor for OH/NH).
        let mut learned = vec![0u16; sites.len()];
        for (k, s) in sites.iter_mut().enumerate() {
            if self.el[s.atom] == 6 && !s.exch && (s.hpos.is_none() || self.h[s.atom] == 1) {
                if let Some((c, n)) = self.db_shift(1, s.atom) {
                    s.shift += c;
                    learned[k] = n;
                }
            }
        }
        let mut peaks: Vec<(u64, Peak)> = Vec::new();
        for (si, s) in sites.iter().enumerate() {
            let key = self.class[s.atom];
            if let Some((_, p)) = peaks.iter_mut().find(|(k, p)| *k == key && (p.shift - s.shift).abs() < 0.005) {
                p.count += s.n;
                if !p.atoms.contains(&self.owner[s.atom]) {
                    p.atoms.push(self.owner[s.atom]);
                }
                continue;
            }
            let groups = self.couplings(s, &sites);
            // H on ¹⁵N: the labelled molecules give a doublet (¹J ≈ 75–97 Hz), the unlabelled
            // ones a peak at the same shift without it. At about 50% enrichment the two
            // overlap as a pseudo-triplet.
            if s.jn > 0.0 {
                let e = self.enrich.clamp(0.0, 1.0);
                let mut with_n = groups.clone();
                with_n.push((1, s.jn));
                with_n.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap());
                let name = |g: &[(usize, f64)]| if s.exch && g.len() <= 1 && g.first().is_none_or(|x| x.1 > 50.0) {
                    if g.is_empty() { "br s".to_string() } else { "br d".to_string() }
                } else {
                    mult_name(g)
                };
                let make = |g: Vec<(usize, f64)>, frac: f64| Peak {
                    shift: s.shift,
                    atoms: vec![self.owner[s.atom]],
                    count: s.n,
                    mult: name(&g),
                    j: g.iter().map(|x| x.1).collect(),
                    pattern: g,
                    what: format!("{} (¹⁵N)", s.what),
                    frac,
                    learned: 0,
                };
                peaks.push((key, make(with_n, e)));
                if e < 0.995 {
                    let mut p = make(groups.clone(), 1.0 - e);
                    p.what = s.what.to_string();
                    peaks.push((key ^ 0x5bd1e995, p));
                }
                continue;
            }
            // Several different neighbours with similar J give an unresolved multiplet
            // (CH2 in a chain), unless they are all alike (isopropyl CH: septet).
            let partner_classes: HashSet<u64> = self.nbrs(s.atom).filter(|b| self.el[*b] == 6 && self.h[*b] > 0 && self.class[*b] != self.class[s.atom]).map(|b| self.class[b]).collect();
            let total: usize = groups.iter().map(|g| g.0).sum();
            let mult = if s.exch {
                if groups.is_empty() { "br s".to_string() } else { "br d".to_string() }
            } else if partner_classes.len() >= 2 && total >= 4 && self.sp3(s.atom) {
                "m".to_string()
            } else {
                mult_name(&groups)
            };
            peaks.push((
                key,
                Peak { shift: s.shift, atoms: vec![self.owner[s.atom]], count: s.n, mult, j: groups.iter().map(|g| g.1).collect(), pattern: groups, what: s.what.to_string(), frac: 1.0, learned: learned[si] },
            ));
        }
        let mut out: Vec<Peak> = peaks.into_iter().map(|x| x.1).collect();
        out.sort_by(|a, b| b.shift.partial_cmp(&a.shift).unwrap());
        out
    }

    // ---------------------------------------------------------------- ¹³C

    fn c_shift(&self, a: usize) -> Option<(f64, &'static str)> {
        if self.el[a] != 6 || !self.alive[a] {
            return None;
        }
        let nb: Vec<usize> = self.nbrs(a).collect();
        let conj = |x: usize| self.arom[x] || self.adj[x].iter().any(|(_, o)| *o >= 2);
        if self.carbonyl(a) {
            let rest: Vec<usize> = nb.iter().copied().filter(|y| !(self.el[*y] == 8 && self.order(a, *y) == 2)).collect();
            let o: Vec<usize> = rest.iter().copied().filter(|y| self.el[*y] == 8).collect();
            let n: Vec<usize> = rest.iter().copied().filter(|y| self.el[*y] == 7).collect();
            let c_conj = rest.iter().filter(|y| self.el[**y] == 6 && conj(**y)).count();
            let s = match (o.len(), n.len()) {
                (2, _) => 155.0,
                (1, 1) => 156.0,
                (0, 2) => 158.0,
                (1, 0) => {
                    let oo = o[0];
                    if self.h[oo] > 0 || self.adj[oo].len() == 1 {
                        if c_conj > 0 { 172.0 } else { 178.0 }
                    } else if self.nbrs(oo).any(|z| z != a && self.carbonyl(z)) {
                        167.0
                    } else if self.h[a] > 0 {
                        161.0
                    } else if c_conj > 0 {
                        166.5
                    } else {
                        171.0
                    }
                }
                (0, 1) => {
                    if self.h[a] > 0 {
                        162.0
                    } else if c_conj > 0 {
                        168.0
                    } else {
                        172.0
                    }
                }
                _ => {
                    if rest.iter().any(|y| self.el[*y] == 17) {
                        170.0
                    } else if self.h[a] > 0 {
                        if c_conj > 0 { 192.0 } else { 201.0 }
                    } else {
                        match c_conj {
                            0 => match self.ring_of(a).map(|r| r.len()) {
                                Some(5) => 219.0,
                                Some(6) => 211.0,
                                _ => 208.0,
                            },
                            1 => 198.0,
                            _ => 196.5,
                        }
                    }
                }
            };
            return Some((s, "C=O"));
        }
        if self.adj[a].iter().any(|(y, o)| *o == 3 && self.el[*y] == 7) {
            let ar = nb.iter().any(|y| self.arom[*y]);
            return Some((if ar { 118.8 } else { 119.5 }, "C≡N"));
        }
        if self.arom[a] {
            return Some((self.aromatic_c(a), "Ar C"));
        }
        if self.adj[a].iter().filter(|(_, o)| *o == 2).count() >= 2 {
            return Some((210.0, "=C="));
        }
        if self.adj[a].iter().any(|(y, o)| *o == 2 && self.el[*y] == 7) {
            let oxime = self.adj[a].iter().any(|(y, o)| *o == 2 && self.el[*y] == 7 && self.nbrs(*y).any(|z| self.el[z] == 8));
            return Some((if oxime { 150.0 } else { 160.0 }, "C=N"));
        }
        if self.adj[a].iter().any(|(y, o)| *o == 2 && self.el[*y] == 16) {
            return Some((200.0, "C=S"));
        }
        if self.arom[a] {
            return Some((self.aromatic_c(a), "Ar C"));
        }
        if let Some(b) = self.adj[a].iter().find(|(_, o)| *o == 3).map(|x| x.0) {
            let mut s: f64 = 72.0;
            for x in nb.iter().copied().filter(|x| *x != b && self.el[*x] > 1) {
                s += match self.sub(a, x) {
                    Sub::Alkyl => 10.5,
                    Sub::Aryl | Sub::Vinyl => 11.5,
                    Sub::Cor | Sub::Coor | Sub::Cooh | Sub::Cho => 6.0,
                    _ => 8.0,
                };
            }
            for x in self.nbrs(b).filter(|x| *x != a && self.el[*x] > 1) {
                s += match self.sub(b, x) {
                    Sub::Alkyl => -5.5,
                    Sub::Aryl | Sub::Vinyl => 5.2,
                    Sub::Cor | Sub::Coor | Sub::Cooh | Sub::Cho => 8.0,
                    _ => 0.0,
                };
            }
            return Some((s, "C≡C"));
        }
        if let Some(b) = self.adj[a].iter().find(|(y, o)| *o == 2 && self.el[*y] == 6).map(|x| x.0) {
            let mut s = 123.3;
            // The carbon's own side: α, β, γ; the other end's: α', β', γ'.
            for (c, sign) in [(a, 0usize), (b, 1)] {
                let other = if c == a { b } else { a };
                for x in self.nbrs(c).filter(|x| *x != other && self.el[*x] > 1) {
                    let k = self.sub(c, x);
                    if k == Sub::Alkyl && self.el[x] == 6 {
                        s += [10.6, -7.9][sign];
                        for y in self.nbrs(x).filter(|y| *y != c && self.el[*y] > 1) {
                            s += [7.2, -1.8][sign];
                            for _z in self.nbrs(y).filter(|z| *z != x && self.el[*z] > 1) {
                                s += [-1.5, 1.5][sign];
                            }
                        }
                    } else {
                        s += incr(k).al_c[sign];
                    }
                }
            }
            return Some((s, "=C"));
        }
        Some((self.sp3_c(a), match self.h[a] {
            3 => "CH3",
            2 => "CH2",
            1 => "CH",
            _ => "C",
        }))
    }

    /// sp3 carbon: Grant–Paul counts of carbons 1–4 bonds away (through sp3 carbons), the α/β/γ
    /// increments of other groups, branching and ring corrections.
    fn sp3_c(&self, a: usize) -> f64 {
        let mut s = -2.3;
        let gp = [9.1, 9.4, -2.5, 0.3];
        let mut dist: HashMap<usize, usize> = HashMap::new();
        dist.insert(a, 0);
        let mut q = VecDeque::new();
        q.push_back(a);
        while let Some(u) = q.pop_front() {
            let d = dist[&u];
            if d >= 4 {
                continue;
            }
            for v in self.nbrs(u) {
                if dist.contains_key(&v) || self.el[v] <= 1 {
                    continue;
                }
                dist.insert(v, d + 1);
                if self.sp3(v) {
                    s += gp[d];
                    q.push_back(v);
                } else if d < 3 {
                    let mut x = incr(self.sub(u, v)).c[d];
                    // Heteroatoms on an inner carbon add less than on a chain end.
                    if d == 0 && matches!(self.el[v], 7 | 8) && self.nbrs(a).filter(|y| self.el[*y] == 6).count() >= 2 {
                        x -= 6.0;
                    }
                    s += x;
                }
            }
        }
        // Branching corrections (Grant–Paul), by numbers of carbon neighbours.
        let cdeg = |x: usize| self.nbrs(x).filter(|y| self.el[*y] == 6).count();
        let da = cdeg(a);
        for x in self.nbrs(a).filter(|x| self.sp3(*x)) {
            s += match (da, cdeg(x)) {
                (1, 3) => -1.1,
                (1, 4) => -3.4,
                (2, 3) => -2.5,
                (2, 4) => -7.2,
                (3, 2) => -3.7,
                (3, 3) => -9.5,
                (3, 4) => -15.0,
                (4, 1) => -1.5,
                (4, 2) => -8.4,
                (4, 3) => -15.0,
                (4, 4) => -25.0,
                _ => 0.0,
            };
        }
        if let Some(r) = self.ring_of(a) {
            s += match r.len() {
                3 => -18.7,
                4 => -3.0,
                5 => -9.0,
                6 => -5.0,
                7 => -1.3,
                _ => 0.0,
            };
        }
        s.clamp(-10.0, 110.0)
    }

    /// Aromatic carbon: base value for its ring and position plus ipso/ortho/meta/para
    /// increments (scaled down on heteroaromatic rings).
    fn aromatic_c(&self, a: usize) -> f64 {
        let Some(r) = self.aring_of(a).cloned() else { return 128.5 };
        let s = self.fused_with(&r);
        let het: Vec<usize> = r.iter().copied().filter(|x| self.hetero(*x)).collect();
        let junction = self.arings.iter().filter(|q| q.contains(&a)).count() > 1;
        let next_to_het = self.nbrs(a).any(|x| self.hetero(x) && s.contains(&x));
        let mut base = if junction {
            if next_to_het { 137.0 } else { 131.0 }
        } else if r.len() == 6 {
            if het.is_empty() {
                if s.len() > r.len() { 127.0 } else { 128.5 }
            } else {
                let near = het.iter().filter(|x| self.ring_dist(&r, a, **x) == 1).count();
                let d = het.iter().map(|x| self.ring_dist(&r, a, *x)).min().unwrap_or(3);
                match (near, d) {
                    (2.., _) => 158.0,
                    (_, 1) => 150.0,
                    (_, 2) => 124.0,
                    _ => 136.0,
                }
            }
        } else {
            let adj_het: Vec<usize> = het.iter().copied().filter(|x| self.ring_dist(&r, a, *x) == 1).collect();
            let kind = |x: usize| match self.el[x] {
                8 => 0,
                16 => 1,
                7 if self.adj[x].len() >= 3 || self.h[x] > 0 => 2,
                7 => 3,
                _ => 4,
            };
            match adj_het.len() {
                2.. => {
                    if adj_het.iter().all(|x| self.el[*x] == 7) {
                        136.0
                    } else {
                        152.0
                    }
                }
                1 => [143.0, 125.5, 118.0, 140.0, 130.0][kind(adj_het[0])],
                _ => het.first().map(|x| [110.0, 127.0, 108.0, 122.0, 125.0][kind(*x)]).unwrap_or(125.0),
            }
        };
        let scale = if het.is_empty() { 1.0 } else { 0.8 };
        for &q in &r {
            let d = self.ring_dist(&r, a, q);
            if d > 3 {
                continue;
            }
            for x in self.nbrs(q) {
                if s.contains(&x) || self.el[x] <= 1 {
                    continue;
                }
                base += scale * incr(self.sub(q, x)).ar_c[d];
            }
        }
        base
    }

    fn c_peaks(&self) -> Vec<Peak> {
        let mut peaks: Vec<(u64, Peak)> = Vec::new();
        for a in 0..self.el.len() {
            let Some((shift, what)) = self.c_shift(a) else { continue };
            // A measured shift for the same surroundings, if the database has one.
            let (shift, learned) = match self.db_shift(13, a) {
                Some((c, n)) => (shift + c, n),
                None => (shift, 0),
            };
            // Deuterium moves its carbon about 0.3 ppm upfield per D.
            let shift = shift - 0.3 * self.dn[a] as f64;
            let what = if self.iso[a] == 13 { format!("{} (¹³C)", what) } else { what.to_string() };
            let key = self.class[a];
            if let Some((_, p)) = peaks.iter_mut().find(|(k, p)| *k == key && (p.shift - shift).abs() < 0.05) {
                p.count += 1;
                if !p.atoms.contains(&self.owner[a]) {
                    p.atoms.push(self.owner[a]);
                }
                continue;
            }
            // Splitting by fluorine on the carbon itself (proton-decoupled spectrum).
            let nf = self.nbrs(a).filter(|x| self.el[*x] == 9).count();
            let mut pattern: Vec<(usize, f64)> = match nf {
                0 => vec![],
                1 => vec![(1, if self.arom[a] { 245.0 } else { 165.0 })],
                2 => vec![(2, 240.0)],
                n => vec![(n, 272.0)],
            };
            // Splitting by phosphorus 1–3 bonds away.
            for (p, d) in self.p_near(a) {
                if let Some(j) = self.j_pc(p, d, a) {
                    pattern.push((1, j));
                }
            }
            // Deuterium (spin 1): CD a 1:1:1 triplet, CD2 a quintet, CD3 a septet.
            if self.dn[a] > 0 {
                pattern.push((2 * self.dn[a], if self.adj[a].iter().any(|(_, o)| *o >= 2) || self.arom[a] { 24.0 } else { 19.5 }));
            }
            // Labelled neighbours: ¹³C–¹³C (both labelled) and ¹³C–¹⁵N.
            let mut n15: Vec<(usize, f64)> = Vec::new();
            for y in self.nbrs(a) {
                if self.iso[a] == 13 && self.el[y] == 6 && self.iso[y] == 13 {
                    pattern.push((1, if self.carbonyl(a) || self.carbonyl(y) { 55.0 } else if self.arom[a] && self.arom[y] { 56.0 } else { 35.0 }));
                }
                if self.el[y] == 7 && self.iso[y] == 15 {
                    n15.push((1, if self.carbonyl(a) { 15.0 } else { 7.0 }));
                }
            }
            // ¹⁵N splitting only in the labelled share of the molecules.
            let e = if n15.is_empty() { 1.0 } else { self.enrich.clamp(0.0, 1.0) };
            if e < 0.995 {
                let mut plain = pattern.clone();
                plain.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap());
                let mult = mult_name(&plain);
                let j = plain.iter().map(|g| g.1).collect();
                peaks.push((key ^ 0x5bd1e995, Peak { shift, atoms: vec![self.owner[a]], count: 1, mult, j, pattern: plain, what: what.clone(), frac: 1.0 - e, learned: 0 }));
            }
            pattern.extend(n15);
            pattern.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap());
            let mult = mult_name(&pattern);
            let j = pattern.iter().map(|g| g.1).collect();
            peaks.push((key, Peak { shift, atoms: vec![self.owner[a]], count: 1, mult, j, pattern, what, frac: e, learned }));
        }
        let mut out: Vec<Peak> = peaks.into_iter().map(|x| x.1).collect();
        out.sort_by(|a, b| b.shift.partial_cmp(&a.shift).unwrap());
        out
    }
}

/// Kind of phosphorus centre, for shifts and coupling constants.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PKind {
    /// Phosphine, phosphite, aminophosphine (P(III)).
    P3,
    /// Phosphonium (four single bonds, positive).
    Plus,
    /// Phosphine oxide or phosphinate (P=O with at most one O–R).
    Oxide,
    /// Phosphonate or phosphate (P=O with two or three O–R).
    Ester,
    /// P=S compounds.
    Sulfide,
}

impl Mol {
    fn p_kind(&self, p: usize) -> PKind {
        let dbl_o = self.adj[p].iter().any(|(y, o)| *o >= 2 && self.el[*y] == 8);
        let dbl_s = self.adj[p].iter().any(|(y, o)| *o >= 2 && self.el[*y] == 16);
        let single_o = self.adj[p].iter().filter(|(y, o)| *o == 1 && self.el[*y] == 8).count();
        if dbl_o {
            if single_o >= 2 { PKind::Ester } else { PKind::Oxide }
        } else if dbl_s {
            PKind::Sulfide
        } else if self.charge[p] > 0 && self.adj[p].len() + self.h[p] >= 4 {
            PKind::Plus
        } else {
            PKind::P3
        }
    }

    /// ¹J(P,F) for phosphorus `p`: PF6⁻ 711 Hz, PF5 938 Hz, P(III)–F 1400 Hz, other P(V)–F 1000 Hz.
    fn j_pf(&self, p: usize) -> f64 {
        let nf = self.nbrs(p).filter(|y| self.el[*y] == 9).count();
        let sigma = self.adj[p].iter().filter(|x| x.1 == 1).count() + self.h[p];
        let dbl = self.adj[p].iter().any(|x| x.1 >= 2);
        if nf >= 6 {
            711.0
        } else if !dbl && sigma == 5 {
            938.0
        } else if !dbl && sigma <= 3 {
            1400.0
        } else {
            1000.0
        }
    }

    /// Phosphorus atoms 1–3 bonds from `a`, with their distance.
    fn p_near(&self, a: usize) -> Vec<(usize, usize)> {
        let mut dist: HashMap<usize, usize> = HashMap::new();
        dist.insert(a, 0);
        let mut q = VecDeque::from([a]);
        let mut out = Vec::new();
        while let Some(u) = q.pop_front() {
            let d = dist[&u];
            if d >= 3 {
                continue;
            }
            for v in self.nbrs(u) {
                if dist.contains_key(&v) {
                    continue;
                }
                dist.insert(v, d + 1);
                if self.el[v] == 15 {
                    out.push((v, d + 1));
                } else {
                    q.push_back(v);
                }
            }
        }
        out
    }

    /// J(P,H) for a hydrogen on atom `a`, `d` bonds from phosphorus `p` (so (d+1)J).
    fn j_ph(&self, p: usize, d: usize, a: usize) -> Option<f64> {
        let k = self.p_kind(p);
        match d {
            1 => Some(match k {
                PKind::P3 => 3.0,
                PKind::Ester => 17.5,
                _ => 13.0,
            }),
            // ³J: aryl H ortho to P(III) ≈ 4 Hz (2-(Ph2P)benzoates 3.6–3.9), to P=O 13.5
            // (2-(Ph2PO)benzamide 14.5, 13.6), to P⁺ 12.5.
            2 => Some(match (k, self.arom[a]) {
                (PKind::P3, true) => 4.0,
                (PKind::P3, false) => 7.0,
                (PKind::Oxide, true) => 13.5,
                (PKind::Ester, false) => 8.0,
                (_, _) => 12.5,
            }),
            // ⁴J: aryl H meta to P, 3.1–4.1 Hz (seen as the small extra splitting of
            // phosphine, phosphine oxide and phosphonium aryl rings).
            3 if self.arom[a] => Some(match k {
                PKind::P3 => 3.7,
                PKind::Ester => 1.5,
                _ => 3.5,
            }),
            _ => None,
        }
    }

    /// J(P,C) for a carbon `d` bonds from phosphorus `p`.
    fn j_pc(&self, p: usize, d: usize, a: usize) -> Option<f64> {
        let k = self.p_kind(p);
        let ar = self.arom[a];
        Some(match d {
            1 => match k {
                PKind::P3 => if ar { 11.0 } else { 14.0 },
                PKind::Plus => if ar { 88.0 } else { 55.0 },
                PKind::Oxide => if ar { 104.0 } else { 73.0 },
                PKind::Ester => if ar { 188.0 } else { 140.0 },
                PKind::Sulfide => 80.0,
            },
            2 => match k {
                PKind::P3 => 19.0,
                PKind::Ester => 6.5,
                _ => 10.0,
            },
            3 => match k {
                PKind::P3 => 7.0,
                PKind::Ester => 6.0,
                _ => 12.5,
            },
            _ => return None,
        })
    }

    // ---------------------------------------------------------------- ¹⁹F

    /// ¹⁹F shift (ppm from CFCl3) of fluorine atom `a`.
    fn f_shift(&self, a: usize) -> (f64, &'static str) {
        let Some(x) = self.nbrs(a).next() else { return (if self.h[a] > 0 { -204.0 } else { -120.0 }, "F⁻") };
        match self.el[x] {
            6 if self.arom[x] => {
                let r = self.aring_of(x).cloned().unwrap_or_default();
                let het: Vec<usize> = r.iter().copied().filter(|y| self.hetero(*y)).collect();
                let mut s = if r.len() == 6 && !het.is_empty() {
                    match het.iter().map(|y| self.ring_dist(&r, x, *y)).min().unwrap_or(3) {
                        1 => -68.0,
                        2 => -128.0,
                        _ => -103.0,
                    }
                } else if r.len() == 5 {
                    -125.0
                } else {
                    -113.1
                };
                let fused = self.fused_with(&r);
                for &q in &r {
                    let d = self.ring_dist(&r, x, q);
                    if !(1..=3).contains(&d) {
                        continue;
                    }
                    for y in self.nbrs(q) {
                        if fused.contains(&y) || self.el[y] <= 1 {
                            continue;
                        }
                        s += f_ar(self.sub(q, y))[d - 1];
                    }
                }
                (s, "ArF")
            }
            6 if self.carbonyl(x) => (if self.nbrs(x).any(|y| self.arom[y]) { 18.0 } else { 45.0 }, "C(O)F"),
            6 if self.adj[x].iter().any(|(_, o)| *o >= 2) => (-100.0, "=CF"),
            6 => {
                let nf = self.nbrs(x).filter(|y| self.el[*y] == 9).count();
                let others: Vec<usize> = self.nbrs(x).filter(|y| self.el[*y] != 9).collect();
                let perfluoro = |y: usize| self.el[y] == 6 && self.nbrs(y).filter(|z| self.el[*z] == 9).count() >= 2;
                let o_beta = |y: usize| self.el[y] == 6 && self.nbrs(y).any(|z| z != x && matches!(self.el[z], 7 | 8));
                match nf {
                    4.. => (-62.0, "CF4"),
                    3 => {
                        let s = match others.first() {
                            None => -78.0,
                            Some(&y) if self.arom[y] => -63.0,
                            Some(&y) if self.carbonyl(y) => -76.5,
                            Some(&y) if self.el[y] == 8 => -58.0,
                            Some(&y) if self.el[y] == 16 => {
                                if self.adj[y].iter().any(|(z, o)| *o == 2 && self.el[*z] == 8) { -78.0 } else { -43.0 }
                            }
                            Some(&y) if self.el[y] == 7 => -55.0,
                            Some(&y) if perfluoro(y) => -81.0,
                            Some(&y) if o_beta(y) => -77.0,
                            Some(&y) if self.adj[y].iter().any(|(_, o)| *o == 2) => -65.0,
                            Some(_) => -66.0,
                        };
                        (s, "CF3")
                    }
                    2 => {
                        let s = if self.h[x] > 0 {
                            match others.first() {
                                Some(&y) if self.arom[y] => -110.0,
                                Some(&y) if self.el[y] == 8 => -82.0,
                                Some(&y) if self.carbonyl(y) => -127.0,
                                _ => -120.0,
                            }
                        } else if !others.is_empty() && others.iter().all(|y| perfluoro(*y)) {
                            -123.0
                        } else if others.iter().any(|y| perfluoro(*y)) {
                            -115.0
                        } else if others.iter().any(|y| self.arom[*y]) {
                            -90.0
                        } else if others.iter().any(|y| self.carbonyl(*y)) {
                            -105.0
                        } else if others.iter().any(|y| self.el[*y] == 8) {
                            -80.0
                        } else {
                            -98.0
                        };
                        (s, if self.h[x] > 0 { "CHF2" } else { "CF2" })
                    }
                    _ => {
                        let ar = others.iter().any(|y| self.arom[*y]);
                        match self.h[x] {
                            3.. => (-272.0, "CH3F"),
                            2 => (if ar { -207.0 } else { -218.0 }, "CH2F"),
                            1 => (if ar { -168.0 } else { -178.0 }, "CHF"),
                            _ => (-138.0, "CF"),
                        }
                    }
                }
            }
            5 => (-152.0, "BF"),
            14 => (-130.0, "SiF"),
            15 => {
                match self.nbrs(x).filter(|y| self.el[*y] == 9).count() {
                    6.. => (-72.0, "PF6⁻"),
                    5 => (-71.5, "PF5"),
                    _ if !self.adj[x].iter().any(|y| y.1 >= 2) && self.adj[x].len() <= 3 => (-34.0, "P(III)–F"),
                    _ => (-75.0, "PF"),
                }
            }
            16 => {
                let nf = self.nbrs(x).filter(|y| self.el[*y] == 9).count();
                if nf >= 6 {
                    (57.0, "SF6")
                } else if nf >= 5 {
                    (75.0, "SF5")
                } else if self.nbrs(x).any(|y| self.arom[y]) {
                    (66.0, "SO2F")
                } else {
                    (52.0, "SO2F")
                }
            }
            7 => (40.0, "NF"),
            _ => (-150.0, "F"),
        }
    }

    /// Couplings of a fluorine with hydrogens 2–4 bonds away and with phosphorus.
    fn f_couplings(&self, a: usize) -> Vec<(usize, f64)> {
        let mut by_j: Vec<(usize, f64)> = Vec::new();
        let mut add = |n: usize, j: f64| {
            if n == 0 {
                return;
            }
            match by_j.iter_mut().find(|g| (g.1 - j).abs() < 0.6) {
                Some(g) => g.0 += n,
                None => by_j.push((n, j)),
            }
        };
        let Some(x) = self.nbrs(a).next() else { return vec![] };
        let nf = self.nbrs(x).filter(|y| self.el[*y] == 9).count();
        if self.el[x] == 15 {
            add(1, self.j_pf(x));
        }
        if self.el[x] == 6 {
            // Geminal H.
            add(self.h[x], match nf {
                1 => if self.h[x] >= 2 { 47.0 } else { 48.0 },
                _ => 56.0,
            });
            for y in self.nbrs(x).filter(|y| *y != a && self.el[*y] == 6) {
                if self.arom[x] && self.arom[y] && self.is_arom_bond(x, y) {
                    // Ortho H, then meta H.
                    add(self.h[y], 9.0);
                    for z in self.nbrs(y).filter(|z| *z != x && self.arom[*z] && self.is_arom_bond(y, *z)) {
                        add(self.h[z], 5.5);
                    }
                } else if self.sp3(x) && self.sp3(y) {
                    add(self.h[y], if nf >= 3 { 9.0 } else { 22.0 });
                }
            }
            for (_, d) in self.p_near(a).into_iter().filter(|(_, d)| *d <= 3) {
                add(1, [0.0, 0.0, 80.0, 10.0][d]);
            }
        }
        by_j.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap());
        by_j
    }

    fn f_peaks(&self) -> Vec<Peak> {
        let mut peaks: Vec<(u64, Peak)> = Vec::new();
        for a in (0..self.el.len()).filter(|&a| self.alive[a] && self.el[a] == 9) {
            let (shift, what) = self.f_shift(a);
            let key = self.class[a];
            if let Some((_, p)) = peaks.iter_mut().find(|(k, p)| *k == key && (p.shift - shift).abs() < 0.05) {
                p.count += 1;
                if !p.atoms.contains(&self.owner[a]) {
                    p.atoms.push(self.owner[a]);
                }
                continue;
            }
            let groups = self.f_couplings(a);
            let mult = mult_name(&groups);
            peaks.push((key, Peak { shift, atoms: vec![self.owner[a]], count: 1, mult, j: groups.iter().map(|g| g.1).collect(), pattern: groups, what: what.into(), frac: 1.0, learned: 0 }));
        }
        let mut out: Vec<Peak> = peaks.into_iter().map(|x| x.1).collect();
        out.sort_by(|a, b| b.shift.partial_cmp(&a.shift).unwrap());
        out
    }

    // ---------------------------------------------------------------- ³¹P

    /// ³¹P shift (ppm from 85% H3PO4) of phosphorus atom `p`.
    fn p_shift(&self, p: usize) -> (f64, &'static str) {
        // Aza-ylides (iminophosphoranes, P=N, about +9.5 ppm) and P=C ylides.
        if self.adj[p].iter().any(|(y, o)| *o >= 2 && self.el[*y] == 7) {
            return (9.5, "aza-ylide P=N");
        }
        if self.adj[p].iter().any(|(y, o)| *o >= 2 && self.el[*y] == 6) {
            return (20.0, "ylide P=C");
        }
        let single: Vec<usize> = self.adj[p].iter().filter(|x| x.1 == 1).map(|x| x.0).collect();
        let n_el = |z: u8| single.iter().filter(|y| self.el[**y] == z).count();
        let (n_o, n_n, n_f) = (n_el(8), n_el(7), n_el(9));
        let n_hal = single.iter().filter(|y| matches!(self.el[**y], 17 | 35 | 53)).count();
        let carbons: Vec<usize> = single.iter().copied().filter(|y| self.el[*y] == 6).collect();
        let nh = self.h[p];
        if n_f >= 6 {
            return (-144.0, "PF6⁻");
        }
        // Five or six single bonds, no P=X: phosphoranes (PF5, PCl5, RPF4) and PX6⁻.
        let has_double = self.adj[p].iter().any(|x| x.1 >= 2);
        let sigma = single.len() + nh;
        if !has_double && sigma >= 5 {
            let s = match (sigma, n_f, n_hal) {
                (6, _, 6) => -296.0,
                (6, _, _) => -150.0,
                (_, 5, _) => -80.0,
                (_, _, 5) => -80.0,
                (_, f, _) if f > 0 => -30.0 - 10.0 * f as f64,
                _ => -50.0,
            };
            return (s, if sigma >= 6 { "PX6⁻" } else { "phosphorane" });
        }
        // Carbon groups: (Grim σ for P(III), oxide increment, phosphonium increment).
        let cgroup = |y: usize| -> (f64, f64, f64) {
            if self.arom[y] {
                return (18.6, -2.3, -0.5);
            }
            if self.adj[y].iter().any(|(_, o)| *o >= 2) {
                return (14.0, -4.0, -5.0);
            }
            let cs = self.nbrs(y).filter(|z| *z != p && self.el[*z] == 6).count();
            let benzyl = self.nbrs(y).any(|z| z != p && self.arom[z]);
            let acyl = self.nbrs(y).any(|z| z != p && self.carbonyl(z));
            match cs {
                _ if benzyl => (16.0, -2.0, 0.5),
                _ if acyl => (10.0, -6.0, 0.0),
                0 => (0.0, 0.0, 0.0),
                1 => {
                    let ethyl = self.nbrs(y).filter(|z| *z != p && self.el[*z] == 6).all(|z| self.h[z] == 3);
                    if ethyl { (14.0, 4.0, 3.7) } else { (10.0, 3.0, 2.0) }
                }
                2 => (27.0, 8.0, 6.0),
                _ => (41.7, 12.0, 9.0),
            }
        };
        let o_aryl = single.iter().filter(|y| self.el[**y] == 8 && self.nbrs(**y).any(|z| z != p && self.arom[z])).count();
        let o_p = single.iter().filter(|y| self.el[**y] == 8 && self.nbrs(**y).any(|z| z != p && self.el[z] == 15)).count();
        match self.p_kind(p) {
            PKind::P3 => {
                if n_o + n_n > 0 {
                    let s = match (n_o, n_n) {
                        (3, _) => 140.0 - 4.0 * o_aryl as f64,
                        (2, _) => 157.0,
                        (1, 0) => 110.0,
                        (0, 3) => 123.0,
                        (0, 2) => 100.0,
                        (0, 1) => 62.0,
                        _ => 130.0,
                    } + 26.0 * n_hal as f64;
                    return (s, if n_o > 0 { "phosphite" } else { "aminophosphine" });
                }
                let mut s = -62.0 + carbons.iter().map(|y| cgroup(*y).0).sum::<f64>() - 59.3 * nh as f64;
                s += single.iter().map(|y| match self.el[*y] {
                    17 => 93.7,
                    35 => 96.0,
                    9 => 53.0,
                    _ => 0.0,
                }).sum::<f64>();
                (s, "phosphine")
            }
            PKind::Plus => (25.0 + carbons.iter().map(|y| cgroup(*y).2).sum::<f64>() - 8.0 * nh as f64, "phosphonium"),
            PKind::Ester => {
                let s = if n_o >= 3 {
                    -6.0 * o_aryl as f64 - 11.0 * o_p as f64
                } else if nh > 0 {
                    7.0
                } else if n_hal > 0 {
                    3.0
                } else {
                    // Phosphonate: by the carbon on phosphorus.
                    let base = match carbons.first() {
                        Some(&y) if self.arom[y] => 19.0,
                        Some(&y) if self.adj[y].iter().any(|(_, o)| *o >= 2) => 17.0,
                        Some(&y) if self.nbrs(y).any(|z| z != p && self.carbonyl(z)) => 20.0,
                        Some(&y) if self.nbrs(y).any(|z| z != p && self.arom[z]) => 26.5,
                        _ => 32.0,
                    };
                    base - 4.0 * o_aryl as f64
                };
                (s, if n_o >= 3 { "phosphate" } else { "phosphonate" })
            }
            PKind::Oxide => {
                let s = if n_hal > 0 {
                    if n_hal >= 3 { 2.0 } else if n_o > 0 { 3.0 } else { 42.0 }
                } else if n_n >= 3 {
                    23.0
                } else if n_n > 0 {
                    15.0
                } else if nh > 0 {
                    21.0
                } else if n_o == 1 {
                    50.0 - 9.0 * carbons.iter().filter(|y| self.arom[**y]).count() as f64
                } else {
                    36.0 + carbons.iter().map(|y| cgroup(*y).1).sum::<f64>()
                };
                (s, if n_o == 1 { "phosphinate" } else { "phosphine oxide" })
            }
            PKind::Sulfide => {
                let s = match n_o {
                    3.. => 68.0,
                    2 => 95.0,
                    1 => 80.0,
                    _ => 34.0 + carbons.iter().map(|y| if self.arom[*y] { 3.0 } else { 6.0 }).sum::<f64>().min(30.0),
                };
                (s, "P=S")
            }
        }
    }

    fn p_peaks(&self) -> Vec<Peak> {
        let mut peaks: Vec<(u64, Peak)> = Vec::new();
        for a in (0..self.el.len()).filter(|&a| self.alive[a] && self.el[a] == 15) {
            let (shift, what) = self.p_shift(a);
            let key = self.class[a];
            if let Some((_, p)) = peaks.iter_mut().find(|(k, p)| *k == key && (p.shift - shift).abs() < 0.05) {
                p.count += 1;
                if !p.atoms.contains(&self.owner[a]) {
                    p.atoms.push(self.owner[a]);
                }
                continue;
            }
            // Proton-decoupled: splitting by fluorine on P and by P–O–P neighbours.
            let mut groups: Vec<(usize, f64)> = Vec::new();
            let nf = self.nbrs(a).filter(|y| self.el[*y] == 9).count();
            if nf > 0 {
                groups.push((nf, self.j_pf(a)));
            }
            let pp = self.p_near(a).into_iter().filter(|(q, d)| *d == 2 && self.class[*q] != self.class[a]).count();
            if pp > 0 {
                groups.push((pp, 20.0));
            }
            let mult = mult_name(&groups);
            peaks.push((key, Peak { shift, atoms: vec![self.owner[a]], count: 1, mult, j: groups.iter().map(|g| g.1).collect(), pattern: groups, what: what.into(), frac: 1.0, learned: 0 }));
        }
        let mut out: Vec<Peak> = peaks.into_iter().map(|x| x.1).collect();
        out.sort_by(|a, b| b.shift.partial_cmp(&a.shift).unwrap());
        out
    }
}

impl Mol {
    // ---------------------------------------------------------------- ¹⁵N

    /// ¹⁵N shift (ppm from liquid NH3; subtract 380.5 for the CH3NO2 scale) of nitrogen `a`.
    /// Values from the person's supplementary data where they exist (amides 110.5 / 119.1, alkyl
    /// azide 70.6 / 211.8, aryl azide Nβ 239 / Nγ 238, acyltriazene 464.5 / 220.5, aza-ylide
    /// 103, anilinic NH 65, phosphorane N 170), otherwise standard literature values.
    fn n_shift(&self, a: usize) -> (f64, &'static str) {
        let nb: Vec<usize> = self.nbrs(a).collect();
        let n_n: Vec<usize> = nb.iter().copied().filter(|y| self.el[*y] == 7).collect();
        let dbl = |y: usize| self.order(a, y) >= 2;
        let aryl_on = |x: usize| self.nbrs(x).any(|z| self.arom[z]);
        // N2 gas.
        if nb.len() == 1 && n_n.len() == 1 && self.nbrs(n_n[0]).count() == 1 {
            return (310.0, "N₂");
        }
        // Azide R–N=N=N: terminal Nγ, middle Nβ, Nα on carbon.
        let is_middle = |m: usize| self.el[m] == 7 && self.nbrs(m).filter(|z| self.el[*z] == 7).count() == 2 && self.nbrs(m).count() == 2 && self.nbrs(m).any(|z| self.el[z] == 7 && self.nbrs(z).count() == 1);
        if nb.len() == 1 && n_n.len() == 1 && is_middle(n_n[0]) {
            let alpha = self.nbrs(n_n[0]).find(|z| *z != a).unwrap_or(a);
            return (if aryl_on(alpha) { 238.0 } else { 211.8 }, "azide Nγ");
        }
        if is_middle(a) {
            let alpha = nb.iter().copied().find(|z| self.nbrs(*z).count() > 1).unwrap_or(a);
            return (if aryl_on(alpha) { 239.0 } else { 248.0 }, "azide Nβ");
        }
        if n_n.len() == 1 && is_middle(n_n[0]) {
            return (if nb.iter().any(|y| self.arom[*y]) { 92.0 } else { 70.6 }, "azide Nα");
        }
        // Nitro, nitrile, isonitrile.
        if nb.iter().filter(|y| self.el[**y] == 8).count() >= 2 {
            return (if nb.iter().any(|y| self.arom[*y]) { 370.0 } else { 380.0 }, "NO₂");
        }
        if nb.len() == 1 && self.order(a, nb[0]) == 3 && self.el[nb[0]] == 6 {
            return (if self.nbrs(nb[0]).any(|z| self.arom[z]) { 258.0 } else { 245.0 }, "C≡N");
        }
        // N=N: middle N of a triazene (acyltriazene 464.5), azo N (≈ 510).
        if let Some(&m) = n_n.iter().find(|y| dbl(**y)) {
            let other_n = self.nbrs(m).any(|z| z != a && self.el[z] == 7) || nb.iter().any(|y| *y != m && self.el[*y] == 7);
            return if other_n && self.nbrs(m).any(|z| z != a && self.el[z] == 7) {
                (380.0, "triazene N1")
            } else if other_n {
                (464.5, "triazene N2")
            } else {
                (510.0, "azo N")
            };
        }
        // N single-bonded to an N=N (triazene N3: acyl/aryl 220.5).
        if n_n.iter().any(|y| self.adj[*y].iter().any(|(z, o)| *o >= 2 && self.el[*z] == 7)) {
            return (220.5, "triazene N3");
        }
        // Aromatic nitrogen.
        if self.arom[a] {
            let r = self.aring_of(a).cloned().unwrap_or_default();
            let fused = self.fused_with(&r).len() > r.len();
            let pyrrole_type = nb.len() >= 3 || self.h[a] > 0;
            let other_n = r.iter().filter(|x| **x != a && self.el[**x] == 7).count();
            let s_or_o = r.iter().any(|x| matches!(self.el[*x], 8 | 16));
            return if r.len() >= 6 {
                if self.charge[a] > 0 || nb.len() >= 3 {
                    (210.0, "pyridinium N")
                } else {
                    (317.0 - 17.0 * fused as i32 as f64 - 25.0 * other_n.min(1) as f64, "pyridine-type N")
                }
            } else if pyrrole_type {
                let s = match (other_n, fused) {
                    (0, false) => 147.0,
                    (0, true) => 130.0,
                    (1, _) => 170.0,
                    _ => 200.0,
                };
                (s, "pyrrole-type N")
            } else {
                let adj_n = nb.iter().any(|y| self.el[*y] == 7);
                let s = if adj_n {
                    300.0
                } else if r.iter().any(|x| self.el[*x] == 16) {
                    302.0
                } else if s_or_o {
                    255.0
                } else {
                    250.0
                };
                (s, "azole N")
            };
        }
        // P=N (aza-ylide) and N on phosphorus.
        if nb.iter().any(|y| self.el[*y] == 15 && dbl(*y)) {
            return (if nb.iter().any(|y| self.arom[*y]) { 103.0 } else { 60.0 }, "P=N");
        }
        if let Some(&p) = nb.iter().find(|y| self.el[**y] == 15) {
            let sigma = self.adj[p].iter().filter(|x| x.1 == 1).count() + self.h[p];
            let p5 = self.adj[p].iter().any(|x| x.1 >= 2);
            return if !p5 && sigma >= 5 {
                (170.0, "N on phosphorane")
            } else if p5 {
                (60.0, "P(V)–N")
            } else {
                (75.0, "P(III)–N")
            };
        }
        // C=N double bonds.
        if let Some(&c) = nb.iter().find(|y| self.el[**y] == 6 && dbl(**y)) {
            return if nb.iter().any(|y| self.el[*y] == 8) {
                (365.0, "oxime N")
            } else if nb.iter().any(|y| self.el[*y] == 7) {
                (310.0, "hydrazone N=")
            } else if self.nbrs(c).any(|z| z != a && self.el[z] == 7) {
                (230.0, "amidine N=")
            } else if nb.iter().any(|y| *y != c && self.arom[*y]) {
                (330.0, "imine N")
            } else {
                (320.0, "imine N")
            };
        }
        // Amides, carbamates, ureas, imides, sulfonamides.
        let acyls: Vec<usize> = nb.iter().copied().filter(|y| self.carbonyl(*y)).collect();
        let sulfonyl = nb.iter().any(|y| self.el[*y] == 16 && self.adj[*y].iter().any(|(z, o)| *o == 2 && self.el[*z] == 8));
        let n_aryl = nb.iter().any(|y| self.arom[*y]);
        let carbons = nb.iter().filter(|y| self.el[**y] == 6).count();
        if acyls.len() >= 2 {
            return (170.0, "imide N");
        }
        if let Some(&c) = acyls.first() {
            let o_on_c = self.nbrs(c).any(|z| z != a && self.el[z] == 8 && self.order(c, z) == 1);
            let n_on_c = self.nbrs(c).any(|z| z != a && self.el[z] == 7);
            if o_on_c {
                return (if n_aryl { 105.0 } else { 83.0 }, "carbamate N");
            }
            if n_on_c {
                return (if n_aryl { 105.0 } else { 85.0 }, "urea N");
            }
            let aroyl = self.nbrs(c).any(|z| self.arom[z]);
            let s = match self.h[a] {
                2.. => if aroyl { 110.5 } else { 105.0 },
                1 if n_aryl => 133.0,
                1 => if aroyl { 119.1 } else { 115.0 },
                _ => 105.0,
            };
            let in_ring = self.ring_of(a).is_some();
            return (if in_ring && self.h[a] == 1 { 120.0 } else { s }, match self.h[a] {
                2.. => "primary amide",
                1 => "secondary amide",
                _ => "tertiary amide",
            });
        }
        if sulfonyl {
            return (if n_aryl { 110.0 } else { 95.0 }, "sulfonamide");
        }
        // Hydrazine, hydroxylamine.
        if !n_n.is_empty() {
            return (55.0, "hydrazine N");
        }
        if nb.iter().any(|y| self.el[*y] == 8) {
            return (100.0, "N–O");
        }
        // Ammonium.
        if self.charge[a] > 0 {
            return (if n_aryl { 50.0 } else if self.h[a] == 0 { 45.0 } else { 35.0 }, "ammonium N");
        }
        // Amines: aryl (anilines), N-vinyl, alkyl by substitution and N-methyl groups.
        let aryls = nb.iter().filter(|y| self.arom[**y]).count();
        if aryls >= 2 {
            return (85.0, "diarylamine N");
        }
        if aryls == 1 {
            let s = match self.h[a] {
                2.. => 56.0,
                1 => 65.0,
                _ => 50.0,
            };
            return (s, "arylamine N");
        }
        if nb.iter().any(|y| self.adj[*y].iter().any(|(_, o)| *o >= 2)) {
            return (75.0, "enamine N");
        }
        let methyls = nb.iter().filter(|y| self.el[**y] == 6 && self.h[**y] == 3).count() as f64;
        let s = match carbons {
            0 => 0.0,
            1 => 25.0 - 18.0 * methyls,
            2 => 40.0 - 16.0 * methyls,
            _ => 45.0 - 11.0 * methyls,
        };
        (s, if carbons == 0 { "NH₃" } else { "amine N" })
    }

    /// ¹⁵N{¹H} peaks of the ¹⁵N-labelled nitrogens, split by ³¹P, ¹³C and ¹⁵N neighbours.
    fn n_peaks(&self) -> Vec<Peak> {
        let mut peaks: Vec<(u64, Peak)> = Vec::new();
        for a in (0..self.el.len()).filter(|&a| self.alive[a] && self.el[a] == 7 && self.iso[a] == 15) {
            let (shift, what) = self.n_shift(a);
            let key = self.class[a];
            if let Some((_, p)) = peaks.iter_mut().find(|(k, p)| *k == key && (p.shift - shift).abs() < 0.05) {
                p.count += 1;
                if !p.atoms.contains(&self.owner[a]) {
                    p.atoms.push(self.owner[a]);
                }
                continue;
            }
            let mut groups: Vec<(usize, f64)> = Vec::new();
            for y in self.nbrs(a) {
                let j = match self.el[y] {
                    // ¹J(P,N): phosphorane 38.9 Hz (measured), P=N and other P–N ≈ 30 Hz.
                    15 => {
                        let sigma = self.adj[y].iter().filter(|x| x.1 == 1).count() + self.h[y];
                        let p5 = self.adj[y].iter().any(|x| x.1 >= 2);
                        Some(if !p5 && sigma >= 5 { 38.9 } else { 30.0 })
                    }
                    7 if self.iso[y] == 15 && self.class[y] != self.class[a] => Some(12.0),
                    6 if self.iso[y] == 13 => Some(if self.carbonyl(y) { 15.0 } else { 7.0 }),
                    _ => None,
                };
                if let Some(j) = j {
                    match groups.iter_mut().find(|g| (g.1 - j).abs() < 0.6) {
                        Some(g) => g.0 += 1,
                        None => groups.push((1, j)),
                    }
                }
            }
            groups.sort_by(|x, y| y.1.partial_cmp(&x.1).unwrap());
            let mult = mult_name(&groups);
            peaks.push((key, Peak { shift, atoms: vec![self.owner[a]], count: 1, mult, j: groups.iter().map(|g| g.1).collect(), pattern: groups, what: what.into(), frac: 1.0, learned: 0 }));
        }
        let mut out: Vec<Peak> = peaks.into_iter().map(|x| x.1).collect();
        out.sort_by(|a, b| b.shift.partial_cmp(&a.shift).unwrap());
        out
    }
}

/// ¹⁹F increments on an aryl fluoride for a substituent ortho, meta, para to the F.
fn f_ar(s: Sub) -> [f64; 3] {
    use Sub::*;
    match s {
        Alkyl => [-5.0, -1.0, -5.0],
        Cf3 => [0.0, 3.0, 6.0],
        Vinyl => [0.0, 0.0, -1.0],
        Alkynyl => [0.0, 1.0, 3.0],
        Aryl => [-5.0, 0.0, -3.0],
        Cho => [-9.0, 2.0, 10.0],
        Cor => [-3.0, 2.0, 8.0],
        Cooh | Coor => [-3.0, 2.0, 7.0],
        Conr => [-2.0, 1.0, 4.0],
        Cocl => [0.0, 3.0, 11.0],
        Cn => [5.0, 3.0, 11.0],
        Imine => [-2.0, 1.0, 4.0],
        Oh => [-28.0, 1.0, -11.0],
        Or => [-23.0, 1.0, -11.0],
        Oar => [-15.0, 0.0, -8.0],
        Ocor => [-16.0, 1.0, -4.0],
        Nr => [-22.0, 1.0, -14.0],
        Nar => [-12.0, 0.0, -7.0],
        Nhcor => [-10.0, 0.0, -6.0],
        No2 => [-5.0, 3.0, 10.0],
        F => [-25.0, 3.0, -7.0],
        Cl => [-2.0, 2.0, -2.0],
        Br => [4.0, 2.0, -2.0],
        I => [9.0, 2.0, -2.0],
        Sr => [-5.0, 1.0, -4.0],
        So2r => [6.0, 3.0, 10.0],
        P3 => [0.0, 1.0, 2.0],
        P5 | Pplus => [0.0, 2.0, 7.0],
        Other => [0.0, 0.0, 0.0],
    }
}

/// Predict the spectra for the given atoms of the drawing (a whole structure or several).
pub fn predict(doc: &Doc, atoms: &[usize]) -> Prediction {
    predict_with(doc, atoms, Options::default())
}

/// Conditions of the measurement.
#[derive(Clone, Debug)]
pub struct Options {
    /// Protic deuterated solvent (D2O, CD3OD): OH/NH/SH protons are exchanged.
    pub exchange: bool,
    /// Isotopic enrichment of labelled atoms, 0–1 (1: fully labelled).
    pub enrich: f64,
    /// Measured shifts to use where an atom's surroundings are known: the built-in database
    /// (default), none (rules only), or another one.
    pub db: DbChoice,
}

#[derive(Clone, Debug, Default)]
pub enum DbChoice {
    #[default]
    Builtin,
    None,
    Custom(std::sync::Arc<ShiftDb>),
}

impl Default for Options {
    fn default() -> Self {
        Options { exchange: false, enrich: 1.0, db: DbChoice::Builtin }
    }
}

pub fn predict_with(doc: &Doc, atoms: &[usize], o: Options) -> Prediction {
    let keep: HashSet<usize> = atoms.iter().copied().collect();
    let mut skipped = Vec::new();
    let mut m = Mol::build(doc, &keep, &mut skipped);
    m.exchange = o.exchange;
    m.enrich = o.enrich;
    m.db = match o.db {
        DbChoice::Builtin => ShiftDb::builtin(),
        DbChoice::None => None,
        DbChoice::Custom(d) => Some(d),
    };
    Prediction { h: m.h_peaks(), c: m.c_peaks(), f: m.f_peaks(), p: m.p_peaks(), n: m.n_peaks(), skipped }
}

// ---------------------------------------------------------------------------
// Measured shifts by atom surroundings (HOSE-code style)

/// Corrections to the rule-based ¹H and ¹³C shifts, keyed by an atom's surroundings out to 2 and
/// 3 bonds, learnt from published spectra (see `nmrlearn`): the median of measured − rules.
#[derive(Debug, Default)]
pub struct ShiftDb {
    /// (nucleus 1 or 13, radius, surroundings key) → (median correction: measured − rules, ppm;
    /// number of measurements).
    pub map: HashMap<(u8, u8, u64), (f32, u16)>,
}

/// The built-in database (from assets/nmr_shifts.bin, made by the `build_nmr_db` test).
const BUILTIN_DB: &[u8] = include_bytes!("../assets/nmr_shifts.bin");

impl ShiftDb {
    pub fn builtin() -> Option<std::sync::Arc<ShiftDb>> {
        static DB: std::sync::OnceLock<Option<std::sync::Arc<ShiftDb>>> = std::sync::OnceLock::new();
        DB.get_or_init(|| ShiftDb::from_bytes(BUILTIN_DB).map(std::sync::Arc::new)).clone()
    }

    /// Entries as fixed records (nucleus, radius, key, median, count), deflate-compressed.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut keys: Vec<_> = self.map.keys().copied().collect();
        keys.sort();
        let mut raw = Vec::with_capacity(keys.len() * 16);
        for k in keys {
            let (s, n) = self.map[&k];
            raw.push(k.0);
            raw.push(k.1);
            raw.extend_from_slice(&k.2.to_le_bytes());
            raw.extend_from_slice(&((s * 100.0).round() as i32).to_le_bytes());
            raw.extend_from_slice(&n.to_le_bytes());
        }
        let mut out = b"NMRDB1".to_vec();
        out.extend(miniz_oxide::deflate::compress_to_vec(&raw, 9));
        out
    }

    pub fn from_bytes(b: &[u8]) -> Option<ShiftDb> {
        let body = b.strip_prefix(b"NMRDB1")?;
        let raw = miniz_oxide::inflate::decompress_to_vec(body).ok()?;
        let mut map = HashMap::new();
        for r in raw.chunks_exact(16) {
            let key = u64::from_le_bytes(r[2..10].try_into().ok()?);
            let s = i32::from_le_bytes(r[10..14].try_into().ok()?) as f32 / 100.0;
            let n = u16::from_le_bytes(r[14..16].try_into().ok()?);
            map.insert((r[0], r[1], key), (s, n));
        }
        (!map.is_empty()).then_some(ShiftDb { map })
    }

    /// The measured shift for these surroundings, from the most specific level with enough
    /// data: 4 bonds (1 compound is enough), 3 bonds (2), 2 bonds (3). Returns the shift, the
    /// number of compounds and the radius used.
    ///
    /// Stored are corrections to the rules (measured − rules): from 3 bonds out if known, else
    /// 2, shrunk towards 0 when few measurements support them (× n / (n + 3)). On articles
    /// left out of the database this cut the mean error by about 10 % (¹H) and 5 % (¹³C);
    /// using measured values outright made unseen compounds worse.
    pub fn lookup(&self, nucleus: u8, keys: &[u64; 3]) -> Option<(f64, u16, u8)> {
        for (k, r) in [(keys[1], 3u8), (keys[0], 2)] {
            if let Some(&(c, n)) = self.map.get(&(nucleus, r, k)) {
                // Large corrections usually come from peaks matched to the wrong atom: capped.
                let cap = if nucleus == 1 { 0.5 } else { 3.0 };
                return Some(((c as f64 * n as f64 / (n as f64 + 3.0)).clamp(-cap, cap), n, r));
            }
        }
        None
    }
}

fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

impl Mol {
    /// An atom's description: element, aromatic, hydrogens, charge.
    fn env_desc(&self, x: usize) -> String {
        format!("{}{}{}{}", crate::elements::symbol(self.el[x]), if self.arom[x] { "a" } else { "" }, self.h[x] + self.dn[x], match self.charge[x] {
            0 => String::new(),
            c => format!("{:+}", c),
        })
    }

    /// The tree of atoms reached from `a` (not going straight back to `from`), `depth` bonds deep.
    fn env_tree(&self, a: usize, from: usize, depth: u8) -> String {
        let mut s = self.env_desc(a);
        if depth == 0 {
            return s;
        }
        let mut kids: Vec<String> = self
            .adj[a]
            .iter()
            .filter(|(y, _)| *y != from && self.alive[*y])
            .map(|(y, o)| {
                let b = if self.is_arom_bond(a, *y) { ':' } else { ['-', '-', '=', '#'][(*o as usize).min(3)] };
                format!("{}{}", b, self.env_tree(*y, a, depth - 1))
            })
            .collect();
        kids.sort();
        s.push('(');
        s.push_str(&kids.join(","));
        s.push(')');
        s
    }

    /// Keys of an atom's surroundings out to 2, 3 and 4 bonds.
    fn env_keys(&self, a: usize) -> [u64; 3] {
        [2u8, 3, 4].map(|r| fnv(&self.env_tree(a, usize::MAX, r)))
    }

    /// The learnt correction for atom `a`, if the database knows its surroundings.
    fn db_shift(&self, nucleus: u8, a: usize) -> Option<(f64, u16)> {
        let db = self.db.as_ref()?;
        // Labelled atoms and atoms of expanded labels keep the rules.
        if self.iso[a] != 0 || self.charge[a] != 0 {
            return None;
        }
        db.lookup(nucleus, &self.env_keys(a)).map(|(s, n, _)| (s, n))
    }
}

/// Surroundings keys of each atom of a drawing (index = atom), for learning from spectra.
/// Only for drawings without group labels, where atoms are not renumbered.
pub fn atom_keys(doc: &Doc) -> Vec<[u64; 3]> {
    let keep: HashSet<usize> = (0..doc.atoms.len()).collect();
    let mut skipped = Vec::new();
    let m = Mol::build(doc, &keep, &mut skipped);
    (0..doc.atoms.len()).map(|a| m.env_keys(a)).collect()
}

/// Integral as written: "2" or, for a partly labelled isotopomer, "0.5".
pub fn count_text(p: &Peak) -> String {
    if (p.frac - 1.0).abs() < 1e-6 {
        p.count.to_string()
    } else {
        let v = p.count as f64 * p.frac;
        let s = format!("{:.2}", v);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// The peak list as written in an experimental section.
pub fn h_text(peaks: &[Peak], mhz: u32, solvent: &str) -> String {
    let items: Vec<String> = peaks
        .iter()
        .map(|p| {
            let j = if p.j.is_empty() || p.mult == "m" || p.mult == "br s" {
                String::new()
            } else {
                format!(", J = {} Hz", p.j.iter().map(|j| format!("{:.1}", j)).collect::<Vec<_>>().join(", "))
            };
            format!("{:.2} ({}{}, {}H)", p.shift, p.mult, j, count_text(p))
        })
        .collect();
    format!("¹H NMR (predicted, {} MHz, {}) δ {}.", mhz, solvent, items.join(", "))
}

pub fn c_text(peaks: &[Peak], mhz: u32, solvent: &str) -> String {
    let items: Vec<String> = peaks
        .iter()
        .map(|p| {
            let mut s = format!("{:.1}", p.shift);
            if p.mult != "s" {
                s += &format!(" ({}, J = {:.0} Hz)", p.mult, p.j.first().copied().unwrap_or(0.0));
            }
            s
        })
        .collect();
    format!("¹³C NMR (predicted, {} MHz, {}) δ {}.", (mhz as f64 * 0.2515).round() as u32, solvent, items.join(", "))
}

/// ¹⁹F or ³¹P{¹H} peak list. `mhz` is the spectrometer's ¹H frequency.
pub fn x_text(peaks: &[Peak], nucleus: u8, mhz: u32, solvent: &str) -> String {
    let (name, ratio, sym) = match nucleus {
        19 => ("¹⁹F", 0.9408, "F"),
        15 => ("¹⁵N{¹H}", 0.1013, "N"),
        _ => ("³¹P{¹H}", 0.4048, "P"),
    };
    let items: Vec<String> = peaks
        .iter()
        .map(|p| {
            let mut inner = vec![p.mult.clone()];
            if !p.j.is_empty() && p.mult != "m" {
                inner.push(format!("J = {} Hz", p.j.iter().map(|j| format!("{:.1}", j)).collect::<Vec<_>>().join(", ")));
            }
            inner.push(format!("{}{}", count_text(p), sym));
            format!("{:.1} ({})", p.shift, inner.join(", ")).replace('-', "−")
        })
        .collect();
    format!("{} NMR (predicted, {} MHz, {}) δ {}.", name, (mhz as f64 * ratio).round() as u32, solvent, items.join(", "))
}

/// Lines of a multiplet at `mhz`: (ppm, relative height), heights summing to `count`.
pub fn lines(p: &Peak, mhz: f64) -> Vec<(f64, f64)> {
    let mut v: Vec<(f64, f64)> = vec![(p.shift, p.count as f64 * p.frac)];
    if p.mult == "m" {
        // Unresolved: a few lines spread over the sum of the couplings.
        let width: f64 = p.pattern.iter().map(|g| g.0 as f64 * g.1).sum::<f64>().max(6.0) / mhz;
        let k = 5;
        return (0..k).map(|i| (p.shift - width / 2.0 + width * i as f64 / (k - 1) as f64, p.count as f64 * p.frac * [0.1, 0.25, 0.3, 0.25, 0.1][i])).collect();
    }
    for &(n, j) in &p.pattern {
        let mut next = Vec::new();
        for &(x, h) in &v {
            // Binomial split into n + 1 lines.
            let mut c = 1.0;
            for k in 0..=n {
                if k > 0 {
                    c = c * (n - k + 1) as f64 / k as f64;
                }
                let off = (k as f64 - n as f64 / 2.0) * j / mhz;
                next.push((x + off, h * c / 2f64.powi(n as i32)));
            }
        }
        v = next;
    }
    v
}
