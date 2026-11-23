//! Chemical names, offline: a built-in list of common compounds plus systematic
//! names for alkanes and cycloalkanes (with alkyl branches).

use crate::chem;
use crate::doc::Doc;
use crate::formats;
use std::collections::HashSet;

pub const COMMON: &[(&str, &str)] = &[
    ("water", "O"),
    ("ammonia", "N"),
    ("hydrogen peroxide", "OO"),
    ("hydrogen sulfide", "S"),
    ("carbon dioxide", "O=C=O"),
    ("carbon monoxide", "[C-]#[O+]"),
    ("hydrochloric acid", "Cl"),
    ("hydrogen chloride", "Cl"),
    ("methanol", "CO"),
    ("ethanol", "CCO"),
    ("propan-1-ol", "CCCO"),
    ("1-propanol", "CCCO"),
    ("propan-2-ol", "CC(C)O"),
    ("isopropanol", "CC(C)O"),
    ("butan-1-ol", "CCCCO"),
    ("tert-butanol", "CC(C)(C)O"),
    ("ethylene glycol", "OCCO"),
    ("glycerol", "OCC(O)CO"),
    ("formaldehyde", "C=O"),
    ("acetaldehyde", "CC=O"),
    ("acetone", "CC(C)=O"),
    ("butanone", "CCC(C)=O"),
    ("formic acid", "OC=O"),
    ("acetic acid", "CC(O)=O"),
    ("propionic acid", "CCC(O)=O"),
    ("butyric acid", "CCCC(O)=O"),
    ("oxalic acid", "OC(=O)C(O)=O"),
    ("lactic acid", "CC(O)C(O)=O"),
    ("citric acid", "OC(=O)CC(O)(CC(O)=O)C(O)=O"),
    ("acrylic acid", "C=CC(O)=O"),
    ("methyl acetate", "COC(C)=O"),
    ("ethyl acetate", "CCOC(C)=O"),
    ("acetic anhydride", "CC(=O)OC(C)=O"),
    ("acetyl chloride", "CC(Cl)=O"),
    ("acetamide", "CC(N)=O"),
    ("urea", "NC(N)=O"),
    ("dimethyl sulfoxide", "CS(C)=O"),
    ("dmso", "CS(C)=O"),
    ("dimethylformamide", "CN(C)C=O"),
    ("dmf", "CN(C)C=O"),
    ("acetonitrile", "CC#N"),
    ("diethyl ether", "CCOCC"),
    ("tetrahydrofuran", "C1CCOC1"),
    ("thf", "C1CCOC1"),
    ("1,4-dioxane", "C1COCCO1"),
    ("dichloromethane", "ClCCl"),
    ("chloroform", "ClC(Cl)Cl"),
    ("carbon tetrachloride", "ClC(Cl)(Cl)Cl"),
    ("chloromethane", "CCl"),
    ("methylamine", "CN"),
    ("ethylamine", "CCN"),
    ("dimethylamine", "CNC"),
    ("trimethylamine", "CN(C)C"),
    ("triethylamine", "CCN(CC)CC"),
    ("ethene", "C=C"),
    ("ethylene", "C=C"),
    ("propene", "CC=C"),
    ("ethyne", "C#C"),
    ("acetylene", "C#C"),
    ("buta-1,3-diene", "C=CC=C"),
    ("isoprene", "CC(=C)C=C"),
    ("benzene", "c1ccccc1"),
    ("toluene", "Cc1ccccc1"),
    ("xylene", "Cc1ccccc1C"),
    ("styrene", "C=Cc1ccccc1"),
    ("phenol", "Oc1ccccc1"),
    ("aniline", "Nc1ccccc1"),
    ("anisole", "COc1ccccc1"),
    ("benzaldehyde", "O=Cc1ccccc1"),
    ("benzoic acid", "OC(=O)c1ccccc1"),
    ("acetophenone", "CC(=O)c1ccccc1"),
    ("benzyl alcohol", "OCc1ccccc1"),
    ("nitrobenzene", "[O-][N+](=O)c1ccccc1"),
    ("chlorobenzene", "Clc1ccccc1"),
    ("bromobenzene", "Brc1ccccc1"),
    ("benzonitrile", "N#Cc1ccccc1"),
    ("naphthalene", "c1ccc2ccccc2c1"),
    ("anthracene", "c1ccc2cc3ccccc3cc2c1"),
    ("phenanthrene", "c1ccc2c(c1)ccc1ccccc12"),
    ("biphenyl", "c1ccc(cc1)-c1ccccc1"),
    ("pyridine", "c1ccncc1"),
    ("pyrimidine", "c1cncnc1"),
    ("pyrazine", "c1cnccn1"),
    ("pyrrole", "c1cc[nH]c1"),
    ("furan", "c1ccoc1"),
    ("thiophene", "c1ccsc1"),
    ("imidazole", "c1c[nH]cn1"),
    ("pyrazole", "c1cc[nH]n1"),
    ("oxazole", "c1cocn1"),
    ("thiazole", "c1cscn1"),
    ("indole", "c1ccc2[nH]ccc2c1"),
    ("quinoline", "c1ccc2ncccc2c1"),
    ("isoquinoline", "c1ccc2cnccc2c1"),
    ("purine", "c1ncc2[nH]cnc2n1"),
    ("piperidine", "C1CCNCC1"),
    ("piperazine", "C1CNCCN1"),
    ("morpholine", "C1COCCN1"),
    ("pyrrolidine", "C1CCNC1"),
    ("cyclohexanone", "O=C1CCCCC1"),
    ("cyclohexanol", "OC1CCCCC1"),
    ("cyclohexene", "C1CCC=CC1"),
    ("glycine", "NCC(O)=O"),
    ("alanine", "CC(N)C(O)=O"),
    ("valine", "CC(C)C(N)C(O)=O"),
    ("leucine", "CC(C)CC(N)C(O)=O"),
    ("serine", "NC(CO)C(O)=O"),
    ("cysteine", "NC(CS)C(O)=O"),
    ("phenylalanine", "NC(Cc1ccccc1)C(O)=O"),
    ("proline", "OC(=O)C1CCCN1"),
    ("glucose", "OCC1OC(O)C(O)C(O)C1O"),
    ("aspirin", "CC(=O)Oc1ccccc1C(O)=O"),
    ("acetylsalicylic acid", "CC(=O)Oc1ccccc1C(O)=O"),
    ("salicylic acid", "OC(=O)c1ccccc1O"),
    ("paracetamol", "CC(=O)Nc1ccc(O)cc1"),
    ("acetaminophen", "CC(=O)Nc1ccc(O)cc1"),
    ("ibuprofen", "CC(C)Cc1ccc(cc1)C(C)C(O)=O"),
    ("caffeine", "Cn1cnc2c1c(=O)n(C)c(=O)n2C"),
    ("nicotine", "CN1CCCC1c1cccnc1"),
    ("vanillin", "COc1cc(C=O)ccc1O"),
    ("menthol", "CC(C)C1CCC(C)CC1O"),
    ("cholesterol", "CC(C)CCCC(C)C1CCC2C3CC=C4CC(O)CCC4(C)C3CCC12C"),
    ("dopamine", "NCCc1ccc(O)c(O)c1"),
    ("serotonin", "NCCc1c[nH]c2ccc(O)cc12"),
    ("adrenaline", "CNCC(O)c1ccc(O)c(O)c1"),
    ("benzophenone", "O=C(c1ccccc1)c1ccccc1"),
    ("triphenylphosphine", "c1ccc(cc1)P(c1ccccc1)c1ccccc1"),
];

