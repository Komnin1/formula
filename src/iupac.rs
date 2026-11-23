//! Systematic (IUPAC-style) chemical names, worked out on this computer.
//!
//! Reading (`name_to_smiles`): a name such as "3-(4-chlorophenyl)-2-methylpropanoic acid" is
//! taken apart into substituent prefixes, a parent (a chain, a ring or benzene) with its double
//! and triple bonds, and a suffix for the main group. The structure is then written as SMILES
//! and drawn by the SMILES reader.
//!
//! Writing (`structure_to_name`): for a drawn structure the main group, the parent chain or
//! ring and the numbering are chosen by the usual rules: most main groups, rings before chains,
//! longest chain, most multiple bonds, lowest locants for the main group, then for multiple
//! bonds, then for prefixes, then in alphabetical order. The name is then put together.
//!
//! Covered: chains, single carbocyclic rings and benzene, with halogens, alcohols, ethers,
//! amines, aldehydes, ketones, carboxylic acids, esters, amides, nitriles, thiols and nitro
//! groups. Fused or heterocyclic rings and stereodescriptors (R/S, E/Z) are not named here;
//! well-known compounds of that kind come from the built-in list in `names`.

use crate::chem;
use crate::doc::Doc;
use crate::formats;
use std::collections::HashSet;

pub const STEMS: [&str; 21] = [
    "", "meth", "eth", "prop", "but", "pent", "hex", "hept", "oct", "non", "dec", "undec", "dodec", "tridec", "tetradec", "pentadec", "hexadec",
    "heptadec", "octadec", "nonadec", "icos",
];
const MULT: [&str; 11] = ["", "", "di", "tri", "tetra", "penta", "hexa", "hepta", "octa", "nona", "deca"];
const MULT_COMPLEX: [&str; 7] = ["", "", "bis", "tris", "tetrakis", "pentakis", "hexakis"];

// ===========================================================================
// Reading names
// ===========================================================================

/// The kind of main group a name's suffix gives.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Suf {
    Ol,
    One,
    Al,
    Acid,
    Amine,
    Nitrile,
    Amide,
    Thiol,
    Oate,
    Yl,
    /// An acyl group: "propanoyl" (attached through its C=O carbon).
    Oyl,
    // On a ring atom, the group's carbon outside the ring.
    CarboxylicAcid,
    Carbaldehyde,
    Carbonitrile,
    Carboxamide,
    Carboxylate,
}

/// A parent skeleton being built: atoms in order, the bonds between neighbours, and the
/// groups hanging from each atom (as SMILES starting with the attached atom).
#[derive(Clone)]
struct Skel {
    atoms: Vec<&'static str>,
    /// `bonds[i]` joins atom i and atom i+1 (for a ring, the last one closes it); 0 = aromatic.
    bonds: Vec<u8>,
    ring: bool,
    branches: Vec<Vec<String>>,
}

impl Skel {
    fn chain(n: usize) -> Skel {
        Skel { atoms: vec!["C"; n], bonds: vec![1; n.saturating_sub(1)], ring: false, branches: vec![Vec::new(); n] }
    }
    fn cyclo(n: usize) -> Skel {
        Skel { atoms: vec!["C"; n], bonds: vec![1; n], ring: true, branches: vec![Vec::new(); n] }
    }
    fn benzene() -> Skel {
        Skel { atoms: vec!["c"; 6], bonds: vec![0; 6], ring: true, branches: vec![Vec::new(); 6] }
    }
    fn len(&self) -> usize {
        self.atoms.len()
    }

    fn bond_text(o: u8) -> &'static str {
        match o {
            2 => "=",
            3 => "#",
            _ => "",
        }
    }

    fn atom_text(&self, i: usize) -> String {
        let mut s = self.atoms[i].to_string();
        for b in &self.branches[i] {
            s.push('(');
            s.push_str(b);
            s.push(')');
        }
        s
    }

    /// SMILES starting at atom `root` (so it can hang from another atom as a group).
    fn smiles(&self, root: usize, depth: usize) -> String {
        let n = self.len();
        if self.ring {
            let digit = format!("%{}", 90 + depth.min(9));
            let mut s = String::new();
            for k in 0..n {
                let i = (root + k) % n;
                if k > 0 {
                    s.push_str(Self::bond_text(self.bonds[(i + n - 1) % n]));
                }
                s.push_str(self.atoms[i]);
                if k == 0 {
                    s.push_str(&digit);
                }
                for b in &self.branches[i] {
                    s.push('(');
                    s.push_str(b);
                    s.push(')');
                }
                if k == n - 1 {
                    s.push_str(Self::bond_text(self.bonds[i]));
                    s.push_str(&digit);
                }
            }
            return s;
        }
        // A chain: the root, then the part before it as a branch and the part after it.
        let walk = |from: usize, forward: bool| -> String {
            let mut s = String::new();
            let mut i = from;
            loop {
                let prev = if forward { i - 1 } else { i + 1 };
                s.push_str(Self::bond_text(self.bonds[prev.min(i)]));
                s.push_str(&self.atom_text(i));
                if forward {
                    if i + 1 >= n {
                        break;
                    }
                    i += 1;
                } else {
                    if i == 0 {
                        break;
                    }
                    i -= 1;
                }
            }
            s
        };
        let mut s = self.atom_text(root);
        let left = if root > 0 { Some(walk(root - 1, false)) } else { None };
        let right = if root + 1 < n { Some(walk(root + 1, true)) } else { None };
        match (left, right) {
            (Some(l), Some(r)) => {
                s.push('(');
                s.push_str(&l);
                s.push(')');
                s.push_str(&r);
            }
            (Some(l), None) => s.push_str(&l),
            (None, Some(r)) => s.push_str(&r),
            (None, None) => {}
        }
        s
    }
}

/// A position-by-position reader over a name.
struct Cur<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> Cur<'a> {
    fn new(s: &'a str) -> Self {
        Cur { s, i: 0 }
    }
    fn rest(&self) -> &'a str {
        &self.s[self.i..]
    }
    fn done(&self) -> bool {
        self.i >= self.s.len()
    }
    fn starts(&self, lit: &str) -> bool {
        self.rest().starts_with(lit)
    }
    fn eat(&mut self, lit: &str) -> bool {
        if self.starts(lit) {
            self.i += lit.len();
            true
        } else {
            false
        }
    }
    /// "1,2,4" (numbers, or "n" for a nitrogen); nothing is consumed if there is none.
    fn locant_list(&mut self) -> Option<Vec<String>> {
        let save = self.i;
        let mut out = Vec::new();
        loop {
            let r = self.rest();
            let digits: String = r.chars().take_while(|c| c.is_ascii_digit()).collect();
            if !digits.is_empty() {
                self.i += digits.len();
                while self.eat("'") {}
                out.push(digits);
            } else if r.starts_with('n') && matches!(r.as_bytes().get(1), Some(b',') | Some(b'-')) {
                self.i += 1;
                out.push("N".into());
            } else {
                break;
            }
            if !self.eat(",") {
                break;
            }
        }
        if out.is_empty() {
            self.i = save;
            None
        } else {
            Some(out)
        }
    }
    /// "-1,2-" as numbers; restores the position if not there.
    fn dash_locs(&mut self) -> Option<Vec<usize>> {
        let save = self.i;
        if !self.eat("-") {
            return None;
        }
        let Some(l) = self.locant_list() else {
            self.i = save;
            return None;
        };
        if !self.eat("-") {
            self.i = save;
            return None;
        }
        let nums: Option<Vec<usize>> = l.iter().map(|x| x.parse().ok()).collect();
        if nums.is_none() {
            self.i = save;
        }
        nums
    }
    /// di, tri, … (1 when there is none).
    fn mult(&mut self) -> usize {
        for (k, m) in MULT.iter().enumerate().rev() {
            if k >= 2 && self.eat(m) {
                return k;
            }
        }
        1
    }
}

/// Lower-case, single spaces, plain hyphens, without stereodescriptors such as "(2e)-" or "(s)-".
fn normalize(name: &str) -> String {
    let mut s: String = name
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| match c {
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2212}' => '-',
            '[' | '{' => '(',
            ']' | '}' => ')',
            _ => c,
        })
        .collect();
    while s.contains("  ") {
        s = s.replace("  ", " ");
    }
    loop {
        let before = s.clone();
        if s.starts_with('(') {
            if let Some(end) = s.find(")-") {
                let inner = &s[1..end];
                let stereo = !inner.is_empty()
                    && inner.split(',').all(|t| {
                        let t = t.trim().trim_start_matches(|c: char| c.is_ascii_digit());
                        matches!(t, "e" | "z" | "r" | "s" | "+" | "-" | "±" | "rs" | "sr")
                    });
                if stereo {
                    s = s[end + 2..].to_string();
                }
            }
        }
        for p in ["dl-", "d-", "l-", "(+)-", "(-)-", "(±)-", "cis-", "trans-", "e-", "z-"] {
            if let Some(r) = s.strip_prefix(p) {
                s = r.to_string();
            }
        }
        if s == before {
            break;
        }
    }
    s
}

/// SMILES for a name, or None when the name can't be read.
pub fn name_to_smiles(name: &str) -> Option<String> {
    let mut s = normalize(name);
    if s.is_empty() {
        return None;
    }
    // Retained names with prefixes: "2-phenylacetic acid", "methyl chloroacetate".
    for (old, new) in [
        ("acetic acid", "ethanoic acid"),
        ("formic acid", "methanoic acid"),
        ("acetate", "ethanoate"),
        ("formate", "methanoate"),
        ("acetamide", "ethanamide"),
        ("formamide", "methanamide"),
        ("acetaldehyde", "ethanal"),
        ("formaldehyde", "methanal"),
        ("acetonitrile", "ethanenitrile"),
    ] {
        if let Some(head) = s.strip_suffix(old) {
            s = format!("{}{}", head, new);
            break;
        }
    }
    // Onium ions and phosphines: every group sits on the central P or N.
    for (end, head, need) in [("phosphonium", "[P+]", 4), ("ammonium", "[N+]", 4), ("phosphine oxide", "P(=O)", 3), ("phosphine", "P", 3), ("phosphane", "P", 3)] {
        if let Some(head_text) = s.strip_suffix(end) {
            let mut c = Cur::new(head_text);
            let mut out = head.to_string();
            let mut k = 0;
            while !c.done() {
                c.eat("-");
                if c.done() {
                    break;
                }
                let (count, g) = substituent(&mut c, 0)?;
                for _ in 0..count {
                    out.push('(');
                    out.push_str(&g);
                    out.push(')');
                    k += 1;
                }
            }
            if k == need || (need == 3 && k < 3 && end != "phosphine oxide") {
                return Some(out);
            }
            return None;
        }
    }
    let words: Vec<&str> = s.split(' ').collect();
    // Esters: "ethyl propanoate", "methyl 4-hydroxybenzoate".
    if words.len() == 2 && (words[1].ends_with("oate") || words[1].ends_with("carboxylate")) {
        let r = substituent_whole(words[0], 0)?;
        let (sk, root) = compound(words[1], false, Some(&r), 0)?;
        return Some(sk.smiles(root, 0));
    }
    let (sk, root) = compound(&s, false, None, 0)?;
    Some(sk.smiles(root, 0))
}

/// The structure for a systematic name, checked for impossible valences.
pub fn name_to_structure(name: &str) -> Option<Doc> {
    let smi = name_to_smiles(name)?;
    let d = formats::parse_smiles(&smi).ok()?;
    if d.atoms.is_empty() || (0..d.atoms.len()).any(|i| d.valence_error(i)) {
        return None;
    }
    Some(d)
}

/// A whole word naming a group (the alkyl part of an ester): "ethyl", "tert-butyl", "benzyl".
fn substituent_whole(word: &str, depth: usize) -> Option<String> {
    let mut c = Cur::new(word);
    if let Some((1, smi)) = substituent(&mut c, depth) {
        if c.done() {
            return Some(smi);
        }
    }
    // A complex alkyl group without parentheses: "2-methylpropyl".
    let (sk, root) = compound(word, true, None, depth + 1)?;
    Some(sk.smiles(root, depth + 1))
}

/// Retained parents with a benzene ring: (name, group on atom 1; "*" = the ester's alkyl).
const BENZENE_PARENTS: &[(&str, &str)] = &[
    ("benzene", ""),
    ("phenol", "O"),
    ("aniline", "N"),
    ("benzenamine", "N"),
    ("toluene", "C"),
    ("benzoic acid", "C(=O)O"),
    ("benzaldehyde", "C=O"),
    ("anisole", "OC"),
    ("benzonitrile", "C#N"),
    ("benzamide", "C(N)=O"),
    ("acetophenone", "C(C)=O"),
    ("styrene", "C=C"),
    ("benzenethiol", "S"),
    ("thiophenol", "S"),
    ("benzoate", "C(=O)O*"),
];

/// Group texts for benzene suffixes ("benzene-1,2-diol").
const RING_SUFFIXES: &[(&str, &str)] = &[
    ("carboxylic acid", "C(=O)O"),
    ("carbaldehyde", "C=O"),
    ("carbonitrile", "C#N"),
    ("carboxamide", "C(N)=O"),
    ("carboxylate", "C(=O)O*"),
    ("amine", "N"),
    ("thiol", "S"),
    ("ol", "O"),
];

/// Parse a whole compound name (or, with `yl`, a group name such as "2-methylpropyl").
/// Returns the skeleton and the atom the group hangs from (or 0).
fn compound(s: &str, yl: bool, ester: Option<&str>, depth: usize) -> Option<(Built, usize)> {
    if depth > 6 {
        return None;
    }
    if let Some((sk, r)) = benzene_compound(s, yl, ester, depth) {
        return Some((Built::S(sk), r));
    }
    if let Some((g, r)) = template_compound(s, yl, ester, depth) {
        return Some((Built::G(g), r));
    }
    chain_compound(s, yl, ester, depth).map(|(sk, r)| (Built::S(sk), r))
}

/// A parsed parent: a chain, ring or benzene (`Skel`) or a named ring system (`GSkel`).
enum Built {
    S(Skel),
    G(GSkel),
}

impl Built {
    fn smiles(&self, root: usize, depth: usize) -> String {
        match self {
            Built::S(s) => s.smiles(root, depth),
            Built::G(g) => g.smiles(root, depth),
        }
    }
}

/// Chains and carbocycles ("2-methylbutan-2-ol", "cyclohex-2-en-1-one").
fn chain_compound(s: &str, yl: bool, ester: Option<&str>, depth: usize) -> Option<(Skel, usize)> {
    // Chains and carbocycles: try each place a stem could start, rightmost first.
    let mut starts: Vec<(usize, usize)> = Vec::new();
    for i in 0..s.len() {
        if !s.is_char_boundary(i) {
            continue;
        }
        let r = &s[i..];
        for (k, st) in STEMS.iter().enumerate().skip(1) {
            if r.starts_with(st) {
                starts.push((i, k));
            }
        }
    }
    // Rightmost start first; at one place, the longest stem first.
    starts.sort_by(|a, b| b.0.cmp(&a.0).then(STEMS[b.1].len().cmp(&STEMS[a.1].len())));
    for &(i, k) in &starts {
        let stem_end = i + STEMS[k].len();
        let (ring, pre_end) = if s[..i].ends_with("cyclo") { (true, i - 5) } else { (false, i) };
        let prefix = &s[..pre_end];
        let tail = &s[stem_end..];
        let Some(t) = parse_tail(tail, ring, yl) else { continue };
        let Some(prefs) = parse_prefixes(prefix, depth) else { continue };
        if let Some(r) = build(k, ring, &t, &prefs, ester) {
            return Some(r);
        }
    }
    None
}

/// What follows the stem: double/triple bonds and the suffix.
struct Tail {
    unsat: Vec<(usize, u8)>,
    suffix: Option<(Suf, Vec<usize>, usize)>,
}

