//! Names and numbering for ring systems without a retained name: bridged and caged
//! polycycles (von Baeyer names: bicyclo[2.2.1]heptane, tricyclo[3.3.1.1³,⁷]decane,
//! 8-azabicyclo[3.2.1]octane) and spiro compounds of two rings (spiro[4.5]decane).
//!
//! The result is a `Template` like the built-in ones, so the namer handles substituents,
//! suffixes and stereo the same way. Each name's template is made once and kept.

use crate::hetero::Template;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

const STEMS: [&str; 31] = [
    "", "meth", "eth", "prop", "but", "pent", "hex", "hept", "oct", "non", "dec", "undec", "dodec", "tridec", "tetradec", "pentadec", "hexadec",
    "heptadec", "octadec", "nonadec", "icos", "henicos", "docos", "tricos", "tetracos", "pentacos", "hexacos", "heptacos", "octacos", "nonacos",
    "triacont",
];
const RINGS: [&str; 11] = ["", "", "bi", "tri", "tetra", "penta", "hexa", "hepta", "octa", "nona", "deca"];
const MULT: [&str; 7] = ["", "", "di", "tri", "tetra", "penta", "hexa"];

/// Replacement prefixes in order of seniority (O before S before N…).
const HETERO: [(u8, &str); 6] = [(8, "oxa"), (16, "thia"), (7, "aza"), (15, "phospha"), (14, "sila"), (5, "bora")];

/// A von Baeyer or spiro name: `true` for templates made here.
pub fn is_generated(t: &Template) -> bool {
    t.name.contains("cyclo[") || t.name.contains("spiro[")
}

fn sup(n: usize) -> String {
    n.to_string()
        .chars()
        .map(|c| match c {
            '0' => '⁰',
            '1' => '¹',
            '2' => '²',
            '3' => '³',
            '4' => '⁴',
            '5' => '⁵',
            '6' => '⁶',
            '7' => '⁷',
            '8' => '⁸',
            _ => '⁹',
        })
        .collect()
}

/// One way of numbering the system: `order[k]` = local atom with locant k+1.
struct Numbering {
    order: Vec<usize>,
    /// Sorts best-first: ring rules (main ring, main bridge, symmetry, superscripts).
    rank: Vec<i64>,
    /// "[3.3.1.1³,⁷]" or "[4.5]".
    descriptor: String,
}

