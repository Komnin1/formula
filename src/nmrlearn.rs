//! Learning measured ¹H and ¹³C shifts from a report of published spectra (the format of
//! docs/NMR-DATA-REPORT-TEMPLATE.md): read each compound's SMILES and peak lists, match the
//! measured peaks to the atoms by aligning them with the rule-based prediction, and keep the
//! measured shift of each atom under the keys of its surroundings (`nmr::ShiftDb`).

use crate::formats;
use crate::nmr::{self, DbChoice, Options, ShiftDb};
use std::collections::HashMap;

/// One compound of the report.
#[derive(Clone, Debug, Default)]
pub struct Entry {
    pub id: String,
    /// DOI or reference, without the compound number.
    pub source: String,
    pub smiles: String,
    pub solvent: String,
    pub h: String,
    pub c: String,
}

/// The compound blocks of a report ("### COMPOUND" followed by "Key: value" lines).
pub fn parse_report(text: &str) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut cur: Option<Entry> = None;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with("### COMPOUND") {
            if let Some(e) = cur.take() {
                out.push(e);
            }
            cur = Some(Entry::default());
            continue;
        }
        let Some(e) = cur.as_mut() else { continue };
        let Some((k, v)) = l.split_once(':') else { continue };
        let v = v.trim().to_string();
        match k.trim() {
            "ID" => e.id = v,
            "Source" => e.source = v.split(',').next().unwrap_or("").trim().to_string(),
            "SMILES" => e.smiles = v,
            "Solvent" => e.solvent = v,
            "1H" => e.h = v,
            "13C" => e.c = v,
            _ => {}
        }
    }
    out.extend(cur);
    out.into_iter().filter(|e| !e.smiles.is_empty()).collect()
}

/// One measured peak: its shift (or range lo–hi) and number of nuclei (0 when not given).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExpPeak {
    pub lo: f64,
    pub hi: f64,
    pub n: usize,
}

/// Decimal numbers in a piece of text (page numbers such as "S15" are skipped).
fn decimals(s: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < b.len() {
        if b[i].is_ascii_digit() && (i == 0 || !b[i - 1].is_ascii_alphanumeric()) {
            let start = i;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == '.') {
                i += 1;
            }
            let t: String = b[start..i].iter().collect();
            if t.contains('.') {
                if let Ok(v) = t.trim_end_matches('.').parse::<f64>() {
                    // A minus sign right before the number (19F, 31P).
                    let neg = start > 0 && matches!(b[start - 1], '-' | '−') && (start < 2 || b[start - 2] == ' ' || b[start - 2] == 'δ');
                    out.push(if neg { -v } else { v });
                }
            }
        } else {
            i += 1;
        }
    }
    out
}

/// A peak list as printed ("δ 7.99 (s, 1H), 7.46–7.37 (m, 1H), …"). Anything after a ";" (a
/// following nucleus pasted on) is ignored.
pub fn parse_peaks(text: &str) -> Vec<ExpPeak> {
    let t = text.split(';').next().unwrap_or("");
    let t = t.split("NMR").next().unwrap_or(t);
    let t = t.trim_start_matches(|c: char| c == 'δ' || c.is_whitespace());
    // Items are separated by commas outside parentheses.
    let mut items = Vec::new();
    let mut depth = 0;
    let mut cur = String::new();
    for ch in t.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth <= 0 => {
                items.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(ch);
    }
    items.push(cur);
    let mut out = Vec::new();
    for it in items {
        let (head, inner) = match it.find('(') {
            Some(p) => (&it[..p], &it[p..]),
            None => (it.as_str(), ""),
        };
        let nums = decimals(head);
        let Some(&last) = nums.last() else { continue };
        // A range "7.46–7.37": the last two numbers of the head.
        let ranged = nums.len() >= 2 && head.contains(['–', '-', '—']) && (nums[nums.len() - 2] - last).abs() < 3.0;
        let (lo, hi) = if ranged { let a = nums[nums.len() - 2]; (a.min(last), a.max(last)) } else { (last, last) };
        // Number of nuclei: "2H", "3H" (or "2C").
        let mut n = 0;
        let ib: Vec<char> = inner.chars().collect();
        for k in 0..ib.len() {
            if matches!(ib[k], 'H' | 'C') && k > 0 && ib[k - 1].is_ascii_digit() && (k + 1 == ib.len() || !ib[k + 1].is_ascii_alphabetic()) {
                let mut s = k;
                while s > 0 && ib[s - 1].is_ascii_digit() {
                    s -= 1;
                }
                n = ib[s..k].iter().collect::<String>().parse().unwrap_or(0);
            }
        }
        out.push(ExpPeak { lo, hi, n });
    }
    out
}