fn parse_tail(t: &str, ring: bool, yl: bool) -> Option<Tail> {
    let mut c = Cur::new(t);
    let mut unsat = Vec::new();
    let mut segs = 0;
    loop {
        let save = c.i;
        if c.starts("a-") {
            c.i += 1;
        }
        let locs = c.dash_locs();
        let m = c.mult();
        let kind = if c.eat("an") {
            1
        } else if c.eat("en") {
            2
        } else if c.eat("yn") {
            3
        } else {
            c.i = save;
            break;
        };
        if kind == 1 {
            if segs > 0 || locs.is_some() || m > 1 {
                c.i = save;
                break;
            }
            // "-an-" (saturated) is never followed by double or triple bonds: what comes next
            // is the suffix ("pentanenitrile", not a double bond in "-enitrile").
            segs += 1;
            break;
        } else {
            // Only the first double/triple bond part may leave out its locants ("propene");
            // a later "en" without locants is the start of the suffix ("prop-2-enenitrile").
            if segs > 0 && locs.is_none() {
                c.i = save;
                break;
            }
            let ls = locs.unwrap_or_else(|| (1..=m).collect());
            if ls.len() != m {
                return None;
            }
            for l in ls {
                unsat.push((l, kind));
            }
        }
        segs += 1;
    }
    if segs == 0 {
        // Plain alkyl groups: "methyl", "cyclohexyl".
        if yl && c.rest() == "yl" {
            return Some(Tail { unsat, suffix: Some((Suf::Yl, vec![1], 1)) });
        }
        return None;
    }
    let after = c.i;
    for with_e in [false, true] {
        c.i = after;
        if with_e && !c.eat("e") {
            continue;
        }
        if c.done() {
            if with_e && !yl {
                return Some(Tail { unsat: unsat.clone(), suffix: None });
            }
            continue;
        }
        let locs = c.dash_locs();
        let m = c.mult();
        let words: &[(&str, Suf)] = &[
            ("carboxylic acid", Suf::CarboxylicAcid),
            ("carbaldehyde", Suf::Carbaldehyde),
            ("carbonitrile", Suf::Carbonitrile),
            ("carboxamide", Suf::Carboxamide),
            ("carboxylate", Suf::Carboxylate),
            ("oic acid", Suf::Acid),
            ("oate", Suf::Oate),
            ("amine", Suf::Amine),
            ("amide", Suf::Amide),
            ("nitrile", Suf::Nitrile),
            ("thiol", Suf::Thiol),
            ("one", Suf::One),
            ("ol", Suf::Ol),
            ("al", Suf::Al),
            ("oyl", Suf::Oyl),
            ("yl", Suf::Yl),
        ];
        for (w, k) in words {
            if c.rest() == *w {
                if matches!(k, Suf::Yl | Suf::Oyl) != yl {
                    break;
                }
                let exo = matches!(k, Suf::CarboxylicAcid | Suf::Carbaldehyde | Suf::Carbonitrile | Suf::Carboxamide | Suf::Carboxylate);
                if exo && !ring {
                    break;
                }
                let ls = locs.clone().unwrap_or_default();
                if !ls.is_empty() && ls.len() != m {
                    return None;
                }
                return Some(Tail { unsat: unsat.clone(), suffix: Some((*k, ls, m)) });
            }
        }
    }
    None
}

/// One prefix: where it goes (empty = not given), how many, and its SMILES.
#[derive(Clone)]
struct Pref {
    locs: Vec<String>,
    count: usize,
    smiles: String,
}

struct Prefixes {
    list: Vec<Pref>,
    /// Locants written just before the parent ("2-butanol", "1,3-butadiene").
    parent_locs: Option<Vec<usize>>,
}

fn parse_prefixes(s: &str, depth: usize) -> Option<Prefixes> {
    let mut c = Cur::new(s);
    let mut list = Vec::new();
    let mut parent_locs = None;
    while !c.done() {
        c.eat("-");
        if c.done() {
            break;
        }
        let save = c.i;
        let mut locs = Vec::new();
        if let Some(l) = c.locant_list() {
            if c.eat("-") {
                locs = l;
            } else {
                c.i = save;
            }
        } else {
            for (p, l) in [("o-", "o"), ("m-", "m"), ("p-", "p")] {
                if c.starts(p) && c.rest().len() > 2 {
                    c.i += 2;
                    locs = vec![l.to_string()];
                    break;
                }
            }
        }
        if c.done() {
            parent_locs = Some(locs.iter().filter_map(|l| l.parse().ok()).collect());
            break;
        }
        let (count, smiles) = substituent(&mut c, depth)?;
        list.push(Pref { locs, count, smiles });
    }
    Some(Prefixes { list, parent_locs })
}

/// Named groups and their SMILES (the first atom is the one attached).
const GROUPS: &[(&str, &str)] = &[
    ("fluoro", "F"),
    ("chloro", "Cl"),
    ("bromo", "Br"),
    ("iodo", "I"),
    ("hydroxy", "O"),
    ("oxo", "=O"),
    ("amino", "N"),
    ("imino", "=N"),
    ("nitro", "[N+](=O)[O-]"),
    ("nitroso", "N=O"),
    ("azido", "N=[N+]=[N-]"),
    ("cyano", "C#N"),
    ("carboxy", "C(=O)O"),
    ("formyl", "C=O"),
    ("acetyl", "C(C)=O"),
    ("benzoyl", "C(=O)c1ccccc1"),
    ("carbamoyl", "C(N)=O"),
    ("sulfanyl", "S"),
    ("mercapto", "S"),
    ("sulfo", "S(=O)(=O)O"),
    ("sulfamoyl", "S(N)(=O)=O"),
    ("hydrazinyl", "NN"),
    ("methoxy", "OC"),
    ("ethoxy", "OCC"),
    ("propoxy", "OCCC"),
    ("isopropoxy", "OC(C)C"),
    ("butoxy", "OCCCC"),
    ("tert-butoxy", "OC(C)(C)C"),
    ("phenoxy", "Oc1ccccc1"),
    ("benzyloxy", "OCc1ccccc1"),
    ("acetoxy", "OC(C)=O"),
    ("acetyloxy", "OC(C)=O"),
    ("benzoyloxy", "OC(=O)c1ccccc1"),
    ("acetamido", "NC(C)=O"),
    ("acetylamino", "NC(C)=O"),
    ("methylamino", "NC"),
    ("dimethylamino", "N(C)C"),
    ("ethylamino", "NCC"),
    ("diethylamino", "N(CC)CC"),
    ("methylthio", "SC"),
    ("methylsulfanyl", "SC"),
    ("methylsulfonyl", "S(C)(=O)=O"),
    ("trifluoromethyl", "C(F)(F)F"),
    ("trichloromethyl", "C(Cl)(Cl)Cl"),
    ("methoxycarbonyl", "C(=O)OC"),
    ("ethoxycarbonyl", "C(=O)OCC"),
    ("isopropyl", "C(C)C"),
    ("isobutyl", "CC(C)C"),
    ("sec-butyl", "C(C)CC"),
    ("tert-butyl", "C(C)(C)C"),
    ("t-butyl", "C(C)(C)C"),
    ("neopentyl", "CC(C)(C)C"),
    ("isopentyl", "CCC(C)C"),
    ("vinyl", "C=C"),
    ("ethenyl", "C=C"),
    ("allyl", "CC=C"),
    ("ethynyl", "C#C"),
    ("propargyl", "CC#C"),
    ("phenyl", "c1ccccc1"),
    ("benzyl", "Cc1ccccc1"),
    ("phenethyl", "CCc1ccccc1"),
];

/// One prefix group at the cursor, with its multiplier: (count, SMILES).
fn substituent(c: &mut Cur, depth: usize) -> Option<(usize, String)> {
    let save = c.i;
    if let Some(r) = one_group(c, depth) {
        return Some((1, r));
    }
    c.i = save;
    for (k, m) in MULT_COMPLEX.iter().enumerate().rev() {
        if k >= 2 && c.starts(m) && c.rest()[m.len()..].starts_with('(') {
            c.i += m.len();
            let g = one_group(c, depth)?;
            return Some((k, g));
        }
    }
    for (k, m) in MULT.iter().enumerate().rev() {
        if k >= 2 && c.eat(m) {
            if let Some(g) = one_group(c, depth) {
                return Some((k, g));
            }
            c.i = save;
        }
    }
    None
}

/// A single group name (no multiplier), with "-oxy", "-amino", "-sulfanyl" endings.
fn one_group(c: &mut Cur, depth: usize) -> Option<String> {
    let start = c.i;
    c.eat("n-");
    let base = if c.starts("(") {
        // A group in parentheses: "(2-methylpropyl)", "(4-chlorophenyl)", "(dimethylamino)".
        let r = c.rest();
        let mut level = 0;
        let mut end = None;
        for (k, ch) in r.char_indices() {
            match ch {
                '(' => level += 1,
                ')' => {
                    level -= 1;
                    if level == 0 {
                        end = Some(k);
                        break;
                    }
                }
                _ => {}
            }
        }
        let end = end?;
        let inner = &r[1..end];
        c.i += end + 1;
        group_word(inner, depth)?
    } else {
        // The longest known name, or a plain alkyl / cycloalkyl group.
        let r = c.rest();
        let mut best: Option<(usize, String)> = None;
        for (n, smi) in GROUPS {
            if r.starts_with(n) && best.as_ref().is_none_or(|b| n.len() > b.0) {
                best = Some((n.len(), smi.to_string()));
            }
        }
        let cyc = r.starts_with("cyclo");
        let rr = if cyc { &r[5..] } else { r };
        for (k, st) in STEMS.iter().enumerate().skip(1) {
            let word = format!("{}yl", st);
            if rr.starts_with(&word) {
                let len = word.len() + if cyc { 5 } else { 0 };
                if best.as_ref().is_none_or(|b| len > b.0) && !(cyc && k < 3) {
                    let sk = if cyc { Skel::cyclo(k) } else { Skel::chain(k) };
                    best = Some((len, sk.smiles(0, depth + 1)));
                }
            }
        }
        let Some((len, smi)) = best else {
            c.i = start;
            return None;
        };
        c.i += len;
        smi
    };
    // Endings that turn a carbon group into an ether, amine or sulfide group.
    if c.eat("oxycarbonyl") {
        return Some(format!("C(=O)O{}", base));
    }
    if c.eat("oxy") {
        return Some(format!("O{}", base));
    }
    if c.eat("amino") {
        return Some(format!("N{}", base));
    }
    if c.eat("sulfanyl") || c.eat("thio") {
        return Some(format!("S{}", base));
    }
    if c.eat("sulfinyl") {
        return Some(format!("S(=O){}", base));
    }
    if c.eat("sulfonyl") {
        return Some(format!("S(=O)(=O){}", base));
    }
    Some(base)
}

/// Without a leading stereodescriptor: "(s)-1-hydroxyethyl" → "1-hydroxyethyl".
fn strip_stereo(w: &str) -> &str {
    if let Some(rest) = w.strip_prefix('(') {
        if let Some(end) = rest.find(")-") {
            let inner = &rest[..end];
            let stereo = !inner.is_empty()
                && inner.split(',').all(|t| {
                    // "s", "2r", "3ae" (a lettered locant), "4e".
                    let t = t.trim().trim_start_matches(|c: char| c.is_ascii_digit());
                    let t = if t.len() == 2 && ('a'..='d').contains(&t.chars().next().unwrap_or(' ')) { &t[1..] } else { t };
                    matches!(t, "e" | "z" | "r" | "s")
                });
            if stereo {
                return &rest[end + 2..];
            }
        }
    }
    w
}

/// The text inside parentheses: a group name ("dimethylamino") or a group with its own
/// prefixes ("2-methylpropyl", "4-chlorophenyl", "propan-2-yl", "2-chloroethoxy").
fn group_word(w: &str, depth: usize) -> Option<String> {
    // The group's own stereodescriptors ("(s)-1-hydroxyethyl") don't change its atoms.
    let w = strip_stereo(w);
    // Nitrogen chains: "3-(…)triaz-2-en-1-yl", "phenyldiazenyl".
    if let Some(head) = w.strip_suffix("triaz-2-en-1-yl") {
        let prefs = parse_prefixes(head, depth + 1)?;
        let mut s = String::from("NN=N");
        for p in &prefs.list {
            if !p.locs.iter().all(|l| l == "3") || p.count != 1 {
                return None;
            }
            s.push('(');
            s.push_str(&p.smiles);
            s.push(')');
        }
        return Some(s);
    }
    if let Some(head) = w.strip_suffix("diazenyl") {
        if head.is_empty() {
            return Some("N=N".into());
        }
        let mut c = Cur::new(head);
        let (1, g) = substituent(&mut c, depth + 1)? else { return None };
        return c.done().then(|| format!("N=N{}", g));
    }
    // Several groups on one P: "diphenylphosphoryl", "diphenylphosphanyl".
    for (end, head_atom) in [("phosphoryl", "P(=O)"), ("phosphanyl", "P"), ("phosphino", "P")] {
        if let Some(head) = w.strip_suffix(end) {
            let mut c = Cur::new(head);
            let mut s = head_atom.to_string();
            let mut k = 0;
            while !c.done() {
                let (count, g) = substituent(&mut c, depth + 1)?;
                for _ in 0..count {
                    s.push('(');
                    s.push_str(&g);
                    s.push(')');
                    k += 1;
                }
            }
            if k > 0 {
                return Some(s);
            }
        }
    }
    let mut c = Cur::new(w);
    if let Some((1, g)) = substituent(&mut c, depth + 1) {
        if c.done() {
            return Some(g);
        }
    }
    if let Some((sk, root)) = compound(w, true, None, depth + 1) {
        return Some(sk.smiles(root, depth + 1));
    }
    // "pyridin-2-ylmethyl", "furan-2-ylmethoxy": a ring group on a short chain, written
    // without inner parentheses.
    for chain in ["methyl", "ethyl", "propyl", "methoxy", "ethoxy", "methylamino"] {
        if let Some(head) = w.strip_suffix(chain) {
            if head.ends_with("yl") && !head.starts_with('(') {
                if let Some(g) = group_word(&format!("({}){}", head, chain), depth + 1) {
                    return Some(g);
                }
            }
        }
    }
    // Acyl groups with prefixes: "2,2-dichloroacetyl" (= ethanoyl), "4-methylbenzoyl".
    for (old, new) in [("acetyl", "ethanoyl"), ("formyl", "methanoyl")] {
        if let Some(head) = w.strip_suffix(old) {
            if let Some((sk, root)) = compound(&format!("{}{}", head, new), true, None, depth + 1) {
                return Some(sk.smiles(root, depth + 1));
            }
        }
    }
    if let Some(head) = w.strip_suffix("benzoyl") {
        let prefs = parse_prefixes(head, depth + 1)?;
        let mut sk = Skel::benzene();
        place_prefixes(&mut sk, &prefs.list, true)?;
        return Some(format!("C(=O){}", sk.smiles(0, depth + 1)));
    }
    if let Some(head) = w.strip_suffix("carbamoyl") {
        // "methylcarbamoyl", "dimethylcarbamoyl".
        let mut c = Cur::new(head);
        let mut n = String::from("C(=O)N");
        while !c.done() {
            let (count, g) = substituent(&mut c, depth + 1)?;
            for _ in 0..count {
                n.push('(');
                n.push_str(&g);
                n.push(')');
            }
        }
        return Some(n);
    }
    if let Some(head) = w.strip_suffix("carbonyl") {
        if let Some(g) = group_word(head, depth + 1) {
            return Some(format!("C(=O){}", g));
        }
    }
    // An amino group with several groups on it: "ethyl(methyl)amino",
    // "bis(2-chloroethyl)amino", "(2-aminoethyl)(carboxymethyl)amino".
    if let Some(head) = w.strip_suffix("amino") {
        let mut c = Cur::new(head);
        let mut n = String::from("N");
        while !c.done() {
            let (count, g) = substituent(&mut c, depth + 1)?;
            for _ in 0..count {
                n.push('(');
                n.push_str(&g);
                n.push(')');
            }
        }
        if n.len() > 1 {
            return Some(n);
        }
    }
    // "…oxy", "…amino" and "…sulfanyl" after a complex group ("2-chloroethoxy").
    for (end, head) in [("oxy", "O"), ("amino", "N"), ("sulfanyl", "S")] {
        if let Some(stem) = w.strip_suffix(end) {
            for word in [format!("{}yl", stem), stem.to_string()] {
                if let Some((sk, root)) = compound(&word, true, None, depth + 1) {
                    return Some(format!("{}{}", head, sk.smiles(root, depth + 1)));
                }
            }
        }
    }
    None
}