/// Name a ring system that matched no template. `atoms`: the system's atoms; `el(i)`: element;
/// `ring_nbs(i)`: neighbours inside the system. Returns the template and every numbering
/// (`map[t]` = the drawn atom for template atom t), like `hetero::match_system`.
pub fn name_system(atoms: &[usize], el: &dyn Fn(usize) -> u8, ring_nbs: &dyn Fn(usize) -> Vec<usize>) -> Option<(&'static Template, Vec<Vec<usize>>)> {
    let n = atoms.len();
    if !(4..=30).contains(&n) {
        return None;
    }
    let idx: HashMap<usize, usize> = atoms.iter().enumerate().map(|(i, &a)| (a, i)).collect();
    let mut adj = vec![Vec::new(); n];
    for (i, &a) in atoms.iter().enumerate() {
        for b in ring_nbs(a) {
            let j = *idx.get(&b)?;
            if !adj[i].contains(&j) {
                adj[i].push(j);
            }
        }
    }
    let edges: usize = adj.iter().map(|v| v.len()).sum::<usize>() / 2;
    if edges < n {
        return None;
    }
    let rings = edges - n + 1;
    if rings < 2 || rings >= RINGS.len() {
        return None;
    }
    let els: Vec<u8> = atoms.iter().map(|&a| el(a)).collect();
    if els.iter().any(|e| *e != 6 && !HETERO.iter().any(|h| h.0 == *e)) {
        return None;
    }
    let (numberings, word) = match articulation(&adj) {
        Some(s) => (spiro(&adj, s)?, "spiro".to_string()),
        None => (von_baeyer(&adj)?, format!("{}cyclo", RINGS[rings])),
    };
    // The ring rules first, then low locants for all heteroatoms together, then in the order
    // O, S, N, P, Si, B.
    let hetero_key = |o: &[usize]| -> (Vec<usize>, Vec<Vec<usize>>) {
        let all: Vec<usize> = (0..n).filter(|&k| els[o[k]] != 6).map(|k| k + 1).collect();
        let each = HETERO.iter().map(|h| (0..n).filter(|&k| els[o[k]] == h.0).map(|k| k + 1).collect()).collect();
        (all, each)
    };
    let best = numberings.iter().map(|x| (&x.rank, hetero_key(&x.order))).min()?;
    let (best_rank, best_het) = (best.0.clone(), best.1);
    let chosen: Vec<&Numbering> = numberings.iter().filter(|x| x.rank == best_rank && hetero_key(&x.order) == best_het).collect();
    let first = chosen[0];
    // "2-oxa-6-aza", "1,4-diaza".
    let mut prefix = String::new();
    for (h, word) in HETERO {
        let locs: Vec<String> = (0..n).filter(|&k| els[first.order[k]] == h).map(|k| (k + 1).to_string()).collect();
        if locs.is_empty() {
            continue;
        }
        if !prefix.is_empty() {
            prefix.push('-');
        }
        prefix.push_str(&format!("{}-{}{}", locs.join(","), MULT[locs.len().min(6)], word));
    }
    let name = format!("{}{}{}{}ane", prefix, word, first.descriptor, STEMS.get(n)?);
    let elements: Vec<u8> = first.order.iter().map(|&i| els[i]).collect();
    let tpl = template(&name, &elements, &adj, &first.order);
    let mut maps: Vec<Vec<usize>> = Vec::new();
    for x in chosen {
        let m: Vec<usize> = x.order.iter().map(|&i| atoms[i]).collect();
        if !maps.contains(&m) {
            maps.push(m);
        }
        if maps.len() >= 48 {
            break;
        }
    }
    Some((tpl, maps))
}

/// The kept template for a name (made the first time).
fn template(name: &str, elements: &[u8], adj: &[Vec<usize>], order: &[usize]) -> &'static Template {
    let pos: HashMap<usize, usize> = order.iter().enumerate().map(|(k, &i)| (i, k)).collect();
    let mut bonds = Vec::new();
    for (i, nbs) in adj.iter().enumerate() {
        for &j in nbs {
            if i < j {
                let (a, b) = (pos[&i], pos[&j]);
                bonds.push((a.min(b) as u8, a.max(b) as u8));
            }
        }
    }
    kept(name, elements, bonds)
}

fn kept(name: &str, elements: &[u8], mut bonds: Vec<(u8, u8)>) -> &'static Template {
    static KEPT: Mutex<Option<HashMap<String, &'static Template>>> = Mutex::new(None);
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    let kept = kept.get_or_insert_with(HashMap::new);
    if let Some(t) = kept.get(name) {
        return t;
    }
    bonds.sort();
    let atoms: Vec<(u8, &'static str)> = elements.iter().enumerate().map(|(k, &e)| (e, &*Box::leak((k + 1).to_string().into_boxed_str()))).collect();
    let t: &'static Template = Box::leak(Box::new(Template {
        name: Box::leak(name.to_string().into_boxed_str()),
        atoms: Box::leak(atoms.into_boxed_slice()),
        bonds: Box::leak(bonds.into_boxed_slice()),
        indicated: 0,
        saturated: true,
        fixed_prefix: None,
    }));
    kept.insert(name.to_string(), t);
    t
}

/// An atom whose removal splits the system (a spiro atom).
fn articulation(adj: &[Vec<usize>]) -> Option<usize> {
    let n = adj.len();
    (0..n).find(|&s| {
        let start = if s == 0 { 1 } else { 0 };
        let mut seen = vec![false; n];
        seen[s] = true;
        seen[start] = true;
        let mut stack = vec![start];
        let mut count = 1;
        while let Some(v) = stack.pop() {
            for &w in &adj[v] {
                if !seen[w] {
                    seen[w] = true;
                    count += 1;
                    stack.push(w);
                }
            }
        }
        count < n - 1
    })
}

