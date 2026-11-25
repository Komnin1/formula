//! Ready-made drawings (templates): amino acids, sugars, nucleotides, nucleic bases and a few
//! ring systems. Amino acids and nucleotides are built from SMILES with their stereocentres
//! drawn as wedges; sugars come from the sugar builder (Fischer, Haworth and ring conformers).

use crate::doc::*;
use crate::formats;
use egui::{Pos2, pos2, vec2};
use std::sync::OnceLock;

pub struct Template {
    /// Short name shown under the picture.
    pub name: String,
    /// Longer description for the tooltip.
    pub about: String,
    pub doc: Doc,
}

pub struct Category {
    pub name: &'static str,
    pub items: Vec<Template>,
}

/// Every template group, built once.
pub fn all() -> &'static [Category] {
    static ALL: OnceLock<Vec<Category>> = OnceLock::new();
    ALL.get_or_init(build_all)
}

fn build_all() -> Vec<Category> {
    vec![
        Category { name: "Amino Acids", items: amino_acids() },
        Category { name: "D-Amino Acids", items: d_amino_acids() },
        Category { name: "Aromatics", items: smiles_list(AROMATICS) },
        Category { name: "Conformers", items: conformers() },
        Category { name: "Cycloalkanes", items: cycloalkanes() },
        Category { name: "DNA Templates", items: nucleotides(true) },
        Category { name: "Hexoses", items: hexoses() },
        Category { name: "Nucleic Bases", items: smiles_list(BASES) },
        Category { name: "Pentoses", items: pentoses() },
        Category { name: "RNA Templates", items: nucleotides(false) },
    ]
}

// ---------------------------------------------------------------------------
// Amino acids

/// The 20 amino acids that make proteins, plus citrulline: name, three- and one-letter codes,
/// and SMILES of the L form.
pub const AMINO_ACIDS: &[(&str, &str, char, &str)] = &[
    ("Alanine", "Ala", 'A', "C[C@@H](C(=O)O)N"),
    ("Arginine", "Arg", 'R', "C(C[C@@H](C(=O)O)N)CN=C(N)N"),
    ("Asparagine", "Asn", 'N', "C([C@@H](C(=O)O)N)C(=O)N"),
    ("Aspartic acid", "Asp", 'D', "C([C@@H](C(=O)O)N)C(=O)O"),
    ("Cysteine", "Cys", 'C', "C([C@@H](C(=O)O)N)S"),
    ("Glutamic acid", "Glu", 'E', "C(CC(=O)O)[C@@H](C(=O)O)N"),
    ("Glutamine", "Gln", 'Q', "C(CC(=O)N)[C@@H](C(=O)O)N"),
    ("Glycine", "Gly", 'G', "C(C(=O)O)N"),
    ("Histidine", "His", 'H', "C1=C(NC=N1)C[C@@H](C(=O)O)N"),
    ("Isoleucine", "Ile", 'I', "CC[C@H](C)[C@@H](C(=O)O)N"),
    ("Leucine", "Leu", 'L', "CC(C)C[C@@H](C(=O)O)N"),
    ("Lysine", "Lys", 'K', "C(CCN)C[C@@H](C(=O)O)N"),
    ("Methionine", "Met", 'M', "CSCC[C@@H](C(=O)O)N"),
    ("Phenylalanine", "Phe", 'F', "C1=CC=C(C=C1)C[C@@H](C(=O)O)N"),
    ("Proline", "Pro", 'P', "C1C[C@H](NC1)C(=O)O"),
    ("Serine", "Ser", 'S', "C([C@@H](C(=O)O)N)O"),
    ("Threonine", "Thr", 'T', "C[C@H]([C@@H](C(=O)O)N)O"),
    ("Tryptophan", "Trp", 'W', "C1=CC=C2C(=C1)C(=CN2)C[C@@H](C(=O)O)N"),
    ("Tyrosine", "Tyr", 'Y', "C1=CC(=CC=C1C[C@@H](C(=O)O)N)O"),
    ("Valine", "Val", 'V', "CC(C)[C@@H](C(=O)O)N"),
    ("Citrulline", "Cit", '-', "C(C[C@@H](C(=O)O)N)CNC(=O)N"),
];