/// Benzene parents: "4-nitrophenol", "1,3-dichlorobenzene", "benzene-1,2-diol", "(4-methylphenyl)".
fn benzene_compound(s: &str, yl: bool, ester: Option<&str>, depth: usize) -> Option<(Skel, usize)> {
    let parents: Vec<(&str, &str)> = if yl { vec![("phenyl", "")] } else { BENZENE_PARENTS.to_vec() };
    for (word, group) in parents {
        let Some(prefix) = s.strip_suffix(word) else { continue };
        if !prefix.is_empty() && !prefix.ends_with(|c: char| c.is_alphabetic() || c == ')' || c == '-') {
            continue;
        }
        let Some(prefs) = parse_prefixes(prefix, depth) else { continue };
        let mut sk = Skel::benzene();
        let fixed = !group.is_empty();
        if group.contains('*') != ester.is_some() {
            return None;
        }
        // "N,N-dimethylaniline", "N-methylbenzamide": groups on the nitrogen.
        let (n_prefs, c_prefs): (Vec<Pref>, Vec<Pref>) = prefs.list.iter().cloned().partition(|p| p.locs.iter().any(|l| l == "N"));
        let group = if n_prefs.is_empty() {
            fill_ester(group, ester)?
        } else {
            match group {
                "N" => n_group(&n_prefs),
                "C(N)=O" => format!("C(=O){}", n_group(&n_prefs)),
                _ => return None,
            }
        };
        if fixed {
            sk.branches[0].push(group);
        }
        place_prefixes(&mut sk, &c_prefs, fixed || yl)?;
        return Some((sk, 0));
    }
    // "benzene-1,4-dicarboxylic acid", "benzene-1,2-diol".
    if !yl {
        if let Some(pos) = s.rfind("benzene-") {
            let prefix = &s[..pos];
            let mut c = Cur::new(&s[pos + "benzene".len()..]);
            let locs = c.dash_locs()?;
            let m = c.mult();
            let (_, g) = RING_SUFFIXES.iter().find(|(w, _)| c.rest() == *w)?;
            if locs.len() != m || locs.iter().any(|&l| l == 0 || l > 6) || g.contains('*') != ester.is_some() {
                return None;
            }
            let prefs = parse_prefixes(prefix, depth)?;
            let mut sk = Skel::benzene();
            for l in locs {
                sk.branches[l - 1].push(fill_ester(g, ester)?);
            }
            place_prefixes(&mut sk, &prefs.list, true)?;
            return Some((sk, 0));
        }
    }
    None
}

/// Put the ester's alkyl group in place of the "*" in a group text.
fn fill_ester(g: &str, ester: Option<&str>) -> Option<String> {
    if g.contains('*') {
        Some(g.replace('*', ester?))
    } else {
        Some(g.to_string())
    }
}

/// Hang the prefixes on a skeleton. Without locants, groups go to the first places (on a
/// benzene with a group at atom 1, "o-/m-/p-" mean 2, 3 and 4).
fn place_prefixes(sk: &mut Skel, list: &[Pref], first_taken: bool) -> Option<()> {
    let n = sk.len();
    for p in list {
        let mut locs: Vec<usize> = Vec::new();
        for l in &p.locs {
            match l.as_str() {
                "o" | "m" | "p" => {
                    let k = match l.as_str() {
                        "o" => 2,
                        "m" => 3,
                        _ => 4,
                    };
                    if !first_taken {
                        locs.push(1);
                    }
                    locs.push(k);
                }
                "N" => return None,
                x => locs.push(x.parse().ok()?),
            }
        }
        if locs.is_empty() {
            if n == 1 {
                // All on the single carbon: "dichloromethane", "diphenylmethanone".
                locs = vec![1; p.count];
            } else {
                let start = if first_taken && sk.ring { 2 } else { 1 };
                locs = (start..start + p.count).collect();
            }
        }
        if locs.len() != p.count {
            return None;
        }
        for l in locs {
            if l == 0 || l > n {
                return None;
            }
            sk.branches[l - 1].push(p.smiles.clone());
        }
    }
    Some(())
}

/// An amine or amide nitrogen with the groups named by "N-" prefixes on it.
fn n_group(n_prefs: &[Pref]) -> String {
    let mut a = String::from("N");
    for p in n_prefs {
        let times = if p.locs.len() == 1 { p.count } else { p.locs.iter().filter(|l| *l == "N").count() };
        for _ in 0..times {
            a.push('(');
            a.push_str(&p.smiles);
            a.push(')');
        }
    }
    a
}

/// Make the skeleton for stem `k` with its bonds, suffix and prefixes.
fn build(k: usize, ring: bool, t: &Tail, prefs: &Prefixes, ester: Option<&str>) -> Option<(Skel, usize)> {
    if ring && k < 3 {
        return None;
    }
    let n = k;
    let mut sk = if ring { Skel::cyclo(n) } else { Skel::chain(n) };
    // Locants written before the parent apply to the suffix (or else the multiple bonds).
    let mut parent_locs = prefs.parent_locs.clone().filter(|l| !l.is_empty());
    let mut unsat = t.unsat.clone();
    let suffix_has_locs = t.suffix.as_ref().is_some_and(|s| !s.1.is_empty() || matches!(s.0, Suf::Yl | Suf::Oyl));
    if t.suffix.is_none() || suffix_has_locs {
        if let Some(pl) = parent_locs.take() {
            if unsat.len() != pl.len() {
                return None;
            }
            for (u, l) in unsat.iter_mut().zip(pl) {
                u.0 = l;
            }
        }
    }
    for (l, o) in &unsat {
        if *l == 0 || *l > sk.bonds.len() {
            return None;
        }
        sk.bonds[l - 1] = *o;
    }
    let mut root = 0;
    // Nitrogen-locant prefixes go onto the amine nitrogen.
    let (n_prefs, c_prefs): (Vec<Pref>, Vec<Pref>) = prefs.list.iter().cloned().partition(|p| p.locs.iter().any(|l| l == "N"));
    if let Some((suf, locs, m)) = &t.suffix {
        let terminal = matches!(suf, Suf::Al | Suf::Acid | Suf::Nitrile | Suf::Amide | Suf::Oate);
        let mut ls = locs.clone();
        if ls.is_empty() {
            ls = parent_locs.take().unwrap_or_default();
        }
        if ls.is_empty() {
            ls = if terminal && *m == 2 { vec![1, n] } else { (1..=*m).collect() };
        }
        if ls.len() != *m {
            return None;
        }
        for &l in &ls {
            if l == 0 || l > n {
                return None;
            }
            if terminal && !ring && l != 1 && l != n {
                return None;
            }
            let b = &mut sk.branches[l - 1];
            match suf {
                Suf::Ol => b.push("O".into()),
                Suf::One | Suf::Al => b.push("=O".into()),
                Suf::Acid => {
                    b.push("=O".into());
                    b.push("O".into());
                }
                Suf::Amine => b.push(n_group(&n_prefs)),
                Suf::Nitrile => b.push("#N".into()),
                Suf::Amide => {
                    b.push("=O".into());
                    b.push(n_group(&n_prefs));
                }
                Suf::Thiol => b.push("S".into()),
                Suf::Oate => {
                    b.push("=O".into());
                    b.push(format!("O{}", ester?));
                }
                Suf::Yl => root = l - 1,
                Suf::Oyl => {
                    b.push("=O".into());
                    root = l - 1;
                }
                Suf::CarboxylicAcid => b.push("C(=O)O".into()),
                Suf::Carbaldehyde => b.push("C=O".into()),
                Suf::Carbonitrile => b.push("C#N".into()),
                Suf::Carboxamide => b.push(format!("C(=O){}", n_group(&n_prefs))),
                Suf::Carboxylate => b.push(format!("C(=O)O{}", ester?)),
            }
        }
        if !n_prefs.is_empty() && !matches!(suf, Suf::Amine | Suf::Amide | Suf::Carboxamide) {
            return None;
        }
        if !n_prefs.is_empty() && ls.len() > 1 {
            return None;
        }
        if matches!(suf, Suf::Oate | Suf::Carboxylate) != ester.is_some() {
            return None;
        }
    } else if ester.is_some() || !n_prefs.is_empty() {
        return None;
    }
    place_prefixes(&mut sk, &c_prefs, false)?;
    Some((sk, root))
}

// ===========================================================================
// Writing names
// ===========================================================================

// ---------------------------------------------------------------------------
// Reading names of named ring systems ("pyridin-3-yl", "4,5-dihydrothiazole-4-carboxylic acid",
// "imidazo[1,2-a]pyrazin-3(7H)-one")

/// A named ring system being built: atoms, ring bonds with orders, groups on each atom.
struct GSkel {
    atoms: Vec<String>,
    bonds: Vec<(usize, usize, u8)>,
    branches: Vec<Vec<String>>,
}

impl GSkel {
    /// SMILES starting at atom `root`, walking the ring bonds depth first.
    fn smiles(&self, root: usize, depth: usize) -> String {
        let n = self.atoms.len();
        let mut adj = vec![Vec::new(); n];
        for &(a, b, o) in &self.bonds {
            adj[a].push((b, o));
            adj[b].push((a, o));
        }
        // Spanning tree and ring closures.
        let mut seen = vec![false; n];
        let mut kids: Vec<Vec<(usize, u8)>> = vec![Vec::new(); n];
        let mut closures: Vec<(usize, usize, u8)> = Vec::new();
        let mut stack = vec![(root, usize::MAX)];
        let mut order = Vec::new();
        while let Some((u, parent)) = stack.pop() {
            if seen[u] {
                continue;
            }
            seen[u] = true;
            order.push(u);
            if parent != usize::MAX {
                let o = adj[u].iter().find(|x| x.0 == parent).map(|x| x.1).unwrap_or(1);
                kids[parent].push((u, o));
            }
            for &(v, _) in adj[u].iter().rev() {
                if !seen[v] {
                    stack.push((v, u));
                }
            }
        }
        // Every ring bond not in the tree closes a ring.
        for &(a, b, o) in &self.bonds {
            let tree = kids[a].iter().any(|x| x.0 == b) || kids[b].iter().any(|x| x.0 == a);
            if !tree {
                closures.push((a, b, o));
            }
        }
        let base = 10 + depth.min(8) * 8;
        let pos: Vec<usize> = (0..n).map(|i| order.iter().position(|&x| x == i).unwrap_or(0)).collect();
        fn write(u: usize, s: &GSkel, kids: &[Vec<(usize, u8)>], closures: &[(usize, usize, u8)], pos: &[usize], base: usize) -> String {
            let mut t = s.atoms[u].clone();
            for (k, &(a, b, o)) in closures.iter().enumerate() {
                if a == u || b == u {
                    let other = if a == u { b } else { a };
                    // The bond order goes on the end written first.
                    if pos[u] < pos[other] {
                        t.push_str(Skel::bond_text(o));
                    }
                    t.push_str(&format!("%{}", base + k));
                }
            }
            for br in &s.branches[u] {
                t.push('(');
                t.push_str(br);
                t.push(')');
            }
            let ks = &kids[u];
            for (i, &(v, o)) in ks.iter().enumerate() {
                let part = format!("{}{}", Skel::bond_text(o), write(v, s, kids, closures, pos, base));
                if i + 1 < ks.len() {
                    t.push('(');
                    t.push_str(&part);
                    t.push(')');
                } else {
                    t.push_str(&part);
                }
            }
            t
        }
        write(root, self, &kids, &closures, &pos, base)
    }
}

/// Ring suffixes after a named ring system.
const RING_WORDS: &[&str] = &["carboxylic acid", "carbaldehyde", "carbonitrile", "carboxamide", "carboxylate", "amine", "thiol", "one", "ol", "yl"];

/// Locants that may carry a letter ("3a", "8a").
fn ring_locants(s: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    for part in s.split(',') {
        let digits: String = part.chars().take_while(|c| c.is_ascii_digit()).collect();
        let rest = &part[digits.len()..];
        if digits.is_empty() || rest.len() > 1 || !rest.chars().all(|c| ('a'..='d').contains(&c)) {
            return None;
        }
        out.push(part.to_string());
    }
    Some(out)
}

/// A named ring system as the parent or as a group: (skeleton, root atom).
fn template_compound(s: &str, yl: bool, ester: Option<&str>, depth: usize) -> Option<(GSkel, usize)> {
    for t in hetero::TEMPLATES {
        // Names are read with square brackets turned into round ones ("benzo(d)thiazole").
        let full_name = t.name.replace('[', "(").replace(']', ")");
        let short = hetero::stem(&full_name).to_string();
        let forms: Vec<&str> = if short != full_name { vec![full_name.as_str(), short.as_str()] } else { vec![full_name.as_str()] };
        for f in forms {
            let mut from = 0;
            let mut hits = Vec::new();
            while let Some(i) = s[from..].find(f) {
                hits.push(from + i);
                from += i + 1;
            }
            for &i in hits.iter().rev() {
                let pre = &s[..i];
                let post = &s[i + f.len()..];
                if let Some(r) = template_parse(t, f == full_name, pre, post, yl, ester, depth, &[]) {
                    return Some(r);
                }
            }
        }
        // Retained names of the saturated forms: "chroman-3-one", "indolin-2-yl".
        for (tn, sat_name, positions) in SATURATED_NAMES {
            if *tn != t.name {
                continue;
            }
            let short = hetero::stem(sat_name);
            for f in [*sat_name, short] {
                let mut from = 0;
                let mut hits = Vec::new();
                while let Some(i) = s[from..].find(f) {
                    hits.push(from + i);
                    from += i + 1;
                }
                for &i in hits.iter().rev() {
                    if let Some(r) = template_parse(t, f == *sat_name, &s[..i], &s[i + f.len()..], yl, ester, depth, positions) {
                        return Some(r);
                    }
                }
            }
        }
    }
    None
}