/// Two simple rings sharing one atom: spiro[a.b], numbered from the smaller ring's atom next
/// to the spiro atom, round the smaller ring, through the spiro atom, round the larger.
fn spiro(adj: &[Vec<usize>], s: usize) -> Option<Vec<Numbering>> {
    if adj[s].len() != 4 {
        return None;
    }
    // Walk each ring from the spiro atom.
    let mut rings: Vec<Vec<usize>> = Vec::new();
    let mut used = HashSet::new();
    for &start in &adj[s] {
        if used.contains(&start) {
            continue;
        }
        let mut ring = vec![start];
        used.insert(start);
        let mut prev = s;
        let mut cur = start;
        loop {
            if adj[cur].len() != 2 {
                return None;
            }
            let next = if adj[cur][0] == prev { adj[cur][1] } else { adj[cur][0] };
            if next == s {
                break;
            }
            if used.contains(&next) {
                return None;
            }
            used.insert(next);
            ring.push(next);
            prev = cur;
            cur = next;
        }
        rings.push(ring);
    }
    if rings.len() != 2 || used.len() + 1 != adj.len() {
        return None;
    }
    rings.sort_by_key(|r| r.len());
    let (a, b) = (rings[0].len(), rings[1].len());
    let mut out = Vec::new();
    let both = |r: &Vec<usize>| {
        let mut rev = r.clone();
        rev.reverse();
        vec![r.clone(), rev]
    };
    let firsts: Vec<(usize, usize)> = if a == b { vec![(0, 1), (1, 0)] } else { vec![(0, 1)] };
    for (f, g) in firsts {
        for small in both(&rings[f]) {
            for large in both(&rings[g]) {
                let mut order = small.clone();
                order.push(s);
                order.extend(large.iter().copied());
                out.push(Numbering { order, rank: vec![], descriptor: format!("[{}.{}]", a, b) });
            }
        }
    }
    Some(out)
}

/// Every simple cycle of the greatest length, as vertex walks (both directions, every start
/// is generated later).
fn longest_cycles(adj: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let n = adj.len();
    let mut best: Vec<Vec<usize>> = Vec::new();
    let mut best_len = 0;
    let mut steps = 0usize;
    fn dfs(adj: &[Vec<usize>], s: usize, path: &mut Vec<usize>, on: &mut [bool], best: &mut Vec<Vec<usize>>, best_len: &mut usize, steps: &mut usize) {
        *steps += 1;
        if *steps > 400_000 {
            return;
        }
        let v = *path.last().unwrap_or(&s);
        for &w in &adj[v] {
            if w == s && path.len() >= 3 && path[1] < path[path.len() - 1] {
                if path.len() > *best_len {
                    *best_len = path.len();
                    best.clear();
                }
                if path.len() == *best_len && best.len() < 400 {
                    best.push(path.clone());
                }
            } else if w > s && !on[w] {
                on[w] = true;
                path.push(w);
                dfs(adj, s, path, on, best, best_len, steps);
                path.pop();
                on[w] = false;
            }
        }
    }
    for s in 0..n {
        let mut on = vec![false; n];
        on[s] = true;
        let mut path = vec![s];
        dfs(adj, s, &mut path, &mut on, &mut best, &mut best_len, &mut steps);
    }
    best
}

/// Paths from `from` through atoms outside `inside` to another atom of `inside`, using only
/// bonds not yet `used`: (interior atoms, end).
fn bridges_from(adj: &[Vec<usize>], inside: &[bool], used: &HashSet<(usize, usize)>, from: usize) -> Vec<(Vec<usize>, usize)> {
    let mut out = Vec::new();
    let key = |a: usize, b: usize| (a.min(b), a.max(b));
    fn walk(adj: &[Vec<usize>], inside: &[bool], used: &HashSet<(usize, usize)>, from: usize, v: usize, path: &mut Vec<usize>, out: &mut Vec<(Vec<usize>, usize)>, key: &dyn Fn(usize, usize) -> (usize, usize)) {
        if out.len() > 2000 {
            return;
        }
        for &w in &adj[v] {
            if used.contains(&key(v, w)) || path.contains(&w) {
                continue;
            }
            if inside[w] {
                if w != from {
                    out.push((path.clone(), w));
                }
            } else {
                path.push(w);
                walk(adj, inside, used, from, w, path, out, key);
                path.pop();
            }
        }
    }
    let mut path = Vec::new();
    walk(adj, inside, used, from, from, &mut path, &mut out, &key);
    out
}