/// The amino-acid backbone: (N, Cα, carboxyl C) if there is one.
fn backbone(d: &Doc) -> Option<(usize, usize, usize)> {
    let el = |i: usize| d.atoms[i].el;
    for c in 0..d.atoms.len() {
        if el(c) != 6 {
            continue;
        }
        let nb = d.neighbors(c);
        let dbl_o = nb.iter().any(|&o| el(o) == 8 && d.find_bond(c, o).is_some_and(|b| d.bonds[b].order == 2));
        let oh = nb.iter().any(|&o| el(o) == 8 && d.neighbors(o).len() == 1 && d.find_bond(c, o).is_some_and(|b| d.bonds[b].order == 1));
        if !(dbl_o && oh) {
            continue;
        }
        for &ca in &nb {
            if el(ca) != 6 {
                continue;
            }
            if let Some(&n) = d.neighbors(ca).iter().find(|&&n| el(n) == 7) {
                return Some((n, ca, c));
            }
        }
    }
    None
}

/// Turn every atom around `pivot` so `from → to` points along +x.
fn level(d: &mut Doc, from: Pos2, to: Pos2) {
    let v = to - from;
    let ang = -v.y.atan2(v.x);
    let (s, c) = ang.sin_cos();
    let all = d.select_all();
    d.transform(&all, |p| {
        let q = p - from;
        from + vec2(q.x * c - q.y * s, q.x * s + q.y * c)
    });
}

fn mirror_y(d: &mut Doc) {
    let all = d.select_all();
    d.transform(&all, |p| pos2(p.x, -p.y));
}

/// Backbone across the page (amine on the left, acid on the right), side chain upwards.
fn orient_amino_acid(d: &mut Doc) {
    let Some((n, ca, c)) = backbone(d) else { return };
    let (pn, pc) = (d.atoms[n].pos, d.atoms[c].pos);
    level(d, pn, pc);
    let y_line = (d.atoms[n].pos.y + d.atoms[c].pos.y) * 0.5;
    let mut skip: Vec<usize> = vec![n, ca, c];
    skip.extend(d.neighbors(c));
    let side: Vec<Pos2> = (0..d.atoms.len()).filter(|i| !skip.contains(i)).map(|i| d.atoms[i].pos).collect();
    if !side.is_empty() {
        let cy = side.iter().map(|p| p.y).sum::<f32>() / side.len() as f32;
        if cy > y_line {
            mirror_y(d);
        }
    }
}

/// The mirror image of a SMILES: every @ becomes @@ and back.
pub fn mirror_smiles(s: &str) -> String {
    s.replace("@@", "\u{1}").replace('@', "@@").replace('\u{1}', "@")
}

/// D-amino acids: the mirror images of the L forms (glycine has none).
fn d_amino_acids() -> Vec<Template> {
    AMINO_ACIDS
        .iter()
        .filter(|a| a.2 != 'G')
        .filter_map(|(name, three, one, smi)| {
            let doc = formats::parse_smiles_oriented(&mirror_smiles(smi), &orient_amino_acid).ok()?;
            let code = if *one == '-' { format!("D-{}", three) } else { format!("D-{}, d{}", three, one) };
            Some(Template { name: format!("D-{}", name), about: format!("D-{} ({})", name.to_lowercase(), code), doc })
        })
        .collect()
}