#[allow(clippy::too_many_arguments)]
fn template_parse(t: &Template, full: bool, pre: &str, post: &str, yl: bool, ester: Option<&str>, depth: usize, presat: &[&str]) -> Option<(GSkel, usize)> {
    let n = t.atoms.len();
    let at = |label: &str| t.atoms.iter().position(|a| a.1 == label);
    // The suffix: "-3-carboxylic acid", "-3(7h)-one", "-2,6(3h,7h)-dione", "-2-yl".
    let mut suffix: Option<(&str, Vec<usize>)> = None;
    let mut added: Vec<usize> = Vec::new();
    if post.is_empty() {
        if yl || !full {
            return None;
        }
    } else {
        let p = post.strip_prefix('-')?;
        let end = p.find('-')?;
        let mut loc_text = &p[..end];
        if let Some(open) = loc_text.find('(') {
            let inner = loc_text[open + 1..].strip_suffix(')')?;
            for h in inner.split(',') {
                added.push(at(h.strip_suffix('h')?)?);
            }
            loc_text = &loc_text[..open];
        }
        let locs: Vec<usize> = ring_locants(loc_text)?.iter().map(|l| at(l)).collect::<Option<_>>()?;
        let mut c = Cur::new(&p[end + 1..]);
        let m = c.mult();
        let w = RING_WORDS.iter().find(|w| c.rest() == **w)?;
        if (*w == "yl") != yl || locs.len() != m {
            return None;
        }
        // "e" is dropped before a vowel only.
        let vowel = MULT[m].chars().next().or_else(|| w.chars().next()).is_some_and(|ch| "aeiouy".contains(ch));
        if full == vowel && t.name.ends_with('e') {
            return None;
        }
        suffix = Some((w, locs));
    }
    // Before the name: fixed prefix, indicated hydrogen ("2h-"), hydro ("4,5-dihydro-").
    let mut rest = pre;
    if let Some(fp) = t.fixed_prefix {
        // Also as plain text, where λ is written "l" ("5h-4l4,5l4-").
        let fpl = fp.to_lowercase();
        let plain = fpl.replace('λ', "l");
        rest = rest.strip_suffix(fpl.as_str()).or_else(|| rest.strip_suffix(plain.as_str()))?;
    }
    let mut indicated = Vec::new();
    if let Some(body) = rest.strip_suffix("h-") {
        let start = body.rfind(['-', ')']).map(|x| x + 1).unwrap_or(0);
        let tok = &body[start..];
        if let Some(ls) = ring_locants(tok) {
            for l in ls {
                indicated.push(at(&l)?);
            }
            rest = &body[..start];
        }
    }
    let mut hydro: Vec<usize> = Vec::new();
    let mut hydro_count = 0;
    for (k, word) in MULT.iter().enumerate().skip(2) {
        let hw = format!("{}hydro", word);
        if let Some(body) = rest.strip_suffix(hw.as_str()).or_else(|| rest.strip_suffix(&format!("{}-", hw))) {
            hydro_count = k;
            let body = body.strip_suffix('-').unwrap_or(body);
            let start = body.rfind(['-', ')']).map(|x| x + 1).unwrap_or(0);
            if let Some(ls) = ring_locants(&body[start..]) {
                for l in ls {
                    hydro.push(at(&l)?);
                }
                rest = &body[..start];
            } else {
                rest = body;
            }
            break;
        }
    }
    let all_prefs = parse_prefixes(rest, depth)?;
    // "N,N-dimethylpyridin-4-amine": groups on the amine nitrogen.
    let (n_prefs, c_list): (Vec<Pref>, Vec<Pref>) = all_prefs.list.iter().cloned().partition(|p| p.locs.iter().any(|l| l == "N"));
    if !n_prefs.is_empty() && !suffix.as_ref().is_some_and(|s| matches!(s.0, "amine" | "carboxamide") && s.1.len() == 1) {
        return None;
    }
    let prefs = Prefixes { list: c_list, parent_locs: all_prefs.parent_locs };
    // Build the ring with its groups.
    let mut sk = GSkel { atoms: t.atoms.iter().map(|a| crate::elements::symbol(a.0).to_string()).collect(), bonds: t.bonds.iter().map(|&(a, b)| (a as usize, b as usize, 1)).collect(), branches: vec![Vec::new(); n] };
    let mut root = 0;
    let mut sat: Vec<bool> = (0..n).map(|k| {
        let el = t.atoms[k].0;
        let tdeg = t.bonds.iter().filter(|b| b.0 as usize == k || b.1 as usize == k).count();
        matches!(el, 8 | 16 | 5) || (el == 7 && tdeg == 3)
    }).collect();
    if t.fixed_prefix.is_some() {
        // The boron–dipyrrole core: N4 carries the double bond and the + charge, B5 the −.
        for (k, a) in t.atoms.iter().enumerate() {
            if a.0 == 5 {
                sk.atoms[k] = "[B-]".into();
            }
            if a.0 == 7 && a.1 == "4" {
                sk.atoms[k] = "[N+]".into();
                sat[k] = false;
            }
        }
    }
    for &k in indicated.iter().chain(hydro.iter()).chain(added.iter()) {
        sat[k] = true;
    }
    // A retained saturated name ("chromane") fixes these positions as saturated.
    for l in presat {
        sat[at(l)?] = true;
    }
    if let Some((w, locs)) = &suffix {
        for &k in locs {
            let g = match *w {
                "carboxylic acid" => "C(=O)O".to_string(),
                "carbaldehyde" => "C=O".to_string(),
                "carbonitrile" => "C#N".to_string(),
                "carboxamide" => format!("C(=O){}", n_group(&n_prefs)),
                "carboxylate" => format!("C(=O)O{}", ester?),
                "amine" => n_group(&n_prefs),
                "thiol" => "S".to_string(),
                "ol" => "O".to_string(),
                "one" => {
                    sat[k] = true;
                    "=O".to_string()
                }
                _ => {
                    root = k;
                    continue;
                }
            };
            sk.branches[k].push(g);
        }
    }
    if (*suffix.as_ref().map(|s| &s.0).unwrap_or(&"")).contains("carboxylate") != ester.is_some() {
        return None;
    }
    for p in &prefs.list {
        let mut locs: Vec<usize> = Vec::new();
        for l in &p.locs {
            locs.push(at(l)?);
        }
        if locs.is_empty() {
            // Without locants: the first ring atoms that can take a group.
            locs = (0..n).filter(|&k| t.atoms[k].0 == 6 && sk.branches[k].is_empty()).take(p.count).collect();
        }
        if locs.len() != p.count {
            return None;
        }
        for k in locs {
            if p.smiles.starts_with('=') {
                sat[k] = true;
            }
            sk.branches[k].push(p.smiles.clone());
        }
    }
    if prefs.parent_locs.is_some_and(|l| !l.is_empty()) {
        return None;
    }
    // "tetrahydro" without locants: every remaining position that could carry a double bond.
    if hydro_count > 0 && hydro.is_empty() {
        let free: Vec<usize> = (0..n).filter(|&k| !sat[k]).collect();
        if free.len() != hydro_count {
            return None;
        }
        for k in free {
            sat[k] = true;
        }
    }
    // Double bonds: a perfect pairing of the remaining atoms along ring bonds.
    if !t.saturated {
        let free: Vec<usize> = (0..n).filter(|&k| !sat[k]).collect();
        let mut pairs = Vec::new();
        if !pair_up(&free, &sk.bonds, &mut vec![false; n], &mut pairs) {
            return None;
        }
        for (a, b) in pairs {
            for bd in sk.bonds.iter_mut() {
                if (bd.0 == a && bd.1 == b) || (bd.0 == b && bd.1 == a) {
                    bd.2 = 2;
                }
            }
        }
    }
    Some((sk, root))
}

/// Pair every atom in `free` with a neighbour in `free` (a Kekulé structure).
fn pair_up(free: &[usize], bonds: &[(usize, usize, u8)], used: &mut Vec<bool>, out: &mut Vec<(usize, usize)>) -> bool {
    let Some(&a) = free.iter().find(|&&a| !used[a]) else { return true };
    used[a] = true;
    for &(x, y, _) in bonds {
        let other = if x == a { y } else if y == a { x } else { continue };
        if free.contains(&other) && !used[other] {
            used[other] = true;
            out.push((a, other));
            if pair_up(free, bonds, used, out) {
                return true;
            }
            out.pop();
            used[other] = false;
        }
    }
    used[a] = false;
    false
}

use crate::cip::Descriptors;
use crate::hetero::{self, Template, locant_key};

/// Main groups, lowest priority first.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Fg {
    Amine,
    Thiol,
    Alcohol,
    Ketone,
    Aldehyde,
    Nitrile,
    Amide,
    Ester,
    Acid,
}

#[derive(Clone, Debug)]
struct Group {
    kind: Fg,
    /// The carbon the group belongs to.
    c: usize,
    /// Its other atoms (O, N, S), named by the suffix.
    atoms: Vec<usize>,
    /// Ester: the first carbon of the alkyl part. Amine and amide: the nitrogen.
    extra: Option<usize>,
}

/// A ring system in the drawing.
enum SysKind {
    /// A single carbon ring, atoms in walking order.
    Carbo { order: Vec<usize>, aromatic: bool },
    /// A named ring system: the template and every allowed numbering.
    Tpl { tpl: usize, maps: Vec<Vec<usize>> },
}

struct Sys {
    atoms: Vec<usize>,
    kind: SysKind,
}

struct Mol {
    el: Vec<u8>,
    chg: Vec<i32>,
    /// Neighbours with bond order (as drawn: 1, 2, 3).
    adj: Vec<Vec<(usize, u8)>>,
    /// Bonds in aromatic rings.
    aro: HashSet<(usize, usize)>,
    sys_of: Vec<Option<usize>>,
    systems: Vec<Sys>,
}

impl Mol {
    fn new(d: &Doc) -> Option<Mol> {
        let n = d.atoms.len();
        let aro_b = chem::aromatic_bonds(d);
        let mut adj = vec![Vec::new(); n];
        let mut aro = HashSet::new();
        for (bi, b) in d.bonds.iter().enumerate() {
            if b.order == 0 || b.order > 3 {
                return None;
            }
            adj[b.a].push((b.b, b.order));
            adj[b.b].push((b.a, b.order));
            if aro_b.contains(&bi) {
                aro.insert((b.a.min(b.b), b.a.max(b.b)));
            }
        }
        let mut el = Vec::with_capacity(n);
        let mut chg = Vec::with_capacity(n);
        for a in &d.atoms {
            if a.label.is_some() || a.radical != 0 || a.isotope != 0 {
                return None;
            }
            if !matches!(a.el, 5 | 6 | 7 | 8 | 15 | 16 | 9 | 17 | 35 | 53) {
                return None;
            }
            el.push(a.el);
            chg.push(a.charge as i32);
        }
        // Ring systems: rings that share atoms belong together.
        let rings = chem::sssr(d);
        let mut sys_id: Vec<Option<usize>> = vec![None; n];
        let mut groups: Vec<Vec<usize>> = Vec::new();
        for r in &rings {
            let hit: Vec<usize> = r.iter().filter_map(|&a| sys_id[a]).collect();
            let target = match hit.first() {
                Some(&g) => g,
                None => {
                    groups.push(Vec::new());
                    groups.len() - 1
                }
            };
            // Merge any other systems this ring touches.
            for &g in &hit {
                if g != target {
                    let moved = std::mem::take(&mut groups[g]);
                    for &a in &moved {
                        sys_id[a] = Some(target);
                    }
                    groups[target].extend(moved);
                }
            }
            for &a in r {
                if sys_id[a] != Some(target) {
                    sys_id[a] = Some(target);
                    groups[target].push(a);
                }
            }
        }
        let mut systems = Vec::new();
        let mut sys_of = vec![None; n];
        for atoms in groups.into_iter().filter(|g| !g.is_empty()) {
            let set: HashSet<usize> = atoms.iter().copied().collect();
            let ring_nbs = |i: usize| -> Vec<usize> {
                // Neighbours in the system joined by a ring bond (not a bond between two
                // rings' atoms that isn't in any ring).
                adj[i].iter().map(|x| x.0).filter(|&j| set.contains(&j) && rings.iter().any(|r| r.contains(&i) && r.contains(&j) && adjacent_in(r, i, j))).collect()
            };
            let all_c = atoms.iter().all(|&a| el[a] == 6);
            let one_ring = rings.iter().filter(|r| r.iter().all(|a| set.contains(a))).count() == 1;
            let kind = if all_c && one_ring {
                // Walk the ring.
                let mut order = vec![atoms[0]];
                let mut cur = atoms[0];
                while order.len() < atoms.len() {
                    let next = ring_nbs(cur).into_iter().find(|v| !order.contains(v))?;
                    cur = next;
                    order.push(next);
                }
                let k = order.len();
                let aromatic = k == 6 && (0..k).all(|i| aro.contains(&(order[i].min(order[(i + 1) % k]), order[i].max(order[(i + 1) % k]))));
                SysKind::Carbo { order, aromatic }
            } else {
                let saturated = atoms.iter().all(|&a| ring_nbs(a).iter().all(|&b| adj[a].iter().any(|x| x.0 == b && x.1 == 1) && !aro.contains(&(a.min(b), a.max(b)))));
                let (tpl, maps) = hetero::match_system(&atoms, &|i| el[i], &ring_nbs, saturated)?;
                SysKind::Tpl { tpl, maps }
            };
            let si = systems.len();
            for &a in &atoms {
                sys_of[a] = Some(si);
            }
            systems.push(Sys { atoms, kind });
        }
        // Aromatic bonds only inside benzene rings or named ring systems.
        for &(a, b) in &aro {
            if sys_of[a].is_none() || sys_of[a] != sys_of[b] {
                return None;
            }
        }
        Some(Mol { el, chg, adj, aro, sys_of, systems })
    }

    fn deg(&self, i: usize) -> usize {
        self.adj[i].len()
    }
    fn order(&self, a: usize, b: usize) -> u8 {
        self.adj[a].iter().find(|x| x.0 == b).map(|x| x.1).unwrap_or(0)
    }
    fn is_c(&self, i: usize) -> bool {
        self.el[i] == 6
    }
    fn is_aromatic_bond(&self, a: usize, b: usize) -> bool {
        self.aro.contains(&(a.min(b), a.max(b)))
    }
    /// A benzene ring.
    fn benzene(&self, s: usize) -> bool {
        matches!(self.systems[s].kind, SysKind::Carbo { aromatic: true, .. })
    }
    fn template(&self, s: usize) -> Option<&'static Template> {
        match &self.systems[s].kind {
            SysKind::Tpl { tpl, .. } => Some(&hetero::TEMPLATES[*tpl]),
            _ => None,
        }
    }
    /// The =O on a carbon (an oxygen with no other bond).
    fn double_o(&self, c: usize) -> Option<usize> {
        self.adj[c].iter().find(|&&(o, b)| b == 2 && self.el[o] == 8 && self.deg(o) == 1 && self.chg[o] == 0).map(|x| x.0)
    }
    fn is_nitro(&self, n: usize) -> bool {
        self.el[n] == 7
            && self.chg[n] == 1
            && self.adj[n].iter().filter(|&&(o, _)| self.el[o] == 8 && self.deg(o) == 1).count() == 2
            && self.adj[n].iter().any(|&(o, b)| self.el[o] == 8 && b == 2)
            && self.adj[n].iter().any(|&(o, b)| self.el[o] == 8 && b == 1 && self.chg[o] == -1)
    }
    /// The atom carries a double bond to another atom of its ring system (or an aromatic one).
    fn ring_unsaturated(&self, a: usize) -> bool {
        let s = self.sys_of[a];
        // Drawn double bonds only: an aromatic ring's NH ("1H-indole") has none.
        self.adj[a].iter().any(|&(b, o)| self.sys_of[b] == s && s.is_some() && o == 2)
    }
}

/// Atoms i and j are next to each other around ring `r` (a list of atoms in ring order or not).
fn adjacent_in(r: &[usize], i: usize, j: usize) -> bool {
    let _ = (i, j);
    // SSSR rings come as atom sets; a bond between two atoms of the same small ring is a ring
    // bond unless it bridges the ring (not possible in an SSSR ring).
    r.contains(&i) && r.contains(&j)
}