const ALK: [&str; 21] = [
    "", "meth", "eth", "prop", "but", "pent", "hex", "hept", "oct", "non", "dec", "undec", "dodec", "tridec", "tetradec", "pentadec", "hexadec",
    "heptadec", "octadec", "nonadec", "icos",
];

const MULT: [&str; 11] = ["", "", "di", "tri", "tetra", "penta", "hexa", "hepta", "octa", "nona", "deca"];

/// Every built-in name: the common list and the longer list of drugs, natural products,
/// vitamins and reagents.
pub fn all_names() -> impl Iterator<Item = &'static (&'static str, &'static str)> {
    COMMON.iter().chain(crate::compounds::MORE.iter())
}

/// Make a structure from a name: the built-in list first, then a systematic (IUPAC) name,
/// then simple alkanes and SMILES. Returns None when the name can't be read.
pub fn name_to_structure(name: &str) -> Option<Doc> {
    let n = name.trim().to_lowercase();
    let plain = n.replace(['\u{2010}', '\u{2011}', '\u{2013}'], "-");
    let lookup = |k: &str| all_names().find(|(name, _)| *name == k).map(|x| x.1);
    // Also without leading stereo or optical-rotation marks ("(-)-menthol", "l-tyrosine").
    let bare = plain
        .trim_start_matches("(+)-")
        .trim_start_matches("(-)-")
        .trim_start_matches("(±)-")
        .trim_start_matches("dl-")
        .trim_start_matches("l-")
        .trim_start_matches("d-");
    if let Some(smi) = lookup(&plain).or_else(|| lookup(bare)) {
        return formats::parse_smiles(smi).ok();
    }
    if let Some(d) = crate::iupac::name_to_structure(&plain) {
        return Some(d);
    }
    // n-alkanes and cycloalkanes.
    let (cyclo, base) = match n.strip_prefix("cyclo") {
        Some(rest) => (true, rest),
        None => (false, n.strip_prefix("n-").unwrap_or(&n)),
    };
    if let Some(stem) = base.strip_suffix("ane") {
        if let Some(k) = ALK.iter().position(|a| *a == stem && !a.is_empty()) {
            if cyclo && k < 3 {
                return None;
            }
            let mut smi = String::from("C");
            if cyclo {
                smi = "C1".into();
                for _ in 1..k - 1 {
                    smi.push('C');
                }
                smi.push_str("C1");
            } else {
                for _ in 1..k {
                    smi.push('C');
                }
            }
            return formats::parse_smiles(&smi).ok();
        }
    }
    // Plain SMILES typed into the name box.
    formats::parse_smiles(name.trim()).ok()
}

