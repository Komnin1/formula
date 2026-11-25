//! Protein, DNA and RNA sequences: reading and writing FASTA text, checking letters,
//! complement / transcription / translation, masses and formulas, the layout used to show a
//! sequence in the drawing, and peptides turned into structures.

use crate::chem::{self, Formula};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SeqKind {
    #[default]
    Protein,
    Dna,
    Rna,
}

impl SeqKind {
    pub fn name(self) -> &'static str {
        match self {
            SeqKind::Protein => "Protein",
            SeqKind::Dna => "DNA",
            SeqKind::Rna => "RNA",
        }
    }
    /// Letters that may appear (IUPAC codes, gap "-" and stop "*").
    pub fn alphabet(self) -> &'static str {
        match self {
            SeqKind::Protein => "ACDEFGHIKLMNPQRSTVWYUOBZJX*-",
            SeqKind::Dna => "ACGTRYSWKMBDHVN-",
            SeqKind::Rna => "ACGURYSWKMBDHVN-",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Record {
    pub header: String,
    pub seq: String,
}

/// FASTA text to records. Text without a ">" line is one record with no header.
pub fn parse_fasta(text: &str) -> Vec<Record> {
    let mut out: Vec<Record> = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if let Some(h) = l.strip_prefix('>') {
            out.push(Record { header: h.trim().to_string(), seq: String::new() });
        } else if l.starts_with(';') || l.is_empty() {
            continue;
        } else {
            if out.is_empty() {
                out.push(Record::default());
            }
            let last = out.last_mut().unwrap();
            last.seq.extend(l.chars().filter(|c| !c.is_whitespace() && !c.is_ascii_digit()));
        }
    }
    out
}

/// Records as FASTA text, sequence lines `width` letters long.
pub fn to_fasta(records: &[Record], width: usize) -> String {
    let mut s = String::new();
    for r in records {
        s.push('>');
        s.push_str(if r.header.is_empty() { "sequence" } else { &r.header });
        s.push('\n');
        let ch: Vec<char> = r.seq.chars().collect();
        for chunk in ch.chunks(width.max(1)) {
            s.extend(chunk);
            s.push('\n');
        }
    }
    s
}

/// A caption read from a file is a sequence when it is FASTA: a ">" header line followed by
/// lines of sequence letters only.
pub fn detect(text: &str) -> Option<SeqKind> {
    let t = text.trim_start();
    if !t.starts_with('>') {
        return None;
    }
    let body: String = t.lines().filter(|l| !l.trim_start().starts_with('>')).collect::<Vec<_>>().join("");
    let body: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    if body.is_empty() || !body.chars().all(|c| c.is_ascii_uppercase() || c == '*' || c == '-') {
        return None;
    }
    let kind = guess_kind(&body);
    body.chars().all(|c| kind.alphabet().contains(c)).then_some(kind)
}

/// Upper-case letters only; also returns the letters that don't belong to `kind`.
pub fn clean(raw: &str, kind: SeqKind) -> (String, Vec<char>) {
    let mut bad: Vec<char> = Vec::new();
    let mut s = String::new();
    for c in raw.chars().filter(|c| !c.is_whitespace() && !c.is_ascii_digit()) {
        let u = c.to_ascii_uppercase();
        if kind.alphabet().contains(u) {
            s.push(u);
        } else if !bad.contains(&u) {
            bad.push(u);
        }
    }
    (s, bad)
}

/// DNA, RNA or protein, judged from the letters.
pub fn guess_kind(seq: &str) -> SeqKind {
    let letters: Vec<char> = seq.chars().filter(|c| c.is_ascii_alphabetic()).map(|c| c.to_ascii_uppercase()).collect();
    if letters.is_empty() {
        return SeqKind::Protein;
    }
    let nuc = letters.iter().filter(|c| "ACGTUN".contains(**c)).count();
    if nuc * 10 >= letters.len() * 9 {
        let t = letters.iter().filter(|&&c| c == 'T').count();
        let u = letters.iter().filter(|&&c| c == 'U').count();
        if u > t { SeqKind::Rna } else { SeqKind::Dna }
    } else {
        SeqKind::Protein
    }
}