/// Find the groups a suffix can name. None for structures this namer doesn't handle.
fn find_groups(m: &Mol) -> Option<Vec<Group>> {
    let n = m.el.len();
    let mut out = Vec::new();
    for i in 0..n {
        if m.chg[i] != 0 {
            // Charges: nitro groups, onium ions (R4P+, R4N+) and charges inside a named ring
            // system (the boron–dipyrrole core).
            let nitro = (m.el[i] == 7 && m.is_nitro(i)) || (m.el[i] == 8 && m.chg[i] == -1 && m.adj[i].len() == 1 && m.is_nitro(m.adj[i][0].0));
            let onium = m.chg[i] == 1 && matches!(m.el[i], 7 | 15) && m.deg(i) == 4 && m.adj[i].iter().all(|x| m.is_c(x.0) && x.1 == 1);
            let in_named = m.sys_of[i].and_then(|s| m.template(s)).is_some_and(|t| t.fixed_prefix.is_some());
            if !(nitro || onium || in_named) {
                return None;
            }
        }
        match m.el[i] {
            7 => {
                if m.chg[i] != 0 {
                    continue;
                }
                if m.adj[i].len() == 1 && m.adj[i][0].1 == 3 && m.is_c(m.adj[i][0].0) {
                    continue; // nitrile N
                }
                // Amines: nitrogen bonded only to carbons by single bonds, not in a ring
                // system's double bonds and not next to a C=O.
                // A nitrogen in a ring is part of the ring ("1-methylpyrrolidine"), not an amine.
                if m.adj[i].iter().any(|x| x.1 != 1 || !m.is_c(x.0)) || m.sys_of[i].is_some() {
                    continue;
                }
                if m.adj[i].iter().any(|x| m.double_o(x.0).is_some()) {
                    continue;
                }
                for &(c, _) in &m.adj[i] {
                    // An NH in a named ring is part of the ring, not an amine.
                    if m.sys_of[i].is_some() && m.sys_of[i] == m.sys_of[c] {
                        continue;
                    }
                    out.push(Group { kind: Fg::Amine, c, atoms: vec![i], extra: Some(i) });
                }
            }
            6 => {
                let dbl_o = m.double_o(i);
                let mut oh = None;
                let mut or = None;
                let mut amide_n = None;
                let mut sh = None;
                let mut nitrile = None;
                let mut other_hetero = false;
                for &(x, b) in &m.adj[i] {
                    match (m.el[x], b) {
                        (8, 1) if m.deg(x) == 1 && m.chg[x] == 0 => oh = Some(x),
                        (8, 1) if m.deg(x) == 2 => {
                            let other = m.adj[x].iter().map(|y| y.0).find(|&y| y != i)?;
                            if m.is_c(other) {
                                or = Some((x, other));
                            } else {
                                other_hetero = true;
                            }
                        }
                        (7, 1) if m.chg[x] == 0 && m.adj[x].iter().all(|y| m.is_c(y.0) && y.1 == 1) => amide_n = Some(x),
                        (7, 3) if m.deg(x) == 1 => nitrile = Some(x),
                        (16, 1) if m.deg(x) == 1 => sh = Some(x),
                        (6, _) => {}
                        (8, 2) => {}
                        _ => other_hetero = true,
                    }
                }
                let carbons = m.adj[i].iter().filter(|x| m.is_c(x.0)).count();
                if let Some(o) = dbl_o {
                    let ring = m.sys_of[i];
                    if ring.is_some_and(|s| m.benzene(s)) {
                        return None;
                    }
                    // A C=O in a ring next to a ring O or N: named as a ring ketone
                    // ("pyridin-2(1H)-one", "2H-chromen-2-one").
                    let hetero_in_ring = |x: Option<usize>| x.is_some_and(|x| ring.is_some() && m.sys_of[x] == ring);
                    if ring.is_some() && (hetero_in_ring(amide_n) || hetero_in_ring(or.map(|r| r.0)) || m.adj[i].iter().all(|x| x.0 == o || m.sys_of[x.0] == ring)) {
                        out.push(Group { kind: Fg::Ketone, c: i, atoms: vec![o], extra: None });
                        continue;
                    }
                    if let Some(h) = oh {
                        out.push(Group { kind: Fg::Acid, c: i, atoms: vec![o, h], extra: None });
                    } else if let Some((eo, r)) = or {
                        // Not an anhydride or carbonate.
                        if m.double_o(r).is_some() || m.adj[i].iter().filter(|x| m.el[x.0] == 8).count() > 2 {
                            return None;
                        }
                        out.push(Group { kind: Fg::Ester, c: i, atoms: vec![o, eo], extra: Some(r) });
                    } else if let Some(nn) = amide_n {
                        if m.adj[nn].iter().filter(|y| m.double_o(y.0).is_some()).count() > 1 {
                            return None; // imides
                        }
                        if m.sys_of[nn].is_some() {
                            // C=O on a ring nitrogen: "(piperazin-1-yl)methanone".
                            out.push(Group { kind: Fg::Ketone, c: i, atoms: vec![o], extra: None });
                        } else {
                            out.push(Group { kind: Fg::Amide, c: i, atoms: vec![o, nn], extra: Some(nn) });
                        }
                    } else if carbons <= 1 && m.deg(i) == carbons + 1 {
                        out.push(Group { kind: Fg::Aldehyde, c: i, atoms: vec![o], extra: None });
                    } else if carbons == 2 && m.deg(i) == 3 {
                        out.push(Group { kind: Fg::Ketone, c: i, atoms: vec![o], extra: None });
                    } else if !other_hetero {
                        return None;
                    }
                    // Otherwise a C=O on another kind of nitrogen (a triazene, a hydrazine…):
                    // named as an acyl prefix from that side.
                } else {
                    if let Some(h) = oh {
                        out.push(Group { kind: Fg::Alcohol, c: i, atoms: vec![h], extra: None });
                    }
                    if let Some(s) = sh {
                        out.push(Group { kind: Fg::Thiol, c: i, atoms: vec![s], extra: None });
                    }
                    if let Some(nn) = nitrile {
                        if m.deg(i) > 2 {
                            return None;
                        }
                        out.push(Group { kind: Fg::Nitrile, c: i, atoms: vec![nn], extra: None });
                    }
                }
            }
            8 => {
                if m.adj[i].iter().any(|x| m.el[x.0] == 8) {
                    return None;
                }
            }
            16 => {
                if m.adj[i].iter().any(|x| m.el[x.0] == 16) {
                    return None;
                }
            }
            9 | 17 | 35 | 53 => {
                if m.deg(i) != 1 || !matches!(m.el[m.adj[i][0].0], 6 | 5 | 15 | 16) {
                    return None;
                }
            }
            5 | 15 => {}
            _ => return None,
        }
    }
    Some(out)
}

/// The atoms on the far side of `start` (coming from `from`), not crossing `stop`.
fn side(m: &Mol, from: usize, start: usize, stop: &HashSet<usize>) -> Vec<usize> {
    let mut seen: HashSet<usize> = HashSet::from([from, start]);
    let mut out = vec![start];
    let mut k = 0;
    while k < out.len() {
        let u = out[k];
        for &(v, _) in &m.adj[u] {
            if !stop.contains(&v) && seen.insert(v) {
                out.push(v);
            }
        }
        k += 1;
    }
    out
}

/// A name for a drawn structure, or None when it is outside what this namer handles.
pub fn structure_to_name(d: &Doc) -> Option<String> {
    if d.atoms.len() > 120 || d.fragments().len() != 1 {
        return None;
    }
    let m = Mol::new(d)?;
    if !m.el.iter().any(|&e| e == 6) {
        return None;
    }
    let groups = find_groups(&m)?;
    let principal = groups.iter().map(|g| g.kind).max();
    let desc = crate::cip::descriptors(d);
    let namer = Namer { m: &m, groups: &groups, principal, desc: &desc };
    // Onium ions: "(4-…phenyl)triphenylphosphonium", "tetramethylammonium".
    if let Some(p) = (0..m.el.len()).find(|&i| m.chg[i] == 1 && matches!(m.el[i], 7 | 15) && m.deg(i) == 4 && m.adj[i].iter().all(|x| m.is_c(x.0))) {
        if principal.is_none() || principal == Some(Fg::Amine) || m.sys_of[p].is_none() {
            let blocked = HashSet::from([p]);
            let mut prefixes = Vec::new();
            for &(x, _) in &m.adj[p] {
                prefixes.push(("1".to_string(), namer.sub_name(p, x, &blocked)?));
            }
            let base = if m.el[p] == 15 { "phosphonium" } else { "ammonium" };
            return Some(format!("{}{}", assemble_list(&prefixes, true), base));
        }
    }
    // Phosphines and phosphine oxides with no other main group: "triphenylphosphine",
    // "triphenylphosphine oxide".
    if principal.is_none() {
        if let Some(p) = (0..m.el.len()).find(|&i| m.el[i] == 15 && m.chg[i] == 0 && m.sys_of[i].is_none() && m.adj[i].iter().filter(|x| m.is_c(x.0)).count() >= 2) {
            let oxide = m.adj[p].iter().any(|&(o, k)| m.el[o] == 8 && k == 2 && m.deg(o) == 1);
            if m.adj[p].iter().all(|&(x, k)| m.is_c(x) && k == 1 || (oxide && m.el[x] == 8 && k == 2)) {
                let blocked = HashSet::from([p]);
                let mut prefixes = Vec::new();
                for &(x, _) in &m.adj[p] {
                    if m.is_c(x) {
                        prefixes.push(("1".to_string(), namer.sub_name(p, x, &blocked)?));
                    }
                }
                return Some(format!("{}phosphine{}", assemble_list(&prefixes, true), if oxide { " oxide" } else { "" }));
            }
        }
    }
    namer.name()
}

struct Namer<'a> {
    m: &'a Mol,
    groups: &'a [Group],
    principal: Option<Fg>,
    desc: &'a Descriptors,
}

/// A candidate parent: a chain (atoms in order) or a ring system.
struct Parent {
    atoms: Vec<usize>,
    sys: Option<usize>,
}

/// One numbering of a parent, with what it needs to be compared and named.
struct Numbered {
    atoms: Vec<usize>,
    /// The locant of each atom ("1", "3a").
    labels: Vec<String>,
    sys: Option<usize>,
    /// Main groups: (position in `atoms`, group index).
    principal: Vec<(usize, usize)>,
    doubles: Vec<usize>,
    triples: Vec<usize>,
    /// Prefixes: (locant text, name).
    prefixes: Vec<(String, String)>,
    /// Named ring systems: indicated hydrogen, added hydrogen ("3(7H)-one") and hydro positions.
    indicated: Vec<usize>,
    added: Option<usize>,
    hydro: Vec<usize>,
}

type Key = (Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>, Vec<u32>);

impl Numbered {
    fn lk(&self, pos: usize) -> u32 {
        locant_key(&self.labels[pos])
    }
    fn key(&self) -> Key {
        let sorted = |mut v: Vec<u32>| {
            v.sort();
            v
        };
        let ind = sorted(self.indicated.iter().map(|&p| self.lk(p)).collect());
        let pl = sorted(self.principal.iter().map(|x| self.lk(x.0)).collect());
        let hy = sorted(self.hydro.iter().chain(self.added.iter()).map(|&p| self.lk(p)).collect());
        let ml = sorted(self.doubles.iter().chain(self.triples.iter()).map(|&p| self.lk(p)).collect());
        let prl = sorted(self.prefixes.iter().filter(|p| p.0 != "N").map(|p| locant_key(&p.0)).collect());
        // Alphabetical order decides last: the locants in the order the prefixes are cited.
        let mut cited: Vec<&(String, String)> = self.prefixes.iter().filter(|p| p.0 != "N").collect();
        cited.sort_by(|a, b| alpha_key(&a.1).cmp(&alpha_key(&b.1)).then(locant_key(&a.0).cmp(&locant_key(&b.0))));
        let al: Vec<u32> = cited.iter().map(|p| locant_key(&p.0)).collect();
        (ind, pl, hy, ml, prl, al)
    }
}

impl<'a> Namer<'a> {
    fn name(&self) -> Option<String> {
        let m = self.m;
        let principal = self.principal;
        // Ester: the alkyl part is named separately, as the first word.
        let mut excluded: HashSet<usize> = HashSet::new();
        let mut ester_alkyl = None;
        if principal == Some(Fg::Ester) {
            let esters: Vec<&Group> = self.groups.iter().filter(|g| g.kind == Fg::Ester).collect();
            if esters.len() != 1 {
                return None;
            }
            let g = esters[0];
            let o = g.atoms[1];
            let r = g.extra?;
            for a in side(m, o, r, &HashSet::from([o])) {
                excluded.insert(a);
            }
            let alkyl = self.sub_name(o, r, &HashSet::from([o]))?;
            ester_alkyl = Some(strip_paren(&alkyl).to_string());
        }
        // Nitrile carbons that are not the main group are named "cyano" and stay out of the
        // parent; amide and ester carbons stay in the chain as "-oxo" with "amino"/"methoxy".
        for g in self.groups {
            if Some(g.kind) != principal && matches!(g.kind, Fg::Nitrile | Fg::Acid) {
                excluded.insert(g.c);
            }
        }
        // Candidate parents: ring systems, and every path between two carbons outside rings.
        let mut parents: Vec<Parent> = Vec::new();
        for (si, s) in m.systems.iter().enumerate() {
            if s.atoms.iter().all(|a| !excluded.contains(a)) {
                parents.push(Parent { atoms: s.atoms.clone(), sys: Some(si) });
            }
        }
        let acyclic: Vec<usize> = (0..m.el.len()).filter(|&i| m.is_c(i) && m.sys_of[i].is_none() && !excluded.contains(&i)).collect();
        let acyc_set: HashSet<usize> = acyclic.iter().copied().collect();
        for &s in &acyclic {
            let mut prev = vec![usize::MAX; m.el.len()];
            let mut seen = HashSet::from([s]);
            let mut q = std::collections::VecDeque::from([s]);
            while let Some(u) = q.pop_front() {
                for &(v, _) in &m.adj[u] {
                    if acyc_set.contains(&v) && seen.insert(v) {
                        prev[v] = u;
                        q.push_back(v);
                    }
                }
            }
            for &t in &acyclic {
                if t < s || !seen.contains(&t) {
                    continue;
                }
                let mut p = vec![t];
                let mut c = t;
                while c != s {
                    c = prev[c];
                    p.push(c);
                }
                parents.push(Parent { atoms: p, sys: None });
            }
        }
        if parents.is_empty() {
            return None;
        }
        // Choose the parent: most main groups, rings first, most atoms, most multiple bonds,
        // most prefixes; then the numbering with the lowest locants.
        let mut best: Option<((usize, usize, usize, usize, usize), Key, Numbered)> = None;
        for p in &parents {
            for (atoms, labels) in self.numberings(p) {
                let Some(nb) = self.describe(atoms, labels, p.sys) else { continue };
                let terminal_inside = p.sys.is_none()
                    && nb.principal.iter().any(|&(pos, gi)| matches!(self.groups[gi].kind, Fg::Acid | Fg::Aldehyde | Fg::Nitrile | Fg::Amide | Fg::Ester) && pos != 0 && pos + 1 != nb.atoms.len());
                if terminal_inside {
                    continue;
                }
                let score = (nb.principal.len(), usize::from(p.sys.is_some()), nb.atoms.len(), nb.doubles.len() + nb.triples.len(), nb.prefixes.len());
                let key = nb.key();
                let better = match &best {
                    None => true,
                    Some((s, k, _)) => score > *s || (score == *s && key < *k),
                };
                if better {
                    best = Some((score, key, nb));
                }
            }
        }
        let (_, _, nb) = best?;
        let parent = self.parent_name(&nb)?;
        let stereo = self.stereo_prefix(&nb.atoms, &nb.labels, nb.sys.is_none() || nb.sys.is_some_and(|s| matches!(m.systems[s].kind, SysKind::Carbo { aromatic: false, .. })), true);
        let whole = format!("{}{}", stereo, cat(&assemble_list(&nb.prefixes, parent.omit_prefix_locants), &parent.text));
        Some(match ester_alkyl {
            Some(a) => format!("{} {}", a, whole),
            None => whole,
        })
    }