/// Monotonic alignment of two lists sorted from high to low: (index in `a`, index in `b`) pairs,
/// at the least total |difference|, where leaving an item unmatched costs `skip`.
fn align(a: &[f64], b: &[f64], skip: f64) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    let mut dp = vec![vec![f64::INFINITY; m + 1]; n + 1];
    let mut how = vec![vec![0u8; m + 1]; n + 1];
    dp[0][0] = 0.0;
    for i in 0..=n {
        for j in 0..=m {
            let d = dp[i][j];
            if !d.is_finite() {
                continue;
            }
            if i < n && j < m {
                let c = d + (a[i] - b[j]).abs();
                if c < dp[i + 1][j + 1] {
                    dp[i + 1][j + 1] = c;
                    how[i + 1][j + 1] = 1;
                }
            }
            if i < n && d + skip < dp[i + 1][j] {
                dp[i + 1][j] = d + skip;
                how[i + 1][j] = 2;
            }
            if j < m && d + skip < dp[i][j + 1] {
                dp[i][j + 1] = d + skip;
                how[i][j + 1] = 3;
            }
        }
    }
    let (mut i, mut j) = (n, m);
    let mut out = Vec::new();
    while i > 0 || j > 0 {
        match how[i][j] {
            1 => {
                out.push((i - 1, j - 1));
                i -= 1;
                j -= 1;
            }
            2 => i -= 1,
            _ => j -= 1,
        }
    }
    out.reverse();
    out
}

/// One learnt measurement: nucleus, surroundings keys, measured shift, rule-based shift.
#[derive(Clone, Debug)]
pub struct Sample {
    pub nucleus: u8,
    pub keys: [u64; 3],
    pub measured: f64,
    pub predicted: f64,
}