/// Graph match: same atoms, charges and bond orders (aromatic bonds compared as aromatic).
fn same_structure(a: &Doc, b: &Doc) -> bool {
    if a.atoms.len() != b.atoms.len() || a.bonds.len() != b.bonds.len() {
        return false;
    }
    let fa = chem::atoms_formula(a, &(0..a.atoms.len()).collect::<Vec<_>>());
    let fb = chem::atoms_formula(b, &(0..b.atoms.len()).collect::<Vec<_>>());
    if fa != fb {
        return false;
    }
    let ar_a = chem::aromatic_bonds(a);
    let ar_b = chem::aromatic_bonds(b);
    let order = |d: &Doc, ar: &HashSet<usize>, bi: usize| if ar.contains(&bi) { 4 } else { d.bonds[bi].order };
    let adj_a = a.adjacency();
    let adj_b = b.adjacency();
    let n = a.atoms.len();
    let key = |d: &Doc, adj: &Vec<Vec<(usize, usize)>>, i: usize| (d.atoms[i].el, d.atoms[i].charge, adj[i].len(), d.atoms[i].label.clone());
    let mut map = vec![usize::MAX; n];
    let mut used = vec![false; n];
    #[allow(clippy::too_many_arguments)]
    fn go(
        k: usize,
        order_a: &[usize],
        a: &Doc,
        b: &Doc,
        adj_a: &Vec<Vec<(usize, usize)>>,
        adj_b: &Vec<Vec<(usize, usize)>>,
        map: &mut Vec<usize>,
        used: &mut Vec<bool>,
        ok: &dyn Fn(usize, usize) -> bool,
        bond_ok: &dyn Fn(usize, usize) -> bool,
        steps: &mut usize,
    ) -> bool {
        *steps += 1;
        if *steps > 500_000 {
            return false;
        }
        if k == order_a.len() {
            return true;
        }
        let i = order_a[k];
        for j in 0..b.atoms.len() {
            if used[j] || !ok(i, j) {
                continue;
            }
            // Every mapped neighbour of i must be a neighbour of j with the same bond.
            let mut good = true;
            for &(ni, bi) in &adj_a[i] {
                if map[ni] == usize::MAX {
                    continue;
                }
                match adj_b[j].iter().find(|x| x.0 == map[ni]) {
                    Some(&(_, bj)) if bond_ok(bi, bj) => {}
                    _ => {
                        good = false;
                        break;
                    }
                }
            }
            if !good {
                continue;
            }
            map[i] = j;
            used[j] = true;
            if go(k + 1, order_a, a, b, adj_a, adj_b, map, used, ok, bond_ok, steps) {
                return true;
            }
            map[i] = usize::MAX;
            used[j] = false;
        }
        false
    }
    // Visit atoms in BFS order so neighbours are mapped early.
    let mut order_a = Vec::new();
    let mut seen = vec![false; n];
    for s in 0..n {
        if seen[s] {
            continue;
        }
        let mut q = std::collections::VecDeque::from([s]);
        seen[s] = true;
        while let Some(u) = q.pop_front() {
            order_a.push(u);
            for &(v, _) in &adj_a[u] {
                if !seen[v] {
                    seen[v] = true;
                    q.push_back(v);
                }
            }
        }
    }
    let ok = |i: usize, j: usize| key(a, &adj_a, i) == key(b, &adj_b, j);
    let bond_ok = |x: usize, y: usize| order(a, &ar_a, x) == order(b, &ar_b, y);
    let mut steps = 0;
    go(0, &order_a, a, b, &adj_a, &adj_b, &mut map, &mut used, &ok, &bond_ok, &mut steps)
}