    /// "(S)-", "(2R,3E)-": the descriptors of these atoms (and the double bonds between
    /// neighbours in the list, when `chain`). The locant is left out when the whole molecule
    /// has a single descriptor and `whole` is set.
    fn stereo_prefix(&self, atoms: &[usize], labels: &[String], chain: bool, whole: bool) -> String {
        let mut items: Vec<(u32, String)> = Vec::new();
        for (k, &a) in atoms.iter().enumerate() {
            if let Some(c) = self.desc.centres.get(&a) {
                items.push((locant_key(&labels[k]), format!("{}{}", labels[k], c)));
            }
            if chain {
                let next = if k + 1 < atoms.len() { Some(k + 1) } else { None };
                if let Some(j) = next {
                    if let Some(c) = self.desc.bond(a, atoms[j]) {
                        items.push((locant_key(&labels[k]), format!("{}{}", labels[k], c)));
                    }
                }
            }
        }
        if items.is_empty() {
            return String::new();
        }
        items.sort();
        if items.len() == 1 && (self.desc.count() == 1 || !whole) {
            let s = &items[0].1;
            return format!("({})-", s.trim_start_matches(|c: char| c.is_ascii_digit() || c.is_ascii_lowercase()));
        }
        format!("({})-", items.iter().map(|x| x.1.clone()).collect::<Vec<_>>().join(","))
    }

    /// All numberings of a parent: (atoms in locant order, their locants).
    fn numberings(&self, p: &Parent) -> Vec<(Vec<usize>, Vec<String>)> {
        let m = self.m;
        let plain = |v: Vec<usize>| {
            let l = (1..=v.len()).map(|k| k.to_string()).collect();
            (v, l)
        };
        match p.sys.map(|s| &m.systems[s].kind) {
            Some(SysKind::Carbo { order, .. }) => {
                let n = order.len();
                let mut out = Vec::new();
                for s in 0..n {
                    out.push(plain((0..n).map(|k| order[(s + k) % n]).collect()));
                    out.push(plain((0..n).map(|k| order[(s + n - k) % n]).collect()));
                }
                out
            }
            Some(SysKind::Tpl { tpl, maps }) => {
                let t = &hetero::TEMPLATES[*tpl];
                maps.iter().map(|mp| (mp.clone(), t.atoms.iter().map(|a| a.1.to_string()).collect())).collect()
            }
            None => {
                if p.atoms.len() == 1 {
                    vec![plain(p.atoms.clone())]
                } else {
                    let mut r = p.atoms.clone();
                    r.reverse();
                    vec![plain(p.atoms.clone()), plain(r)]
                }
            }
        }
    }

    /// Main groups, multiple bonds, ring hydrogen and prefixes for one numbering.
    fn describe(&self, atoms: Vec<usize>, labels: Vec<String>, sys: Option<usize>) -> Option<Numbered> {
        let m = self.m;
        let set: HashSet<usize> = atoms.iter().copied().collect();
        let pos = |a: usize| atoms.iter().position(|&x| x == a);
        let mut principal = Vec::new();
        // Atoms named by the suffix (left out of the prefixes).
        let mut consumed: HashSet<usize> = HashSet::new();
        let mut amine_ns: Vec<usize> = Vec::new();
        for (gi, g) in self.groups.iter().enumerate() {
            if Some(g.kind) != self.principal {
                continue;
            }
            if set.contains(&g.c) {
                if g.kind == Fg::Amine {
                    let nn = g.extra?;
                    if amine_ns.contains(&nn) {
                        continue;
                    }
                    amine_ns.push(nn);
                }
                principal.push((pos(g.c)?, gi));
                consumed.extend(g.atoms.iter().copied());
            } else if sys.is_some() && matches!(g.kind, Fg::Acid | Fg::Aldehyde | Fg::Nitrile | Fg::Amide | Fg::Ester) {
                // On a ring: the group's carbon sits outside it ("…carboxylic acid").
                if let Some(&(r, _)) = m.adj[g.c].iter().find(|x| set.contains(&x.0)) {
                    principal.push((pos(r)?, gi));
                    consumed.insert(g.c);
                    consumed.extend(g.atoms.iter().copied());
                }
            }
        }
        if self.principal == Some(Fg::Ester) {
            for g in self.groups.iter().filter(|g| g.kind == Fg::Ester) {
                if let Some(r) = g.extra {
                    consumed.insert(r);
                }
            }
        }
        let mut doubles = Vec::new();
        let mut triples = Vec::new();
        let n = atoms.len();
        let tpl = sys.and_then(|s| m.template(s));
        let plain_ring = sys.is_some_and(|s| matches!(m.systems[s].kind, SysKind::Carbo { aromatic: false, .. }));
        if sys.is_none() || plain_ring {
            for k in 0..n {
                let next = if k + 1 < n {
                    k + 1
                } else if plain_ring && n > 2 {
                    0
                } else {
                    break;
                };
                match m.order(atoms[k], atoms[next]) {
                    2 => doubles.push(k),
                    3 => triples.push(k),
                    _ => {}
                }
            }
        }
        // Named ring systems: indicated, added and "hydro" hydrogen.
        let (mut indicated, mut added, mut hydro) = (Vec::new(), None, Vec::new());
        if let Some(t) = tpl {
            let ketones: Vec<usize> = principal.iter().filter(|x| self.groups[x.1].kind == Fg::Ketone).map(|x| x.0).collect();
            let (i, a, h) = self.ring_hydrogen(t, &atoms, &ketones)?;
            indicated = i;
            added = a;
            hydro = h;
        }
        let mut prefixes = Vec::new();
        for (k, &a) in atoms.iter().enumerate() {
            for &(b, _) in &m.adj[a] {
                if set.contains(&b) {
                    continue;
                }
                if amine_ns.contains(&b) {
                    // N-substituents of the main amine.
                    for &(x, _) in &m.adj[b] {
                        if x != a && !set.contains(&x) {
                            let mut bl = set.clone();
                            bl.insert(b);
                            prefixes.push(("N".to_string(), self.sub_name(b, x, &bl)?));
                        }
                    }
                    continue;
                }
                if consumed.contains(&b) {
                    continue;
                }
                prefixes.push((labels[k].clone(), self.sub_name(a, b, &set)?));
            }
        }
        // Groups on the nitrogen of the main amide: "N-methyl", "N,N-diethyl".
        for &(_, gi) in &principal {
            let g = &self.groups[gi];
            if g.kind != Fg::Amide {
                continue;
            }
            let Some(nn) = g.extra else { continue };
            for &(x, _) in &m.adj[nn] {
                if x != g.c {
                    let mut bl = set.clone();
                    bl.insert(nn);
                    bl.insert(g.c);
                    prefixes.push(("N".to_string(), self.sub_name(nn, x, &bl)?));
                }
            }
        }
        Some(Numbered { atoms, labels, sys, principal, doubles, triples, prefixes, indicated, added, hydro })
    }

    /// Indicated hydrogen, added hydrogen and hydro positions of a named ring system, compared
    /// with its fully unsaturated form. `ketones`: positions with a main C=O.
    fn ring_hydrogen(&self, t: &Template, atoms: &[usize], ketones: &[usize]) -> Option<(Vec<usize>, Option<usize>, Vec<usize>)> {
        if t.saturated || t.fixed_prefix.is_some() {
            return Some((Vec::new(), None, Vec::new()));
        }
        let m = self.m;
        let tdeg = |k: usize| t.bonds.iter().filter(|b| b.0 as usize == k || b.1 as usize == k).count();
        let mut r: Vec<usize> = (0..atoms.len())
            .filter(|&k| {
                let el = t.atoms[k].0;
                let can = !matches!(el, 8 | 16 | 5) && !(el == 7 && tdeg(k) == 3);
                can && !m.ring_unsaturated(atoms[k]) && !ketones.contains(&k)
            })
            .collect();
        r.sort_by_key(|&k| locant_key(t.atoms[k].1));
        if r.len() < t.indicated {
            // The indicated hydrogen sits on the C=O carbon itself ("2H-chromen-2-one").
            let mut ks: Vec<usize> = ketones.to_vec();
            ks.sort_by_key(|&k| locant_key(t.atoms[k].1));
            let need = t.indicated - r.len();
            if ks.len() < need {
                return None;
            }
            let mut ind = r.clone();
            ind.extend(ks.into_iter().take(need));
            ind.sort_by_key(|&k| locant_key(t.atoms[k].1));
            return Some((ind, None, Vec::new()));
        }
        let indicated = r[..t.indicated].to_vec();
        let mut rest = r[t.indicated..].to_vec();
        let mut added = None;
        if !ketones.is_empty() && rest.len() % 2 == 1 {
            added = Some(rest.remove(0));
        }
        if rest.len() % 2 == 1 {
            return None;
        }
        Some((indicated, added, rest))
    }

    /// The name of the group that starts at `b` (attached to `a`), not entering `blocked`.
    fn sub_name(&self, a: usize, b: usize, blocked: &HashSet<usize>) -> Option<String> {
        let m = self.m;
        let ob = m.order(a, b);
        let mut bl = blocked.clone();
        bl.insert(b);
        bl.insert(a);
        let others: Vec<usize> = m.adj[b].iter().map(|x| x.0).filter(|&x| x != a).collect();
        // Any atom of a ring system (a ring nitrogen too) is named through its ring:
        // "(piperazin-1-yl)", "(pyridin-3-yl)".
        if let Some(si) = m.sys_of[b] {
            return match m.systems[si].kind {
                SysKind::Carbo { .. } => self.ring_sub_name(si, b, a, blocked),
                SysKind::Tpl { .. } => self.tpl_sub_name(si, b, a, blocked),
            };
        }
        match m.el[b] {
            9 => return Some("fluoro".into()),
            17 => return Some("chloro".into()),
            35 => return Some("bromo".into()),
            53 => return Some("iodo".into()),
            8 => {
                if ob == 2 {
                    return Some("oxo".into());
                }
                if others.is_empty() {
                    return if m.chg[b] == 0 { Some("hydroxy".into()) } else { None };
                }
                let x = others[0];
                if !m.is_c(x) || others.len() > 1 {
                    return None;
                }
                let r = self.sub_name(b, x, &bl)?;
                if m.double_o(x).is_some() && m.sys_of[x].is_none() {
                    // O-C(=O)R: acyloxy ("acetyloxy").
                    return Some(format!("{}oxy", paren_if_complex(&r)));
                }
                return Some(oxy(&r));
            }
            7 => {
                if m.is_nitro(b) {
                    return Some("nitro".into());
                }
                if m.chg[b] != 0 {
                    return None;
                }
                // Nitrogen chains: triazenyl (N–N=N), diazenyl (N=N).
                if let Some(&n2) = others.iter().find(|&&x| m.el[x] == 7) {
                    return self.nitrogen_chain(a, b, n2, &bl);
                }
                if ob != 1 {
                    return None;
                }
                if others.iter().any(|&x| !m.is_c(x)) || m.adj[b].iter().any(|x| x.1 != 1) {
                    return None;
                }
                if others.is_empty() {
                    return Some("amino".into());
                }
                let mut names: Vec<String> = Vec::new();
                for &x in &others {
                    names.push(self.sub_name(b, x, &bl)?);
                }
                if names.len() == 1 && names[0] == "acetyl" {
                    return Some("acetamido".into());
                }
                return Some(group_on(&names, "amino"));
            }
            16 => {
                if ob != 1 {
                    return None;
                }
                if others.is_empty() {
                    return Some("sulfanyl".into());
                }
                let oth: Vec<(usize, u8)> = m.adj[b].iter().copied().filter(|x| x.0 != a).collect();
                let dbl_os = oth.iter().filter(|x| m.el[x.0] == 8 && x.1 == 2).count();
                let singles: Vec<usize> = oth.iter().filter(|x| x.1 == 1).map(|x| x.0).collect();
                if dbl_os == 2 && singles.len() == 1 {
                    let x = singles[0];
                    if m.el[x] == 8 && m.deg(x) == 1 {
                        return Some("sulfo".into());
                    }
                    if m.is_c(x) {
                        let r = self.sub_name(b, x, &bl)?;
                        return Some(format!("{}sulfonyl", paren_if_complex(&r)));
                    }
                    if m.el[x] == 7 && m.deg(x) == 1 {
                        return Some("sulfamoyl".into());
                    }
                    return None;
                }
                if dbl_os == 1 && singles.len() == 1 && m.is_c(singles[0]) {
                    let r = self.sub_name(b, singles[0], &bl)?;
                    return Some(format!("{}sulfinyl", paren_if_complex(&r)));
                }
                if dbl_os == 0 && singles.len() == 1 && m.is_c(singles[0]) {
                    let r = self.sub_name(b, singles[0], &bl)?;
                    return Some(format!("{}sulfanyl", paren_if_complex(&r)));
                }
                return None;
            }
            15 => {
                // Phosphorus: (diphenylphosphoryl) P(=O)R2, (diphenylphosphanyl) PR2.
                if ob != 1 || m.chg[b] != 0 {
                    return None;
                }
                let oxo = m.adj[b].iter().any(|&(o, k)| m.el[o] == 8 && k == 2 && m.deg(o) == 1);
                let mut names = Vec::new();
                for &(x, k) in &m.adj[b] {
                    if x == a || (m.el[x] == 8 && k == 2) {
                        continue;
                    }
                    if m.el[x] == 8 && m.deg(x) == 1 {
                        names.push("hydroxy".to_string());
                        continue;
                    }
                    names.push(self.sub_name(b, x, &bl)?);
                }
                let word = if oxo { "phosphoryl" } else { "phosphanyl" };
                if names.is_empty() {
                    return Some(word.into());
                }
                return Some(group_on(&names, word));
            }
            6 => {}
            _ => return None,
        }
        // Carbon groups.
        if let Some(si) = m.sys_of[b] {
            return match m.systems[si].kind {
                SysKind::Carbo { .. } => self.ring_sub_name(si, b, a, blocked),
                SysKind::Tpl { .. } => self.tpl_sub_name(si, b, a, blocked),
            };
        }
        // Whole groups on one carbon: cyano, carboxy, carbamoyl, alkoxycarbonyl, formyl.
        if let Some(g) = self.groups.iter().find(|g| g.c == b && g.kind != Fg::Amine) {
            let carbons_besides = m.adj[b].iter().filter(|x| m.is_c(x.0) && x.0 != a).count();
            match g.kind {
                Fg::Nitrile => return Some("cyano".into()),
                Fg::Acid if carbons_besides == 0 => return Some("carboxy".into()),
                Fg::Amide if carbons_besides == 0 => {
                    let nn = g.extra?;
                    if nn == a || blocked.contains(&nn) {
                        return self.alkyl_name(a, b, blocked);
                    }
                    let on_n: Vec<usize> = m.adj[nn].iter().map(|x| x.0).filter(|&x| x != b).collect();
                    if on_n.is_empty() {
                        return Some("carbamoyl".into());
                    }
                    bl.insert(nn);
                    let mut names = Vec::new();
                    for x in on_n {
                        names.push(self.sub_name(nn, x, &bl)?);
                    }
                    return Some(format!("({})", strip_paren(&group_on(&names, "carbamoyl"))));
                }
                Fg::Ester if carbons_besides == 0 && !blocked.contains(&g.atoms[1]) => {
                    let o = g.atoms[1];
                    let r = g.extra?;
                    bl.insert(o);
                    let alk = self.sub_name(o, r, &bl)?;
                    return Some(format!("({}carbonyl)", strip_paren(&oxy(&alk))));
                }
                Fg::Aldehyde if carbons_besides == 0 => return Some("formyl".into()),
                _ => {}
            }
        }
        self.alkyl_name(a, b, blocked)
    }