/// Von Baeyer numberings: the largest main ring, the largest main bridge, the most symmetric
/// division, then secondary bridges (largest first) with the lowest superscripts.
fn von_baeyer(adj: &[Vec<usize>]) -> Option<Vec<Numbering>> {
    let n = adj.len();
    let key = |a: usize, b: usize| (a.min(b), a.max(b));
    let cycles = longest_cycles(adj);
    let mut out: Vec<Numbering> = Vec::new();
    for c in &cycles {
        let l = c.len();
        let mut inside = vec![false; n];
        for &v in c {
            inside[v] = true;
        }
        let mut used: HashSet<(usize, usize)> = HashSet::new();
        for k in 0..l {
            used.insert(key(c[k], c[(k + 1) % l]));
        }
        // Main bridges.
        let mut mains: Vec<(Vec<usize>, usize, usize)> = Vec::new();
        for &u in c {
            for (p, v) in bridges_from(adj, &inside, &used, u) {
                if u < v {
                    mains.push((p, u, v));
                }
            }
        }
        let Some(bmax) = mains.iter().map(|m| m.0.len()).max() else { continue };
        for (bridge, u, v) in mains.iter().filter(|m| m.0.len() == bmax) {
            let pu = c.iter().position(|x| x == u)?;
            let pv = c.iter().position(|x| x == v)?;
            let d = (pv + l - pu) % l;
            let (seg_a, seg_b) = (d - 1, l - d - 1);
            let (s1, s2) = (seg_a.max(seg_b), seg_a.min(seg_b));
            for (start, other) in [(*u, *v), (*v, *u)] {
                let ps = c.iter().position(|&x| x == start)?;
                for dir in [1usize, l - 1] {
                    // Walk from the start; the first stretch to the other bridgehead must be
                    // the longer one.
                    let walk: Vec<usize> = (0..l).map(|k| c[(ps + dir * k) % l]).collect();
                    let first_len = walk.iter().position(|&x| x == other)? - 1;
                    if first_len != s1 {
                        continue;
                    }
                    let mut order = walk;
                    // Main bridge from the end next to atom 1.
                    let mut br = bridge.clone();
                    if br.first().is_some_and(|&f| !adj[start].contains(&f)) {
                        br.reverse();
                    }
                    order.extend(br.iter().copied());
                    if let Some(x) = secondary(adj, order, &used, bridge, c, s1, s2, bmax) {
                        out.push(x);
                    }
                }
            }
        }
    }
    if out.is_empty() {
        return None;
    }
    Some(out)
}