/// The measurements of one compound, matched to its atoms.
pub fn samples(e: &Entry) -> Vec<Sample> {
    let Ok(doc) = formats::parse_smiles(&e.smiles) else { return vec![] };
    if doc.atoms.iter().any(|a| a.label.is_some()) || doc.atoms.len() > 150 {
        return vec![];
    }
    let all: Vec<usize> = (0..doc.atoms.len()).collect();
    let p = nmr::predict_with(&doc, &all, Options { db: DbChoice::None, ..Default::default() });
    let keys = nmr::atom_keys(&doc);
    let mut out = Vec::new();

    // ¹³C: one measured line per set of equivalent carbons.
    let exp_c: Vec<f64> = {
        let mut v: Vec<f64> = parse_peaks(&e.c).iter().map(|x| (x.lo + x.hi) / 2.0).filter(|s| (-10.0..240.0).contains(s)).collect();
        v.sort_by(|a, b| b.partial_cmp(a).unwrap());
        v
    };
    let pred_c: Vec<&nmr::Peak> = p.c.iter().filter(|x| x.frac > 0.99).collect();
    if !exp_c.is_empty() && !pred_c.is_empty() {
        let ps: Vec<f64> = pred_c.iter().map(|x| x.shift).collect();
        let pairs = align(&ps, &exp_c, 6.0);
        let err: f64 = pairs.iter().map(|&(i, j)| (ps[i] - exp_c[j]).abs()).sum::<f64>() / pairs.len().max(1) as f64;
        // A poor overall match means the structure or the list is likely wrong.
        if pairs.len() * 10 >= ps.len() * 8 && err < 6.0 {
            for (i, j) in pairs {
                if (ps[i] - exp_c[j]).abs() < 12.0 {
                    let a = pred_c[i].atoms[0];
                    out.push(Sample { nucleus: 13, keys: keys[a], measured: exp_c[j], predicted: ps[i] });
                }
            }
        }
    }

    // ¹H: aligned proton by proton; a site is learnt when all its protons fall on narrow peaks.
    let exp_h = parse_peaks(&e.h);
    let mut ex: Vec<(f64, bool, usize)> = Vec::new(); // (shift, narrow, peak index)
    for (k, x) in exp_h.iter().enumerate() {
        let n = x.n.max(1).min(30);
        for q in 0..n {
            let s = if n == 1 { (x.lo + x.hi) / 2.0 } else { x.hi - (x.hi - x.lo) * q as f64 / (n - 1) as f64 };
            ex.push((s, x.hi - x.lo <= 0.12, k));
        }
    }
    ex.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    let mut px: Vec<(f64, usize)> = Vec::new(); // (shift, peak index)
    for (k, x) in p.h.iter().enumerate() {
        if x.frac < 0.99 {
            continue;
        }
        for _ in 0..x.count {
            px.push((x.shift, k));
        }
    }
    px.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());
    if !ex.is_empty() && !px.is_empty() && (ex.len() as isize - px.len() as isize).abs() <= 4 {
        let ps: Vec<f64> = px.iter().map(|x| x.0).collect();
        let es: Vec<f64> = ex.iter().map(|x| x.0).collect();
        let pairs = align(&ps, &es, 0.6);
        let err: f64 = pairs.iter().map(|&(i, j)| (ps[i] - es[j]).abs()).sum::<f64>() / pairs.len().max(1) as f64;
        if pairs.len() * 10 >= ps.len() * 8 && err < 0.5 {
            let mut per: HashMap<usize, Vec<(f64, bool)>> = HashMap::new();
            for (i, j) in pairs {
                per.entry(px[i].1).or_default().push((es[j], ex[j].1));
            }
            for (k, v) in per {
                let peak = &p.h[k];
                if v.len() != peak.count || !v.iter().all(|x| x.1) || peak.what == "OH" || peak.what.starts_with("NH") || peak.what == "SH" || peak.what == "COOH" {
                    continue;
                }
                let mean = v.iter().map(|x| x.0).sum::<f64>() / v.len() as f64;
                let spread = v.iter().map(|x| (x.0 - mean).abs()).fold(0.0, f64::max);
                if spread > 0.1 || (mean - peak.shift).abs() > 1.2 {
                    continue;
                }
                out.push(Sample { nucleus: 1, keys: keys[peak.atoms[0]], measured: mean, predicted: peak.shift });
            }
        }
    }
    out
}

/// The database of median corrections (measured − rules) from many samples.
pub fn build_db(samples: &[Sample]) -> ShiftDb {
    let mut acc: HashMap<(u8, u8, u64), Vec<f32>> = HashMap::new();
    for s in samples {
        for (r, k) in [(2u8, s.keys[0]), (3, s.keys[1])] {
            acc.entry((s.nucleus, r, k)).or_default().push((s.measured - s.predicted) as f32);
        }
    }
    let mut db = ShiftDb::default();
    for (k, mut v) in acc {
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = if v.len() % 2 == 1 { v[v.len() / 2] } else { (v[v.len() / 2 - 1] + v[v.len() / 2]) / 2.0 };
        db.map.insert(k, (med, v.len().min(u16::MAX as usize) as u16));
    }
    db
}

/// Mean absolute error of the rules and of the database (rules where it has nothing) on
/// samples of compounds the database was not built from: (nucleus, n, rules MAE, with-db MAE,
/// share of atoms the database covered).
pub fn evaluate(db: &ShiftDb, test: &[Sample]) -> Vec<(u8, usize, f64, f64, f64)> {
    let mut out = Vec::new();
    for nuc in [1u8, 13] {
        let t: Vec<&Sample> = test.iter().filter(|s| s.nucleus == nuc).collect();
        if t.is_empty() {
            continue;
        }
        let mut rules = 0.0;
        let mut with = 0.0;
        let mut covered = 0;
        for s in &t {
            rules += (s.predicted - s.measured).abs();
            let v = match db.lookup(nuc, &s.keys) {
                Some((c, _, _)) => {
                    covered += 1;
                    s.predicted + c
                }
                None => s.predicted,
            };
            with += (v - s.measured).abs();
        }
        let n = t.len() as f64;
        out.push((nuc, t.len(), rules / n, with / n, covered as f64 / n));
    }
    out
}