    /// N–N=N–R as "(3-R-triaz-2-en-1-yl)", N=N–R as "(R-diazenyl)" (attached at `b`).
    fn nitrogen_chain(&self, a: usize, b: usize, n2: usize, bl: &HashSet<usize>) -> Option<String> {
        let m = self.m;
        let o12 = m.order(b, n2);
        let mut bl = bl.clone();
        bl.insert(n2);
        let beyond: Vec<usize> = m.adj[n2].iter().map(|x| x.0).filter(|&x| x != b).collect();
        if o12 == 2 {
            // Diazenyl: b=N2–X.
            if beyond.len() != 1 {
                return Some("diazenyl".into());
            }
            let x = beyond[0];
            let r = self.sub_name(n2, x, &bl)?;
            let ez = self.desc.bond(b, n2).map(|c| format!("({})-", c)).unwrap_or_default();
            return Some(format!("({}{}diazenyl)", ez, strip_paren(&r)));
        }
        if o12 == 1 && m.adj[b].iter().all(|x| x.1 == 1) {
            if let Some(&n3) = beyond.iter().find(|&&x| m.el[x] == 7 && m.order(n2, x) == 2) {
                bl.insert(n3);
                let after: Vec<usize> = m.adj[n3].iter().map(|x| x.0).filter(|&x| x != n2).collect();
                let others_on_b: Vec<usize> = m.adj[b].iter().map(|x| x.0).filter(|&x| x != a && x != n2).collect();
                if !others_on_b.is_empty() {
                    return None;
                }
                // The N=N bond's own E/Z goes inside the group's parentheses.
                let ez = self.desc.bond(n2, n3).map(|c| format!("({})-", c)).unwrap_or_default();
                return Some(match after.as_slice() {
                    [] if ez.is_empty() => "triaz-2-en-1-yl".to_string(),
                    [] => format!("({}triaz-2-en-1-yl)", ez),
                    [x] => format!("({}3-{}triaz-2-en-1-yl)", ez, self.sub_name(n3, *x, &bl)?),
                    _ => return None,
                });
            }
            // Hydrazinyl: b–N2H2 / b–N2R.
            let names: Vec<String> = beyond.iter().map(|&x| self.sub_name(n2, x, &bl)).collect::<Option<_>>()?;
            if names.is_empty() {
                return Some("hydrazinyl".into());
            }
            return Some(format!("(2-{}hydrazinyl)", names.join("")));
        }
        let _ = a;
        None
    }

    /// Put a group's own stereodescriptors inside its parentheses: "((S)-1-hydroxyethyl)".
    fn with_stereo(&self, name: String, atoms: &[usize], labels: &[String], chain: bool) -> String {
        let st = self.stereo_prefix(atoms, labels, chain, false);
        if st.is_empty() {
            return name;
        }
        format!("({}{})", st, strip_paren(&name))
    }

    /// A carbon chain group starting at `b`: methyl, (2-chloroethyl), (prop-2-en-1-yl), acetyl…
    fn alkyl_name(&self, a: usize, b: usize, blocked: &HashSet<usize>) -> Option<String> {
        let m = self.m;
        let usable = |v: usize| v != a && m.is_c(v) && m.sys_of[v].is_none() && !blocked.contains(&v) && !self.groups.iter().any(|g| g.c == v && matches!(g.kind, Fg::Nitrile | Fg::Acid));
        // Longest chain from b through carbons outside rings.
        let mut best: Vec<usize> = vec![b];
        let mut stack = vec![vec![b]];
        let mut steps = 0;
        while let Some(path) = stack.pop() {
            steps += 1;
            if steps > 20000 {
                return None;
            }
            let u = *path.last()?;
            if path.len() > best.len() {
                best = path.clone();
            }
            for &(v, _) in &m.adj[u] {
                if usable(v) && !path.contains(&v) {
                    let mut np = path.clone();
                    np.push(v);
                    stack.push(np);
                }
            }
        }
        let chain = best;
        let n = chain.len();
        if n > 20 {
            return None;
        }
        let labels: Vec<String> = (1..=n).map(|k| k.to_string()).collect();
        let set: HashSet<usize> = chain.iter().copied().collect();
        let mut bl = blocked.clone();
        bl.extend(chain.iter().copied());
        bl.insert(a);
        let mut subs: Vec<(String, String)> = Vec::new();
        for (k, &c) in chain.iter().enumerate() {
            for &(x, _) in &m.adj[c] {
                if x == a || set.contains(&x) {
                    continue;
                }
                subs.push(((k + 1).to_string(), self.sub_name(c, x, &bl)?));
            }
        }
        let mut doubles = Vec::new();
        let mut triples = Vec::new();
        for k in 0..n.saturating_sub(1) {
            match m.order(chain[k], chain[k + 1]) {
                2 => doubles.push(k + 1),
                3 => triples.push(k + 1),
                _ => {}
            }
        }
        let fin = |name: String| Some(self.with_stereo(name, &chain, &labels, true));
        let unsat_text = |doubles: &[usize], triples: &[usize]| {
            let mut u = String::new();
            if !doubles.is_empty() {
                if doubles.len() > 1 {
                    u.push('a');
                }
                u.push_str(&format!("-{}-{}en", join(doubles), MULT[doubles.len()]));
            }
            if !triples.is_empty() {
                if doubles.is_empty() && triples.len() > 1 {
                    u.push('a');
                }
                u.push_str(&format!("-{}-{}yn", join(triples), MULT[triples.len()]));
            }
            u
        };
        // Acyl groups: an oxo on the attached carbon.
        if let Some(i) = subs.iter().position(|s| s.0 == "1" && s.1 == "oxo") {
            let mut rest = subs.clone();
            rest.remove(i);
            if !doubles.is_empty() || !triples.is_empty() {
                let base = format!("{}{}oyl", STEMS[n], unsat_text(&doubles, &triples));
                return fin(if rest.is_empty() { format!("({})", base) } else { format!("({}{})", assemble_list(&rest, false), base) });
            }
            if n == 1 && rest.len() == 1 {
                // A ring on the C=O carbon: benzoyl, (4-methylbenzoyl), (piperidine-1-carbonyl).
                let r = &rest[0].1;
                if r == "phenyl" {
                    return Some("benzoyl".into());
                }
                if let Some(head) = r.strip_prefix('(').and_then(|x| x.strip_suffix("phenyl)")) {
                    return Some(format!("({}benzoyl)", head));
                }
                return Some(format!("({}carbonyl)", strip_paren(r)));
            }
            let base = match n {
                1 => "formyl".to_string(),
                2 => "acetyl".to_string(),
                _ => format!("{}anoyl", STEMS[n]),
            };
            if rest.is_empty() {
                return fin(base);
            }
            return fin(format!("({}{})", assemble_list(&rest, n == 1), base));
        }
        // Common short names.
        let chiral = chain.iter().any(|c| self.desc.centres.contains_key(c));
        if doubles.is_empty() && triples.is_empty() && !chiral {
            let names: Vec<&str> = subs.iter().map(|s| s.1.as_str()).collect();
            let locs: Vec<&str> = subs.iter().map(|s| s.0.as_str()).collect();
            if n == 1 && names == ["phenyl"] {
                return Some("benzyl".into());
            }
            if n == 2 && names == ["methyl"] && locs == ["1"] {
                return Some("isopropyl".into());
            }
            if n == 2 && names == ["methyl", "methyl"] && locs == ["1", "1"] {
                return Some("tert-butyl".into());
            }
            if n == 3 && names == ["methyl"] && locs == ["2"] {
                return Some("isobutyl".into());
            }
            if n == 3 && names == ["methyl"] && locs == ["1"] {
                return Some("sec-butyl".into());
            }
        }
        let mut base = STEMS[n].to_string();
        if doubles.is_empty() && triples.is_empty() {
            base.push_str("yl");
        } else if n == 2 {
            base.push_str(if doubles.is_empty() { "ynyl" } else { "enyl" });
        } else {
            base = format!("{}{}-1-yl", base, unsat_text(&doubles, &triples));
        }
        if subs.is_empty() {
            return fin(if base.contains('-') { format!("({})", base) } else { base });
        }
        // A plain ring group on one carbon: "(furan-2-ylmethyl)", "(pyridin-3-ylmethyl)".
        if n == 1 && subs.len() == 1 && base == "methyl" {
            let r = &subs[0].1;
            let inner = strip_paren(r);
            if r.starts_with('(') && inner.ends_with("-yl") && !inner.contains('(') && inner.matches('-').count() == 2 && !inner.starts_with(|c: char| c.is_ascii_digit()) {
                return fin(format!("({}methyl)", inner));
            }
        }
        // On a one-carbon group the locant is left out: "(carboxymethyl)", "(methoxymethyl)".
        fin(format!("({}{})", assemble_list(&subs, n == 1), base))
    }

    /// A carbon ring group: phenyl, (4-methylphenyl), cyclohexyl, (2-hydroxycyclopentyl).
    fn ring_sub_name(&self, si: usize, b: usize, a: usize, blocked: &HashSet<usize>) -> Option<String> {
        let m = self.m;
        let SysKind::Carbo { order: ring, aromatic } = &m.systems[si].kind else { return None };
        let n = ring.len();
        let start = ring.iter().position(|&x| x == b)?;
        let set: HashSet<usize> = ring.iter().copied().collect();
        let mut bl = blocked.clone();
        bl.extend(ring.iter().copied());
        bl.insert(a);
        let mut best: Option<(Vec<usize>, Vec<(String, String)>, Vec<usize>, Vec<usize>)> = None;
        for dir in [1usize, n - 1] {
            let order: Vec<usize> = (0..n).map(|k| ring[(start + k * dir) % n]).collect();
            let mut subs = Vec::new();
            for (k, &c) in order.iter().enumerate() {
                for &(x, _) in &m.adj[c] {
                    if set.contains(&x) || x == a || blocked.contains(&x) {
                        continue;
                    }
                    subs.push(((k + 1).to_string(), self.sub_name(c, x, &bl)?));
                }
            }
            let mut doubles = Vec::new();
            if !aromatic {
                for k in 0..n {
                    if m.order(order[k], order[(k + 1) % n]) == 2 {
                        doubles.push(k + 1);
                    }
                }
            }
            let mut locs: Vec<usize> = doubles.clone();
            let mut sl: Vec<usize> = subs.iter().filter_map(|s| s.0.parse().ok()).collect();
            sl.sort();
            locs.extend(sl);
            if best.as_ref().is_none_or(|bst| locs < bst.0) {
                best = Some((locs, subs, doubles, order));
            }
        }
        let (_, subs, doubles, order) = best?;
        let base = if *aromatic {
            "phenyl".to_string()
        } else if doubles.is_empty() {
            format!("cyclo{}yl", STEMS.get(n)?)
        } else {
            format!("cyclo{}{}-{}-{}en-1-yl", STEMS.get(n)?, if doubles.len() > 1 { "a" } else { "" }, join(&doubles), MULT[doubles.len()])
        };
        let labels: Vec<String> = (1..=n).map(|k| k.to_string()).collect();
        let name = if subs.is_empty() {
            if base.contains('-') { format!("({})", base) } else { base }
        } else {
            format!("({}{})", assemble_list(&subs, false), base)
        };
        Some(self.with_stereo(name, &order, &labels, false))
    }

    /// A named ring system as a group: (furan-2-yl), (6-aminobenzo[d]thiazol-2-yl),
    /// (4,5-dihydrothiazol-2-yl), (piperazin-1-yl).
    fn tpl_sub_name(&self, si: usize, b: usize, a: usize, blocked: &HashSet<usize>) -> Option<String> {
        let m = self.m;
        let SysKind::Tpl { tpl, maps } = &m.systems[si].kind else { return None };
        let t = &hetero::TEMPLATES[*tpl];
        let set: HashSet<usize> = m.systems[si].atoms.iter().copied().collect();
        let mut bl = blocked.clone();
        bl.extend(set.iter().copied());
        bl.insert(a);
        let mut best: Option<(Key, String)> = None;
        for mp in maps {
            let labels: Vec<String> = t.atoms.iter().map(|x| x.1.to_string()).collect();
            let at = mp.iter().position(|&x| x == b)?;
            let mut subs = Vec::new();
            for (k, &c) in mp.iter().enumerate() {
                for &(x, _) in &m.adj[c] {
                    if set.contains(&x) || x == a || blocked.contains(&x) {
                        continue;
                    }
                    subs.push((labels[k].clone(), self.sub_name(c, x, &bl)?));
                }
            }
            let Some((indicated, added, hydro)) = self.ring_hydrogen(t, mp, &[]) else { continue };
            let mut hy: Vec<usize> = hydro.clone();
            hy.extend(added);
            let nb = Numbered { atoms: mp.clone(), labels: labels.clone(), sys: Some(si), principal: vec![(at, 0)], doubles: vec![], triples: vec![], prefixes: subs.clone(), indicated: indicated.clone(), added: None, hydro: hy.clone() };
            // Lowest locant for the attachment first, then indicated hydrogen, hydro, prefixes.
            let k = nb.key();
            let key = (k.1, k.0, k.2, k.3, k.4, k.5);
            let all_sat: Vec<usize> = indicated.iter().chain(hy.iter()).copied().collect();
            let base = match saturated_name(t, &all_sat, &labels) {
                Some(nm) => format!("{}-{}-yl", hetero::stem(nm), labels[at]),
                None => cat(&cat(&hydro_text(&hy, &labels), &indicated_text(t, &indicated, &labels)), &format!("{}-{}-yl", hetero::stem(t.name), labels[at])),
            };
            let name = if subs.is_empty() { format!("({})", base) } else { format!("({})", cat(&assemble_list(&subs, false), &base)) };
            let name = self.with_stereo(name, mp, &labels, false);
            if best.as_ref().is_none_or(|b| key < b.0) {
                best = Some((key, name));
            }
        }
        best.map(|b| b.1)
    }