fn complement_base(c: char, rna: bool) -> char {
    match c {
        'A' => if rna { 'U' } else { 'T' },
        'T' | 'U' => 'A',
        'G' => 'C',
        'C' => 'G',
        'R' => 'Y',
        'Y' => 'R',
        'K' => 'M',
        'M' => 'K',
        'B' => 'V',
        'V' => 'B',
        'D' => 'H',
        'H' => 'D',
        x => x,
    }
}

pub fn reverse_complement(seq: &str, kind: SeqKind) -> String {
    seq.chars().rev().map(|c| complement_base(c, kind == SeqKind::Rna)).collect()
}

pub fn complement(seq: &str, kind: SeqKind) -> String {
    seq.chars().map(|c| complement_base(c, kind == SeqKind::Rna)).collect()
}

pub fn transcribe(dna: &str) -> String {
    dna.chars().map(|c| if c == 'T' { 'U' } else { c }).collect()
}

pub fn back_transcribe(rna: &str) -> String {
    rna.chars().map(|c| if c == 'U' { 'T' } else { c }).collect()
}

/// The standard genetic code, codons ordered T, C, A, G at each position.
const CODE: &[u8; 64] = b"FFLLSSSSYY**CC*WLLLLPPPPHHQQRRRRIIIMTTTTNNKKSSRRVVVVAAAADDEEGGGG";

/// Translate DNA or RNA (reading frame 1) with the standard code; "*" marks stops, "X" unknown codons.
pub fn translate(nuc: &str) -> String {
    let idx = |c: char| match c {
        'T' | 'U' => Some(0),
        'C' => Some(1),
        'A' => Some(2),
        'G' => Some(3),
        _ => None,
    };
    let ch: Vec<char> = nuc.chars().filter(|c| *c != '-').collect();
    ch.chunks_exact(3)
        .map(|c| match (idx(c[0]), idx(c[1]), idx(c[2])) {
            (Some(a), Some(b), Some(d)) => CODE[a * 16 + b * 4 + d] as char,
            _ => 'X',
        })
        .collect()
}

/// Amino-acid residues as they sit in a chain (amino acid minus water).
fn residue_formula(c: char, kind: SeqKind) -> Option<&'static str> {
    Some(match kind {
        SeqKind::Protein => match c {
            'A' => "C3H5NO",
            'R' => "C6H12N4O",
            'N' => "C4H6N2O2",
            'D' => "C4H5NO3",
            'C' => "C3H5NOS",
            'E' => "C5H7NO3",
            'Q' => "C5H8N2O2",
            'G' => "C2H3NO",
            'H' => "C6H7N3O",
            'I' | 'L' => "C6H11NO",
            'K' => "C6H12N2O",
            'M' => "C5H9NOS",
            'F' => "C9H9NO",
            'P' => "C5H7NO",
            'S' => "C3H5NO2",
            'T' => "C4H7NO2",
            'W' => "C11H10N2O",
            'Y' => "C9H9NO2",
            'V' => "C5H9NO",
            'U' => "C3H5NOSe",
            'O' => "C12H19N3O2",
            _ => return None,
        },
        // Nucleoside monophosphates minus water.
        SeqKind::Dna => match c {
            'A' => "C10H12N5O5P",
            'C' => "C9H12N3O6P",
            'G' => "C10H12N5O6P",
            'T' => "C10H13N2O7P",
            _ => return None,
        },
        SeqKind::Rna => match c {
            'A' => "C10H12N5O6P",
            'C' => "C9H12N3O7P",
            'G' => "C10H12N5O7P",
            'U' => "C9H11N2O8P",
            _ => return None,
        },
    })
}

fn add(f: &mut Formula, g: &Formula, times: i32) {
    for (k, v) in g {
        *f.entry(*k).or_default() += v * times;
    }
    f.retain(|_, v| *v != 0);
}