/// Name for one connected structure, if it can be worked out offline.
pub fn structure_to_name(d: &Doc) -> Option<String> {
    if d.atoms.is_empty() {
        return None;
    }
    // Labels are expanded first so "OMe" etc. compare as atoms.
    let d = formats::expand_labels(d);
    if d.fragments().len() != 1 {
        return None;
    }
    // Well-known compounds by their usual name (each built-in structure is read once).
    let n = d.atoms.len();
    let formula = chem::atoms_formula(&d, &(0..n).collect::<Vec<_>>());
    for (name, c) in known() {
        if c.atoms.len() == n && chem::atoms_formula(c, &(0..n).collect::<Vec<_>>()) == formula && same_structure(&d, c) {
            return Some(name.to_string());
        }
    }
    // Otherwise a systematic name worked out from the structure.
    crate::iupac::structure_to_name(&d).or_else(|| hydrocarbon_name(&d))
}

/// The built-in structures, read once: (name, structure). Later names for the same
/// structure (aliases) are skipped so the first, usual name is the one given.
fn known() -> &'static [(&'static str, Doc)] {
    static KNOWN: std::sync::OnceLock<Vec<(&'static str, Doc)>> = std::sync::OnceLock::new();
    KNOWN.get_or_init(|| {
        let mut seen: HashSet<&'static str> = HashSet::new();
        let mut out = Vec::new();
        for (name, smi) in all_names() {
            if !seen.insert(smi) {
                continue;
            }
            if let Ok(d) = formats::parse_smiles(smi) {
                out.push((*name, formats::expand_labels(&d)));
            }
        }
        out
    })
}

/// Saturated hydrocarbons: alkanes with alkyl branches, or a single carbon ring with alkyl groups.
fn hydrocarbon_name(d: &Doc) -> Option<String> {
    if d.atoms.iter().any(|a| a.el != 6 || a.charge != 0 || a.label.is_some() || a.radical != 0) {
        return None;
    }
    if d.bonds.iter().any(|b| b.order != 1) {
        return None;
    }
    let adj: Vec<Vec<usize>> = (0..d.atoms.len()).map(|i| d.neighbors(i)).collect();
    let rings = chem::sssr(d);
    match rings.len() {
        0 => {
            let best = longest_chain_choice(&adj, None)?;
            let (chain, subs) = best;
            Some(assemble(&subs, &format!("{}ane", ALK.get(chain.len())?)))
        }
        1 => {
            let ring = &rings[0];
            let n = ring.len();
            if n < 3 || n > 20 {
                return None;
            }
            let ring_set: HashSet<usize> = ring.iter().copied().collect();
            // Substituents on ring atoms.
            let mut best: Option<(Vec<usize>, Vec<(usize, String)>)> = None;
            for start in 0..n {
                for dir in [1isize, -1] {
                    let mut subs = Vec::new();
                    for k in 0..n {
                        let idx = ((start as isize + dir * k as isize).rem_euclid(n as isize)) as usize;
                        let atom = ring[idx];
                        for &nb in &adj[atom] {
                            if !ring_set.contains(&nb) {
                                subs.push((k + 1, substituent_name(&adj, nb, atom)?));
                            }
                        }
                    }
                    let mut locs: Vec<usize> = subs.iter().map(|s| s.0).collect();
                    locs.sort();
                    let better = match &best {
                        None => true,
                        Some((bl, bs)) => locs < *bl || (locs == *bl && first_alpha_locant(&subs) < first_alpha_locant(bs)),
                    };
                    if better {
                        best = Some((locs, subs));
                    }
                }
            }
            let (_, subs) = best?;
            let parent = format!("cyclo{}ane", ALK.get(n)?);
            if subs.len() == 1 {
                return Some(format!("{}{}", subs[0].1, parent));
            }
            Some(assemble(&subs, &parent))
        }
        _ => None,
    }
}

fn first_alpha_locant(subs: &[(usize, String)]) -> usize {
    let mut s: Vec<&(usize, String)> = subs.iter().collect();
    s.sort_by(|a, b| alpha_key(&a.1).cmp(&alpha_key(&b.1)));
    s.first().map(|x| x.0).unwrap_or(0)
}

fn alpha_key(name: &str) -> String {
    name.trim_start_matches('(').trim_start_matches("tert-").trim_start_matches("sec-").to_string()
}

/// Longest chain in a tree; picks the one with most substituents, then lowest locants.
fn longest_chain_choice(adj: &[Vec<usize>], from: Option<usize>) -> Option<(Vec<usize>, Vec<(usize, String)>)> {
    let n = adj.len();
    let leaves: Vec<usize> = (0..n).filter(|&i| adj[i].len() <= 1).collect();
    let mut paths: Vec<Vec<usize>> = Vec::new();
    for &s in &leaves {
        // BFS for parents, then paths to all other leaves.
        let mut prev = vec![usize::MAX; n];
        let mut seen = vec![false; n];
        let mut q = std::collections::VecDeque::from([s]);
        seen[s] = true;
        while let Some(u) = q.pop_front() {
            for &v in &adj[u] {
                if !seen[v] {
                    seen[v] = true;
                    prev[v] = u;
                    q.push_back(v);
                }
            }
        }
        for &t in &leaves {
            let mut p = vec![t];
            let mut c = t;
            while c != s {
                c = prev[c];
                if c == usize::MAX {
                    break;
                }
                p.push(c);
            }
            p.reverse();
            paths.push(p);
        }
    }
    if n == 1 {
        paths.push(vec![0]);
    }
    let _ = from;
    let maxlen = paths.iter().map(|p| p.len()).max()?;
    let mut best: Option<(usize, Vec<usize>, Vec<usize>, Vec<(usize, String)>)> = None;
    for p in paths.into_iter().filter(|p| p.len() == maxlen) {
        let set: HashSet<usize> = p.iter().copied().collect();
        let mut subs = Vec::new();
        for (k, &a) in p.iter().enumerate() {
            for &nb in &adj[a] {
                if !set.contains(&nb) {
                    subs.push((k + 1, substituent_name(adj, nb, a)?));
                }
            }
        }
        let mut locs: Vec<usize> = subs.iter().map(|s| s.0).collect();
        locs.sort();
        let better = match &best {
            None => true,
            Some((cnt, bl, _, bs)) => subs.len() > *cnt || (subs.len() == *cnt && (locs < *bl || (locs == *bl && first_alpha_locant(&subs) < first_alpha_locant(bs)))),
        };
        if better {
            best = Some((subs.len(), locs, p, subs));
        }
    }
    best.map(|(_, _, p, s)| (p, s))
}

/// Alkyl group name for the branch starting at `start` (attached to `parent`).
fn substituent_name(adj: &[Vec<usize>], start: usize, parent: usize) -> Option<String> {
    // Collect the branch.
    let mut branch = vec![start];
    let mut seen: HashSet<usize> = HashSet::from([start, parent]);
    let mut k = 0;
    while k < branch.len() {
        let u = branch[k];
        for &v in &adj[u] {
            if seen.insert(v) {
                branch.push(v);
            }
        }
        k += 1;
    }
    let size = branch.len();
    let deg = |i: usize| adj[i].iter().filter(|&&x| x != parent || i != start).filter(|&&x| branch.contains(&x)).count();
    // Common branched names.
    let start_deg = deg(start);
    if size == 3 && start_deg == 2 {
        return Some("isopropyl".into());
    }
    if size == 4 && start_deg == 3 {
        return Some("tert-butyl".into());
    }
    if size == 4 && start_deg == 2 {
        let others: Vec<usize> = adj[start].iter().copied().filter(|&x| x != parent).collect();
        if others.iter().any(|&o| adj[o].len() == 2) {
            return Some("sec-butyl".into());
        }
    }
    if size == 4 && start_deg == 1 {
        let next = *adj[start].iter().find(|&&x| x != parent)?;
        if adj[next].len() == 3 {
            return Some("isobutyl".into());
        }
    }
    // Longest chain from the attachment point.
    let mut best_path = vec![start];
    let mut stack = vec![(start, vec![start])];
    while let Some((u, path)) = stack.pop() {
        if path.len() > best_path.len() {
            best_path = path.clone();
        }
        for &v in &adj[u] {
            if v != parent && !path.contains(&v) && branch.contains(&v) {
                let mut np = path.clone();
                np.push(v);
                stack.push((v, np));
            }
        }
    }
    let set: HashSet<usize> = best_path.iter().copied().collect();
    let mut subs = Vec::new();
    for (k, &a) in best_path.iter().enumerate() {
        for &nb in &adj[a] {
            if nb != parent && !set.contains(&nb) {
                subs.push((k + 1, substituent_name(adj, nb, a)?));
            }
        }
    }
    let base = format!("{}yl", ALK.get(best_path.len())?);
    if subs.is_empty() {
        Some(base)
    } else {
        Some(format!("({})", assemble(&subs, &base)))
    }
}

fn assemble(subs: &[(usize, String)], parent: &str) -> String {
    if subs.is_empty() {
        return parent.to_string();
    }
    let mut groups: Vec<(String, Vec<usize>)> = Vec::new();
    for (l, n) in subs {
        match groups.iter_mut().find(|g| g.0 == *n) {
            Some(g) => g.1.push(*l),
            None => groups.push((n.clone(), vec![*l])),
        }
    }
    groups.sort_by(|a, b| alpha_key(&a.0).cmp(&alpha_key(&b.0)));
    let parts: Vec<String> = groups
        .into_iter()
        .map(|(n, mut l)| {
            l.sort();
            let locs: Vec<String> = l.iter().map(|x| x.to_string()).collect();
            format!("{}-{}{}", locs.join(","), MULT.get(l.len()).copied().unwrap_or(""), n)
        })
        .collect();
    let mut s = parts.join("-");
    let first = parent.chars().next().unwrap_or('a');
    if s.ends_with(')') || !first.is_alphabetic() {
        s.push_str(parent);
    } else {
        s.push_str(parent);
    }
    s
}