    /// The parent's name with its suffix, e.g. "butan-2-ol", "4,5-dihydrothiazole-4-carboxylic
    /// acid", "imidazo[1,2-a]pyrazin-3(7H)-one".
    fn parent_name(&self, nb: &Numbered) -> Option<ParentText> {
        let m = self.m;
        let n = nb.atoms.len();
        let kind = nb.principal.first().map(|&(_, gi)| self.groups[gi].kind);
        let count = nb.principal.len();
        let mut pidx: Vec<usize> = nb.principal.iter().map(|x| x.0).collect();
        pidx.sort_by_key(|&p| nb.lk(p));
        let plocs = pidx.iter().map(|&p| nb.labels[p].clone()).collect::<Vec<_>>().join(",");
        // Prefixes on carbon atoms (N-substituents don't count for leaving out locants).
        let any_prefix = nb.prefixes.iter().any(|p| p.0 != "N");
        let ring_word = |k: Fg| match k {
            Fg::Alcohol => Some("ol"),
            Fg::Amine => Some("amine"),
            Fg::Acid => Some("carboxylic acid"),
            Fg::Aldehyde => Some("carbaldehyde"),
            Fg::Nitrile => Some("carbonitrile"),
            Fg::Amide => Some("carboxamide"),
            Fg::Thiol => Some("thiol"),
            Fg::Ester => Some("carboxylate"),
            Fg::Ketone => Some("one"),
        };
        // Named ring systems.
        if let Some(t) = nb.sys.and_then(|s| m.template(s)) {
            // Retained names for a fully saturated ring fused to benzene: "chroman-3-one".
            let ketones: Vec<usize> = nb.principal.iter().filter(|x| self.groups[x.1].kind == Fg::Ketone).map(|x| x.0).collect();
            let saturated: Vec<usize> = nb.indicated.iter().chain(nb.hydro.iter()).chain(nb.added.iter()).chain(ketones.iter()).copied().collect();
            if let Some(nm) = saturated_name(t, &saturated, &nb.labels) {
                let Some(kind) = kind else {
                    return Some(ParentText { text: nm.to_string(), omit_prefix_locants: false });
                };
                let w = ring_word(kind)?;
                let mult = MULT[count.min(10)];
                let first = mult.chars().next().or_else(|| w.chars().next()).unwrap_or('a');
                let base = if "aeiouy".contains(first) { hetero::stem(nm) } else { nm };
                return Some(ParentText { text: format!("{}-{}-{}{}", base, plocs, mult, w), omit_prefix_locants: false });
            }
            let mut text = hydro_text(&nb.hydro, &nb.labels);
            if let Some(fp) = t.fixed_prefix {
                text = cat(&text, fp);
            }
            text = cat(&text, &indicated_text(t, &nb.indicated, &nb.labels));
            let Some(kind) = kind else {
                text.push_str(t.name);
                return Some(ParentText { text, omit_prefix_locants: false });
            };
            let w = ring_word(kind)?;
            let mult = MULT[count.min(10)];
            let first = mult.chars().next().or_else(|| w.chars().next()).unwrap_or('a');
            text.push_str(if "aeiouy".contains(first) { hetero::stem(t.name) } else { t.name });
            let added = nb.added.map(|p| format!("({}H)", nb.labels[p])).unwrap_or_default();
            text.push_str(&format!("-{}{}-{}{}", plocs, added, mult, w));
            return Some(ParentText { text, omit_prefix_locants: false });
        }
        // Benzene.
        if nb.sys.is_some_and(|s| m.benzene(s)) {
            let lone = count + nb.prefixes.len() <= 1;
            let text = match (kind, count) {
                (None, _) => "benzene".to_string(),
                (Some(k), 1) => match k {
                    Fg::Alcohol => "phenol".into(),
                    Fg::Amine => "aniline".into(),
                    Fg::Acid => "benzoic acid".into(),
                    Fg::Aldehyde => "benzaldehyde".into(),
                    Fg::Nitrile => "benzonitrile".into(),
                    Fg::Amide => "benzamide".into(),
                    Fg::Thiol => "benzenethiol".into(),
                    Fg::Ester => "benzoate".into(),
                    Fg::Ketone => return None,
                },
                (Some(k), c) => {
                    if matches!(k, Fg::Ester | Fg::Ketone) {
                        return None;
                    }
                    format!("benzene-{}-{}{}", plocs, MULT[c], ring_word(k)?)
                }
            };
            return Some(ParentText { text, omit_prefix_locants: lone && kind.is_none() });
        }
        let stem = STEMS.get(n).filter(|s| !s.is_empty())?;
        let ring = nb.sys.is_some();
        let lab = |v: &[usize]| {
            let mut v: Vec<usize> = v.to_vec();
            v.sort_by_key(|&p| nb.lk(p));
            v.iter().map(|&p| nb.labels[p].clone()).collect::<Vec<_>>().join(",")
        };
        let d = &nb.doubles;
        let t = &nb.triples;
        // Locants of multiple bonds are left out for ethene/ethyne and a ring's lone double bond.
        let show_unsat_locs = !(n == 2 || (ring && d.len() + t.len() == 1 && count == 0 && !any_prefix));
        let mut body = format!("{}{}", if ring { "cyclo" } else { "" }, stem);
        if d.is_empty() && t.is_empty() {
            body.push_str("an");
        } else {
            if !d.is_empty() {
                if d.len() > 1 {
                    body.push('a');
                }
                if show_unsat_locs {
                    body.push_str(&format!("-{}-{}en", lab(d), MULT[d.len()]));
                } else {
                    body.push_str(&format!("{}en", MULT[d.len()]));
                }
            }
            if !t.is_empty() {
                if d.is_empty() && t.len() > 1 {
                    body.push('a');
                }
                if show_unsat_locs {
                    body.push_str(&format!("-{}-{}yn", lab(t), MULT[t.len()]));
                } else {
                    body.push_str(&format!("{}yn", MULT[t.len()]));
                }
            }
        }
        let Some(kind) = kind else {
            let lone_ring_sub = ring && nb.prefixes.len() == 1 && d.is_empty() && t.is_empty();
            return Some(ParentText { text: format!("{}e", body), omit_prefix_locants: n == 1 || lone_ring_sub });
        };
        // The suffix word; for a ring, groups whose carbon is outside it.
        let exo = ring && matches!(kind, Fg::Acid | Fg::Aldehyde | Fg::Nitrile | Fg::Amide | Fg::Ester);
        let word = match (kind, exo) {
            (Fg::Acid, false) => "oic acid",
            (Fg::Aldehyde, false) => "al",
            (Fg::Nitrile, false) => "nitrile",
            (Fg::Amide, false) => "amide",
            (Fg::Ester, false) => "oate",
            (k, _) => ring_word(k)?,
        };
        // Retained names: formic and acetic acid and their relatives.
        if !ring && d.is_empty() && t.is_empty() && count == 1 {
            let retained = match (kind, n) {
                (Fg::Acid, 1) => Some(("formic acid", true)),
                (Fg::Acid, 2) => Some(("acetic acid", false)),
                (Fg::Ester, 1) => Some(("formate", true)),
                (Fg::Ester, 2) => Some(("acetate", false)),
                (Fg::Amide, 1) => Some(("formamide", true)),
                (Fg::Amide, 2) => Some(("acetamide", false)),
                (Fg::Aldehyde, 1) => Some(("formaldehyde", true)),
                (Fg::Aldehyde, 2) => Some(("acetaldehyde", false)),
                (Fg::Nitrile, 2) => Some(("acetonitrile", false)),
                _ => None,
            };
            if let Some((text, omit)) = retained {
                return Some(ParentText { text: text.into(), omit_prefix_locants: omit });
            }
        }
        let terminal = !ring && matches!(kind, Fg::Acid | Fg::Aldehyde | Fg::Nitrile | Fg::Amide | Fg::Ester);
        let mult = MULT[count.min(10)];
        let first = mult.chars().next().or_else(|| word.chars().next()).unwrap_or('a');
        let keep_e = !"aeiouy".contains(first);
        // The suffix locant is left out on one-carbon parents and on a bare two-carbon chain
        // ("ethanol"), but kept when there are prefixes ("2-chloroethan-1-ol").
        let show_locs = !(terminal || n == 1 || (n == 2 && count == 1 && !any_prefix) || (ring && count == 1 && d.is_empty() && t.is_empty() && !any_prefix));
        let mut text = body;
        if keep_e {
            text.push('e');
        }
        if show_locs {
            text.push_str(&format!("-{}-", plocs));
        }
        text.push_str(mult);
        text.push_str(word);
        Some(ParentText { text, omit_prefix_locants: n == 1 })
    }
}

struct ParentText {
    text: String,
    omit_prefix_locants: bool,
}

/// Retained names for a ring fused to benzene whose own ring is fully saturated:
/// (template, name, the positions that are then saturated).
const SATURATED_NAMES: &[(&str, &str, &[&str])] = &[
    ("chromene", "chromane", &["2", "3", "4"]),
    ("isochromene", "isochromane", &["1", "3", "4"]),
    ("thiochromene", "thiochromane", &["2", "3", "4"]),
    ("indole", "indoline", &["1", "2", "3"]),
];

/// "chromane" when exactly the ring's positions 2, 3 and 4 are saturated, and so on.
fn saturated_name(t: &Template, saturated: &[usize], labels: &[String]) -> Option<&'static str> {
    let (_, name, need) = SATURATED_NAMES.iter().find(|x| x.0 == t.name)?;
    let mut have: Vec<&str> = saturated.iter().map(|&p| labels[p].as_str()).collect();
    have.sort();
    have.dedup();
    let mut want: Vec<&str> = need.to_vec();
    want.sort();
    (have == want).then_some(*name)
}

/// Join a prefix part and what follows, with a hyphen before a locant
/// ("6-(hydroxymethyl)" + "3,4-dihydro…" → "6-(hydroxymethyl)-3,4-dihydro…").
fn cat(a: &str, b: &str) -> String {
    if !a.is_empty() && !a.ends_with('-') && b.starts_with(|c: char| c.is_ascii_digit()) {
        format!("{}-{}", a, b)
    } else {
        format!("{}{}", a, b)
    }
}

/// "4,5-dihydro" for the hydro positions of a ring system.
fn hydro_text(hydro: &[usize], labels: &[String]) -> String {
    if hydro.is_empty() {
        return String::new();
    }
    let mut v: Vec<&String> = hydro.iter().map(|&p| &labels[p]).collect();
    v.sort_by_key(|l| locant_key(l));
    format!("{}-{}hydro", v.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(","), MULT[hydro.len().min(10)])
}

/// "2H-" for the indicated hydrogen of a ring system.
fn indicated_text(_t: &Template, ind: &[usize], labels: &[String]) -> String {
    if ind.is_empty() {
        return String::new();
    }
    format!("{}-", ind.iter().map(|&p| format!("{}H", labels[p])).collect::<Vec<_>>().join(","))
}

/// Groups on one atom: "dimethylamino", "ethyl(methyl)amino", "diphenylphosphoryl".
fn group_on(names: &[String], word: &str) -> String {
    let mut names = names.to_vec();
    names.sort_by_key(|x| alpha_key(x));
    let all_same = names.iter().all(|x| *x == names[0]);
    let body = if all_same {
        let n0 = &names[0];
        if names.len() == 1 {
            n0.clone()
        } else if is_complex(n0) {
            format!("{}({})", MULT_COMPLEX[names.len().min(6)], strip_paren(n0))
        } else {
            format!("{}{}", MULT[names.len()], n0)
        }
    } else {
        // "ethyl(methyl)", "(2-chloroethyl)(methyl)": a complex first group keeps its
        // parentheses so the name reads back unambiguously.
        let mut s = names[0].clone();
        for x in &names[1..] {
            s.push_str(&format!("({})", strip_paren(x)));
        }
        s
    };
    if is_complex(&body) || !all_same {
        format!("({}{})", body, word)
    } else {
        format!("{}{}", body, word)
    }
}

/// "methoxy" for methyl, "pentyloxy" for pentyl, "(2-chloroethoxy)" for (2-chloroethyl).
fn oxy(r: &str) -> String {
    let short = [
        ("methyl", "methoxy"),
        ("ethyl", "ethoxy"),
        ("propyl", "propoxy"),
        ("butyl", "butoxy"),
        ("phenyl", "phenoxy"),
        ("isopropyl", "isopropoxy"),
        ("tert-butyl", "tert-butoxy"),
        ("benzyl", "benzyloxy"),
    ];
    if let Some((_, ox)) = short.iter().find(|(yl, _)| *yl == r) {
        return ox.to_string();
    }
    if let Some(inner) = r.strip_prefix('(').and_then(|x| x.strip_suffix(')')) {
        for (yl, ox) in &short[..4] {
            if let Some(head) = inner.strip_suffix(yl) {
                if !STEMS.iter().any(|s| !s.is_empty() && head.ends_with(s)) && !head.ends_with(')') {
                    return format!("({}{})", head, ox);
                }
            }
        }
        return format!("({}oxy)", inner);
    }
    format!("{}oxy", r)
}

fn join(v: &[usize]) -> String {
    v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")
}

/// A prefix made of a group on a group, written in parentheses ("(dimethylamino)",
/// "(methylsulfanyl)", "(pentyloxy)", "(diphenylphosphoryl)"); simple ones like "amino",
/// "methoxy" or "acetamido" are not.
fn is_compound_prefix(name: &str) -> bool {
    let simple = ["amino", "acetamido", "sulfanyl", "carbamoyl", "methoxy", "ethoxy", "propoxy", "butoxy", "phenoxy", "isopropoxy", "tert-butoxy", "benzyloxy", "phosphoryl", "phosphanyl", "sulfamoyl", "hydrazinyl", "diazenyl"];
    if simple.contains(&name) {
        return false;
    }
    ["amino", "sulfanyl", "sulfinyl", "sulfonyl", "carbamoyl", "yloxy", "oyloxy", "phosphoryl", "phosphanyl", "diazenyl"].iter().any(|e| name.ends_with(e))
}

/// A group name that needs parentheses when used inside another name ("2-chloroethyl",
/// "bis(carboxymethyl)amino").
fn is_complex(name: &str) -> bool {
    name.starts_with('(') || name.contains(|c: char| c.is_ascii_digit() || c == '(')
}

fn strip_paren(name: &str) -> &str {
    if name.starts_with('(') && name.ends_with(')') {
        // Only when the outer pair encloses the whole name.
        let mut level = 0;
        for (i, ch) in name.char_indices() {
            match ch {
                '(' => level += 1,
                ')' => {
                    level -= 1;
                    if level == 0 && i + 1 < name.len() {
                        return name;
                    }
                }
                _ => {}
            }
        }
        &name[1..name.len() - 1]
    } else {
        name
    }
}

fn paren_if_complex(name: &str) -> String {
    if is_complex(name) && !name.starts_with('(') {
        format!("({})", name)
    } else {
        name.to_string()
    }
}

/// Sort key for alphabetical order: ignores parentheses, locants, stereodescriptors,
/// "tert-"/"sec-" and hydro/indicated hydrogen.
fn alpha_key(name: &str) -> String {
    let mut s = name;
    loop {
        let t = s.trim_start_matches(|c: char| c == '(' || c == ')' || c.is_ascii_digit() || c == ',' || c == '-');
        // Stereodescriptors such as "(S)-", "(2R,3E)-".
        let t = t.trim_start_matches(|c: char| matches!(c, 'R' | 'S' | 'E' | 'Z'));
        let t = t.strip_prefix("tert-").or_else(|| t.strip_prefix("sec-")).unwrap_or(t);
        // Indicated hydrogen ("1H-indol-3-yl" sorts under "i").
        let t = t.strip_prefix("H-").unwrap_or(t);
        if t == s {
            break;
        }
        s = t;
    }
    s.to_string()
}

/// Prefixes in alphabetical order, with multipliers and locants: "2-chloro-3,3-dimethyl".
fn assemble_list(prefixes: &[(String, String)], omit_locants: bool) -> String {
    if prefixes.is_empty() {
        return String::new();
    }
    let mut groups: Vec<(String, Vec<String>)> = Vec::new();
    for (l, n) in prefixes {
        match groups.iter_mut().find(|g| g.0 == *n) {
            Some(g) => g.1.push(l.clone()),
            None => groups.push((n.clone(), vec![l.clone()])),
        }
    }
    groups.sort_by(|a, b| alpha_key(&a.0).cmp(&alpha_key(&b.0)));
    let parts: Vec<String> = groups
        .into_iter()
        .map(|(name, mut locs)| {
            // Compound prefixes go in parentheses: "(diethylamino)", "(methylsulfanyl)".
            let enclosed = name.starts_with('(') && strip_paren(&name) != name;
            let name = if !enclosed && (is_compound_prefix(&name) || (is_complex(&name) && omit_locants)) { format!("({})", name) } else { name };
            locs.sort_by(|a, b| match (a == "N", b == "N") {
                (false, false) => locant_key(a).cmp(&locant_key(b)),
                (false, true) => std::cmp::Ordering::Greater,
                (true, false) => std::cmp::Ordering::Less,
                _ => std::cmp::Ordering::Equal,
            });
            let k = locs.len();
            let multiplied = if k == 1 {
                name.clone()
            } else if is_complex(&name) {
                format!("{}({})", MULT_COMPLEX.get(k).copied().unwrap_or(""), strip_paren(&name))
            } else {
                format!("{}{}", MULT.get(k).copied().unwrap_or(""), name)
            };
            if omit_locants && locs.iter().all(|l| l != "N") {
                multiplied
            } else {
                format!("{}-{}", locs.join(","), multiplied)
            }
        })
        .collect();
    // Without locants the prefixes run together ("trifluorosulfomethane").
    parts.join(if omit_locants { "" } else { "-" })
}