/// Formula of the whole chain: a peptide with free ends, or a single strand with 5′- and
/// 3′-OH ends. None when a letter has no fixed formula (X, N, gaps, stops…).
pub fn formula(seq: &str, kind: SeqKind) -> Option<Formula> {
    if seq.is_empty() {
        return None;
    }
    let mut f = Formula::new();
    for c in seq.chars() {
        add(&mut f, &chem::label_formula(residue_formula(c, kind)?)?, 1);
    }
    add(&mut f, &chem::label_formula("H2O")?, 1);
    if kind != SeqKind::Protein {
        add(&mut f, &chem::label_formula("HPO3")?, -1);
    }
    Some(f)
}

pub struct Stats {
    pub length: usize,
    pub counts: Vec<(char, usize)>,
    pub gc: Option<f64>,
    pub formula: Option<Formula>,
}

pub fn stats(seq: &str, kind: SeqKind) -> Stats {
    let mut counts: Vec<(char, usize)> = Vec::new();
    for c in seq.chars() {
        match counts.iter_mut().find(|x| x.0 == c) {
            Some(x) => x.1 += 1,
            None => counts.push((c, 1)),
        }
    }
    counts.sort();
    let gc = if kind != SeqKind::Protein && !seq.is_empty() {
        let n = seq.chars().filter(|c| "GCS".contains(*c)).count();
        Some(n as f64 * 100.0 / seq.chars().filter(|c| *c != '-').count().max(1) as f64)
    } else {
        None
    };
    Stats { length: seq.chars().filter(|c| *c != '-').count(), counts, gc, formula: formula(seq, kind) }
}

// ---------------------------------------------------------------------------
// Showing a sequence in the drawing

pub const PER_LINE: usize = 50;
pub const GROUP: usize = 10;

/// One shown line: the residue number (empty for headers) and the text.
pub struct ShownLine {
    pub number: String,
    pub text: String,
    pub header: bool,
}