/// Add the secondary bridges to a numbering of the main ring and main bridge.
#[allow(clippy::too_many_arguments)]
fn secondary(adj: &[Vec<usize>], mut order: Vec<usize>, ring_bonds: &HashSet<(usize, usize)>, main: &[usize], c: &[usize], s1: usize, s2: usize, bmax: usize) -> Option<Numbering> {
    let n = adj.len();
    let key = |a: usize, b: usize| (a.min(b), a.max(b));
    let mut used = ring_bonds.clone();
    // Bonds of the main bridge.
    let start = order[0];
    let other = order[s1 + 1];
    if main.is_empty() {
        used.insert(key(start, other));
    } else {
        // The bridge was numbered from the end next to atom 1.
        let mut ch = vec![start];
        let mut br: Vec<usize> = main.to_vec();
        if !adj[start].contains(&br[0]) {
            br.reverse();
        }
        ch.extend(br);
        ch.push(other);
        for w in ch.windows(2) {
            used.insert(key(w[0], w[1]));
        }
    }
    let mut inside = vec![false; n];
    for &v in &order {
        inside[v] = true;
    }
    let mut sec: Vec<(usize, usize, usize)> = Vec::new();
    let total_bonds: usize = adj.iter().map(|v| v.len()).sum::<usize>() / 2;
    while used.len() < total_bonds {
        let pos: HashMap<usize, usize> = order.iter().enumerate().map(|(k, &v)| (v, k + 1)).collect();
        // The largest bridge; among equals the lowest bridgehead locants.
        let mut best: Option<(Vec<usize>, usize, usize)> = None;
        for &u in &order {
            for (p, v) in bridges_from(adj, &inside, &used, u) {
                let (lu, lv) = (pos[&u], pos[&v]);
                if lu > lv {
                    continue;
                }
                let better = match &best {
                    None => true,
                    Some((bp, bu, bv)) => p.len() > bp.len() || (p.len() == bp.len() && (lu, lv) < (pos[bu], pos[bv])),
                };
                if better {
                    best = Some((p, u, v));
                }
            }
        }
        let (mut p, u, v) = best?;
        // Numbered from the end nearer the higher-numbered bridgehead.
        if !p.is_empty() && adj[u].contains(&p[0]) {
            p.reverse();
        }
        let mut ch = vec![v];
        ch.extend(p.iter().copied());
        ch.push(u);
        for w in ch.windows(2) {
            used.insert(key(w[0], w[1]));
        }
        sec.push((p.len(), pos[&u], pos[&v]));
        for &x in &p {
            inside[x] = true;
            order.push(x);
        }
    }
    if order.len() != n {
        return None;
    }
    // Descriptor: the main ring's two stretches, the main bridge, then the secondary bridges
    // (largest first) with the locants of their bridgeheads.
    let mut cited = sec.clone();
    cited.sort_by(|a, b| b.0.cmp(&a.0).then((a.1, a.2).cmp(&(b.1, b.2))));
    let mut descriptor = format!("[{}.{}.{}", s1, s2, bmax);
    for (size, a, b) in &cited {
        descriptor.push_str(&format!(".{}{},{}", size, sup(*a), sup(*b)));
    }
    descriptor.push(']');
    let mut supers: Vec<i64> = sec.iter().flat_map(|x| [x.1 as i64, x.2 as i64]).collect();
    supers.sort();
    let mut rank = vec![-(c.len() as i64), -(bmax as i64), s1 as i64];
    rank.extend(supers);
    Some(Numbering { order, rank, descriptor })
}

/// A bridged or spiro parent found in a (lower-case) name: `s[..start]` comes before it (its
/// groups), `s[end..]` after it (the suffix). `full`: written with its final "e".
pub struct Reading {
    pub start: usize,
    pub end: usize,
    pub template: &'static Template,
    pub full: bool,
    /// Ring double bonds, as template atom pairs.
    pub doubles: Vec<(usize, usize)>,
}

/// Turn superscripts ("1³,⁷"), "1^3,7", "1^{3,7}" and "1~3,7~" into "1(3,7)".
fn plain_descriptor(d: &str) -> String {
    let mut out = String::new();
    let mut run = String::new();
    let flush = |out: &mut String, run: &mut String| {
        if !run.is_empty() {
            out.push('(');
            out.push_str(run.trim_end_matches(','));
            out.push(')');
            run.clear();
        }
    };
    let mut chars = d.chars().peekable();
    while let Some(c) = chars.next() {
        if let Some(v) = "⁰¹²³⁴⁵⁶⁷⁸⁹".chars().position(|x| x == c) {
            run.push(char::from(b'0' + v as u8));
        } else if c == ',' && !run.is_empty() {
            run.push(',');
        } else if c == '^' || c == '~' {
            flush(&mut out, &mut run);
            let mut inner = String::new();
            while let Some(&x) = chars.peek() {
                if x.is_ascii_digit() || x == ',' {
                    inner.push(x);
                    chars.next();
                } else if x == '{' && inner.is_empty() {
                    chars.next();
                } else if x == '}' || x == '~' {
                    chars.next();
                    break;
                } else {
                    break;
                }
            }
            out.push('(');
            out.push_str(&inner);
            out.push(')');
        } else {
            flush(&mut out, &mut run);
            out.push(c);
        }
    }
    flush(&mut out, &mut run);
    out
}

