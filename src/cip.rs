//! Stereodescriptors by the Cahn–Ingold–Prelog (CIP) rules, worked out from the drawing:
//! R/S for stereocentres drawn with a wedged or hashed bond, E/Z for double bonds.
//!
//! Priorities follow rule 1 (higher atomic number first), explored sphere by sphere through
//! the hierarchical digraph: multiple bonds add duplicate atoms, ring closures end in
//! duplicates, and implicit hydrogens count as atomic number 1.

use crate::chem;
use crate::doc::{Doc, Stereo};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

/// R/S for atoms and E/Z for double bonds (keyed by the smaller atom index first).
#[derive(Default, Debug, Clone)]
pub struct Descriptors {
    pub centres: HashMap<usize, char>,
    pub bonds: HashMap<(usize, usize), char>,
}

impl Descriptors {
    pub fn count(&self) -> usize {
        self.centres.len() + self.bonds.len()
    }
    pub fn bond(&self, a: usize, b: usize) -> Option<char> {
        self.bonds.get(&(a.min(b), a.max(b))).copied()
    }
}

struct Graph {
    z: Vec<u8>,
    adj: Vec<Vec<(usize, u8)>>,
    h: Vec<u8>,
}

#[derive(Clone)]
struct Node {
    /// usize::MAX for a hydrogen.
    atom: usize,
    z: u8,
    dup: bool,
    /// The path from the centre, ending with this atom.
    path: Vec<usize>,
}

impl Graph {
    fn new(d: &Doc) -> Graph {
        let n = d.atoms.len();
        let mut adj = vec![Vec::new(); n];
        for b in &d.bonds {
            let o = b.order.clamp(1, 3);
            adj[b.a].push((b.b, o));
            adj[b.b].push((b.a, o));
        }
        let z = d.atoms.iter().map(|a| a.el).collect();
        let h = (0..n).map(|i| d.implicit_h(i).max(0) as u8).collect();
        Graph { z, adj, h }
    }

    fn children(&self, node: &Node) -> Vec<Node> {
        if node.dup || node.atom == usize::MAX {
            return Vec::new();
        }
        let a = node.atom;
        let parent = if node.path.len() >= 2 { Some(node.path[node.path.len() - 2]) } else { None };
        let mut out = Vec::new();
        let dup = |x: usize| Node { atom: x, z: self.z[x], dup: true, path: Vec::new() };
        for &(nb, ord) in &self.adj[a] {
            if Some(nb) == parent {
                // The other half of a multiple bond to the parent.
                for _ in 1..ord {
                    out.push(dup(nb));
                }
                continue;
            }
            if node.path.contains(&nb) {
                out.push(dup(nb));
            } else {
                let mut p = node.path.clone();
                p.push(nb);
                out.push(Node { atom: nb, z: self.z[nb], dup: false, path: p });
            }
            for _ in 1..ord {
                out.push(dup(nb));
            }
        }
        for _ in 0..self.h[a] {
            out.push(Node { atom: usize::MAX, z: 1, dup: false, path: Vec::new() });
        }
        out.sort_by(|x, y| y.z.cmp(&x.z));
        out
    }

    /// Compare the branches starting at `a` and `b` (both attached to `centre`).
    /// `None` for a branch means an implicit hydrogen.
    fn compare(&self, centre: usize, a: Option<usize>, b: Option<usize>) -> Ordering {
        let root = |x: Option<usize>| match x {
            Some(x) => Node { atom: x, z: self.z[x], dup: false, path: vec![centre, x] },
            None => Node { atom: usize::MAX, z: 1, dup: false, path: Vec::new() },
        };
        let (ra, rb) = (root(a), root(b));
        if ra.z != rb.z {
            return ra.z.cmp(&rb.z);
        }
        let mut la = vec![ra];
        let mut lb = vec![rb];
        for _ in 0..20 {
            let ca: Vec<Vec<Node>> = la.iter().map(|n| self.children(n)).collect();
            let cb: Vec<Vec<Node>> = lb.iter().map(|n| self.children(n)).collect();
            let k = ca.len().max(cb.len());
            for i in 0..k {
                let za: Vec<u8> = ca.get(i).map(|v| v.iter().map(|n| n.z).collect()).unwrap_or_default();
                let zb: Vec<u8> = cb.get(i).map(|v| v.iter().map(|n| n.z).collect()).unwrap_or_default();
                let m = za.len().max(zb.len());
                for j in 0..m {
                    let x = za.get(j).copied().unwrap_or(0);
                    let y = zb.get(j).copied().unwrap_or(0);
                    if x != y {
                        return x.cmp(&y);
                    }
                }
            }
            la = ca.into_iter().flatten().collect();
            lb = cb.into_iter().flatten().collect();
            if la.is_empty() && lb.is_empty() || la.len() > 4000 || lb.len() > 4000 {
                break;
            }
        }
        Ordering::Equal
    }
}