fn amino_acids() -> Vec<Template> {
    AMINO_ACIDS
        .iter()
        .filter_map(|(name, three, one, smi)| {
            let doc = formats::parse_smiles_oriented(smi, &orient_amino_acid).ok()?;
            let code = if *one == '-' { three.to_string() } else { format!("{}, {}", three, one) };
            Some(Template { name: name.to_string(), about: format!("L-{} ({})", name.to_lowercase(), code), doc })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Nucleotides, bases, rings

/// The ribose (or 2′-deoxyribose) 5′-phosphate, attached at C1′ with ring number `n`.
fn sugar_phosphate(n: u32, deoxy: bool) -> String {
    if deoxy {
        format!("[C@H]{n}C[C@@H]([C@H](O{n})COP(=O)(O)O)O")
    } else {
        format!("[C@H]{n}[C@@H]([C@@H]([C@H](O{n})COP(=O)(O)O)O)O")
    }
}

fn nucleotides(dna: bool) -> Vec<Template> {
    let s = |n: u32| sugar_phosphate(n, dna);
    let list: Vec<(&str, &str, String)> = if dna {
        vec![
            ("dAMP", "2′-Deoxyadenosine 5′-monophosphate (A)", format!("C1=NC(=C2C(=N1)N(C=N2){})N", s(3))),
            ("dGMP", "2′-Deoxyguanosine 5′-monophosphate (G)", format!("C1=NC2=C(N1{})N=C(NC2=O)N", s(3))),
            ("dCMP", "2′-Deoxycytidine 5′-monophosphate (C)", format!("C1=CN(C(=O)N=C1N){}", s(2))),
            ("dTMP", "Thymidine 5′-monophosphate (T)", format!("CC1=CN(C(=O)NC1=O){}", s(2))),
        ]
    } else {
        vec![
            ("AMP", "Adenosine 5′-monophosphate (A)", format!("C1=NC(=C2C(=N1)N(C=N2){})N", s(3))),
            ("GMP", "Guanosine 5′-monophosphate (G)", format!("C1=NC2=C(N1{})N=C(NC2=O)N", s(3))),
            ("CMP", "Cytidine 5′-monophosphate (C)", format!("C1=CN(C(=O)N=C1N){}", s(2))),
            ("UMP", "Uridine 5′-monophosphate (U)", format!("C1=CN(C(=O)NC1=O){}", s(2))),
        ]
    };
    list.into_iter()
        .filter_map(|(name, about, smi)| Some(Template { name: name.into(), about: about.into(), doc: formats::parse_smiles(&smi).ok()? }))
        .collect()
}

const BASES: &[(&str, &str, &str)] = &[
    ("Adenine", "Adenine (A), a purine base of DNA and RNA", "NC1=C2N=CNC2=NC=N1"),
    ("Guanine", "Guanine (G), a purine base of DNA and RNA", "NC1=NC2=C(N=CN2)C(=O)N1"),
    ("Cytosine", "Cytosine (C), a pyrimidine base of DNA and RNA", "NC1=NC(=O)NC=C1"),
    ("Thymine", "Thymine (T), a pyrimidine base of DNA", "CC1=CNC(=O)NC1=O"),
    ("Uracil", "Uracil (U), a pyrimidine base of RNA", "O=C1C=CNC(=O)N1"),
];

const AROMATICS: &[(&str, &str, &str)] = &[
    ("Benzene", "Benzene", "c1ccccc1"),
    ("Naphthalene", "Naphthalene", "c1ccc2ccccc2c1"),
    ("Anthracene", "Anthracene", "c1ccc2cc3ccccc3cc2c1"),
    ("Phenanthrene", "Phenanthrene", "c1ccc2c(c1)ccc1ccccc12"),
    ("Pyridine", "Pyridine", "c1ccncc1"),
    ("Pyrimidine", "Pyrimidine", "c1cncnc1"),
    ("Pyrrole", "Pyrrole", "c1cc[nH]c1"),
    ("Furan", "Furan", "c1ccoc1"),
    ("Thiophene", "Thiophene", "c1ccsc1"),
    ("Imidazole", "Imidazole", "c1c[nH]cn1"),
    ("Indole", "Indole", "c1ccc2[nH]ccc2c1"),
    ("Quinoline", "Quinoline", "c1ccc2ncccc2c1"),
    ("Purine", "Purine", "c1ncc2[nH]cnc2n1"),
];

const CYCLOALKANES: &[(&str, &str, &str)] = &[
    ("Cyclopropane", "Cyclopropane", "C1CC1"),
    ("Cyclobutane", "Cyclobutane", "C1CCC1"),
    ("Cyclopentane", "Cyclopentane", "C1CCCC1"),
    ("Cyclohexane", "Cyclohexane", "C1CCCCC1"),
    ("Cycloheptane", "Cycloheptane", "C1CCCCCC1"),
    ("Cyclooctane", "Cyclooctane", "C1CCCCCCC1"),
    ("Decalin", "Decahydronaphthalene", "C1CCC2CCCCC2C1"),
];

/// Cycloalkanes, with the bridged ones drawn in perspective as chemists draw them (traced from
/// the person's drawings): the bridge above, the bond that runs behind broken where it passes.
fn cycloalkanes() -> Vec<Template> {
    let mut v = smiles_list(CYCLOALKANES);
    // Norbornane / norbornene: C7 bridge on top, C1 behind, C4 in front.
    let nb_pts = [(152.0, 15.0), (224.0, 145.0), (88.0, 180.0), (15.0, 303.0), (152.0, 268.0), (292.0, 305.0), (362.0, 180.0)];
    let nb_bonds = |c2c3: u8| vec![(0, 1, 1), (0, 4, 1), (1, 2, 1), (2, 3, c2c3), (3, 4, 1), (4, 5, 1), (5, 6, 1), (6, 1, 1)];
    v.push(Template { name: "Norbornane".into(), about: "Bicyclo[2.2.1]heptane".into(), doc: drawn(&nb_pts, &nb_bonds(1)) });
    v.push(Template { name: "Norbornene".into(), about: "Bicyclo[2.2.1]hept-2-ene".into(), doc: drawn(&nb_pts, &nb_bonds(2)) });
    // Adamantane: three chairs seen from the side.
    let ad_pts = [(192.0, 38.0), (93.0, 66.0), (292.0, 66.0), (140.0, 127.0), (93.0, 168.0), (292.0, 168.0), (192.0, 197.0), (40.0, 260.0), (140.0, 230.0), (240.0, 258.0)];
    let ad_bonds = [(1, 0, 1), (0, 2, 1), (0, 3, 1), (1, 4, 1), (2, 5, 1), (3, 8, 1), (4, 6, 1), (6, 5, 1), (4, 7, 1), (7, 8, 1), (8, 9, 1), (9, 5, 1)];
    v.push(Template { name: "Adamantane".into(), about: "Tricyclo[3.3.1.1³,⁷]decane".into(), doc: drawn(&ad_pts, &ad_bonds) });
    v
}

/// A carbon skeleton from picture coordinates (y down), scaled to the standard bond length.
fn drawn(pts: &[(f32, f32)], bonds: &[(usize, usize, u8)]) -> crate::doc::Doc {
    use crate::doc::*;
    let mut lens: Vec<f32> = bonds.iter().map(|&(a, b, _)| ((pts[a].0 - pts[b].0).powi(2) + (pts[a].1 - pts[b].1).powi(2)).sqrt()).collect();
    lens.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let k = BOND_LEN / lens[lens.len() / 2];
    let mut d = Doc::default();
    for &(x, y) in pts {
        d.atoms.push(Atom::new(egui::pos2(x * k, y * k), crate::elements::C));
    }
    for &(a, b, o) in bonds {
        d.add_bond(a, b, o, Stereo::None);
    }
    d
}

fn smiles_list(list: &[(&str, &str, &str)]) -> Vec<Template> {
    list.iter()
        .filter_map(|(name, about, smi)| Some(Template { name: name.to_string(), about: about.to_string(), doc: formats::parse_smiles(smi).ok()? }))
        .collect()
}

// ---------------------------------------------------------------------------
// Sugars (drawn by the sugar module in Fischer, Haworth and conformer forms)

use crate::sugars::{self, Choice, Drawing, Ring};

fn sugar_named(list: &[sugars::Sugar], name: &str) -> sugars::Sugar {
    *list.iter().find(|s| s.name.starts_with(name)).expect("known sugar")
}

/// One sugar drawing as a template.
fn sugar(list: &[sugars::Sugar], name: &str, ring: Option<Ring>, beta: bool, drawing: Drawing) -> Option<Template> {
    let sg = sugar_named(list, name);
    let c = Choice { sugar: sg, l_form: sg.natural_l, ring, beta, drawing };
    let doc = sugars::draw(&c)?;
    let title = c.title();
    let about = format!("{}, {}", title, drawing.name().to_lowercase());
    let name = if matches!(drawing, Drawing::Fischer | Drawing::Haworth) { title } else { format!("{} ({})", title, drawing.name().to_lowercase()) };
    Some(Template { name, about, doc })
}

fn hexoses() -> Vec<Template> {
    let h = sugars::HEXOSES;
    let mut v = Vec::new();
    for (name, ring) in [("Glucose", Ring::Pyranose), ("Galactose", Ring::Pyranose), ("Mannose", Ring::Pyranose), ("Fructose", Ring::Furanose)] {
        for beta in [false, true] {
            v.extend(sugar(h, name, Some(ring), beta, Drawing::Haworth));
        }
    }
    v.extend(sugar(h, "Fructose", Some(Ring::Pyranose), true, Drawing::Haworth));
    v.extend(sugar(h, "Fucose", Some(Ring::Pyranose), false, Drawing::Haworth));
    v.extend(sugar(h, "Rhamnose", Some(Ring::Pyranose), false, Drawing::Haworth));
    for sg in h {
        v.extend(sugar(h, sg.name, None, false, Drawing::Fischer));
    }
    v
}

fn pentoses() -> Vec<Template> {
    let p = sugars::PENTOSES;
    let mut v = Vec::new();
    for beta in [false, true] {
        v.extend(sugar(p, "Ribose", Some(Ring::Furanose), beta, Drawing::Haworth));
    }
    v.extend(sugar(p, "2-Deoxyribose", Some(Ring::Furanose), true, Drawing::Haworth));
    v.extend(sugar(p, "Arabinose", Some(Ring::Furanose), true, Drawing::Haworth));
    v.extend(sugar(p, "Xylose", Some(Ring::Pyranose), true, Drawing::Haworth));
    v.extend(sugar(p, "Arabinose", Some(Ring::Pyranose), false, Drawing::Haworth));
    for sg in p {
        v.extend(sugar(p, sg.name, None, false, Drawing::Fischer));
    }
    v
}

/// Ring shapes: every pyranose conformer of β-D-glucose, chairs of other pyranoses, and the
/// envelope and twist forms of the furanoses.
fn conformers() -> Vec<Template> {
    let (h, p) = (sugars::HEXOSES, sugars::PENTOSES);
    let mut v = Vec::new();
    for dr in [Drawing::Chair, Drawing::Boat, Drawing::SkewBoat, Drawing::HalfChair, Drawing::Envelope] {
        v.extend(sugar(h, "Glucose", Some(Ring::Pyranose), true, dr));
    }
    v.extend(sugar(h, "Glucose", Some(Ring::Pyranose), false, Drawing::Chair));
    v.extend(sugar(h, "Galactose", Some(Ring::Pyranose), true, Drawing::Chair));
    v.extend(sugar(h, "Mannose", Some(Ring::Pyranose), true, Drawing::Chair));
    v.extend(sugar(p, "Xylose", Some(Ring::Pyranose), true, Drawing::Chair));
    for (list, name) in [(p, "Ribose"), (p, "2-Deoxyribose"), (h, "Fructose")] {
        for dr in [Drawing::Envelope, Drawing::Twist] {
            v.extend(sugar(list, name, Some(Ring::Furanose), true, dr));
        }
    }
    v
}