/// "3.3.1.1(3,7)" → [(3, None), (3, None), (1, None), (1, Some((3, 7)))].
fn parse_descriptor(d: &str) -> Option<Vec<(usize, Option<(usize, usize)>)>> {
    let d = plain_descriptor(d);
    let mut out = Vec::new();
    let mut rest = d.as_str();
    loop {
        let len = rest.chars().take_while(|c| c.is_ascii_digit()).count();
        if len == 0 {
            return None;
        }
        let size: usize = rest[..len].parse().ok()?;
        rest = &rest[len..];
        let mut sup = None;
        if let Some(r) = rest.strip_prefix('(') {
            let close = r.find(')')?;
            let (a, b) = r[..close].split_once(',')?;
            sup = Some((a.trim().parse().ok()?, b.trim().parse().ok()?));
            rest = &r[close + 1..];
        }
        out.push((size, sup));
        if rest.is_empty() {
            return Some(out);
        }
        rest = rest.strip_prefix('.')?;
    }
}

/// Every bridged or spiro parent in a lower-case name whose brackets are written "( )".
pub fn read(s: &str) -> Vec<Reading> {
    let mut at: Vec<usize> = s.match_indices("cyclo(").chain(s.match_indices("spiro(")).map(|x| x.0).collect();
    at.sort();
    at.into_iter().filter_map(|i| read_at(s, i)).collect()
}