/// The caption text (FASTA) laid out as headers and numbered lines of 50 letters in blocks of 10.
pub fn shown_lines(fasta: &str) -> Vec<ShownLine> {
    let mut out = Vec::new();
    let recs = parse_fasta(fasta);
    let width = recs.iter().map(|r| r.seq.chars().count()).max().unwrap_or(1).to_string().len().max(3);
    for r in recs {
        if !r.header.is_empty() {
            out.push(ShownLine { number: String::new(), text: r.header.clone(), header: true });
        }
        let ch: Vec<char> = r.seq.chars().collect();
        for (k, line) in ch.chunks(PER_LINE).enumerate() {
            let text = line.chunks(GROUP).map(|g| g.iter().collect::<String>()).collect::<Vec<_>>().join(" ");
            out.push(ShownLine { number: format!("{:>w$} ", k * PER_LINE + 1, w = width), text, header: false });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Peptides as structures

/// Side chains written from Cβ; {a} and {b} become ring numbers.
fn side_chain(c: char) -> Option<&'static str> {
    Some(match c {
        'A' => "C",
        'R' => "CCCNC(N)=N",
        'N' => "CC(N)=O",
        'D' => "CC(O)=O",
        'C' => "CS",
        'E' => "CCC(O)=O",
        'Q' => "CCC(N)=O",
        'H' => "CC{a}=CNC=N{a}",
        'I' => "[C@@H](C)CC",
        'L' => "CC(C)C",
        'K' => "CCCCN",
        'M' => "CCSC",
        'F' => "CC{a}=CC=CC=C{a}",
        'S' => "CO",
        'T' => "[C@H](O)C",
        'W' => "CC{a}=CNC{b}=CC=CC=C{a}{b}",
        'Y' => "CC{a}=CC=C(O)C=C{a}",
        'V' => "C(C)C",
        'U' => "C[SeH]",
        _ => return None,
    })
}

/// SMILES of a linear peptide of L-amino acids (free amine and acid ends).
pub fn peptide_smiles(seq: &str) -> Option<String> {
    let res: Vec<Residue> = seq.chars().map(|c| Residue { code: c, d: false }).collect();
    peptide_smiles_of(&res, false)
}

/// One residue of a peptide: its one-letter code and whether it is the D form.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Residue {
    pub code: char,
    pub d: bool,
}

/// Residues as typed: a lower-case letter is the D-amino acid ("fwKt": D-Phe, D-Trp, L-Lys,
/// D-Thr). Glycine has no D form.
pub fn peptide_residues(raw: &str) -> Vec<Residue> {
    raw.chars()
        .filter(|c| c.is_ascii_alphabetic() || *c == '*')
        .map(|c| {
            let code = c.to_ascii_uppercase();
            Residue { code, d: c.is_ascii_lowercase() && code != 'G' }
        })
        .collect()
}

/// Whether any residue is written as a D-amino acid.
pub fn has_d(raw: &str) -> bool {
    peptide_residues(raw).iter().any(|r| r.d)
}

/// SMILES of a peptide of L- and D-amino acids: linear with free ends, or cyclic (head to
/// tail, the last carbonyl bonded to the first amine).
pub fn peptide_smiles_of(res: &[Residue], cyclic: bool) -> Option<String> {
    let mut s = String::new();
    let n = res.len();
    // Every side-chain ring closes inside its residue, so the same two ring numbers serve
    // them all; %99 closes the macrocycle.
    for (k, r) in res.iter().enumerate() {
        let mut frag = match r.code {
            'G' => "NCC(=O)".to_string(),
            'P' => "N%11[C@@H](CCC%11)C(=O)".to_string(),
            c => {
                let side = side_chain(c)?.replace("{a}", "%11").replace("{b}", "%12");
                format!("N[C@@H]({})C(=O)", side)
            }
        };
        if r.d {
            frag = crate::templates::mirror_smiles(&frag);
        }
        if cyclic && n >= 2 {
            if k == 0 {
                frag = format!("N%99{}", &frag[1..]);
            }
            if k == n - 1 {
                let cut = frag.len() - "C(=O)".len();
                frag = format!("{}C%99(=O)", &frag[..cut]);
            }
        }
        s.push_str(&frag);
    }
    if s.is_empty() {
        return None;
    }
    if !(cyclic && n >= 2) {
        s.push('O');
    }
    Some(s)
}

/// Formula of a peptide, linear (with water for the free ends) or cyclic.
pub fn peptide_formula(res: &[Residue], cyclic: bool) -> Option<Formula> {
    let mut f = Formula::new();
    for r in res {
        add(&mut f, &chem::label_formula(residue_formula(r.code, SeqKind::Protein)?)?, 1);
    }
    if res.is_empty() {
        return None;
    }
    if !(cyclic && res.len() >= 2) {
        add(&mut f, &chem::label_formula("H2O")?, 1);
    }
    Some(f)
}

/// The peptide as a chain of monomers (as the original's HELM editor draws it): coloured
/// circles with their codes ("dF" for D-phenylalanine), H- and -OH caps on a linear chain, or a
/// ring for a cyclic peptide. Long linear chains run back and forth in rows of ten.
pub fn peptide_monomers(res: &[Residue], cyclic: bool) -> crate::doc::Doc {
    use crate::doc::*;
    use egui::{pos2, vec2};
    let mut d = Doc::default();
    let n = res.len();
    if n == 0 {
        return d;
    }
    let step = BOND_LEN * 1.3;
    let cyclic = cyclic && n >= 3;
    let pos: Vec<egui::Pos2> = if cyclic {
        let r = n as f32 * step / std::f32::consts::TAU;
        (0..n)
            .map(|k| {
                let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::TAU * k as f32 / n as f32;
                pos2(r * a.cos(), r * a.sin())
            })
            .collect()
    } else {
        (0..n)
            .map(|k| {
                let (row, col) = (k / 10, k % 10);
                let x = if row % 2 == 0 { col } else { 9 - col };
                pos2(step * (x as f32 + 1.0), step * 1.6 * row as f32)
            })
            .collect()
    };
    let name_of = |c: char| crate::templates::AMINO_ACIDS.iter().find(|a| a.2 == c).map(|a| a.0.to_string()).unwrap_or_else(|| match c {
        'U' => "Selenocysteine".into(),
        'O' => "Pyrrolysine".into(),
        _ => c.to_string(),
    });
    let mut ids = Vec::new();
    for (k, r) in res.iter().enumerate() {
        let symbol = if r.d { format!("d{}", r.code) } else { r.code.to_string() };
        let name = if r.d { format!("D-{}", name_of(r.code)) } else { name_of(r.code) };
        let mut a = Atom::new(pos[k], 0);
        a.label = Some(symbol.clone());
        a.monomer = Some(Box::new(Monomer {
            role: MonomerRole::Unit,
            polymer: Polymer::Peptide,
            branch: false,
            symbol,
            name,
            analog: r.code.to_string(),
            formula: residue_formula(r.code, SeqKind::Protein).unwrap_or("").to_string(),
        }));
        d.atoms.push(a);
        ids.push(d.atoms.len() - 1);
    }
    for k in 1..n {
        d.add_bond(ids[k - 1], ids[k], 1, Stereo::None);
    }
    if cyclic {
        d.add_bond(ids[n - 1], ids[0], 1, Stereo::None);
    } else {
        // Caps: H- on the amine end, -OH on the acid end, pointing away along the chain.
        let cap = |d: &mut Doc, at: usize, dir: egui::Vec2, text: &str| {
            let mut a = Atom::new(d.atoms[at].pos + dir * step, 0);
            a.label = Some(text.into());
            a.monomer = Some(Box::new(Monomer { role: MonomerRole::Terminus, symbol: text.into(), ..Default::default() }));
            d.atoms.push(a);
            let c = d.atoms.len() - 1;
            d.add_bond(at, c, 1, Stereo::None);
        };
        cap(&mut d, ids[0], vec2(-1.0, 0.0), "H");
        let last_row = (n - 1) / 10;
        cap(&mut d, ids[n - 1], vec2(if last_row % 2 == 0 { 1.0 } else { -1.0 }, 0.0), "OH");
    }
    d
}

/// A DNA or RNA strand as HELM monomers, as the original's HELM editor draws it: a backbone of
/// sugars (R ribose or dR deoxyribose, heptagons) joined by phosphates (P, diamonds), each sugar
/// carrying its base (A, C, G, T/U, trapezoids) below. Formulas are the original's (ribose
/// C5H7O4, deoxyribose C5H7O3, phosphate HO2P, bases without their H); the 5′ and 3′ ends carry
/// hidden H caps, so the formula is the strand's (adenosine for a lone "A" in RNA). Long strands
/// run back and forth in rows of ten nucleotides. Letters other than A, C, G, T and U are skipped.
pub fn nucleic_monomers(seq: &str, kind: SeqKind) -> crate::doc::Doc {
    use crate::doc::*;
    use egui::{pos2, vec2};
    let mut d = Doc::default();
    let rna = kind == SeqKind::Rna;
    let bases: Vec<char> = seq
        .chars()
        .map(|c| c.to_ascii_uppercase())
        .filter_map(|c| match c {
            'T' if rna => Some('U'),
            'U' if !rna => Some('T'),
            'A' | 'C' | 'G' | 'T' | 'U' => Some(c),
            _ => None,
        })
        .collect();
    let n = bases.len();
    if n == 0 {
        return d;
    }
    let step = BOND_LEN * 1.3;
    let unit = |d: &mut Doc, at: egui::Pos2, symbol: &str, name: &str, analog: &str, formula: &str, branch: bool| -> usize {
        let mut a = Atom::new(at, 0);
        a.label = Some(symbol.to_string());
        a.monomer = Some(Box::new(Monomer {
            role: MonomerRole::Unit,
            polymer: Polymer::Nucleic,
            branch,
            symbol: symbol.into(),
            name: name.into(),
            analog: analog.into(),
            formula: formula.into(),
        }));
        d.atoms.push(a);
        d.atoms.len() - 1
    };
    let cap = |d: &mut Doc, at: usize, dir: egui::Vec2, text: &str| {
        let mut a = Atom::new(d.atoms[at].pos + dir * step * 0.6, 0);
        a.label = Some(text.into());
        a.monomer = Some(Box::new(Monomer { role: MonomerRole::Branch, polymer: Polymer::Nucleic, symbol: text.into(), ..Default::default() }));
        d.atoms.push(a);
        let c = d.atoms.len() - 1;
        d.add_bond(at, c, 1, Stereo::None);
    };
    let (sugar, sugar_name, sugar_formula) = if rna { ("R", "Ribose", "C5H7O4") } else { ("dR", "Deoxyribose", "C5H7O3") };
    let mut prev: Option<usize> = None;
    let mut first = 0;
    let mut last_dir = vec2(1.0, 0.0);
    for (k, &b) in bases.iter().enumerate() {
        // Each nucleotide takes two places along the row: its sugar, then the phosphate.
        let (row, col) = (k / 10, k % 10);
        let right = row % 2 == 0;
        let x = |slot: f32| if right { step * slot } else { step * (19.0 - slot) };
        let y = step * 2.8 * row as f32;
        let s = unit(&mut d, pos2(x(2.0 * col as f32), y), sugar, sugar_name, "R", sugar_formula, false);
        if let Some(p) = prev {
            d.add_bond(p, s, 1, Stereo::None);
        } else {
            first = s;
        }
        let (name, formula) = match b {
            'A' => ("Adenine", "C5H4N5"),
            'C' => ("Cytosine", "C4H4N3O"),
            'G' => ("Guanine", "C5H4N5O"),
            'T' => ("Thymine", "C5H5N2O2"),
            _ => ("Uracil", "C4H3N2O2"),
        };
        let code = b.to_string();
        let base = unit(&mut d, pos2(x(2.0 * col as f32), y + step), &code, name, &code, formula, true);
        d.add_bond(s, base, 1, Stereo::None);
        last_dir = vec2(if right { 1.0 } else { -1.0 }, 0.0);
        if k + 1 < n {
            // The phosphate to the next nucleotide (at a row's end, above the next row's first
            // sugar, so the strand turns down without crossing a base).
            let p = unit(&mut d, pos2(x(2.0 * col as f32 + 1.0), y), "P", "Phosphate", "P", "HO2P", false);
            d.add_bond(s, p, 1, Stereo::None);
            prev = Some(p);
        } else {
            prev = Some(s);
        }
    }
    // 5′-OH and 3′-OH: hidden H caps.
    cap(&mut d, first, vec2(-1.0, 0.0), "H");
    if let Some(l) = prev {
        cap(&mut d, l, last_dir, "H");
    }
    d
}

// ---------------------------------------------------------------------------
// FASTA files

/// A sequence block for the drawing.
pub fn caption(rec: &Record, kind: SeqKind, pos: egui::Pos2, size: f32) -> crate::doc::Caption {
    crate::doc::Caption {
        pos,
        text: to_fasta(std::slice::from_ref(rec), 60),
        size,
        bold: false,
        italic: false,
        serif: false,
        formula: false,
        color: None,
        extra: None,
        seq: Some(kind),
    }
}

/// A FASTA file as a drawing: one sequence block per record, one under the other.
pub fn doc_from_fasta(text: &str) -> Result<crate::doc::Doc, String> {
    let recs: Vec<Record> = parse_fasta(text).into_iter().filter(|r| !r.seq.is_empty()).collect();
    if recs.is_empty() {
        return Err("no sequences found in the file".into());
    }
    let mut d = crate::doc::Doc::default();
    let size = d.style.caption_size.max(8.0);
    let mut y = 0.0;
    for r in recs {
        let kind = guess_kind(&r.seq);
        let c = caption(&r, kind, egui::pos2(0.0, y), size);
        y = crate::doc::caption_rect(&c).max.y + size * 1.5;
        d.captions.push(c);
    }
    Ok(d)
}

/// Every sequence block in the drawing, as FASTA.
pub fn doc_to_fasta(d: &crate::doc::Doc) -> Result<String, String> {
    let recs: Vec<Record> = d.captions.iter().filter(|c| c.seq.is_some()).flat_map(|c| parse_fasta(&c.text)).collect();
    if recs.is_empty() {
        return Err("the drawing has no sequences (add one in the Sequence panel)".into());
    }
    Ok(to_fasta(&recs, 60))
}