/// Work out every R/S and E/Z descriptor the drawing defines.
pub fn descriptors(d: &Doc) -> Descriptors {
    let g = Graph::new(d);
    let mut out = Descriptors::default();
    // Stereocentres: the narrow end of a wedged or hashed bond.
    let mut z_of: HashMap<(usize, usize), f32> = HashMap::new();
    let mut centres: Vec<usize> = Vec::new();
    for b in &d.bonds {
        let z = match b.stereo {
            Stereo::Wedge => 1.0,
            Stereo::Hash => -1.0,
            _ => continue,
        };
        z_of.insert((b.a, b.b), z);
        if !centres.contains(&b.a) {
            centres.push(b.a);
        }
    }
    for &c in &centres {
        if let Some(desc) = centre_descriptor(d, &g, c, &z_of) {
            out.centres.insert(c, desc);
        }
    }
    // Double bonds outside small rings.
    let aro = chem::aromatic_bonds(d);
    let rings = chem::sssr(d);
    for (bi, b) in d.bonds.iter().enumerate() {
        if b.order != 2 || aro.contains(&bi) {
            continue;
        }
        let in_small_ring = rings.iter().any(|r| r.len() < 8 && r.contains(&b.a) && r.contains(&b.b));
        if in_small_ring {
            continue;
        }
        if let Some(desc) = bond_descriptor(d, &g, b.a, b.b) {
            out.bonds.insert((b.a.min(b.b), b.a.max(b.b)), desc);
        }
    }
    out
}

fn centre_descriptor(d: &Doc, g: &Graph, c: usize, z_of: &HashMap<(usize, usize), f32>) -> Option<char> {
    let nbs: Vec<usize> = g.adj[c].iter().map(|x| x.0).collect();
    let h = g.h[c] as usize;
    if nbs.len() + h != 4 || h > 1 || g.adj[c].iter().any(|x| x.1 != 1) {
        return None;
    }
    // Neighbour vectors: x right, y up, z towards the viewer for a wedge.
    let p = d.atoms[c].pos;
    let mut items: Vec<(Option<usize>, [f32; 3])> = Vec::new();
    let (mut sx, mut sy, mut sz) = (0.0, 0.0, 0.0);
    for &n in &nbs {
        let v = d.atoms[n].pos - p;
        let len = v.length().max(1e-3);
        let z = z_of.get(&(c, n)).copied().unwrap_or(0.0);
        let vec = [v.x / len, -v.y / len, z];
        sx += vec[0];
        sy += vec[1];
        sz += vec[2];
        items.push((Some(n), vec));
    }
    if h == 1 {
        // The hydrogen points away from the other three.
        let l = (sx * sx + sy * sy).sqrt();
        let (hx, hy) = if l > 1e-3 { (-sx / l, -sy / l) } else { (0.0, 0.0) };
        items.push((None, [hx * 0.5, hy * 0.5, if sz.abs() > 1e-3 { -sz.signum() } else { 0.0 }]));
    }
    // Highest priority first; all four must differ.
    items.sort_by(|x, y| g.compare(c, y.0, x.0));
    for w in items.windows(2) {
        if g.compare(c, w[0].0, w[1].0) == Ordering::Equal {
            return None;
        }
    }
    let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let d4 = items[3].1;
    let (a, b, cc) = (sub(items[0].1, d4), sub(items[1].1, d4), sub(items[2].1, d4));
    let det = a[0] * (b[1] * cc[2] - b[2] * cc[1]) - a[1] * (b[0] * cc[2] - b[2] * cc[0]) + a[2] * (b[0] * cc[1] - b[1] * cc[0]);
    if det.abs() < 1e-4 {
        return None;
    }
    Some(if det < 0.0 { 'R' } else { 'S' })
}

fn bond_descriptor(d: &Doc, g: &Graph, x: usize, y: usize) -> Option<char> {
    // The higher-priority group on each end.
    let top = |end: usize, other: usize| -> Option<usize> {
        let subs: Vec<usize> = g.adj[end].iter().map(|n| n.0).filter(|&n| n != other).collect();
        match (subs.len(), g.h[end]) {
            (1, _) => Some(subs[0]),
            (2, 0) => match g.compare(end, Some(subs[0]), Some(subs[1])) {
                Ordering::Greater => Some(subs[0]),
                Ordering::Less => Some(subs[1]),
                Ordering::Equal => None,
            },
            _ => None,
        }
    };
    let tx = top(x, y)?;
    let ty = top(y, x)?;
    // An end whose other group is the same as the top one has no E/Z.
    let px = d.atoms[x].pos;
    let py = d.atoms[y].pos;
    let axis = py - px;
    let side = |p: egui::Pos2, from: egui::Pos2| {
        let v = p - from;
        axis.x * v.y - axis.y * v.x
    };
    let sx = side(d.atoms[tx].pos, px);
    let sy = side(d.atoms[ty].pos, py);
    if sx.abs() < 1e-3 || sy.abs() < 1e-3 {
        return None;
    }
    let _ = HashSet::<usize>::new();
    Some(if (sx > 0.0) == (sy > 0.0) { 'Z' } else { 'E' })
}