fn read_at(s: &str, at: usize) -> Option<Reading> {
    let spiro = s[at..].starts_with("spiro(");
    let open = at + 5;
    // The ring count before "cyclo".
    let mut start = at;
    let mut rings = 0;
    if !spiro {
        let (k, w) = RINGS.iter().enumerate().skip(2).filter(|(_, w)| s[..at].ends_with(**w)).max_by_key(|(_, w)| w.len())?;
        start = at - w.len();
        rings = k;
    }
    // The bracket, with nested "(3,7)".
    let mut depth = 0;
    let mut close = None;
    for (i, c) in s[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(open + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    let items = parse_descriptor(&s[open + 1..close])?;
    let mut bonds: Vec<(usize, usize)> = Vec::new();
    let n;
    if spiro {
        if items.len() != 2 || items.iter().any(|x| x.1.is_some()) {
            return None;
        }
        let (a, b) = (items[0].0, items[1].0);
        if a == 0 || b == 0 {
            return None;
        }
        n = a + b + 1;
        // The smaller ring 1…a with the spiro atom a+1, then the larger ring.
        for k in 0..a {
            bonds.push((k, k + 1));
        }
        bonds.push((a, 0));
        let mut prev = a;
        for k in a + 1..n {
            bonds.push((prev, k));
            prev = k;
        }
        bonds.push((prev, a));
    } else {
        if items.len() != rings + 1 || items[..3].iter().any(|x| x.1.is_some()) || items[3..].iter().any(|x| x.1.is_none()) {
            return None;
        }
        let (a, b, c) = (items[0].0, items[1].0, items[2].0);
        if b > a || c > b || b == 0 {
            return None;
        }
        // Main ring: 1, the first bridge, the second bridgehead, the second bridge.
        let main = a + b + 2;
        for k in 0..main {
            bonds.push((k, (k + 1) % main));
        }
        // The main bridge, numbered from the end next to atom 1.
        let mut next = main;
        let mut prev = 0;
        for _ in 0..c {
            bonds.push((prev, next));
            prev = next;
            next += 1;
        }
        bonds.push((prev, a + 1));
        // Secondary bridges, largest first, each from the end next to its higher bridgehead.
        let mut sec: Vec<(usize, usize, usize)> = items[3..].iter().filter_map(|x| x.1.map(|p| (x.0, p.0.min(p.1), p.0.max(p.1)))).collect();
        sec.sort_by(|p, q| q.0.cmp(&p.0).then((p.1, p.2).cmp(&(q.1, q.2))));
        for (size, x, y) in sec {
            if x == 0 || x == y || y > next {
                return None;
            }
            let mut prev = y - 1;
            for _ in 0..size {
                bonds.push((prev, next));
                prev = next;
                next += 1;
            }
            bonds.push((prev, x - 1));
        }
        n = next;
    }
    if !(3..=30).contains(&n) {
        return None;
    }
    // The alkane after the bracket, with any double bonds: "heptane", "heptan",
    // "hept-2-ene", "hepta-2,5-dien".
    let after = &s[close + 1..];
    let stem = STEMS.get(n).filter(|x| !x.is_empty())?;
    let r = after.strip_prefix(*stem)?;
    let mut doubles: Vec<usize> = Vec::new();
    let (tail_len, full);
    if r.starts_with("ane") {
        tail_len = 3;
        full = true;
    } else if r.starts_with("an") {
        tail_len = 2;
        full = false;
    } else {
        let several = r.starts_with("a-");
        let body = r.strip_prefix("a-").or_else(|| r.strip_prefix('-'))?;
        let dash = body.find('-')?;
        for l in body[..dash].split(',') {
            doubles.push(l.parse::<usize>().ok()?.checked_sub(1)?);
        }
        let word = &body[dash + 1..];
        let m = MULT.iter().enumerate().skip(2).find(|(_, w)| word.starts_with(**w)).map(|x| x.0).unwrap_or(1);
        if m != doubles.len() || (m > 1) != several {
            return None;
        }
        let skip = if m > 1 { MULT[m].len() } else { 0 };
        let w = &word[skip..];
        let used = r.len() - word.len() + skip;
        if w.starts_with("ene") {
            tail_len = used + 3;
            full = true;
        } else if w.starts_with("en") {
            tail_len = used + 2;
            full = false;
        } else {
            return None;
        }
    }
    let end = close + 1 + stem.len() + tail_len;
    if full && s[end..].chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    // Replacement prefixes before it: "3-oxa-9-aza", "1,4-diaza".
    let mut elements = vec![6u8; n];
    let mut pre = &s[..start];
    while let Some((el, w)) = HETERO.iter().find(|(_, w)| pre.ends_with(*w)) {
        let body = &pre[..pre.len() - w.len()];
        let (body, m) = match MULT.iter().enumerate().skip(2).find(|(_, mw)| body.ends_with(**mw)) {
            Some((m, mw)) => (&body[..body.len() - mw.len()], m),
            None => (body, 1),
        };
        let body = body.strip_suffix('-')?;
        let from = body.rfind(|c: char| !(c.is_ascii_digit() || c == ',')).map(|i| i + 1).unwrap_or(0);
        let locs: Vec<usize> = body[from..].split(',').map(|l| l.parse::<usize>().ok().filter(|&x| x >= 1 && x <= n)).collect::<Option<_>>()?;
        if locs.len() != m {
            return None;
        }
        for l in locs {
            elements[l - 1] = *el;
        }
        pre = body[..from].strip_suffix('-').unwrap_or(&body[..from]);
    }
    let pairs: Vec<(usize, usize)> = doubles
        .iter()
        .map(|&k| {
            let mut nbs: Vec<usize> = bonds.iter().filter_map(|&(p, q)| if p == k { Some(q) } else if q == k { Some(p) } else { None }).filter(|&o| o > k).collect();
            nbs.sort();
            nbs.first().map(|&o| (k, o))
        })
        .collect::<Option<_>>()?;
    let text = &s[start..end];
    let name = if full { format!("read:{}", text) } else { format!("read:{}e", text) };
    let tpl = kept(&name, &elements, bonds.iter().map(|&(p, q)| (p.min(q) as u8, p.max(q) as u8)).collect());
    Some(Reading { start: pre.len(), end, template: tpl, full, doubles: pairs })
}
