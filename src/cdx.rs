//! CDX documents: the binary format (.cdx, .ctp stationery) and the XML format (.cdxml).
//! Anything the editor doesn't understand is kept and written back on save.

use crate::chem;
use crate::doc::*;
use crate::elements;
use crate::xml::{self, Child, Node};
use egui::{Pos2, pos2, vec2};
use std::collections::{HashMap, HashSet};

// Object tags
const T_DOCUMENT: u16 = 0x8000;
const T_PAGE: u16 = 0x8001;
const T_GROUP: u16 = 0x8002;
const T_FRAGMENT: u16 = 0x8003;
const T_NODE: u16 = 0x8004;
const T_BOND: u16 = 0x8005;
const T_TEXT: u16 = 0x8006;
const T_GRAPHIC: u16 = 0x8007;
const T_ARROW: u16 = 0x8021;
/// The arrow object that replaces an older line graphic.
const P_SUPERSEDED: u16 = 0x0013;

// Property tags
const P_FONTTABLE: u16 = 0x0100;
const P_POS2D: u16 = 0x0200;
const P_BBOX: u16 = 0x0204;
const P_HEAD3D: u16 = 0x0207;
const P_TAIL3D: u16 = 0x0208;
const P_CENTER3D: u16 = 0x020D;
const P_MAJOR3D: u16 = 0x020E;
const P_MINOR3D: u16 = 0x020F;
const P_COLORTABLE: u16 = 0x0300;
const P_FGCOLOR: u16 = 0x0301;
const P_NODETYPE: u16 = 0x0400;
/// How a node connects in a biopolymer (6 peptide, 8 RNA, 9 terminus, 12 unlinked branch).
const P_EXT_CONNECTION: u16 = 0x0440;
/// Tags on objects (monomer symbol, name, formula…): type (2 number, 3 text), name, value.
const P_TAG_TYPE: u16 = 0x0D00;
const P_TAG_NAME: u16 = 0x0008;
const P_TAG_VALUE: u16 = 0x0D05;
const P_ELEMENT: u16 = 0x0402;
const P_ISOTOPE: u16 = 0x0420;
const P_CHARGE: u16 = 0x0421;
const P_RADICAL: u16 = 0x0422;
const P_NUMH: u16 = 0x042B;
const P_ORDER: u16 = 0x0600;
const P_DISPLAY: u16 = 0x0601;
const P_DBLPOS: u16 = 0x0603;
const P_BEGIN: u16 = 0x0604;
const P_END: u16 = 0x0605;
const P_TEXT: u16 = 0x0700;
const P_GRAPHICTYPE: u16 = 0x0A00;
/// A graphic's symbol kind (8 = plus, 9 = minus) and the atom property it stands for.
const P_SYMBOLTYPE: u16 = 0x0A07;
const P_REPRESENTS: u16 = 0x000E;
const P_ARROWTYPE: u16 = 0x0A02;
const P_RECTTYPE: u16 = 0x0A03;
// Document drawing settings (tag numbers from the published CDX specification)
const P_CHAINANGLE: u16 = 0x0803;
const P_BONDSPACING: u16 = 0x0804; // INT16, tenths of a percent of the bond length
const P_BONDLENGTH: u16 = 0x0805;
const P_BOLDWIDTH: u16 = 0x0806;
const P_LINEWIDTH: u16 = 0x0807;
const P_MARGINWIDTH: u16 = 0x0808;
const P_HASHSPACING: u16 = 0x0809;
const P_LABELSTYLE: u16 = 0x080A; // font, face, size (1/20 pt), colour
const P_CAPTIONSTYLE: u16 = 0x080B;
const P_WIDTHPAGES: u16 = 0x080F;
const P_HEIGHTPAGES: u16 = 0x0810;
const P_NAME: u16 = 0x0008;
// Brackets
const P_BRACKETTYPE: u16 = 0x0A06;
const P_BRACKETUSAGE: u16 = 0x0A24;
const P_BRACKETED: u16 = 0x0A27;
const P_REPEATCOUNT: u16 = 0x0A28;
const P_SRULABEL: u16 = 0x0A2A;
const P_GRAPHICID: u16 = 0x0A2B;
const P_BONDID: u16 = 0x0A2C;
const P_INNERATOM: u16 = 0x0A2D;
const P_TAGTYPE: u16 = 0x0D00;
const P_TAGVALUE: u16 = 0x0D05;
const T_OBJECTTAG: u16 = 0x8011;
const T_BRACKETGROUP: u16 = 0x8017;
const T_BRACKETATTACH: u16 = 0x8018;
const T_CROSSINGBOND: u16 = 0x8019;
/// Usable page area (A4 less 0.5 inch margins), as in documents from other editors.

const FIX: f32 = 65536.0;
pub const HEADER: &[u8] = b"VjCD0100";
const FROM_ARROW_OBJECT: u32 = 1;

#[derive(Clone, Debug)]
struct CObj {
    tag: u16,
    id: u32,
    props: Vec<(u16, Vec<u8>)>,
    children: Vec<CObj>,
    raw: Vec<u8>,
}

impl CObj {
    fn prop(&self, t: u16) -> Option<&[u8]> {
        self.props.iter().find(|p| p.0 == t).map(|p| p.1.as_slice())
    }
    fn i16(&self, t: u16) -> Option<i16> {
        self.prop(t).filter(|d| d.len() >= 2).map(|d| i16::from_le_bytes([d[0], d[1]]))
    }
    fn u32(&self, t: u16) -> Option<u32> {
        self.prop(t).filter(|d| d.len() >= 4).map(|d| u32::from_le_bytes([d[0], d[1], d[2], d[3]]))
    }
    fn int(&self, t: u16) -> Option<i32> {
        self.prop(t).map(|d| match d.len() {
            1 => d[0] as i8 as i32,
            2 | 3 => i16::from_le_bytes([d[0], d[1]]) as i32,
            n if n >= 4 => i32::from_le_bytes([d[0], d[1], d[2], d[3]]),
            _ => 0,
        })
    }
}

/// Formula 0.0.1 wrote its drawing settings under the wrong tags (some of them belong to
/// printer settings); move them to the right ones.
fn fix_legacy_style_props(props: &mut Vec<(u16, Vec<u8>)>) {
    let old: Vec<(u16, Vec<u8>)> = props.iter().filter(|p| (0x0800..=0x0807).contains(&p.0)).cloned().collect();
    if old.is_empty() {
        return;
    }
    props.retain(|p| !(0x0800..=0x0807).contains(&p.0));
    for (t, d) in old {
        let new = match (t, d.len()) {
            (0x0800, 4) => P_BONDLENGTH,
            (0x0801, 2) => P_BONDSPACING,
            (0x0804, 4) => P_LINEWIDTH,
            (0x0805, 4) => P_BOLDWIDTH,
            (0x0806, 4) => P_HASHSPACING,
            (0x0807, 4) => P_MARGINWIDTH,
            _ => continue,
        };
        props.push((new, d));
    }
}

fn rd_i32(d: &[u8], k: usize) -> f32 {
    if d.len() < k * 4 + 4 {
        return 0.0;
    }
    i32::from_le_bytes([d[k * 4], d[k * 4 + 1], d[k * 4 + 2], d[k * 4 + 3]]) as f32 / FIX
}

fn parse_obj(buf: &[u8], pos: &mut usize, tag: u16, start: usize, depth: usize) -> Result<CObj, String> {
    if depth > 200 {
        return Err("file nests objects too deeply".into());
    }
    if *pos + 4 > buf.len() {
        return Err("file ends early".into());
    }
    let id = u32::from_le_bytes([buf[*pos], buf[*pos + 1], buf[*pos + 2], buf[*pos + 3]]);
    *pos += 4;
    let mut o = CObj { tag, id, props: vec![], children: vec![], raw: vec![] };
    loop {
        if *pos + 2 > buf.len() {
            break; // tolerate a missing final terminator
        }
        let t = u16::from_le_bytes([buf[*pos], buf[*pos + 1]]);
        let child_start = *pos;
        *pos += 2;
        if t == 0 {
            break;
        }
        if t & 0x8000 != 0 {
            let c = parse_obj(buf, pos, t, child_start, depth + 1)?;
            o.children.push(c);
        } else {
            if *pos + 2 > buf.len() {
                return Err("file ends early".into());
            }
            let mut len = u16::from_le_bytes([buf[*pos], buf[*pos + 1]]) as usize;
            *pos += 2;
            if len == 0xFFFF {
                if *pos + 4 > buf.len() {
                    return Err("file ends early".into());
                }
                len = u32::from_le_bytes([buf[*pos], buf[*pos + 1], buf[*pos + 2], buf[*pos + 3]]) as usize;
                *pos += 4;
            }
            if *pos + len > buf.len() {
                return Err("a property runs past the end of the file".into());
            }
            o.props.push((t, buf[*pos..*pos + len].to_vec()));
            *pos += len;
        }
    }
    o.raw = buf[start..(*pos).min(buf.len())].to_vec();
    Ok(o)
}

/// Text of a CDX string property (style runs, then the characters) and its first style.
fn cdx_string(d: &[u8]) -> (String, Option<(u16, u16, f32, u16)>) {
    if d.len() < 2 {
        return (String::new(), None);
    }
    let n = u16::from_le_bytes([d[0], d[1]]) as usize;
    let start = 2 + n * 10;
    let mut style = None;
    if n > 0 && d.len() >= 12 {
        let font = u16::from_le_bytes([d[4], d[5]]);
        let face = u16::from_le_bytes([d[6], d[7]]);
        let size = u16::from_le_bytes([d[8], d[9]]) as f32 / 20.0;
        let color = u16::from_le_bytes([d[10], d[11]]);
        style = Some((font, face, size, color));
    }
    let text: String = d.get(start..).unwrap_or(&[]).iter().map(|&b| b as char).collect();
    (text.replace('\r', "\n"), style)
}

fn make_cdx_string(text: &str, font: u16, face: u16, size: f32, color: u16) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&0u16.to_le_bytes());
    v.extend_from_slice(&font.to_le_bytes());
    v.extend_from_slice(&face.to_le_bytes());
    v.extend_from_slice(&((size * 20.0).round() as u16).to_le_bytes());
    v.extend_from_slice(&color.to_le_bytes());
    for ch in text.replace('\n', "\r").chars() {
        let c = ch as u32;
        v.push(if c < 256 { c as u8 } else if ch == '−' { b'-' } else { b'?' });
    }
    v
}

fn parse_colors(d: &[u8]) -> Vec<Rgb> {
    if d.len() < 2 {
        return vec![];
    }
    let n = u16::from_le_bytes([d[0], d[1]]) as usize;
    (0..n)
        .filter_map(|k| {
            let o = 2 + k * 6;
            if o + 6 > d.len() {
                return None;
            }
            let c = |j: usize| (u16::from_le_bytes([d[o + j], d[o + j + 1]]) >> 8) as u8;
            Some([c(0), c(2), c(4)])
        })
        .collect()
}

/// Colour index: 0 is black, 1 is white, 2 and up are entries of the colour table.
fn color_from_index(colors: &[Rgb], idx: i32) -> Option<Rgb> {
    match idx {
        1 => Some([255, 255, 255]),
        i if i >= 2 => colors.get((i - 2) as usize).copied().filter(|c| *c != [0, 0, 0]),
        _ => None,
    }
}

/// A font table: platform, count, then id, charset, name length and name for each font.
fn cdx_fonts(d: &[u8]) -> Vec<(u16, String)> {
    let mut out = Vec::new();
    if d.len() < 4 {
        return out;
    }
    let n = u16::from_le_bytes([d[2], d[3]]) as usize;
    let mut p = 4;
    for _ in 0..n {
        if p + 6 > d.len() {
            break;
        }
        let id = u16::from_le_bytes([d[p], d[p + 1]]);
        let len = u16::from_le_bytes([d[p + 4], d[p + 5]]) as usize;
        p += 6;
        if p + len > d.len() {
            break;
        }
        out.push((id, String::from_utf8_lossy(&d[p..p + len]).into_owned()));
        p += len;
    }
    out
}

fn xml_fonts(root: &Node) -> Vec<(u16, String)> {
    root.elements()
        .filter(|e| e.name == "fonttable")
        .flat_map(|t| t.elements().filter(|c| c.name == "font").collect::<Vec<_>>())
        .filter_map(|f| Some((f.attr("id")?.parse().ok()?, f.attr("name").unwrap_or("").to_string())))
        .collect()
}

/// Times New Roman and other fonts with serifs.
fn is_serif_font(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["times", "georgia", "garamond", "palatino", "cambria", "book antiqua", "century schoolbook"].iter().any(|k| n.contains(k))
        || (n.contains("serif") && !n.contains("sans"))
}

fn serif_ids(fonts: &[(u16, String)]) -> Vec<u16> {
    fonts.iter().filter(|f| is_serif_font(&f.1)).map(|f| f.0).collect()
}

/// Font ids to write: (sans-serif, serif). A file's own table is kept, so its fonts are used
/// when it has them; new files get Arial (3) and Times New Roman (4).
fn font_ids(fonts: Option<&[(u16, String)]>) -> (u16, u16) {
    match fonts {
        Some(f) if !f.is_empty() => {
            let sans = f.iter().find(|x| !is_serif_font(&x.1)).map(|x| x.0).unwrap_or(f[0].0);
            let serif = f.iter().find(|x| is_serif_font(&x.1)).map(|x| x.0).unwrap_or(sans);
            (sans, serif)
        }
        _ => (3, 4),
    }
}

/// Element and label from a node's label text ("NH2" -> N, "OMe" -> label).
pub fn interpret_label(text: &str) -> (u8, Option<String>) {
    let t = text.trim();
    if t.is_empty() {
        return (elements::C, None);
    }
    if let Some(toks) = chem::tokenize_label(t) {
        let all_elements = toks.iter().all(|x| elements::by_symbol(&x.text).is_some() && !chem::ABBREVIATIONS.iter().any(|a| a.0 == x.text));
        let heavy: Vec<&chem::LabelToken> = toks.iter().filter(|x| x.text != "H").collect();
        if all_elements && heavy.len() == 1 && heavy[0].count == 1 && toks.len() <= 2 {
            if let Some(z) = elements::by_symbol(&heavy[0].text) {
                return (z, None);
            }
        }
    }
    (chem::label_main_element(t), Some(t.to_string()))
}

#[derive(Default)]
struct Reader {
    doc: Doc,
    fx: FileExtra,
    id_map: HashMap<u32, usize>,
    pending_bonds: Vec<(CObj, u32)>,
    graphic_arrows: Vec<Arrow>,
    object_arrows: Vec<Arrow>,
    /// Line graphics replaced by an arrow object (graphic id → arrow id).
    superseded: HashMap<u32, u32>,
    max_id: u32,
    /// Bracket graphics, paired up when reading is done.
    sides: Vec<BracketSide>,
    groups: Vec<BracketGroupRec>,
    /// Ids of the file's fonts with serifs.
    serif_fonts: Vec<u16>,
    /// Font size (pt) of each atom label read from a binary file, to find scaled structures.
    label_pt: Vec<(usize, f32)>,
}

pub fn read_cdx(buf: &[u8]) -> Result<Doc, String> {
    if buf.len() < 24 || &buf[0..8] != HEADER {
        return Err("This is not a CDX file (its header is missing).".into());
    }
    // Header: 8-byte signature, 4-byte byte-order mark, then 16 bytes that normally hold
    // the document object's tag and id; its properties start at byte 28. If the bytes
    // there are zero, the document object is implied.
    let tag_at = |p: usize| if p + 2 <= buf.len() { u16::from_le_bytes([buf[p], buf[p + 1]]) } else { 0 };
    // Files from Formula 0.0.1 had the document object after 28 zero bytes.
    let legacy = tag_at(28) == T_DOCUMENT && buf.len() > 34 && buf[22..28].iter().all(|b| *b == 0);
    let start = if legacy { 28 } else { 22 };
    let mut pos = start + 2;
    if buf.len() < pos + 4 {
        return Err("The CDX document object was not found.".into());
    }
    let mut root = parse_obj(buf, &mut pos, T_DOCUMENT, start, 0)?;
    if legacy {
        fix_legacy_style_props(&mut root.props);
    }
    let mut r = Reader::default();
    r.fx.colors = root.prop(P_COLORTABLE).map(parse_colors).unwrap_or_default();
    r.serif_fonts = serif_ids(&root.prop(P_FONTTABLE).map(cdx_fonts).unwrap_or_default());
    for (t, d) in &root.props {
        if *t != P_COLORTABLE {
            r.fx.cdx_doc_props.push((*t, d.clone()));
        }
    }
    let mut first_page = true;
    for c in &root.children {
        if c.tag == T_PAGE && first_page {
            first_page = false;
            r.fx.cdx_page_props = c.props.clone();
            r.fx.max_id = c.id;
            r.fx.page_id = c.id;
            r.page_children(&c.children);
        } else {
            r.fx.cdx_doc_children.push(c.raw.clone());
        }
    }
    r.finish()
}

impl Reader {
    fn page_children(&mut self, kids: &[CObj]) {
        for c in kids {
            self.max_id = self.max_id.max(c.id);
            match c.tag {
                T_FRAGMENT => self.fragment(c),
                T_GROUP => self.page_children(&c.children),
                T_TEXT => self.caption(c),
                // A "+"/"−" symbol standing for an atom's charge (or radical): rebuilt from
                // the atom when saving, so it isn't kept twice.
                T_GRAPHIC if c.i16(P_GRAPHICTYPE) == Some(7) && c.prop(P_REPRESENTS).is_some_and(|d| d.len() >= 6 && matches!(u16::from_le_bytes([d[4], d[5]]), 0x0421 | 0x0422)) => {}
                T_GRAPHIC => {
                    if !self.graphic(c) {
                        self.fx.cdx_page_children.push(c.raw.clone());
                    }
                }
                T_ARROW => self.arrow_obj(c),
                T_BRACKETGROUP => {
                    let graphics = c
                        .children
                        .iter()
                        .filter(|a| a.tag == T_BRACKETATTACH)
                        .filter_map(|a| a.u32(P_GRAPHICID))
                        .collect();
                    let label = c
                        .prop(P_SRULABEL)
                        .and_then(|d| clean_label(&cdx_string(d).0))
                        .or_else(|| c.prop(P_REPEATCOUNT).filter(|d| d.len() == 8).and_then(|d| count_label(f64::from_le_bytes(d.try_into().unwrap()))));
                    self.groups.push(BracketGroupRec { id: c.id, graphics, label, raw: Raw::Cdx(c.raw.clone()) });
                }
                _ => self.fx.cdx_page_children.push(c.raw.clone()),
            }
        }
    }

    fn fragment(&mut self, f: &CObj) {
        let mut unknown = Vec::new();
        for c in &f.children {
            self.max_id = self.max_id.max(c.id);
            match c.tag {
                T_NODE => self.node(c, f.id),
                T_BOND => self.pending_bonds.push((c.clone(), f.id)),
                T_FRAGMENT => self.fragment(c),
                // Brackets drawn inside a structure are read like page brackets.
                T_GRAPHIC if c.i16(P_GRAPHICTYPE) == Some(6) && c.prop(P_BBOX).is_some() => {
                    self.graphic(c);
                }
                _ => unknown.push(c.raw.clone()),
            }
        }
        let props: Vec<(u16, Vec<u8>)> = f.props.iter().filter(|p| p.0 != P_BBOX).cloned().collect();
        self.fx.cdx_frags.push((f.id, props, unknown));
    }

    fn node(&mut self, n: &CObj, frag: u32) {
        let p = n.prop(P_POS2D).map(|d| pos2(rd_i32(d, 1), rd_i32(d, 0))).unwrap_or(Pos2::ZERO);
        let ntype = n.i16(P_NODETYPE).unwrap_or(1);
        let mut a = Atom::new(p, n.i16(P_ELEMENT).map(|z| z.clamp(0, 118) as u8).unwrap_or(elements::C));
        a.charge = n.int(P_CHARGE).unwrap_or(0).clamp(-9, 9) as i8;
        a.isotope = n.int(P_ISOTOPE).unwrap_or(0).max(0) as u16;
        a.radical = match n.int(P_RADICAL).unwrap_or(0) {
            2 => 1,
            1 | 3 => 2,
            _ => 0,
        };
        a.color = n.int(P_FGCOLOR).and_then(|i| color_from_index(&self.fx.colors, i));
        let label_text = n.children.iter().find(|c| c.tag == T_TEXT).and_then(|t| t.prop(P_TEXT)).map(|d| cdx_string(d).0);
        if let Some(pt) = n.children.iter().find(|c| c.tag == T_TEXT).and_then(|t| t.prop(P_TEXT)).and_then(|d| cdx_string(d).1).map(|s| s.2).filter(|v| *v > 0.5) {
            self.label_pt.push((self.doc.atoms.len(), pt));
        }
        if ntype == 12 {
            a.el = 0;
            a.label = Some("*".into());
        } else if ntype != 1 {
            if let Some(t) = &label_text {
                let (z, l) = interpret_label(t);
                a.el = z;
                a.label = l.or_else(|| if ntype >= 4 { Some(t.trim().to_string()) } else { None });
            }
        } else if a.el == elements::C && label_text.as_deref().is_some_and(|t| !t.trim().is_empty()) {
            // A carbon written out ("CH3"): keep showing it, the H count following its bonds.
            a.show = true;
        }
        // A monomer of a biopolymer (node type 14), or a cap of its chain (connection type 9:
        // terminus, 12: unlinked branch): its code is not an element.
        let connection = match n.prop(P_EXT_CONNECTION).and_then(|d| d.first().copied()) {
            Some(6) => "Peptide",
            Some(8) => "RNA",
            Some(9) => "Terminus",
            Some(12) => "UnlinkedBranch",
            _ => "",
        };
        let node_kind = if ntype == 14 { "Monomer" } else { "" };
        if !node_kind.is_empty() || matches!(connection, "Terminus" | "UnlinkedBranch") {
            let tags: HashMap<String, String> = n
                .children
                .iter()
                .filter(|c| c.tag == T_OBJECTTAG)
                .filter_map(|c| {
                    let name = String::from_utf8_lossy(c.prop(P_TAG_NAME)?).into_owned();
                    let v = c.prop(P_TAG_VALUE).unwrap_or(&[]);
                    let value = match c.i16(P_TAG_TYPE) {
                        Some(2) if v.len() >= 4 => i32::from_le_bytes([v[0], v[1], v[2], v[3]]).to_string(),
                        _ => cdx_string(v).0,
                    };
                    Some((name, value))
                })
                .collect();
            if let Some(m) = monomer_of(node_kind, connection, &tags, label_text.as_deref().unwrap_or("")) {
                a.el = 0;
                a.show = false;
                a.label = Some(if m.role == MonomerRole::Unit { m.symbol.clone() } else { label_text.as_deref().unwrap_or("H").trim().to_string() });
                a.monomer = Some(Box::new(m));
            }
        }
        // A group label ("P(Ph)3"): ChemDraw keeps its charge on the atoms of the group's own
        // hidden structure, so the label takes their total.
        if a.label.is_some() && a.charge == 0 && a.monomer.is_none() {
            let inside: i32 = n
                .children
                .iter()
                .filter(|c| c.tag == T_FRAGMENT)
                .flat_map(|f| f.children.iter())
                .filter(|c| c.tag == T_NODE)
                .filter_map(|c| c.int(P_CHARGE))
                .sum();
            a.charge = inside.clamp(-9, 9) as i8;
        }
        let text_id = n.children.iter().find(|c| c.tag == T_TEXT).map(|t| t.id).unwrap_or(0);
        let mut extra = Extra { id: n.id, frag, source_label: a.label.clone(), text_id, ..Default::default() };
        let handled = [P_POS2D, P_NODETYPE, P_ELEMENT, P_CHARGE, P_ISOTOPE, P_RADICAL, P_FGCOLOR, P_NUMH, P_BBOX];
        extra.cdx_props = n.props.iter().filter(|p| !handled.contains(&p.0)).cloned().collect();
        for c in &n.children {
            if c.tag != T_TEXT {
                extra.cdx_children.push(c.raw.clone());
            }
        }
        self.id_map.insert(n.id, self.doc.atoms.len());
        a.extra = Some(Box::new(extra));
        self.doc.atoms.push(a);
    }

    fn caption(&mut self, t: &CObj) {
        let Some(d) = t.prop(P_TEXT) else {
            self.fx.cdx_page_children.push(t.raw.clone());
            return;
        };
        let (text, style) = cdx_string(d);
        let (font, face, size, color) = style.unwrap_or((0, 0, 10.0, 0));
        let base = t.prop(P_POS2D).map(|d| pos2(rd_i32(d, 1), rd_i32(d, 0))).unwrap_or(Pos2::ZERO);
        let size = if size > 0.5 { size } else { 10.0 };
        self.doc.captions.push(Caption {
            pos: pos2(base.x, base.y - size * 0.8),
            text: text.clone(),
            size,
            bold: face & 1 != 0,
            italic: face & 2 != 0,
            serif: self.serif_fonts.contains(&(font as u16)),
            formula: face & 0x60 == 0x60,
            color: color_from_index(&self.fx.colors, color as i32),
            seq: crate::sequence::detect(&text),
            extra: Some(Box::new(Extra {
                id: t.id,
                cdx_props: t.props.iter().filter(|p| p.0 != P_POS2D && p.0 != P_BBOX).cloned().collect(),
                cdx_children: t.children.iter().map(|c| c.raw.clone()).collect(),
                source_label: Some(text),
                ..Default::default()
            })),
        });
    }

    fn graphic(&mut self, g: &CObj) -> bool {
        let gt = g.i16(P_GRAPHICTYPE).unwrap_or(0);
        let Some(bb) = g.prop(P_BBOX) else { return false };
        // Stored as top, left, bottom, right; for lines the first point is the head.
        let p1 = pos2(rd_i32(bb, 1), rd_i32(bb, 0));
        let p2 = pos2(rd_i32(bb, 3), rd_i32(bb, 2));
        let handled = [P_BBOX, P_GRAPHICTYPE, P_ARROWTYPE, P_RECTTYPE, P_FGCOLOR];
        let mut extra = Extra {
            id: g.id,
            cdx_props: g.props.iter().filter(|p| !handled.contains(&p.0)).cloned().collect(),
            cdx_children: g.children.iter().map(|c| c.raw.clone()).collect(),
            ..Default::default()
        };
        let color = g.int(P_FGCOLOR).and_then(|i| color_from_index(&self.fx.colors, i));
        // Replaced by an arrow object: the object is read instead.
        if let Some(by) = g.u32(P_SUPERSEDED) {
            self.superseded.insert(g.id, by);
            return true;
        }
        match gt {
            1 => {
                let kind = match g.i16(P_ARROWTYPE).unwrap_or(0) {
                    0 => ArrowKind::Line,
                    4 => ArrowKind::Resonance,
                    8 => ArrowKind::Equilibrium,
                    32 => ArrowKind::Retro,
                    64 => ArrowKind::NoGo,
                    _ => ArrowKind::Reaction,
                };
                extra.source_label = Some(format!("{:?}", kind));
                self.graphic_arrows.push(Arrow { from: p2, to: p1, kind, color, extra: Some(Box::new(extra)), style: Default::default() });
                true
            }
            3 => {
                let kind = if g.i16(P_RECTTYPE).unwrap_or(0) & 1 != 0 { ShapeKind::RoundRect } else { ShapeKind::Rect };
                let r = egui::Rect::from_two_pos(p1, p2);
                self.doc.shapes.push(Shape { color, extra: Some(Box::new(extra)), ..Shape::new(r.min, r.max, kind) });
                true
            }
            6 => {
                let bt = g.i16(P_BRACKETTYPE).unwrap_or(0);
                let kind = if bt == 0 || bt == 5 { ShapeKind::Parens } else { ShapeKind::Brackets };
                let label = cdx_bracket_label(g);
                extra.source_label = label.clone();
                self.sides.push(BracketSide { a: p1, b: p2, kind, single: bt >= 3, label, color, extra, raw: Raw::Cdx(g.raw.clone()) });
                true
            }
            4 => {
                let (Some(c), Some(ma), Some(mi)) = (g.prop(P_CENTER3D), g.prop(P_MAJOR3D), g.prop(P_MINOR3D)) else { return false };
                let c = pos2(rd_i32(c, 0), rd_i32(c, 1));
                let rx = (pos2(rd_i32(ma, 0), rd_i32(ma, 1)) - c).length();
                let ry = (pos2(rd_i32(mi, 0), rd_i32(mi, 1)) - c).length();
                extra.cdx_props.retain(|p| ![P_CENTER3D, P_MAJOR3D, P_MINOR3D].contains(&p.0));
                self.doc.shapes.push(Shape { color, extra: Some(Box::new(extra)), ..Shape::new(c - vec2(rx, ry), c + vec2(rx, ry), ShapeKind::Ellipse) });
                true
            }
            _ => false,
        }
    }

    fn arrow_obj(&mut self, a: &CObj) {
        let (Some(h), Some(t)) = (a.prop(P_HEAD3D), a.prop(P_TAIL3D)) else {
            self.fx.cdx_page_children.push(a.raw.clone());
            return;
        };
        let _ = (h, t);
        let Some(cd) = CdArrow::from_cdx(a) else {
            self.fx.cdx_page_children.push(a.raw.clone());
            return;
        };
        let (kind, style, from, to) = cd.to_formula();
        let extra = Extra {
            id: a.id,
            frag: FROM_ARROW_OBJECT,
            // Everything not part of the arrow's description is kept as it was.
            cdx_props: a.props.iter().filter(|p| !ARROW_PROPS.contains(&p.0) && p.0 != P_FGCOLOR).cloned().collect(),
            cdx_children: a.children.iter().map(|c| c.raw.clone()).collect(),
            // The arrow object itself says what kind it is: the line graphic beside it doesn't.
            xml_attrs: vec![("#cdxarrow".into(), String::new())],
            source_label: Some(arrow_label(kind, &style)),
            ..Default::default()
        };
        let color = a.int(P_FGCOLOR).and_then(|i| color_from_index(&self.fx.colors, i));
        self.object_arrows.push(Arrow { from, to, kind, color, extra: Some(Box::new(extra)), style });
    }

    fn finish(mut self) -> Result<Doc, String> {
        for (b, frag) in std::mem::take(&mut self.pending_bonds) {
            let (Some(bg), Some(en)) = (b.u32(P_BEGIN), b.u32(P_END)) else { continue };
            let (Some(&x), Some(&y)) = (self.id_map.get(&bg), self.id_map.get(&en)) else { continue };
            let color = b.int(P_FGCOLOR).and_then(|i| color_from_index(&self.fx.colors, i));
            let mut bond = make_bond(x, y, b.i16(P_ORDER).unwrap_or(1) as i32, b.i16(P_DISPLAY).unwrap_or(0) as i32, b.i16(P_DBLPOS).map(|v| v as i32), color);
            let handled = [P_BEGIN, P_END, P_ORDER, P_DISPLAY, P_DBLPOS, P_FGCOLOR, P_BBOX];
            bond.extra = Some(Box::new(Extra {
                id: b.id,
                frag,
                cdx_props: b.props.iter().filter(|p| !handled.contains(&p.0)).cloned().collect(),
                cdx_children: b.children.iter().map(|c| c.raw.clone()).collect(),
                ..Default::default()
            }));
            self.doc.bonds.push(bond);
        }
        self.finish_common()
    }

    fn finish_common(mut self) -> Result<Doc, String> {
        self.pair_brackets();
        // "CH3", "CH2", "NH2"… written as fixed text become their element, shown, so the
        // hydrogens follow the bonds. The file's text stays in `source_label`, so the atom
        // is written afresh rather than copied back unchanged.
        for a in &mut self.doc.atoms {
            a.simplify_label();
        }
        // Arrow objects replace older line graphics drawn at the same place;
        // in binary files the line graphic tells which kind of arrow it is.
        let mut arrows = std::mem::take(&mut self.object_arrows);
        // Line graphics that the file says an arrow object replaces are dropped.
        let object_ids: HashSet<u32> = arrows.iter().filter_map(|a| a.extra.as_ref().map(|e| e.id)).collect();
        let superseded = std::mem::take(&mut self.superseded);
        self.graphic_arrows.retain(|g| !g.extra.as_ref().and_then(|e| superseded.get(&e.id)).is_some_and(|by| object_ids.contains(by)));
        for g in std::mem::take(&mut self.graphic_arrows) {
            let same = |a: &Arrow| (a.from - g.from).length() < 1.5 && (a.to - g.to).length() < 1.5;
            if let Some(a) = arrows.iter_mut().find(|a| same(a)) {
                let from_xml = a.extra.as_ref().map(|e| !e.xml_attrs.is_empty()).unwrap_or(false);
                if !from_xml {
                    a.kind = g.kind;
                    if let Some(e) = &mut a.extra {
                        e.source_label = Some(format!("{:?}", g.kind));
                    }
                }
            } else {
                arrows.push(g);
            }
        }
        self.doc.arrows = arrows;
        // A structure whose labels are all larger or smaller than the document's label size by
        // the same factor was scaled as a whole (ChemDraw's Scale, or Formula's): it keeps that size.
        let base = self.file_label_pt();
        if base > 0.5 && !self.label_pt.is_empty() {
            let pts: HashMap<usize, f32> = std::mem::take(&mut self.label_pt).into_iter().collect();
            for frag in self.doc.fragments() {
                let mut r: Vec<f32> = frag.iter().filter_map(|i| pts.get(i)).map(|p| p / base).collect();
                if r.is_empty() || frag.iter().any(|&i| (self.doc.atoms[i].size - 1.0).abs() > 1e-4) {
                    continue;
                }
                r.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let med = r[r.len() / 2];
                if (med - 1.0).abs() > 0.05 && r.iter().all(|x| (x - med).abs() < med * 0.03) {
                    for &i in &frag {
                        self.doc.atoms[i].size = med;
                    }
                }
            }
        }
        // Scale so bonds have the editor's standard length (a scaled structure's bonds count at
        // their unscaled length).
        let mut lens: Vec<f32> = self
            .doc
            .bonds
            .iter()
            .map(|b| (self.doc.atoms[b.a].pos - self.doc.atoms[b.b].pos).length() * 2.0 / (self.doc.atoms[b.a].size + self.doc.atoms[b.b].size))
            .filter(|l| *l > 0.01)
            .collect();
        lens.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = lens.get(lens.len() / 2).copied().unwrap_or(BOND_LEN);
        // Snapped so saving and reopening gives the same scale (no drift).
        let med = ((med * 10.0).round() / 10.0).max(0.1);
        let s = BOND_LEN / med;
        let all = self.doc.select_all();
        self.doc.transform(&all, |p| pos2(p.x * s, p.y * s));
        for a in &mut self.doc.atoms {
            for o in [&mut a.charge_off, &mut a.iso_off] {
                o[0] *= s;
                o[1] *= s;
            }
        }
        for c in &mut self.doc.captions {
            c.size *= s;
        }
        self.doc.style.label_size = 10.0 * s;
        self.doc.style.caption_size = 10.0 * s;
        self.read_style(med, s);
        self.fx.scale = s;
        self.fx.max_id = self.fx.max_id.max(self.max_id);
        self.doc.file = Some(Box::new(self.fx));
        Ok(self.doc)
    }
}

impl Reader {
    /// The document's label font size in points (10 if the file doesn't say).
    fn file_label_pt(&self) -> f32 {
        let fx = &self.fx;
        fx.xml_doc_attrs
            .iter()
            .find(|(a, _)| a == "LabelSize")
            .and_then(|(_, v)| v.trim().parse::<f32>().ok())
            .or_else(|| fx.cdx_doc_props.iter().find(|p| p.0 == P_LABELSTYLE).filter(|p| p.1.len() >= 6).map(|p| u16::from_le_bytes([p.1[4], p.1[5]]) as f32 / 20.0))
            .filter(|v| (2.0..=100.0).contains(v))
            .unwrap_or(10.0)
    }

    /// Drawing settings stored in the document (line widths, spacing, font sizes).
    fn read_style(&mut self, med: f32, s: f32) {
        let fx = &self.fx;
        let xa = |k: &str| fx.xml_doc_attrs.iter().find(|(a, _)| a == k).and_then(|(_, v)| v.trim().parse::<f32>().ok());
        let bp = |t: u16| fx.cdx_doc_props.iter().find(|p| p.0 == t).map(|p| p.1.as_slice());
        let coord = |t: u16| bp(t).filter(|d| d.len() >= 4).map(|d| rd_i32(d, 0));
        let get = |k: &str, t: u16| xa(k).or_else(|| coord(t));
        let st = &mut self.doc.style;
        st.name = "From the file".into();
        st.bond_length = med;
        let ok = |v: &f32, lo: f32, hi: f32| (lo..=hi).contains(v);
        if let Some(v) = get("LineWidth", P_LINEWIDTH).filter(|v| ok(v, 0.05, 10.0)) {
            st.line_width = v * s;
        }
        if let Some(v) = get("BoldWidth", P_BOLDWIDTH).filter(|v| ok(v, 0.1, 20.0)) {
            st.bold_width = v * s;
            st.wedge_width = v * s * 1.5;
        }
        if let Some(v) = get("HashSpacing", P_HASHSPACING).filter(|v| ok(v, 0.1, 20.0)) {
            st.hash_spacing = v * s;
        }
        if let Some(v) = get("MarginWidth", P_MARGINWIDTH).filter(|v| ok(v, 0.0, 20.0)) {
            st.margin = v * s;
        }
        let spacing = xa("BondSpacing")
            .map(|v| v / 100.0)
            .or_else(|| bp(P_BONDSPACING).filter(|d| d.len() >= 2).map(|d| i16::from_le_bytes([d[0], d[1]]) as f32 / 1000.0));
        if let Some(v) = spacing.filter(|v| ok(v, 0.04, 0.6)) {
            st.dbl_spacing = v;
        }
        let style_size = |t: u16| bp(t).filter(|d| d.len() >= 6).map(|d| u16::from_le_bytes([d[4], d[5]]) as f32 / 20.0);
        if let Some(v) = xa("LabelSize").or_else(|| style_size(P_LABELSTYLE)).filter(|v| ok(v, 2.0, 100.0)) {
            st.label_size = v * s;
        }
        if let Some(v) = xa("CaptionSize").or_else(|| style_size(P_CAPTIONSTYLE)).filter(|v| ok(v, 2.0, 100.0)) {
            st.caption_size = v * s;
        }
        let style_font = |t: u16| bp(t).filter(|d| d.len() >= 2).map(|d| u16::from_le_bytes([d[0], d[1]]));
        let font_of = |k: &str, t: u16| xa(k).map(|v| v as u16).or_else(|| style_font(t));
        let serif = |id: Option<u16>| if id.is_some_and(|i| self.serif_fonts.contains(&i)) { LabelFont::Serif } else { LabelFont::Sans };
        st.label_font = serif(font_of("LabelFont", P_LABELSTYLE));
        st.caption_font = serif(font_of("CaptionFont", P_CAPTIONSTYLE));
        // The page: the file gives the whole document's size and how many pages it spans;
        // the margins stay as they are and the paper is the drawing area plus them.
        let pa = |k: &str| fx.xml_page_attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str());
        let pp = |t: u16| fx.cdx_page_props.iter().find(|p| p.0 == t).map(|p| p.1.as_slice());
        let size = nums(pa("BoundingBox"))
            .get(0..4)
            .map(|v| (v[2] - v[0], v[3] - v[1]))
            .or_else(|| pp(P_BBOX).filter(|d| d.len() >= 16).map(|d| (rd_i32(d, 3) - rd_i32(d, 1), rd_i32(d, 2) - rd_i32(d, 0))));
        let count = |k: &str, t: u16| {
            pa(k).and_then(|v| v.trim().parse::<f32>().ok()).or_else(|| pp(t).filter(|d| d.len() >= 2).map(|d| i16::from_le_bytes([d[0], d[1]]) as f32)).filter(|n| *n >= 1.0).unwrap_or(1.0)
        };
        if let Some((w, h)) = size {
            let (w, h) = (w / count("WidthPages", P_WIDTHPAGES) * 2.54 / 72.0, h / count("HeightPages", P_HEIGHTPAGES) * 2.54 / 72.0);
            if (3.0..200.0).contains(&w) && (3.0..200.0).contains(&h) {
                let pg = &mut st.page;
                pg.paper_w = w + pg.left + pg.right;
                pg.paper_h = h + pg.top + pg.bottom;
            }
        }
    }
}

/// Set a numeric attribute unless the file already holds (nearly) the same value.
fn set_num_attr(attrs: &mut Vec<(String, String)>, key: &str, v: f32) {
    let text = f2(v);
    match attrs.iter_mut().find(|(k, _)| k == key) {
        Some((_, old)) => {
            if old.trim().parse::<f32>().map(|o| (o - v).abs() > 0.006).unwrap_or(true) {
                *old = text;
            }
        }
        None => attrs.push((key.to_string(), text)),
    }
}

/// Same for a binary property: a coordinate (16.16 fixed point).
fn set_coord_prop(props: &mut Vec<(u16, Vec<u8>)>, tag: u16, v: f32) {
    let data = fx32(v).to_vec();
    match props.iter_mut().find(|p| p.0 == tag) {
        Some((_, old)) => {
            if old.len() < 4 || (rd_i32(old, 0) - v).abs() > 0.006 {
                *old = data;
            }
        }
        None => props.push((tag, data)),
    }
}

/// Drawing settings for a new or changed document, in points.
fn style_props(doc: &Doc, scale: f32, font_id: u16, caption_id: u16, props: &mut Vec<(u16, Vec<u8>)>) {
    let st = &doc.style;
    let serif = serif_ids(&props.iter().find(|p| p.0 == P_FONTTABLE).map(|p| cdx_fonts(&p.1)).unwrap_or_default());
    set_coord_prop(props, P_BONDLENGTH, BOND_LEN / scale);
    set_coord_prop(props, P_LINEWIDTH, st.line_width / scale);
    set_coord_prop(props, P_BOLDWIDTH, st.bold_width / scale);
    set_coord_prop(props, P_HASHSPACING, st.hash_spacing / scale);
    set_coord_prop(props, P_MARGINWIDTH, st.margin / scale);
    let sp = (st.dbl_spacing * 1000.0).round() as i16;
    match props.iter_mut().find(|p| p.0 == P_BONDSPACING) {
        Some((_, old)) => {
            if old.len() < 2 || (i16::from_le_bytes([old[0], old[1]]) - sp).abs() > 1 {
                *old = sp.to_le_bytes().to_vec();
            }
        }
        None => props.push((P_BONDSPACING, sp.to_le_bytes().to_vec())),
    }
    if !props.iter().any(|p| p.0 == P_CHAINANGLE) {
        props.push((P_CHAINANGLE, ((120 * 65536) as i32).to_le_bytes().to_vec()));
    }
    // Default label and caption styles: font, face, size in 1/20 pt, colour.
    for (tag, face, size, fid) in [(P_LABELSTYLE, 0x60u16, st.label_size / scale, font_id), (P_CAPTIONSTYLE, 0u16, st.caption_size / scale, caption_id)] {
        let twentieths = (size * 20.0).round().clamp(20.0, 4000.0) as u16;
        match props.iter_mut().find(|p| p.0 == tag) {
            Some((_, old)) if old.len() >= 6 => {
                let cur = u16::from_le_bytes([old[4], old[5]]);
                if cur.abs_diff(twentieths) > 1 {
                    old[4..6].copy_from_slice(&twentieths.to_le_bytes());
                }
                // Keep the file's own font unless the style asks for the other kind.
                let was = u16::from_le_bytes([old[0], old[1]]);
                if serif.contains(&was) != serif.contains(&fid) {
                    old[0..2].copy_from_slice(&fid.to_le_bytes());
                }
            }
            _ => {
                props.retain(|p| p.0 != tag);
                let mut v = Vec::new();
                for x in [fid, face, twentieths, 0u16] {
                    v.extend_from_slice(&x.to_le_bytes());
                }
                props.push((tag, v));
            }
        }
    }
}

fn make_bond(a: usize, b: usize, ord: i32, disp: i32, dp: Option<i32>, color: Option<Rgb>) -> Bond {
    let order = match ord {
        2 => 2,
        4 => 3,
        _ => 1,
    };
    let (stereo, swap) = match disp {
        1 => (Stereo::Dashed, false),
        2 => (Stereo::Hashed, false),
        3 => (Stereo::Hash, false),
        4 => (Stereo::Hash, true),
        5 => (Stereo::Bold, false),
        6 => (Stereo::Wedge, false),
        7 => (Stereo::Wedge, true),
        8 => (Stereo::Wavy, false),
        _ => (Stereo::None, false),
    };
    let mut dbl = match dp {
        Some(v) if v & 0x100 != 0 => match v & 0xFF {
            0 => DblPos::Center,
            1 => DblPos::Right,
            2 => DblPos::Left,
            _ => DblPos::Auto,
        },
        _ => DblPos::Auto,
    };
    let (a, b) = if swap { (b, a) } else { (a, b) };
    if swap {
        dbl = match dbl {
            DblPos::Left => DblPos::Right,
            DblPos::Right => DblPos::Left,
            x => x,
        };
    }
    Bond { a, b, order, stereo, dbl, color, extra: None }
}

// ---------------------------------------------------------------------------
// Shared writing helpers

/// Ids, colours and scale shared by both writers.
struct Ctx {
    scale: f32,
    /// Added to file coordinates so the drawing sits on the page.
    offset: egui::Vec2,
    next_id: u32,
    used: HashSet<u32>,
    colors: Vec<Rgb>,
}

impl Ctx {
    fn new(doc: &Doc) -> Self {
        let fx = doc.file.as_deref();
        let mut colors = fx.map(|f| f.colors.clone()).unwrap_or_default();
        if colors.is_empty() {
            colors = vec![[255, 255, 255], [0, 0, 0]];
        }
        let mut max = fx.map(|f| f.max_id).unwrap_or(0);
        let ids = doc
            .atoms
            .iter()
            .filter_map(|a| a.extra.as_ref().map(|e| e.id))
            .chain(doc.bonds.iter().filter_map(|b| b.extra.as_ref().map(|e| e.id)))
            .chain(doc.arrows.iter().filter_map(|b| b.extra.as_ref().map(|e| e.id)))
            .chain(doc.captions.iter().filter_map(|b| b.extra.as_ref().map(|e| e.id)))
            .chain(doc.shapes.iter().filter_map(|b| b.extra.as_ref().map(|e| e.id)));
        let text_ids = doc.atoms.iter().filter_map(|a| a.extra.as_ref().map(|e| e.text_id));
        for i in ids.chain(text_ids) {
            max = max.max(i);
        }
        if let Some(f) = fx {
            for (i, _, _) in &f.cdx_frags {
                max = max.max(*i);
            }
            for (i, _, _) in &f.xml_frags {
                max = max.max(*i);
            }
        }
        // Leave room above ids used inside kept-as-is data.
        // Drawing units per point: the file's own scale while the bond length is unchanged.
        let want = BOND_LEN / doc.style.bond_length.max(1.0);
        let scale = match fx.map(|f| f.scale).filter(|s| *s > 0.0) {
            Some(s) if (BOND_LEN / s - doc.style.bond_length).abs() < 0.01 => s,
            _ => want,
        };
        // Keep the drawing on the page: other editors don't show what lies above or left of it.
        let mut offset = egui::Vec2::ZERO;
        if let Some(b) = doc.bounds_of(None) {
            let (x0, y0) = (b.min.x / scale, b.min.y / scale);
            let margin = 36.0;
            if x0 < 12.0 {
                offset.x = (margin - x0).round();
            }
            if y0 < 12.0 {
                offset.y = (margin - y0).round();
            }
        }
        Ctx { scale, offset, next_id: max + 1000, used: HashSet::new(), colors }
    }
    fn id(&mut self, want: Option<u32>) -> u32 {
        if let Some(w) = want {
            if w != 0 && self.used.insert(w) {
                return w;
            }
        }
        loop {
            let i = self.next_id;
            self.next_id += 1;
            if self.used.insert(i) {
                return i;
            }
        }
    }
    fn p(&self, p: Pos2) -> Pos2 {
        pos2(p.x / self.scale + self.offset.x, p.y / self.scale + self.offset.y)
    }
    /// Page size in file units, in whole pages, large enough for the drawing.
    fn page_box(&self, doc: &Doc) -> (f32, f32, i16, i16) {
        let (mut w, mut h) = (1i16, 1i16);
        // One page's drawing area in points, from the document's page settings.
        let (aw, ah) = doc.style.page.area_cm();
        let (pw, ph) = (aw * 72.0 / 2.54, ah * 72.0 / 2.54);
        if let Some(b) = doc.bounds_of(None) {
            let m = self.p(b.max);
            w = ((m.x + 36.0) / pw).ceil().clamp(1.0, 50.0) as i16;
            h = ((m.y + 36.0) / ph).ceil().clamp(1.0, 50.0) as i16;
        }
        (pw * w as f32, ph * h as f32, w, h)
    }
    fn color_index(&mut self, c: Option<Rgb>) -> Option<u16> {
        let c = c?;
        if c == [0, 0, 0] {
            return None;
        }
        if let Some(i) = self.colors.iter().position(|x| *x == c) {
            return Some(i as u16 + 2);
        }
        self.colors.push(c);
        Some(self.colors.len() as u16 + 1)
    }
}

fn label_display(doc: &Doc, i: usize) -> Option<String> {
    let a = &doc.atoms[i];
    if let Some(l) = &a.label {
        return Some(l.clone());
    }
    if a.el == elements::C && a.isotope == 0 && doc.degree(i) > 0 && !a.show {
        return None;
    }
    let h = doc.implicit_h(i);
    let mut s = elements::symbol(a.el).to_string();
    if h > 0 {
        s.push('H');
        if h > 1 {
            s.push_str(&h.to_string());
        }
    }
    Some(s)
}

/// Node type for an atom: 1 element, 5 fragment (abbreviation), 7 generic, 0 unspecified.
/// Monomer formula as the original writes it, without the attachment placeholders:
/// "C3H5(20F)(21F)NO" → "C3H5NO". Empty for ambiguity codes ("20F(21F)A").
fn monomer_formula(f: &str) -> String {
    let mut out = String::new();
    let b = f.as_bytes();
    let mut i = 0;
    while i < b.len() {
        // "(20F)" or a leading "20F": an attachment point.
        if b[i] == b'(' {
            if let Some(j) = f[i..].find(')') {
                let inner = &f[i + 1..i + j];
                if inner.ends_with('F') && inner[..inner.len() - 1].chars().all(|c| c.is_ascii_digit()) && inner.len() > 1 {
                    i += j + 1;
                    continue;
                }
            }
        }
        if b[i].is_ascii_digit() && (i == 0 || !b[i - 1].is_ascii_alphanumeric() && b[i - 1] != b')') {
            let j = f[i..].find(|c: char| !c.is_ascii_digit()).map(|k| i + k).unwrap_or(b.len());
            if j < b.len() && b[j] == b'F' && f[i..j].len() >= 2 {
                i = j + 1;
                continue;
            }
        }
        out.push(b[i] as char);
        i += 1;
    }
    if chem::label_formula(&out).is_some_and(|m| !m.is_empty()) { out } else { String::new() }
}

/// A monomer or sequence cap from the original's node: node type, connection type and the
/// monomer's tags (symbol, name, analog, formula, polymer and monomer type).
fn monomer_of(node_type: &str, connection: &str, tags: &HashMap<String, String>, label: &str) -> Option<Monomer> {
    let role = match (node_type, connection) {
        ("Monomer", _) => MonomerRole::Unit,
        (_, "Terminus") => MonomerRole::Terminus,
        (_, "UnlinkedBranch") => MonomerRole::Branch,
        _ => return None,
    };
    let polymer = match (tags.get("monomerPolymerType").map(|s| s.as_str()), connection) {
        (Some("4"), _) | (None, "Peptide") => Polymer::Peptide,
        (Some("8"), _) | (None, "RNA") => Polymer::Nucleic,
        _ if role != MonomerRole::Unit => Polymer::Peptide,
        _ => Polymer::Chem,
    };
    let symbol = tags.get("monomerSymbol").cloned().unwrap_or_else(|| label.trim().to_string());
    Some(Monomer {
        role,
        polymer,
        branch: tags.get("monomerType").map(|s| s == "4").unwrap_or(false),
        symbol,
        name: tags.get("monomerName").cloned().unwrap_or_default(),
        analog: tags.get("monomerNaturalAnalog").cloned().unwrap_or_default(),
        formula: tags.get("monomerFormula").map(|f| monomer_formula(f)).unwrap_or_default(),
    })
}

/// The original's monomer tags: (name, type, value).
fn monomer_tags(m: &Monomer) -> Vec<(&'static str, &'static str, String)> {
    let poly = match m.polymer {
        Polymer::Peptide => "4",
        Polymer::Nucleic => "8",
        Polymer::Chem => "2",
    };
    // The formula with the attachment placeholders the original writes.
    let formula = if m.formula.is_empty() { String::new() } else if m.polymer == Polymer::Peptide { format!("{}(20F)(21F)", m.formula) } else { m.formula.clone() };
    vec![
        ("monomerSymbol", "String", m.symbol.clone()),
        ("monomerPolymerType", "Long", poly.into()),
        ("monomerType", "Long", if m.branch { "4".into() } else { "2".into() }),
        ("monomerName", "String", m.name.clone()),
        ("monomerNaturalAnalog", "String", m.analog.clone()),
        ("monomerFormula", "String", formula),
    ]
}

fn node_type(a: &Atom) -> i16 {
    if let Some(m) = &a.monomer {
        return if m.role == MonomerRole::Unit { 14 } else { 5 };
    }
    match &a.label {
        None => 1,
        Some(l) if l == "R" || l == "X" || l == "*" || l == "Ar" => 7,
        Some(l) if chem::expansion_smiles(l).is_some() => 5,
        Some(_) => 0,
    }
}

/// Fragments in document order, with the file fragment id to reuse.
fn fragments_with_ids(doc: &Doc) -> Vec<(Vec<usize>, Option<u32>)> {
    let mut used = HashSet::new();
    doc.fragments()
        .into_iter()
        .map(|f| {
            let mut counts: HashMap<u32, usize> = HashMap::new();
            for &i in &f {
                if let Some(e) = &doc.atoms[i].extra {
                    *counts.entry(e.frag).or_default() += 1;
                }
            }
            let mut best: Vec<(u32, usize)> = counts.into_iter().collect();
            best.sort_by_key(|x| std::cmp::Reverse(x.1));
            let id = best.into_iter().map(|x| x.0).find(|id| *id != 0 && used.insert(*id));
            (f, id)
        })
        .collect()
}

fn same_kind(a: &Arrow) -> bool {
    a.extra.as_ref().map(|e| e.source_label.as_deref() == Some(arrow_label(a.kind, &a.style).as_str())).unwrap_or(false)
}

/// What an arrow looked like when it was read, to tell whether it has been changed since.
fn arrow_label(kind: ArrowKind, style: &ArrowStyle) -> String {
    if style.is_default() { format!("{:?}", kind) } else { format!("{:?} {:?}", kind, style) }
}

const ALL_ARROW_KINDS: [ArrowKind; 12] = [
    ArrowKind::Reaction,
    ArrowKind::Line,
    ArrowKind::Equilibrium,
    ArrowKind::Retro,
    ArrowKind::Resonance,
    ArrowKind::Curved,
    ArrowKind::Dashed,
    ArrowKind::NoGo,
    ArrowKind::Hollow,
    ArrowKind::Unbalanced,
    ArrowKind::NoGoHash,
    ArrowKind::Rotation,
];

/// Formula 0.0.4 and earlier kept an arrow's exact style in its own note:
/// "kind size wide half arc both".
fn parse_arrow_note(v: &str) -> Option<(ArrowKind, ArrowStyle)> {
    let p: Vec<&str> = v.split_whitespace().collect();
    if p.len() < 6 {
        return None;
    }
    let kind = *ALL_ARROW_KINDS.iter().find(|k| format!("{:?}", k) == p[0])?;
    let size = [HeadSize::Large, HeadSize::Medium, HeadSize::Small].into_iter().find(|k| format!("{:?}", k) == p[1])?;
    let half = [Half::Full, Half::Left, Half::Right].into_iter().find(|k| format!("{:?}", k) == p[3])?;
    Some((kind, ArrowStyle { size, wide: p[2] == "1", half, arc: p[4].parse().ok()?, both: p[5] == "1" }))
}

const ARROW_NOTE: &str = "FormulaArrowStyle";
/// An atom's size relative to the document style (scaled structures), kept in CDXML for Formula.
const ATOM_SIZE_NOTE: &str = "FormulaSize";
/// Where a dragged charge sign and isotope number sit, relative to their usual place
/// ("charge-x charge-y isotope-x isotope-y", in points).
const MARKS_NOTE: &str = "FormulaMarkOffsets";

/// Arrow properties Formula writes itself (binary): the rest of a file's arrow is kept as is.
const ARROW_PROPS: [u16; 18] = [
    P_BBOX,
    P_HEAD3D,
    P_TAIL3D,
    P_CENTER3D,
    P_MAJOR3D,
    P_MINOR3D,
    P_ARROW_LINETYPE,
    P_ARROW_HEADSIZE,
    P_ARROW_ANGULAR,
    P_ARROW_TYPE,
    P_ARROW_CENTERSIZE,
    P_ARROW_WIDTH,
    P_ARROW_SHAFT,
    P_ARROW_RATIO,
    P_ARROW_HEAD,
    P_ARROW_TAIL,
    P_ARROW_FILL,
    P_ARROW_NOGO,
];

/// Arrow attributes Formula writes itself (CDXML).
const ARROW_ATTRS: [&str; 19] = [
    "BoundingBox",
    "Head3D",
    "Tail3D",
    "Center3D",
    "MajorAxisEnd3D",
    "MinorAxisEnd3D",
    "LineType",
    "FillType",
    "ArrowheadHead",
    "ArrowheadTail",
    "ArrowheadType",
    "HeadSize",
    "ArrowheadCenterSize",
    "ArrowheadWidth",
    "ArrowShaftSpacing",
    "ArrowEquilibriumRatio",
    "AngularSize",
    "NoGo",
    ARROW_NOTE,
];

// Arrow object properties (binary), learnt from an arrow sheet saved by the original as CDX
// and CDXML.
const P_ARROW_LINETYPE: u16 = 0x0A01;
const P_ARROW_HEADSIZE: u16 = 0x0A20;
const P_ARROW_ANGULAR: u16 = 0x0A21;
const P_ARROW_TYPE: u16 = 0x0A2F;
const P_ARROW_CENTERSIZE: u16 = 0x0A30;
const P_ARROW_WIDTH: u16 = 0x0A31;
const P_ARROW_SHAFT: u16 = 0x0A33;
const P_ARROW_RATIO: u16 = 0x0A34;
const P_ARROW_HEAD: u16 = 0x0A35;
const P_ARROW_TAIL: u16 = 0x0A36;
const P_ARROW_FILL: u16 = 0x0A37;
const P_ARROW_NOGO: u16 = 0x0A3B;

/// An arrow as the original describes it, in file coordinates. The same fields appear as
/// CDXML attributes and as CDX properties:
/// - heads: 0 none, 2 full, 3 half on the left, 4 half on the right;
/// - type: 1 solid, 2 hollow (block arrow), 3 open "angle" heads (retrosynthesis);
/// - line: 0 plain, 1 dashed, 2 bold; no-go: 2 cross, 3 two slashes;
/// - head sizes in 1/1000 of the line width: small 1000, medium 1500, large 2250 (doubled for
///   bold, 800 on curved arrows); a straight half-right head stores a negative size;
/// - curved arrows: AngularSize in degrees (CDX: tenths), negative bending to the left of the
///   tail-to-head direction on screen; the arc is the circle through Center3D with radius
///   |MajorAxisEnd3D - Center3D|. Different major/minor radii make an ellipse: the original's
///   rotation arrow;
/// - unbalanced equilibrium: two half heads, shaft spacing 300 and an equilibrium ratio (300).
#[derive(Clone, Debug, Default)]
struct CdArrow {
    head: u8,
    tail: u8,
    atype: u8,
    line: u8,
    nogo: u8,
    head_size: f32,
    center_size: f32,
    width: f32,
    shaft: f32,
    ratio: f32,
    angular: Option<f32>,
    head3: Pos2,
    tail3: Pos2,
    center3: Option<Pos2>,
    major3: Option<Pos2>,
    minor3: Option<Pos2>,
}

fn head_code(s: &str) -> u8 {
    match s {
        "Full" => 2,
        "HalfLeft" => 3,
        "HalfRight" => 4,
        _ => 0,
    }
}

fn head_name(c: u8) -> &'static str {
    match c {
        3 => "HalfLeft",
        4 => "HalfRight",
        _ => "Full",
    }
}

impl CdArrow {
    fn from_xml(a: &Node) -> Option<CdArrow> {
        let pt = |k: &str| {
            let v = nums(a.attr(k));
            (v.len() >= 2).then(|| pos2(v[0], v[1]))
        };
        let num = |k: &str| a.attr(k).and_then(|v| v.trim().parse::<f32>().ok());
        Some(CdArrow {
            head: head_code(a.attr("ArrowheadHead").unwrap_or("")),
            tail: head_code(a.attr("ArrowheadTail").unwrap_or("")),
            atype: match a.attr("ArrowheadType") {
                Some("Hollow") => 2,
                Some("Angle") => 3,
                _ => 1,
            },
            line: match a.attr("LineType") {
                Some(v) if v.contains("Dash") => 1,
                Some(v) if v.contains("Bold") => 2,
                _ => 0,
            },
            nogo: match a.attr("NoGo") {
                Some("Cross") => 2,
                Some("Hash") => 3,
                _ => 0,
            },
            head_size: num("HeadSize").unwrap_or(1000.0),
            center_size: num("ArrowheadCenterSize").unwrap_or(0.0),
            width: num("ArrowheadWidth").unwrap_or(0.0),
            shaft: num("ArrowShaftSpacing").unwrap_or(0.0),
            ratio: num("ArrowEquilibriumRatio").unwrap_or(0.0),
            angular: num("AngularSize"),
            head3: pt("Head3D")?,
            tail3: pt("Tail3D")?,
            center3: pt("Center3D"),
            major3: pt("MajorAxisEnd3D"),
            minor3: pt("MinorAxisEnd3D"),
        })
    }

    fn from_cdx(o: &CObj) -> Option<CdArrow> {
        let pt = |t: u16| o.prop(t).filter(|d| d.len() >= 8).map(|d| pos2(rd_i32(d, 0), rd_i32(d, 1)));
        let small = |t: u16| o.prop(t).and_then(|d| d.first().copied()).unwrap_or(0);
        let num = |t: u16| o.i16(t).map(|v| v as f32);
        Some(CdArrow {
            head: small(P_ARROW_HEAD),
            tail: small(P_ARROW_TAIL),
            atype: o.prop(P_ARROW_TYPE).and_then(|d| d.first().copied()).unwrap_or(1),
            line: small(P_ARROW_LINETYPE),
            nogo: small(P_ARROW_NOGO),
            head_size: num(P_ARROW_HEADSIZE).unwrap_or(1000.0),
            center_size: num(P_ARROW_CENTERSIZE).unwrap_or(0.0),
            width: num(P_ARROW_WIDTH).unwrap_or(0.0),
            shaft: num(P_ARROW_SHAFT).unwrap_or(0.0),
            ratio: num(P_ARROW_RATIO).unwrap_or(0.0),
            angular: num(P_ARROW_ANGULAR).map(|v| v / 10.0),
            head3: pt(P_HEAD3D)?,
            tail3: pt(P_TAIL3D)?,
            center3: pt(P_CENTER3D),
            major3: pt(P_MAJOR3D),
            minor3: pt(P_MINOR3D),
        })
    }

    /// Formula's arrow: kind, style, and the two points it is drawn between.
    fn to_formula(&self) -> (ArrowKind, ArrowStyle, Pos2, Pos2) {
        let mut s = ArrowStyle::default();
        let block = matches!(self.atype, 2 | 3);
        let mut hs = self.head_size.abs();
        if self.line == 2 && !block {
            hs /= 2.0;
            s.wide = true;
        }
        let half = |c: u8| match c {
            3 => Half::Left,
            4 => Half::Right,
            _ => Half::Full,
        };
        if let Some(ang) = self.angular {
            s.size = if hs >= 1200.0 { HeadSize::Large } else if hs >= 700.0 { HeadSize::Medium } else { HeadSize::Small };
            s.half = half(self.head);
            s.both = self.head != 0 && self.tail != 0;
            // An ellipse (different axis lengths): the rotation arrow.
            if let (Some(c), Some(ma), Some(mi)) = (self.center3, self.major3, self.minor3) {
                let (va, vi) = (ma - c, mi - c);
                let (ra, ri) = (va.length(), vi.length());
                if ra.min(ri) > 0.01 && ra.max(ri) / ra.min(ri) > 1.25 {
                    let long = if ra >= ri { va } else { vi };
                    let (e1, e2) = (c - long, c + long);
                    let (from, to) = if (e1 - self.tail3).length() <= (e2 - self.tail3).length() { (e1, e2) } else { (e2, e1) };
                    // Only the direction matters (Formula's default, 0, is the original's positive
                    // angle; checked against the original's own drawing of the arrow sheet).
                    s.arc = if ang < 0.0 { -1.0 } else { 0.0 };
                    s.half = Half::Full;
                    s.both = false;
                    return (ArrowKind::Rotation, s, from, to);
                }
            }
            // The original's negative angle bends to the side Formula calls positive.
            s.arc = -ang;
            return (ArrowKind::Curved, s, self.tail3, self.head3);
        }
        s.size = if block {
            if hs >= 1000.0 { HeadSize::Large } else if hs >= 750.0 { HeadSize::Medium } else { HeadSize::Small }
        } else if hs >= 1900.0 {
            HeadSize::Large
        } else if hs >= 1250.0 {
            HeadSize::Medium
        } else {
            HeadSize::Small
        };
        let kind = if self.nogo == 2 {
            ArrowKind::NoGo
        } else if self.nogo == 3 {
            ArrowKind::NoGoHash
        } else if matches!(self.head, 3 | 4) && matches!(self.tail, 3 | 4) {
            if self.ratio > 0.0 { ArrowKind::Unbalanced } else { ArrowKind::Equilibrium }
        } else if self.head != 0 && self.tail != 0 {
            ArrowKind::Resonance
        } else if self.atype == 2 {
            ArrowKind::Hollow
        } else if self.atype == 3 || (self.shaft > 0.0 && self.head != 0) {
            ArrowKind::Retro
        } else if self.head == 0 {
            ArrowKind::Line
        } else if self.line == 1 {
            ArrowKind::Dashed
        } else {
            ArrowKind::Reaction
        };
        if matches!(kind, ArrowKind::Reaction | ArrowKind::Dashed | ArrowKind::NoGo | ArrowKind::NoGoHash) {
            s.half = half(self.head);
        }
        (kind, s, self.tail3, self.head3)
    }

    /// The original's description of a Formula arrow, in file coordinates.
    fn from_formula(a: &Arrow, cx: &Ctx) -> CdArrow {
        let st = &a.style;
        let mut c = CdArrow { atype: 1, ..Default::default() };
        let (tail, head) = (cx.p(a.from), cx.p(a.to));
        c.tail3 = tail;
        c.head3 = head;
        let half = |h: Half| match h {
            Half::Full => 2,
            Half::Left => 3,
            Half::Right => 4,
        };
        let curved = matches!(a.kind, ArrowKind::Curved | ArrowKind::Rotation);
        let block = matches!(a.kind, ArrowKind::Hollow | ArrowKind::Retro);
        match a.kind {
            ArrowKind::Line => {}
            ArrowKind::Reaction | ArrowKind::Dashed | ArrowKind::NoGo | ArrowKind::NoGoHash => c.head = half(st.half),
            ArrowKind::Curved => {
                c.head = half(st.half);
                if st.both {
                    c.tail = half(match st.half {
                        Half::Left => Half::Right,
                        Half::Right => Half::Left,
                        Half::Full => Half::Full,
                    });
                }
            }
            ArrowKind::Rotation => c.head = 2,
            ArrowKind::Resonance => {
                c.head = 2;
                c.tail = 2;
            }
            ArrowKind::Equilibrium | ArrowKind::Unbalanced => {
                c.head = 3;
                c.tail = 3;
                c.shaft = 300.0;
                if a.kind == ArrowKind::Unbalanced {
                    c.ratio = 300.0;
                }
            }
            ArrowKind::Hollow => {
                c.head = 2;
                c.atype = 2;
            }
            ArrowKind::Retro => {
                c.head = 2;
                c.atype = 3;
            }
        }
        if a.kind == ArrowKind::Dashed {
            c.line = 1;
        }
        c.nogo = match a.kind {
            ArrowKind::NoGo => 2,
            ArrowKind::NoGoHash => 3,
            _ => 0,
        };
        // Head sizes as the original's palette gives them.
        let mut hs = match (st.size, curved, block) {
            (HeadSize::Large, _, true) => 1200.0,
            (HeadSize::Medium, _, true) => 900.0,
            (HeadSize::Small, _, true) => 600.0,
            (HeadSize::Large, true, _) => 1500.0,
            (HeadSize::Medium, true, _) => 800.0,
            (HeadSize::Small, true, _) => 600.0,
            (HeadSize::Large, ..) => 2250.0,
            (HeadSize::Medium, ..) => 1500.0,
            (HeadSize::Small, ..) => 1000.0,
        };
        if st.wide && !block {
            hs *= 2.0;
            c.line = 2;
        }
        c.head_size = hs;
        if block {
            c.center_size = hs;
            c.shaft = hs;
        } else {
            c.center_size = (hs * 0.875).round();
        }
        c.width = (hs * 0.25).round();
        if !curved && c.head == 4 && c.tail == 0 {
            c.head_size = -(hs - 1.0);
        }
        // Geometry: straight arrows carry their own frame; arcs their circle; rotation arrows
        // their ellipse.
        match a.kind {
            ArrowKind::Curved => {
                let deg = if st.arc.abs() > 1.0 { st.arc.clamp(-340.0, 340.0) } else { 90.0 };
                let d = a.to - a.from;
                let len = d.length().max(1e-3);
                let th = deg.abs().clamp(5.0, 340.0).to_radians();
                let side = rot(d, -std::f32::consts::FRAC_PI_2).normalized() * deg.signum();
                let r = len / (2.0 * (th / 2.0).sin());
                let centre = a.from + d * 0.5 - side * (r * (th / 2.0).cos());
                let (cf, rf) = (cx.p(centre), r / cx.scale);
                c.angular = Some(-deg);
                c.center3 = Some(cf);
                c.major3 = Some(cf + vec2(rf, 0.0));
                c.minor3 = Some(cf + vec2(0.0, rf));
            }
            ArrowKind::Rotation => {
                let reverse = st.arc < 0.0;
                let pts = rotation_points(a.from, a.to, reverse, 64);
                let d = a.to - a.from;
                let ax = d.length() / 2.0;
                let u = d.normalized();
                let w = vec2(-u.y, u.x);
                let cen = a.from + d * 0.5;
                c.tail3 = cx.p(pts[0]);
                c.head3 = cx.p(*pts.last().unwrap());
                c.angular = Some(if reverse { -290.0 } else { 290.0 });
                c.center3 = Some(cx.p(cen));
                c.major3 = Some(cx.p(cen + u * ax));
                c.minor3 = Some(cx.p(cen + w * (ax * 0.42)));
            }
            _ => {
                let m = pos2((head.x + tail.x) / 2.0, (head.y + tail.y) / 2.0);
                let v = head - tail;
                c.center3 = Some(m);
                c.major3 = Some(m + v);
                c.minor3 = Some(m + vec2(-v.y, v.x));
            }
        }
        c
    }

    fn bbox(&self) -> egui::Rect {
        let mut r = egui::Rect::from_two_pos(self.head3, self.tail3);
        if let (Some(cen), Some(ma), Some(mi), Some(_)) = (self.center3, self.major3, self.minor3, self.angular) {
            let rr = (ma - cen).length().max((mi - cen).length());
            r = r.union(egui::Rect::from_center_size(cen, vec2(rr * 2.0, rr * 2.0)));
        }
        r
    }

    fn write_xml(&self, n: &mut Node) {
        let p3 = |p: Pos2| format!("{} {} 0", f2(p.x), f2(p.y));
        let push = |n: &mut Node, k: &str, v: String| match n.attrs.iter_mut().find(|x| x.0 == k) {
            Some(x) => x.1 = v,
            None => n.attrs.push((k.into(), v)),
        };
        let r = self.bbox();
        push(n, "BoundingBox", format!("{} {} {} {}", f2(r.min.x), f2(r.min.y), f2(r.max.x), f2(r.max.y)));
        match self.line {
            1 => push(n, "LineType", "Dashed".into()),
            2 => push(n, "LineType", "Bold".into()),
            _ => {}
        }
        push(n, "FillType", "None".into());
        if self.head != 0 {
            push(n, "ArrowheadHead", head_name(self.head).into());
        }
        if self.tail != 0 {
            push(n, "ArrowheadTail", head_name(self.tail).into());
        }
        push(n, "ArrowheadType", match self.atype {
            2 => "Hollow",
            3 => "Angle",
            _ => "Solid",
        }
        .into());
        push(n, "HeadSize", format!("{:.0}", self.head_size));
        push(n, "ArrowheadCenterSize", format!("{:.0}", self.center_size));
        push(n, "ArrowheadWidth", format!("{:.0}", self.width));
        if self.shaft > 0.0 {
            push(n, "ArrowShaftSpacing", format!("{:.0}", self.shaft));
        }
        if self.ratio > 0.0 {
            push(n, "ArrowEquilibriumRatio", format!("{:.0}", self.ratio));
        }
        if let Some(a) = self.angular {
            push(n, "AngularSize", f2(a));
        }
        match self.nogo {
            2 => push(n, "NoGo", "Cross".into()),
            3 => push(n, "NoGo", "Hash".into()),
            _ => {}
        }
        push(n, "Head3D", p3(self.head3));
        push(n, "Tail3D", p3(self.tail3));
        if let (Some(c), Some(ma), Some(mi)) = (self.center3, self.major3, self.minor3) {
            push(n, "Center3D", p3(c));
            push(n, "MajorAxisEnd3D", p3(ma));
            push(n, "MinorAxisEnd3D", p3(mi));
        }
    }

    fn write_cdx(&self, w: &mut W) {
        let r = self.bbox();
        w.prop(P_BBOX, &rect4(r.min.y, r.min.x, r.max.y, r.max.x));
        if self.line != 0 {
            w.prop(P_ARROW_LINETYPE, &(self.line as u16).to_le_bytes());
        }
        w.prop(P_ARROW_FILL, &1u16.to_le_bytes());
        w.prop(P_ARROW_TYPE, &(self.atype as u16).to_le_bytes());
        if self.head != 0 {
            w.prop(P_ARROW_HEAD, &(self.head as u16).to_le_bytes());
        }
        if self.tail != 0 {
            w.prop(P_ARROW_TAIL, &(self.tail as u16).to_le_bytes());
        }
        w.i16(P_ARROW_HEADSIZE, self.head_size.round() as i16);
        w.i16(P_ARROW_CENTERSIZE, self.center_size.round() as i16);
        w.i16(P_ARROW_WIDTH, self.width.round() as i16);
        if self.shaft > 0.0 {
            w.i16(P_ARROW_SHAFT, self.shaft.round() as i16);
        }
        if self.ratio > 0.0 {
            w.i16(P_ARROW_RATIO, self.ratio.round() as i16);
        }
        if let Some(a) = self.angular {
            w.i16(P_ARROW_ANGULAR, (a * 10.0).round() as i16);
        }
        if self.nogo != 0 {
            // One byte, unlike the other arrow properties.
            w.prop(P_ARROW_NOGO, &[self.nogo]);
        }
        w.prop(P_HEAD3D, &point3d(self.head3));
        w.prop(P_TAIL3D, &point3d(self.tail3));
        if let (Some(c), Some(ma), Some(mi)) = (self.center3, self.major3, self.minor3) {
            w.prop(P_CENTER3D, &point3d(c));
            w.prop(P_MAJOR3D, &point3d(ma));
            w.prop(P_MINOR3D, &point3d(mi));
        }
    }
}

// ---------------------------------------------------------------------------
// Writing (binary)

struct W {
    out: Vec<u8>,
}

impl W {
    fn obj(&mut self, tag: u16, id: u32) {
        self.out.extend_from_slice(&tag.to_le_bytes());
        self.out.extend_from_slice(&id.to_le_bytes());
    }
    fn end(&mut self) {
        self.out.extend_from_slice(&0u16.to_le_bytes());
    }
    fn prop(&mut self, tag: u16, data: &[u8]) {
        self.out.extend_from_slice(&tag.to_le_bytes());
        if data.len() >= 0xFFFF {
            self.out.extend_from_slice(&0xFFFFu16.to_le_bytes());
            self.out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        } else {
            self.out.extend_from_slice(&(data.len() as u16).to_le_bytes());
        }
        self.out.extend_from_slice(data);
    }
    fn i16(&mut self, tag: u16, v: i16) {
        self.prop(tag, &v.to_le_bytes());
    }
    fn u32(&mut self, tag: u16, v: u32) {
        self.prop(tag, &v.to_le_bytes());
    }
    fn raw(&mut self, d: &[u8]) {
        self.out.extend_from_slice(d);
    }
    fn props(&mut self, ps: &[(u16, Vec<u8>)]) {
        for (t, d) in ps {
            self.prop(*t, d);
        }
    }
}

fn fx32(v: f32) -> [u8; 4] {
    ((v * FIX).round() as i32).to_le_bytes()
}

fn point2d(p: Pos2) -> Vec<u8> {
    let mut v = fx32(p.y).to_vec();
    v.extend_from_slice(&fx32(p.x));
    v
}

fn point3d(p: Pos2) -> Vec<u8> {
    let mut v = fx32(p.x).to_vec();
    v.extend_from_slice(&fx32(p.y));
    v.extend_from_slice(&fx32(0.0));
    v
}

fn rect4(top: f32, left: f32, bottom: f32, right: f32) -> Vec<u8> {
    let mut v = Vec::new();
    for x in [top, left, bottom, right] {
        v.extend_from_slice(&fx32(x));
    }
    v
}

pub fn write_cdx(doc: &Doc) -> Vec<u8> {
    let mut cx = Ctx::new(doc);
    let fx = doc.file.as_deref();
    let mut body = W { out: Vec::new() };
    let page_id = cx.id(fx.map(|f| f.page_id).filter(|i| *i != 0));
    body.obj(T_PAGE, page_id);
    if let Some(f) = fx {
        body.props(&f.cdx_page_props);
    }
    if !fx.map(|f| f.cdx_page_props.iter().any(|p| p.0 == P_BBOX)).unwrap_or(false) {
        let (pw, ph, nw, nh) = cx.page_box(doc);
        body.prop(P_BBOX, &rect4(0.0, 0.0, ph, pw));
        body.i16(P_HEIGHTPAGES, nh);
        body.i16(P_WIDTHPAGES, nw);
    }
    let table = fx.and_then(|f| f.cdx_doc_props.iter().find(|p| p.0 == P_FONTTABLE)).map(|p| cdx_fonts(&p.1));
    let (sans_id, serif_id) = font_ids(table.as_deref());
    let font_id = if doc.style.label_font == LabelFont::Serif { serif_id } else { sans_id };
    let frag_extra: HashMap<u32, (Vec<(u16, Vec<u8>)>, Vec<Vec<u8>>)> =
        fx.map(|f| f.cdx_frags.iter().map(|(i, p, c)| (*i, (p.clone(), c.clone()))).collect()).unwrap_or_default();
    let atom_ids: Vec<u32> = doc.atoms.iter().map(|a| cx.id(a.extra.as_ref().map(|e| e.id))).collect();
    let bond_ids: Vec<u32> = doc.bonds.iter().map(|b| cx.id(b.extra.as_ref().map(|e| e.id))).collect();
    for (frag, fid) in fragments_with_ids(doc) {
        let id = cx.id(fid);
        body.obj(T_FRAGMENT, id);
        if let Some((p, _)) = fid.and_then(|f| frag_extra.get(&f)) {
            body.props(p);
        }
        for &i in &frag {
            let a = &doc.atoms[i];
            body.obj(T_NODE, atom_ids[i]);
            let p = cx.p(a.pos);
            body.prop(P_POS2D, &point2d(p));
            let nt = node_type(a);
            if nt != 1 {
                body.i16(P_NODETYPE, nt);
            }
            if a.label.is_none() && a.el != elements::C {
                body.i16(P_ELEMENT, a.el as i16);
            }
            if a.charge != 0 {
                body.prop(P_CHARGE, &[a.charge as u8]);
            }
            if a.isotope != 0 {
                body.i16(P_ISOTOPE, a.isotope as i16);
            }
            if a.radical != 0 {
                body.prop(P_RADICAL, &[if a.radical == 1 { 2 } else { 3 }]);
            }
            if let Some(ci) = cx.color_index(a.color) {
                body.i16(P_FGCOLOR, ci as i16);
            }
            if let Some(e) = &a.extra {
                body.props(&e.cdx_props);
            }
            // A monomer made in Formula: its connection type (peptide, RNA, terminus).
            if let Some(m) = &a.monomer {
                if !a.extra.as_ref().is_some_and(|e| e.cdx_props.iter().any(|p| p.0 == P_EXT_CONNECTION)) {
                    let ct: u8 = match (m.role, m.polymer) {
                        (MonomerRole::Terminus, _) => 9,
                        (MonomerRole::Branch, _) => 12,
                        (_, Polymer::Nucleic) => 8,
                        _ => 6,
                    };
                    body.prop(P_EXT_CONNECTION, &[ct]);
                }
            }
            if let Some(text) = label_display(doc, i) {
                // A scaled structure's labels are written at their real size.
                let size = doc.style.label_size * a.size / cx.scale;
                let tid = cx.id(a.extra.as_ref().map(|e| e.text_id).filter(|i| *i != 0));
                body.obj(T_TEXT, tid);
                body.prop(P_POS2D, &point2d(pos2(p.x - size * 0.35, p.y + size * 0.36)));
                body.prop(P_TEXT, &make_cdx_string(&text, font_id, 0x60, size, 0));
                body.end();
            }
            let reuse_nested = a.extra.as_ref().map(|e| e.source_label == a.label).unwrap_or(false);
            if reuse_nested {
                for c in &a.extra.as_ref().unwrap().cdx_children {
                    body.raw(c);
                }
            } else if let Some(m) = a.monomer.as_ref().filter(|m| m.role == MonomerRole::Unit) {
                // A monomer made in Formula: the original's tags (type 2 number, 3 text).
                for (name, ty, v) in monomer_tags(m) {
                    let id = cx.id(None);
                    body.obj(T_OBJECTTAG, id);
                    if ty == "Long" {
                        body.i16(P_TAG_TYPE, 2);
                        body.prop(0x0011, &[0]);
                        body.prop(P_TAG_NAME, name.as_bytes());
                        body.prop(P_TAG_VALUE, &v.parse::<i32>().unwrap_or(0).to_le_bytes());
                    } else {
                        body.i16(P_TAG_TYPE, 3);
                        body.prop(0x0011, &[0]);
                        body.prop(P_TAG_NAME, name.as_bytes());
                        let mut s = vec![0u8, 0u8];
                        s.extend_from_slice(v.as_bytes());
                        body.prop(P_TAG_VALUE, &s);
                    }
                    body.end();
                }
            } else if nt == 5 && a.monomer.is_none() {
                if let Some(nested) = nested_fragment_cdx(a, &mut cx, p) {
                    body.raw(&nested);
                }
            }
            body.end();
        }
        for (bi, b) in doc.bonds.iter().enumerate() {
            if !frag.contains(&b.a) {
                continue;
            }
            body.obj(T_BOND, bond_ids[bi]);
            body.u32(P_BEGIN, atom_ids[b.a]);
            body.u32(P_END, atom_ids[b.b]);
            if b.order != 1 {
                body.i16(P_ORDER, if b.order == 3 { 4 } else { 2 });
            }
            let disp = match b.stereo {
                Stereo::Dashed => 1,
                Stereo::Hash => 3,
                Stereo::Hashed => 2,
                Stereo::Bold => 5,
                Stereo::Wedge => 6,
                Stereo::Wavy => 8,
                Stereo::None => 0,
            };
            if disp != 0 {
                body.i16(P_DISPLAY, disp);
            }
            match b.dbl {
                DblPos::Center => body.i16(P_DBLPOS, 0x100),
                DblPos::Right => body.i16(P_DBLPOS, 0x101),
                DblPos::Left => body.i16(P_DBLPOS, 0x102),
                DblPos::Auto => {}
            }
            if let Some(ci) = cx.color_index(b.color) {
                body.i16(P_FGCOLOR, ci as i16);
            }
            if let Some(e) = &b.extra {
                body.props(&e.cdx_props);
                for c in &e.cdx_children {
                    body.raw(c);
                }
            }
            body.end();
        }
        if let Some((_, kids)) = fid.and_then(|f| frag_extra.get(&f)) {
            for k in kids {
                body.raw(k);
            }
        }
        body.end();
    }
    // ChemDraw shows a charge through a "+"/"−" symbol linked to the atom's charge.
    for (i, a) in doc.atoms.iter().enumerate() {
        if a.charge == 0 {
            continue;
        }
        // The same number on every save: derived from the atom's.
        let id = cx.id(Some(atom_ids[i] | 0x0100_0000));
        body.obj(T_GRAPHIC, id);
        let size = doc.style.label_size * a.size / cx.scale;
        let p = cx.p(a.pos);
        let s = pos2(p.x + size * 0.55, p.y - size * 0.55);
        body.prop(P_BBOX, &rect4(s.y, s.x, s.y, s.x));
        let mut rep = atom_ids[i].to_le_bytes().to_vec();
        rep.extend_from_slice(&P_CHARGE.to_le_bytes());
        body.prop(P_REPRESENTS, &rep);
        body.i16(P_GRAPHICTYPE, 7);
        body.i16(P_SYMBOLTYPE, if a.charge > 0 { 8 } else { 9 });
        body.end();
    }
    for c in &doc.captions {
        let id = cx.id(c.extra.as_ref().map(|e| e.id));
        body.obj(T_TEXT, id);
        let size = c.size / cx.scale;
        let p = cx.p(c.pos);
        body.prop(P_POS2D, &point2d(pos2(p.x, p.y + size * 0.8)));
        let unchanged = c.extra.as_ref().map(|e| e.source_label.as_deref() == Some(c.text.as_str())).unwrap_or(false);
        if let Some(e) = &c.extra {
            for (t, d) in &e.cdx_props {
                if *t == P_TEXT && !unchanged {
                    continue;
                }
                body.prop(*t, d);
            }
        }
        if !unchanged {
            let face = (if c.bold { 1 } else { 0 }) | (if c.italic { 2 } else { 0 }) | (if c.formula { 0x60 } else { 0 });
            let ci = cx.color_index(c.color).unwrap_or(0);
            body.prop(P_TEXT, &make_cdx_string(&c.text, if c.serif { serif_id } else { sans_id }, face, size, ci));
        }
        if let Some(e) = &c.extra {
            for k in &e.cdx_children {
                body.raw(k);
            }
        }
        body.end();
    }
    for a in &doc.arrows {
        let keep = same_kind(a);
        let is_object = a.extra.as_ref().map(|e| e.frag == FROM_ARROW_OBJECT).unwrap_or(false);
        let head = cx.p(a.to);
        let tail = cx.p(a.from);
        let id = cx.id(a.extra.as_ref().map(|e| e.id));
        // A plain line graphic read from a file and left as it was stays a graphic; every other
        // arrow is written as the original's arrow object, which holds its full style.
        if is_object || !keep {
            body.obj(T_ARROW, id);
            if keep {
                if let Some(e) = &a.extra {
                    body.props(&e.cdx_props);
                }
            }
            CdArrow::from_formula(a, &cx).write_cdx(&mut body);
            if let Some(ci) = cx.color_index(a.color) {
                body.i16(P_FGCOLOR, ci as i16);
            }
            if keep {
                if let Some(e) = &a.extra {
                    for k in &e.cdx_children {
                        body.raw(k);
                    }
                }
            }
            body.end();
            continue;
        } else {
            body.obj(T_GRAPHIC, id);
            body.prop(P_BBOX, &rect4(head.y, head.x, tail.y, tail.x));
            body.i16(P_GRAPHICTYPE, 1);
            let at = match a.kind {
                ArrowKind::Line => 0,
                ArrowKind::Resonance => 4,
                ArrowKind::Equilibrium => 8,
                ArrowKind::Retro => 32,
                ArrowKind::NoGo => 64,
                _ => 2,
            };
            body.i16(P_ARROWTYPE, at);
        }
        if let Some(ci) = cx.color_index(a.color) {
            body.i16(P_FGCOLOR, ci as i16);
        }
        if keep {
            if let Some(e) = &a.extra {
                body.props(&e.cdx_props);
                for k in &e.cdx_children {
                    body.raw(k);
                }
            }
        }
        body.end();
    }
    for s in &doc.shapes {
        if s.is_bracket() {
            write_bracket_cdx(&mut body, &mut cx, doc, s, &atom_ids, &bond_ids, font_id);
            continue;
        }
        let id = cx.id(s.extra.as_ref().map(|e| e.id));
        body.obj(T_GRAPHIC, id);
        let a = cx.p(s.min);
        let b = cx.p(s.max);
        body.prop(P_BBOX, &rect4(a.y, a.x, b.y, b.x));
        match s.kind {
            ShapeKind::Rect | ShapeKind::RoundRect => {
                body.i16(P_GRAPHICTYPE, 3);
                if s.kind == ShapeKind::RoundRect {
                    body.i16(P_RECTTYPE, 1);
                }
            }
            ShapeKind::Ellipse => {
                body.i16(P_GRAPHICTYPE, 4);
                let c = pos2((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
                body.prop(P_CENTER3D, &point3d(c));
                body.prop(P_MAJOR3D, &point3d(pos2(b.x, c.y)));
                body.prop(P_MINOR3D, &point3d(pos2(c.x, b.y)));
            }
            ShapeKind::Brackets | ShapeKind::Parens => unreachable!(),
        }
        if let Some(ci) = cx.color_index(s.color) {
            body.i16(P_FGCOLOR, ci as i16);
        }
        if let Some(e) = &s.extra {
            body.props(&e.cdx_props);
            for k in &e.cdx_children {
                body.raw(k);
            }
        }
        body.end();
    }
    if let Some(f) = fx {
        for k in &f.cdx_page_children {
            body.raw(k);
        }
    }
    body.end(); // page

    let mut w = W { out: Vec::new() };
    // Signature, byte-order mark and 10 reserved bytes; the document object's tag and id
    // complete the 28-byte header.
    w.raw(HEADER);
    w.raw(&[0x04, 0x03, 0x02, 0x01]);
    w.raw(&[0u8; 10]);
    w.obj(T_DOCUMENT, 0);
    let has_fonts = fx.map(|f| f.cdx_doc_props.iter().any(|p| p.0 == P_FONTTABLE)).unwrap_or(false);
    let mut doc_props = fx.map(|f| f.cdx_doc_props.clone()).unwrap_or_default();
    let caption_id = if doc.style.caption_font == LabelFont::Serif { serif_id } else { sans_id };
    style_props(doc, cx.scale, font_id, caption_id, &mut doc_props);
    w.props(&doc_props);
    if !has_fonts {
        let mut ft = Vec::new();
        ft.extend_from_slice(&1u16.to_le_bytes()); // Windows platform
        ft.extend_from_slice(&2u16.to_le_bytes()); // two fonts
        for (id, name) in [(sans_id, "Arial"), (serif_id, "Times New Roman")] {
            ft.extend_from_slice(&id.to_le_bytes());
            ft.extend_from_slice(&1252u16.to_le_bytes());
            ft.extend_from_slice(&(name.len() as u16).to_le_bytes());
            ft.extend_from_slice(name.as_bytes());
        }
        w.prop(P_FONTTABLE, &ft);
    }
    let mut ct = Vec::new();
    ct.extend_from_slice(&(cx.colors.len() as u16).to_le_bytes());
    for c in &cx.colors {
        for v in c {
            ct.extend_from_slice(&((*v as u16) * 257).to_le_bytes());
        }
    }
    w.prop(P_COLORTABLE, &ct);
    w.raw(&body.out);
    if let Some(f) = fx {
        for k in &f.cdx_doc_children {
            w.raw(k);
        }
    }
    w.end(); // document
    w.end(); // end of file
    w.out
}

/// A nested fragment for an abbreviation, attached through a connection point.
fn nested_fragment_cdx(a: &Atom, cx: &mut Ctx, at: Pos2) -> Option<Vec<u8>> {
    let smi = chem::expansion_smiles(a.label.as_ref()?)?;
    let g = crate::formats::parse_smiles(smi).ok()?;
    let mut w = W { out: Vec::new() };
    let fid = cx.id(None);
    w.obj(T_FRAGMENT, fid);
    let ids: Vec<u32> = (0..g.atoms.len()).map(|_| cx.id(None)).collect();
    let origin = g.atoms[0].pos;
    for (k, ga) in g.atoms.iter().enumerate() {
        w.obj(T_NODE, ids[k]);
        w.prop(P_POS2D, &point2d(at + (ga.pos - origin) / cx.scale));
        if ga.el != elements::C {
            w.i16(P_ELEMENT, ga.el as i16);
        }
        // The label's own charge ("P(Ph)3" with +) sits on the group's attaching atom, where
        // ChemDraw keeps it.
        let charge = if k == 0 { (ga.charge + a.charge).clamp(-9, 9) } else { ga.charge };
        if charge != 0 {
            w.prop(P_CHARGE, &[charge as u8]);
        }
        w.end();
    }
    let ext = cx.id(None);
    w.obj(T_NODE, ext);
    w.prop(P_POS2D, &point2d(at - vec2(BOND_LEN, 0.0) / cx.scale));
    w.i16(P_NODETYPE, 12);
    w.end();
    let mut bonds: Vec<(u32, u32, u8)> = g.bonds.iter().map(|b| (ids[b.a], ids[b.b], b.order)).collect();
    bonds.push((ext, ids[0], 1));
    for (x, y, o) in bonds {
        let id = cx.id(None);
        w.obj(T_BOND, id);
        w.u32(P_BEGIN, x);
        w.u32(P_END, y);
        if o != 1 {
            w.i16(P_ORDER, if o == 3 { 4 } else { 2 });
        }
        w.end();
    }
    w.end();
    Some(w.out)
}

// ---------------------------------------------------------------------------
// CDXML

fn nums(s: Option<&str>) -> Vec<f32> {
    s.unwrap_or("").split_whitespace().filter_map(|x| x.parse().ok()).collect()
}

fn xml_colors(root: &Node) -> Vec<Rgb> {
    root.elements()
        .find(|e| e.name == "colortable")
        .map(|t| {
            t.elements()
                .filter(|c| c.name == "color")
                .map(|c| {
                    let f = |k: &str| (c.attr(k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0) * 255.0).round().clamp(0.0, 255.0) as u8;
                    [f("r"), f("g"), f("b")]
                })
                .collect()
        })
        .unwrap_or_default()
}

fn id_of(n: &Node) -> u32 {
    n.attr("id").and_then(|v| v.parse().ok()).unwrap_or(0)
}

fn keep_attrs(n: &Node, handled: &[&str]) -> Vec<(String, String)> {
    n.attrs.iter().filter(|(k, _)| !handled.contains(&k.as_str())).cloned().collect()
}

pub fn read_cdxml(text: &str) -> Result<Doc, String> {
    let root = xml::parse(text)?;
    if root.name != "CDXML" {
        return Err("This is not a CDXML file.".into());
    }
    let mut r = XReader { colors: xml_colors(&root), serif_fonts: serif_ids(&xml_fonts(&root)), ..Default::default() };
    r.fx.colors = r.colors.clone();
    r.fx.xml_doc_attrs = root.attrs.clone();
    let mut first_page = true;
    for c in root.elements() {
        if c.name == "page" && first_page {
            first_page = false;
            r.fx.xml_page_attrs = c.attrs.clone();
            r.page(c);
        } else if c.name != "colortable" {
            r.fx.xml_doc_children.push(xml::to_string(c));
        }
    }
    r.finish()
}

#[derive(Default)]
struct XReader {
    doc: Doc,
    fx: FileExtra,
    colors: Vec<Rgb>,
    ids: HashMap<u32, usize>,
    bonds: Vec<(Node, u32)>,
    graphic_arrows: Vec<Arrow>,
    object_arrows: Vec<Arrow>,
    /// Line graphics replaced by an arrow object (graphic id → arrow id).
    superseded: HashMap<u32, u32>,
    max_id: u32,
    sides: Vec<BracketSide>,
    groups: Vec<BracketGroupRec>,
    serif_fonts: Vec<u16>,
    label_pt: Vec<(usize, f32)>,
}

impl XReader {
    fn color(&self, n: &Node) -> Option<Rgb> {
        n.attr("color").and_then(|v| v.parse::<i32>().ok()).and_then(|i| color_from_index(&self.colors, i))
    }
    fn page(&mut self, p: &Node) {
        for c in p.elements() {
            self.max_id = self.max_id.max(id_of(c));
            match c.name.as_str() {
                "fragment" => self.fragment(c),
                "group" => self.page(c),
                "t" => self.caption(c),
                "graphic" => {
                    // A charge symbol linked to an atom is rebuilt from the atom when saving.
                    let text = xml::to_string(c);
                    let charge_symbol = c.attr("GraphicType") == Some("Symbol") && text.contains("<represent") && (text.contains("\"Charge\"") || text.contains("\"Radical\""));
                    if !charge_symbol && !self.graphic(c) {
                        self.fx.xml_page_children.push(text);
                    }
                }
                "arrow" => {
                    if !self.arrow(c) {
                        self.fx.xml_page_children.push(xml::to_string(c));
                    }
                }
                "bracketedgroup" => {
                    let graphics = c
                        .elements()
                        .filter(|a| a.name == "bracketattachment")
                        .filter_map(|a| a.attr("GraphicID").and_then(|v| v.parse().ok()))
                        .collect();
                    let label = c
                        .attr("SRULabel")
                        .and_then(clean_label)
                        .or_else(|| c.attr("RepeatCount").and_then(|v| v.parse::<f64>().ok()).and_then(count_label));
                    self.groups.push(BracketGroupRec { id: id_of(c), graphics, label, raw: Raw::Xml(xml::to_string(c)) });
                }
                _ => self.fx.xml_page_children.push(xml::to_string(c)),
            }
        }
    }
    fn fragment(&mut self, f: &Node) {
        let fid = id_of(f);
        let mut unknown = Vec::new();
        for c in f.elements() {
            self.max_id = self.max_id.max(id_of(c));
            match c.name.as_str() {
                "n" => self.node(c, fid),
                "b" => self.bonds.push((c.clone(), fid)),
                "fragment" => self.fragment(c),
                "graphic" if c.attr("GraphicType") == Some("Bracket") && nums(c.attr("BoundingBox")).len() >= 4 => {
                    self.graphic(c);
                }
                _ => unknown.push(xml::to_string(c)),
            }
        }
        self.fx.xml_frags.push((fid, keep_attrs(f, &["BoundingBox", "id"]), unknown));
    }
    fn node(&mut self, n: &Node, frag: u32) {
        let p = nums(n.attr("p"));
        let pos = pos2(*p.first().unwrap_or(&0.0), *p.get(1).unwrap_or(&0.0));
        let ntype = n.attr("NodeType").unwrap_or("Element");
        let mut a = Atom::new(pos, n.attr("Element").and_then(|v| v.parse::<u8>().ok()).unwrap_or(elements::C));
        a.charge = n.attr("Charge").and_then(|v| v.parse().ok()).unwrap_or(0);
        a.isotope = n.attr("Isotope").and_then(|v| v.parse().ok()).unwrap_or(0);
        a.radical = match n.attr("Radical") {
            Some("Doublet") => 1,
            Some("Singlet") | Some("Triplet") => 2,
            _ => 0,
        };
        a.color = self.color(n);
        let label_text = n.elements().find(|c| c.name == "t").map(|t| t.text());
        if let Some(v) = n.attr(ATOM_SIZE_NOTE).and_then(|v| v.trim().parse::<f32>().ok()).filter(|v| *v > 0.0) {
            a.size = v;
        }
        let marks = nums(n.attr(MARKS_NOTE));
        if marks.len() == 4 {
            a.charge_off = [marks[0], marks[1]];
            a.iso_off = [marks[2], marks[3]];
        }
        if let Some(pt) = n.elements().find(|c| c.name == "t").and_then(|t| t.elements().find(|s| s.name == "s")).and_then(|s| s.attr("size")).and_then(|v| v.parse::<f32>().ok()).filter(|v| *v > 0.5) {
            self.label_pt.push((self.doc.atoms.len(), pt));
        }
        if ntype == "ExternalConnectionPoint" {
            a.el = 0;
            a.label = Some("*".into());
        } else if ntype != "Element" {
            if let Some(t) = &label_text {
                let (z, l) = interpret_label(t);
                a.el = z;
                a.label = l.or_else(|| if ntype == "Unspecified" { None } else { Some(t.trim().to_string()) });
            }
        } else if a.el == elements::C && label_text.as_deref().is_some_and(|t| !t.trim().is_empty()) {
            // A carbon written out ("CH3"): keep showing it, the H count following its bonds.
            a.show = true;
        }
        // A monomer of a biopolymer, or a cap of its chain: its code is not an element.
        let tags: HashMap<String, String> = n
            .elements()
            .filter(|c| c.name == "objecttag")
            .filter_map(|c| Some((c.attr("Name")?.to_string(), c.attr("Value").unwrap_or("").to_string())))
            .collect();
        if let Some(m) = monomer_of(ntype, n.attr("ExternalConnectionType").unwrap_or(""), &tags, label_text.as_deref().unwrap_or("")) {
            a.el = 0;
            a.show = false;
            a.label = Some(if m.role == MonomerRole::Unit { m.symbol.clone() } else { label_text.as_deref().unwrap_or("H").trim().to_string() });
            a.monomer = Some(Box::new(m));
        }
        let extra = Extra {
            id: id_of(n),
            frag,
            xml_attrs: keep_attrs(n, &["id", "p", "NodeType", "Element", "Charge", "Isotope", "Radical", "color", "NumHydrogens", "BoundingBox"]),
            xml_children: n.elements().filter(|c| c.name != "t").map(xml::to_string).collect(),
            source_label: a.label.clone(),
            text_id: n.elements().find(|c| c.name == "t").map(id_of).unwrap_or(0),
            ..Default::default()
        };
        self.ids.insert(extra.id, self.doc.atoms.len());
        a.extra = Some(Box::new(extra));
        self.doc.atoms.push(a);
    }
    fn caption(&mut self, t: &Node) {
        let p = nums(t.attr("p"));
        let base = pos2(*p.first().unwrap_or(&0.0), *p.get(1).unwrap_or(&0.0));
        let first = t.elements().find(|c| c.name == "s");
        let size = first.and_then(|s| s.attr("size")).and_then(|v| v.parse().ok()).unwrap_or(10.0f32);
        let face: u32 = first.and_then(|s| s.attr("face")).and_then(|v| v.parse().ok()).unwrap_or(0);
        let font: Option<u16> = first.and_then(|s| s.attr("font")).and_then(|v| v.parse().ok());
        let text = t.text();
        self.doc.captions.push(Caption {
            pos: pos2(base.x, base.y - size * 0.8),
            text: text.clone(),
            size,
            bold: face & 1 != 0,
            italic: face & 2 != 0,
            serif: font.is_some_and(|f| self.serif_fonts.contains(&f)),
            formula: face & 0x60 == 0x60,
            color: first.and_then(|s| s.attr("color")).and_then(|v| v.parse::<i32>().ok()).and_then(|i| color_from_index(&self.colors, i)),
            seq: crate::sequence::detect(&text),
            extra: Some(Box::new(Extra {
                id: id_of(t),
                xml_attrs: keep_attrs(t, &["id", "p", "BoundingBox"]),
                xml_children: t.elements().map(xml::to_string).collect(),
                source_label: Some(text),
                ..Default::default()
            })),
        });
    }
    fn graphic(&mut self, g: &Node) -> bool {
        let bb = nums(g.attr("BoundingBox"));
        if bb.len() < 4 {
            return false;
        }
        let p1 = pos2(bb[0], bb[1]);
        let p2 = pos2(bb[2], bb[3]);
        let color = self.color(g);
        // Replaced by an arrow object: the object is read instead.
        if let Some(by) = g.attr("SupersededBy").and_then(|v| v.trim().parse::<u32>().ok()) {
            self.superseded.insert(id_of(g), by);
            return true;
        }
        let mut extra = Extra {
            id: id_of(g),
            xml_attrs: keep_attrs(g, &["id", "BoundingBox", "GraphicType", "ArrowType", "RectangleType", "color", "Center3D", "MajorAxisEnd3D", "MinorAxisEnd3D", "SupersededBy"]),
            xml_children: g.elements().map(xml::to_string).collect(),
            ..Default::default()
        };
        match g.attr("GraphicType").unwrap_or("") {
            "Line" => {
                let kind = match g.attr("ArrowType").unwrap_or("NoHead") {
                    "NoHead" => ArrowKind::Line,
                    "Resonance" => ArrowKind::Resonance,
                    "Equilibrium" => ArrowKind::Equilibrium,
                    "RetroSynthetic" => ArrowKind::Retro,
                    "NoGo" => ArrowKind::NoGo,
                    _ => ArrowKind::Reaction,
                };
                extra.source_label = Some(format!("{:?}", kind));
                // Marks this as a line graphic (not an arrow object) for writing back.
                extra.xml_attrs.insert(0, ("#graphic".into(), String::new()));
                self.graphic_arrows.push(Arrow { from: p2, to: p1, kind, color, extra: Some(Box::new(extra)), style: Default::default() });
                true
            }
            "Rectangle" => {
                let kind = if g.attr("RectangleType").map(|v| v.contains("RoundEdge")).unwrap_or(false) { ShapeKind::RoundRect } else { ShapeKind::Rect };
                let r = egui::Rect::from_two_pos(p1, p2);
                self.doc.shapes.push(Shape { color, extra: Some(Box::new(extra)), ..Shape::new(r.min, r.max, kind) });
                true
            }
            "Bracket" => {
                let bt = g.attr("BracketType").unwrap_or("RoundPair");
                let kind = if bt.contains("Round") { ShapeKind::Parens } else { ShapeKind::Brackets };
                if !extra.xml_attrs.iter().any(|(k, _)| k == "BracketType") {
                    extra.xml_attrs.push(("BracketType".into(), bt.into()));
                }
                let label = xml_bracket_label(g);
                extra.source_label = label.clone();
                self.sides.push(BracketSide { a: p1, b: p2, kind, single: !bt.ends_with("Pair"), label, color, extra, raw: Raw::Xml(xml::to_string(g)) });
                true
            }
            "Oval" => {
                let c = nums(g.attr("Center3D"));
                let ma = nums(g.attr("MajorAxisEnd3D"));
                let mi = nums(g.attr("MinorAxisEnd3D"));
                if c.len() < 2 || ma.len() < 2 || mi.len() < 2 {
                    return false;
                }
                let cc = pos2(c[0], c[1]);
                let rx = (pos2(ma[0], ma[1]) - cc).length();
                let ry = (pos2(mi[0], mi[1]) - cc).length();
                self.doc.shapes.push(Shape { color, extra: Some(Box::new(extra)), ..Shape::new(cc - vec2(rx, ry), cc + vec2(rx, ry), ShapeKind::Ellipse) });
                true
            }
            _ => false,
        }
    }
    fn arrow(&mut self, a: &Node) -> bool {
        let h = nums(a.attr("Head3D"));
        let t = nums(a.attr("Tail3D"));
        if h.len() < 2 || t.len() < 2 {
            return false;
        }
        // Files written by Formula 0.0.4 and earlier carry Formula's own note of the style;
        // otherwise the arrow is read from the original's attributes.
        let (kind, style, from, to) = match a.attr(ARROW_NOTE).and_then(parse_arrow_note) {
            Some((k, s)) => (k, s, pos2(t[0], t[1]), pos2(h[0], h[1])),
            None => match CdArrow::from_xml(a) {
                Some(cd) => cd.to_formula(),
                None => return false,
            },
        };
        let mut xml_attrs = keep_attrs(a, &["id", "color"]);
        xml_attrs.retain(|(k, _)| !ARROW_ATTRS.contains(&k.as_str()));
        // Mark as XML so a superseded line graphic doesn't override the kind.
        xml_attrs.insert(0, ("#arrow".into(), String::new()));
        let extra = Extra {
            id: id_of(a),
            frag: FROM_ARROW_OBJECT,
            xml_attrs,
            xml_children: a.elements().map(xml::to_string).collect(),
            source_label: Some(arrow_label(kind, &style)),
            ..Default::default()
        };
        let color = self.color(a);
        self.object_arrows.push(Arrow { from, to, kind, color, extra: Some(Box::new(extra)), style });
        true
    }
    fn finish(mut self) -> Result<Doc, String> {
        for (b, frag) in std::mem::take(&mut self.bonds) {
            let bg: u32 = b.attr("B").and_then(|v| v.parse().ok()).unwrap_or(0);
            let en: u32 = b.attr("E").and_then(|v| v.parse().ok()).unwrap_or(0);
            let (Some(&x), Some(&y)) = (self.ids.get(&bg), self.ids.get(&en)) else { continue };
            let ord = match b.attr("Order").unwrap_or("1") {
                "2" => 2,
                "3" => 4,
                _ => 1,
            };
            let disp = match b.attr("Display").unwrap_or("") {
                "Dash" => 1,
                "Hash" => 2,
                "WedgedHashBegin" => 3,
                "WedgedHashEnd" => 4,
                "Bold" => 5,
                "WedgeBegin" => 6,
                "WedgeEnd" => 7,
                "Wavy" => 8,
                _ => 0,
            };
            let dp = b.attr("DoublePosition").map(|v| match v {
                "Center" => 0x100,
                "Right" => 0x101,
                "Left" => 0x102,
                _ => 0,
            });
            let color = self.color(&b);
            let mut bond = make_bond(x, y, ord, disp, dp, color);
            bond.extra = Some(Box::new(Extra {
                id: id_of(&b),
                frag,
                xml_attrs: keep_attrs(&b, &["id", "B", "E", "Order", "Display", "DoublePosition", "color", "BoundingBox"]),
                xml_children: b.elements().map(xml::to_string).collect(),
                ..Default::default()
            }));
            self.doc.bonds.push(bond);
        }
        let r = Reader {
            doc: self.doc,
            fx: FileExtra { max_id: self.max_id, ..self.fx },
            graphic_arrows: self.graphic_arrows,
            object_arrows: self.object_arrows,
            superseded: self.superseded,
            max_id: self.max_id,
            sides: self.sides,
            groups: self.groups,
            serif_fonts: self.serif_fonts,
            label_pt: self.label_pt,
            ..Default::default()
        };
        r.finish_common()
    }
}

/// Colour channel with enough digits to come back as the same 0-255 value.
fn f4(v: f32) -> String {
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s.is_empty() { "0".into() } else { s }
}

fn f2(v: f32) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" { "0".into() } else { s }
}

fn push_raw(children: &mut Vec<Child>, raws: &[String]) {
    for r in raws {
        if let Ok(n) = xml::parse(r) {
            children.push(Child::El(n));
        }
    }
}

fn clean_attrs(v: &[(String, String)]) -> impl Iterator<Item = (String, String)> + '_ {
    v.iter().filter(|(k, _)| !k.starts_with('#')).cloned()
}

pub fn write_cdxml(doc: &Doc) -> String {
    let mut cx = Ctx::new(doc);
    let fx = doc.file.as_deref();
    let mut root = Node { name: "CDXML".into(), ..Default::default() };
    if let Some(f) = fx {
        root.attrs = f.xml_doc_attrs.clone();
    }
    if root.attr("BondLength").is_none() {
        root.attrs.push(("BondLength".into(), f2(BOND_LEN / cx.scale)));
    }
    // The file's own font table, if it has one.
    let table: Option<Vec<(u16, String)>> = fx
        .and_then(|f| f.xml_doc_children.iter().find(|c| c.trim_start().starts_with("<fonttable")))
        .and_then(|c| xml::parse(c).ok())
        .map(|t| t.elements().filter(|c| c.name == "font").filter_map(|f| Some((f.attr("id")?.parse().ok()?, f.attr("name").unwrap_or("").to_string()))).collect());
    let (sans_id, serif_id) = font_ids(table.as_deref());
    let serif_set = table.as_deref().map(serif_ids).unwrap_or_else(|| vec![4]);
    let label_id = if doc.style.label_font == LabelFont::Serif { serif_id } else { sans_id };
    let caption_id = if doc.style.caption_font == LabelFont::Serif { serif_id } else { sans_id };
    if root.attr("LabelSize").is_none() {
        root.attrs.push(("LabelFont".into(), label_id.to_string()));
        root.attrs.push(("LabelSize".into(), f2(doc.style.label_size / cx.scale)));
        root.attrs.push(("CaptionFont".into(), caption_id.to_string()));
        root.attrs.push(("CaptionSize".into(), f2(doc.style.caption_size / cx.scale)));
    } else {
        // Keep the file's fonts unless the style asks for the other kind.
        for (k, want) in [("LabelFont", label_id), ("CaptionFont", caption_id)] {
            match root.attrs.iter_mut().find(|(a, _)| a == k) {
                Some((_, v)) => {
                    let was: u16 = v.trim().parse().unwrap_or(0);
                    if serif_set.contains(&was) != serif_set.contains(&want) {
                        *v = want.to_string();
                    }
                }
                None if want != sans_id => root.attrs.push((k.into(), want.to_string())),
                None => {}
            }
        }
    }
    {
        let st = &doc.style;
        let k = cx.scale;
        set_num_attr(&mut root.attrs, "BondLength", BOND_LEN / k);
        set_num_attr(&mut root.attrs, "LineWidth", st.line_width / k);
        set_num_attr(&mut root.attrs, "BoldWidth", st.bold_width / k);
        set_num_attr(&mut root.attrs, "HashSpacing", st.hash_spacing / k);
        set_num_attr(&mut root.attrs, "MarginWidth", st.margin / k);
        set_num_attr(&mut root.attrs, "BondSpacing", st.dbl_spacing * 100.0);
        set_num_attr(&mut root.attrs, "LabelSize", st.label_size / k);
        set_num_attr(&mut root.attrs, "CaptionSize", st.caption_size / k);
    }
    let mut page = Node { name: "page".into(), ..Default::default() };
    page.attrs = fx.map(|f| f.xml_page_attrs.clone()).unwrap_or_default();
    if page.attr("BoundingBox").is_none() {
        let (pw, ph, nw, nh) = cx.page_box(doc);
        page.attrs.push(("BoundingBox".into(), format!("0 0 {} {}", f2(pw), f2(ph))));
        page.attrs.push(("HeightPages".into(), nh.to_string()));
        page.attrs.push(("WidthPages".into(), nw.to_string()));
    }
    match page.attr("id").and_then(|v| v.parse::<u32>().ok()) {
        Some(pid) => {
            cx.used.insert(pid);
        }
        None => {
            let pid = cx.id(None);
            page.attrs.insert(0, ("id".into(), pid.to_string()));
        }
    }
    let frag_extra: HashMap<u32, (Vec<(String, String)>, Vec<String>)> =
        fx.map(|f| f.xml_frags.iter().map(|(i, a, c)| (*i, (a.clone(), c.clone()))).collect()).unwrap_or_default();
    let atom_ids: Vec<u32> = doc.atoms.iter().map(|a| cx.id(a.extra.as_ref().map(|e| e.id))).collect();
    let bond_ids: Vec<u32> = doc.bonds.iter().map(|b| cx.id(b.extra.as_ref().map(|e| e.id))).collect();
    let label_font = label_id.to_string();
    let label_font = label_font.as_str();
    for (frag, fid) in fragments_with_ids(doc) {
        let id = cx.id(fid);
        let mut fnode = xml::el("fragment", vec![("id", id.to_string())]);
        if let Some((a, _)) = fid.and_then(|f| frag_extra.get(&f)) {
            fnode.attrs.extend(a.iter().cloned());
        }
        if fnode.attr("SequenceType").is_none() && frag.iter().any(|&i| doc.atoms[i].monomer.is_some()) {
            fnode.attrs.push(("SequenceType".into(), "Biopolymer".into()));
        }
        for &i in &frag {
            let a = &doc.atoms[i];
            let p = cx.p(a.pos);
            let mut n = xml::el("n", vec![("id", atom_ids[i].to_string()), ("p", format!("{} {}", f2(p.x), f2(p.y)))]);
            let nt = node_type(a);
            match nt {
                5 => n.attrs.push(("NodeType".into(), "Fragment".into())),
                14 => n.attrs.push(("NodeType".into(), "Monomer".into())),
                7 => n.attrs.push(("NodeType".into(), "GenericNickname".into())),
                0 => n.attrs.push(("NodeType".into(), "Unspecified".into())),
                _ => {}
            }
            if a.label.is_none() && a.el != elements::C {
                n.attrs.push(("Element".into(), a.el.to_string()));
                n.attrs.push(("NumHydrogens".into(), doc.implicit_h(i).to_string()));
            }
            if a.charge != 0 {
                n.attrs.push(("Charge".into(), a.charge.to_string()));
            }
            if a.isotope != 0 {
                n.attrs.push(("Isotope".into(), a.isotope.to_string()));
            }
            if a.radical != 0 {
                n.attrs.push(("Radical".into(), if a.radical == 1 { "Doublet" } else { "Triplet" }.into()));
            }
            if let Some(ci) = cx.color_index(a.color) {
                n.attrs.push(("color".into(), ci.to_string()));
            }
            if let Some(e) = &a.extra {
                n.attrs.extend(clean_attrs(&e.xml_attrs).filter(|(k, _)| k != ATOM_SIZE_NOTE && k != MARKS_NOTE));
            }
            if (a.size - 1.0).abs() > 1e-4 {
                n.attrs.push((ATOM_SIZE_NOTE.into(), format!("{:.4}", a.size)));
            }
            if a.charge_off != [0.0; 2] || a.iso_off != [0.0; 2] {
                let s = cx.scale;
                n.attrs.push((MARKS_NOTE.into(), format!("{} {} {} {}", f2(a.charge_off[0] / s), f2(a.charge_off[1] / s), f2(a.iso_off[0] / s), f2(a.iso_off[1] / s))));
            }
            if let Some(text) = label_display(doc, i) {
                // A scaled structure's labels are written at their real size.
                let size = doc.style.label_size * a.size / cx.scale;
                let tid = cx.id(a.extra.as_ref().map(|e| e.text_id).filter(|i| *i != 0));
                let mut t = xml::el("t", vec![("id", tid.to_string()), ("p", format!("{} {}", f2(p.x - size * 0.35), f2(p.y + size * 0.36))), ("LabelJustification", "Left".into())]);
                let mut s = xml::el("s", vec![("font", label_font.into()), ("size", f2(size)), ("face", "96".into())]);
                s.children.push(Child::Text(text));
                t.children.push(Child::El(s));
                n.children.push(Child::El(t));
            }
            let reuse = a.extra.as_ref().map(|e| e.source_label == a.label).unwrap_or(false);
            // A monomer made in Formula: the original's connection type and monomer tags.
            if let (Some(m), false) = (&a.monomer, reuse) {
                if n.attr("ExternalConnectionType").is_none() {
                    let ct = match (m.role, m.polymer) {
                        (MonomerRole::Terminus, _) => "Terminus",
                        (MonomerRole::Branch, _) => "UnlinkedBranch",
                        (_, Polymer::Nucleic) => "RNA",
                        _ => "Peptide",
                    };
                    n.attrs.push(("ExternalConnectionType".into(), ct.into()));
                }
                if m.role == MonomerRole::Unit {
                    for (name, ty, v) in monomer_tags(m) {
                        n.children.push(Child::El(xml::el("objecttag", vec![("TagType", ty.into()), ("Visible", "no".into()), ("Name", name.into()), ("Value", v)])));
                    }
                }
            }
            if reuse {
                push_raw(&mut n.children, &a.extra.as_ref().unwrap().xml_children);
            } else if nt == 5 && a.monomer.is_none() {
                if let Some(nf) = nested_fragment_xml(a, &mut cx, p) {
                    n.children.push(Child::El(nf));
                }
            }
            fnode.children.push(Child::El(n));
        }
        for (bi, b) in doc.bonds.iter().enumerate() {
            if !frag.contains(&b.a) {
                continue;
            }
            let mut bn = xml::el("b", vec![("id", bond_ids[bi].to_string()), ("B", atom_ids[b.a].to_string()), ("E", atom_ids[b.b].to_string())]);
            if b.order != 1 {
                bn.attrs.push(("Order".into(), b.order.to_string()));
            }
            let disp = match b.stereo {
                Stereo::Dashed => "Dash",
                Stereo::Hash => "WedgedHashBegin",
                Stereo::Hashed => "Hash",
                Stereo::Bold => "Bold",
                Stereo::Wedge => "WedgeBegin",
                Stereo::Wavy => "Wavy",
                Stereo::None => "",
            };
            if !disp.is_empty() {
                bn.attrs.push(("Display".into(), disp.into()));
            }
            match b.dbl {
                DblPos::Center => bn.attrs.push(("DoublePosition".into(), "Center".into())),
                DblPos::Right => bn.attrs.push(("DoublePosition".into(), "Right".into())),
                DblPos::Left => bn.attrs.push(("DoublePosition".into(), "Left".into())),
                DblPos::Auto => {}
            }
            if let Some(ci) = cx.color_index(b.color) {
                bn.attrs.push(("color".into(), ci.to_string()));
            }
            if let Some(e) = &b.extra {
                bn.attrs.extend(clean_attrs(&e.xml_attrs));
                push_raw(&mut bn.children, &e.xml_children);
            }
            fnode.children.push(Child::El(bn));
        }
        if let Some((_, kids)) = fid.and_then(|f| frag_extra.get(&f)) {
            push_raw(&mut fnode.children, kids);
        }
        page.children.push(Child::El(fnode));
    }
    // Charge symbols linked to their atoms, as ChemDraw writes them.
    for (i, a) in doc.atoms.iter().enumerate() {
        if a.charge == 0 {
            continue;
        }
        let id = cx.id(Some(atom_ids[i] | 0x0100_0000));
        let size = doc.style.label_size * a.size / cx.scale;
        let p = cx.p(a.pos);
        let (x, y) = (f2(p.x + size * 0.55), f2(p.y - size * 0.55));
        let mut g = xml::el(
            "graphic",
            vec![
                ("id", id.to_string()),
                ("BoundingBox", format!("{} {} {} {}", x, y, x, y)),
                ("GraphicType", "Symbol".into()),
                ("SymbolType", if a.charge > 0 { "Plus" } else { "Minus" }.into()),
            ],
        );
        g.children.push(Child::El(xml::el("represent", vec![("attribute", "Charge".into()), ("object", atom_ids[i].to_string())])));
        page.children.push(Child::El(g));
    }
    for c in &doc.captions {
        let id = cx.id(c.extra.as_ref().map(|e| e.id));
        let size = c.size / cx.scale;
        let p = cx.p(c.pos);
        let mut t = xml::el("t", vec![("id", id.to_string()), ("p", format!("{} {}", f2(p.x), f2(p.y + size * 0.8)))]);
        let unchanged = c.extra.as_ref().map(|e| e.source_label.as_deref() == Some(c.text.as_str()) && !e.xml_children.is_empty()).unwrap_or(false);
        if let Some(e) = &c.extra {
            t.attrs.extend(clean_attrs(&e.xml_attrs));
        }
        if unchanged {
            push_raw(&mut t.children, &c.extra.as_ref().unwrap().xml_children);
        } else {
            let face = (if c.bold { 1 } else { 0 }) | (if c.italic { 2 } else { 0 }) | (if c.formula { 0x60 } else { 0 });
            let cfont = if c.serif { serif_id } else { sans_id };
            let mut s = xml::el("s", vec![("font", cfont.to_string()), ("size", f2(size)), ("face", face.to_string())]);
            if let Some(ci) = cx.color_index(c.color) {
                s.attrs.push(("color".into(), ci.to_string()));
            }
            s.children.push(Child::Text(c.text.clone()));
            t.children.push(Child::El(s));
        }
        page.children.push(Child::El(t));
    }
    for a in &doc.arrows {
        let id = cx.id(a.extra.as_ref().map(|e| e.id));
        let head = cx.p(a.to);
        let tail = cx.p(a.from);
        let keep = same_kind(a);
        let was_graphic = a.extra.as_ref().map(|e| e.xml_attrs.iter().any(|x| x.0 == "#graphic")).unwrap_or(false);
        let mut n = if keep && was_graphic {
            let mut g = xml::el("graphic", vec![("id", id.to_string()), ("BoundingBox", format!("{} {} {} {}", f2(head.x), f2(head.y), f2(tail.x), f2(tail.y))), ("GraphicType", "Line".into())]);
            let at = match a.kind {
                ArrowKind::Line => "NoHead",
                ArrowKind::Resonance => "Resonance",
                ArrowKind::Equilibrium => "Equilibrium",
                ArrowKind::Retro => "RetroSynthetic",
                ArrowKind::NoGo => "NoGo",
                _ => "FullHead",
            };
            g.attrs.push(("ArrowType".into(), at.into()));
            if let Some(e) = &a.extra {
                g.attrs.extend(clean_attrs(&e.xml_attrs));
                push_raw(&mut g.children, &e.xml_children);
            }
            g
        } else {
            let r = egui::Rect::from_two_pos(head, tail);
            let mut n = xml::el(
                "arrow",
                vec![
                    ("id", id.to_string()),
                    ("BoundingBox", format!("{} {} {} {}", f2(r.min.x), f2(r.min.y), f2(r.max.x), f2(r.max.y))),
                    ("Head3D", format!("{} {} 0", f2(head.x), f2(head.y))),
                    ("Tail3D", format!("{} {} 0", f2(tail.x), f2(tail.y))),
                ],
            );
            // Attributes of the file's own that Formula doesn't handle are kept; the arrow's
            // style and geometry are always written afresh in the original's terms.
            if let (Some(e), true) = (&a.extra, keep) {
                if e.xml_attrs.iter().any(|x| x.0 == "#arrow") {
                    n.attrs.extend(clean_attrs(&e.xml_attrs).filter(|(k, _)| !ARROW_ATTRS.contains(&k.as_str())));
                    push_raw(&mut n.children, &e.xml_children);
                }
            }
            CdArrow::from_formula(a, &cx).write_xml(&mut n);
            n
        };
        if let Some(ci) = cx.color_index(a.color) {
            n.attrs.push(("color".into(), ci.to_string()));
        }
        page.children.push(Child::El(n));
    }
    let mut groups = Vec::new();
    for s in &doc.shapes {
        if s.is_bracket() {
            let (gs, g) = write_bracket_xml(&mut cx, doc, s, &atom_ids, &bond_ids, label_font);
            page.children.extend(gs.into_iter().map(Child::El));
            groups.extend(g);
            continue;
        }
        let id = cx.id(s.extra.as_ref().map(|e| e.id));
        let a = cx.p(s.min);
        let b = cx.p(s.max);
        let mut n = xml::el("graphic", vec![("id", id.to_string()), ("BoundingBox", format!("{} {} {} {}", f2(a.x), f2(a.y), f2(b.x), f2(b.y)))]);
        match s.kind {
            ShapeKind::Rect => n.attrs.push(("GraphicType".into(), "Rectangle".into())),
            ShapeKind::RoundRect => {
                n.attrs.push(("GraphicType".into(), "Rectangle".into()));
                n.attrs.push(("RectangleType".into(), "RoundEdge".into()));
            }
            ShapeKind::Ellipse => {
                let c = pos2((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
                n.attrs.push(("GraphicType".into(), "Oval".into()));
                n.attrs.push(("Center3D".into(), format!("{} {} 0", f2(c.x), f2(c.y))));
                n.attrs.push(("MajorAxisEnd3D".into(), format!("{} {} 0", f2(b.x), f2(c.y))));
                n.attrs.push(("MinorAxisEnd3D".into(), format!("{} {} 0", f2(c.x), f2(b.y))));
            }
            ShapeKind::Brackets | ShapeKind::Parens => unreachable!(),
        }
        if let Some(ci) = cx.color_index(s.color) {
            n.attrs.push(("color".into(), ci.to_string()));
        }
        if let Some(e) = &s.extra {
            n.attrs.extend(clean_attrs(&e.xml_attrs));
            push_raw(&mut n.children, &e.xml_children);
        }
        page.children.push(Child::El(n));
    }
    page.children.extend(groups.into_iter().map(Child::El));
    if let Some(f) = fx {
        push_raw(&mut page.children, &f.xml_page_children);
    }
    let mut ct = xml::el("colortable", vec![]);
    for c in &cx.colors {
        ct.children.push(Child::El(xml::el("color", vec![("r", f4(c[0] as f32 / 255.0)), ("g", f4(c[1] as f32 / 255.0)), ("b", f4(c[2] as f32 / 255.0))])));
    }
    root.children.push(Child::El(ct));
    let has_fonts = fx.map(|f| f.xml_doc_children.iter().any(|c| c.trim_start().starts_with("<fonttable"))).unwrap_or(false);
    if !has_fonts {
        let mut ft = xml::el("fonttable", vec![]);
        ft.children.push(Child::El(xml::el("font", vec![("id", sans_id.to_string()), ("charset", "iso-8859-1".into()), ("name", "Arial".into())])));
        ft.children.push(Child::El(xml::el("font", vec![("id", serif_id.to_string()), ("charset", "iso-8859-1".into()), ("name", "Times New Roman".into())])));
        root.children.push(Child::El(ft));
    }
    if let Some(f) = fx {
        push_raw(&mut root.children, &f.xml_doc_children);
    }
    root.children.push(Child::El(page));
    format!("<?xml version=\"1.0\" encoding=\"UTF-8\" ?>\n<!DOCTYPE CDXML SYSTEM \"http://www.cambridgesoft.com/xml/cdxml.dtd\" >\n{}", xml::to_string(&root))
}

fn nested_fragment_xml(a: &Atom, cx: &mut Ctx, at: Pos2) -> Option<Node> {
    let smi = chem::expansion_smiles(a.label.as_ref()?)?;
    let g = crate::formats::parse_smiles(smi).ok()?;
    let fid = cx.id(None);
    let mut f = xml::el("fragment", vec![("id", fid.to_string())]);
    let ids: Vec<u32> = (0..g.atoms.len()).map(|_| cx.id(None)).collect();
    let origin = g.atoms[0].pos;
    for (k, ga) in g.atoms.iter().enumerate() {
        let p = at + (ga.pos - origin) / cx.scale;
        let mut n = xml::el("n", vec![("id", ids[k].to_string()), ("p", format!("{} {}", f2(p.x), f2(p.y)))]);
        if ga.el != elements::C {
            n.attrs.push(("Element".into(), ga.el.to_string()));
        }
        if ga.charge != 0 {
            n.attrs.push(("Charge".into(), ga.charge.to_string()));
        }
        f.children.push(Child::El(n));
    }
    let ext = cx.id(None);
    let ep = at - vec2(BOND_LEN, 0.0) / cx.scale;
    f.children.push(Child::El(xml::el("n", vec![("id", ext.to_string()), ("p", format!("{} {}", f2(ep.x), f2(ep.y))), ("NodeType", "ExternalConnectionPoint".into())])));
    let mut bonds: Vec<(u32, u32, u8)> = g.bonds.iter().map(|b| (ids[b.a], ids[b.b], b.order)).collect();
    bonds.push((ext, ids[0], 1));
    for (x, y, o) in bonds {
        let id = cx.id(None);
        let mut b = xml::el("b", vec![("id", id.to_string()), ("B", x.to_string()), ("E", y.to_string())]);
        if o != 1 {
            b.attrs.push(("Order".into(), o.to_string()));
        }
        f.children.push(Child::El(b));
    }
    Some(f)
}

// ---------------------------------------------------------------------------
// Brackets around repeating units.
//
// Files store a pair of brackets either as one graphic ("SquarePair") or, for repeating
// units, as two one-sided graphics ("Square"): a line from one end to the other, with
// the lips on the right of the direction of travel (y pointing down). The index ("n" or
// a repeat count) is an object tag named "bracketusage" with a text, on the closing
// bracket. A bracketed-group record lists the atoms inside and the bonds that cross.

#[derive(Clone, Debug)]
enum Raw {
    Cdx(Vec<u8>),
    Xml(String),
}

#[derive(Clone, Debug)]
struct BracketSide {
    a: Pos2,
    b: Pos2,
    kind: ShapeKind,
    single: bool,
    label: Option<String>,
    color: Option<Rgb>,
    extra: Extra,
    raw: Raw,
}

#[derive(Clone, Debug)]
struct BracketGroupRec {
    id: u32,
    graphics: Vec<u32>,
    /// Repeat count or repeating-unit label written in the record.
    label: Option<String>,
    raw: Raw,
}

fn count_label(n: f64) -> Option<String> {
    (n.is_finite() && n > 0.0).then(|| if n.fract() == 0.0 { format!("{}", n as i64) } else { format!("{}", n) })
}

const BRACKET_TAG: &str = "bracketusage";

fn clean_label(s: &str) -> Option<String> {
    let t = s.trim();
    if t.is_empty() { None } else { Some(t.to_string()) }
}

/// An object tag's name: a plain string (older Formula files put a style count first).
fn tag_name(d: &[u8]) -> String {
    let d = if d.len() > 2 && d[0] == 0 && d[1] == 0 { &d[2..] } else { d };
    String::from_utf8_lossy(d).trim_end_matches('\0').to_string()
}

/// Index text of a bracket graphic read from a CDX file.
fn cdx_bracket_label(g: &CObj) -> Option<String> {
    for c in g.children.iter().filter(|c| c.tag == T_OBJECTTAG) {
        let name = c.prop(P_NAME).map(tag_name).unwrap_or_default();
        if name != BRACKET_TAG {
            continue;
        }
        if let Some(t) = c.children.iter().find(|t| t.tag == T_TEXT).and_then(|t| t.prop(P_TEXT)) {
            return clean_label(&cdx_string(t).0);
        }
        return c.prop(P_TAGVALUE).and_then(|v| clean_label(&if v.len() >= 2 && v[0] == 0 && v[1] == 0 { cdx_string(v).0 } else { String::from_utf8_lossy(v).to_string() }));
    }
    None
}

fn xml_bracket_label(g: &Node) -> Option<String> {
    let tag = g.elements().find(|c| c.name == "objecttag" && c.attr("Name") == Some(BRACKET_TAG))?;
    match tag.elements().find(|t| t.name == "t") {
        Some(t) => clean_label(&t.text()),
        None => tag.attr("Value").and_then(clean_label),
    }
}

/// Is a raw CDX child an object tag holding a bracket index?
fn is_cdx_bracket_tag(raw: &[u8]) -> bool {
    if raw.len() < 6 || u16::from_le_bytes([raw[0], raw[1]]) != T_OBJECTTAG {
        return false;
    }
    let mut pos = 2;
    parse_obj(raw, &mut pos, T_OBJECTTAG, 0, 0)
        .ok()
        .and_then(|o| o.prop(P_NAME).map(|d| tag_name(d) == BRACKET_TAG))
        .unwrap_or(false)
}

fn is_xml_bracket_tag(raw: &str) -> bool {
    xml::parse(raw).map(|n| n.name == "objecttag" && n.attr("Name") == Some(BRACKET_TAG)).unwrap_or(false)
}

/// CDX string without style runs.
fn plain_cdx_string(t: &str) -> Vec<u8> {
    let mut v = 0u16.to_le_bytes().to_vec();
    v.extend(t.chars().map(|c| if (c as u32) < 256 { c as u8 } else { b'?' }));
    v
}

/// Usage of a bracket index: a whole number repeats a group that many times, anything
/// else ("n", "m", "x") marks a structural repeating unit.
fn bracket_usage(label: &str) -> (u8, &'static str, Option<i16>) {
    match label.parse::<i16>() {
        Ok(n) if n > 0 => (16, "MultipleGroup", Some(n)),
        _ => (3, "SRU", None),
    }
}

/// Atoms inside a bracket pair, and the bonds crossing each side (bond, inner atom, right side?).
fn bracket_contents(doc: &Doc, s: &Shape) -> (Vec<usize>, Vec<(usize, usize, bool)>) {
    let r = egui::Rect::from_two_pos(s.min, s.max);
    let inside = |i: usize| {
        let p = doc.atoms[i].pos;
        p.x > r.min.x && p.x < r.max.x && p.y > r.min.y - 2.0 && p.y < r.max.y + 2.0
    };
    let atoms: Vec<usize> = (0..doc.atoms.len()).filter(|&i| inside(i)).collect();
    let mut crossing = Vec::new();
    for (bi, b) in doc.bonds.iter().enumerate() {
        let (ia, ib) = (inside(b.a), inside(b.b));
        if ia != ib {
            let (inner, outer) = if ia { (b.a, b.b) } else { (b.b, b.a) };
            crossing.push((bi, inner, doc.atoms[outer].pos.x > r.center().x));
        }
    }
    (atoms, crossing)
}

impl Reader {
    /// Turn bracket graphics into bracket shapes: pairs as they are, one-sided brackets
    /// matched left with right. Anything that doesn't pair up is kept as it was.
    fn pair_brackets(&mut self) {
        let sides = std::mem::take(&mut self.sides);
        let mut used = vec![false; sides.len()];
        let vertical = |s: &BracketSide| (s.a.x - s.b.x).abs() < 0.5 && (s.a.y - s.b.y).abs() > 1.0;
        let opens = |s: &BracketSide| s.b.y < s.a.y; // drawn upwards: lips to the right
        let span = |s: &BracketSide| (s.a.y.min(s.b.y), s.a.y.max(s.b.y));
        let mut shapes = Vec::new();
        for i in 0..sides.len() {
            let s = &sides[i];
            if s.single {
                continue;
            }
            used[i] = true;
            let r = egui::Rect::from_two_pos(s.a, s.b);
            let mut extra = s.extra.clone();
            extra.source_label = s.label.clone();
            shapes.push(Shape { color: s.color, extra: Some(Box::new(extra)), label: s.label.clone(), ..Shape::new(r.min, r.max, s.kind) });
        }
        for i in 0..sides.len() {
            let l = &sides[i];
            if used[i] || !l.single || !vertical(l) || !opens(l) {
                continue;
            }
            let (t0, b0) = span(l);
            let best = (0..sides.len())
                .filter(|&j| {
                    let r = &sides[j];
                    let (t1, b1) = span(r);
                    !used[j] && r.single && vertical(r) && !opens(r) && r.kind == l.kind && r.a.x > l.a.x + 1.0 && (t0 - t1).abs() < 1.5 && (b0 - b1).abs() < 1.5
                })
                .min_by(|&x, &y| sides[x].a.x.partial_cmp(&sides[y].a.x).unwrap());
            let Some(j) = best else { continue };
            used[i] = true;
            used[j] = true;
            let r = &sides[j];
            let label = r.label.clone().or(l.label.clone());
            let mut le = l.extra.clone();
            let mut re = r.extra.clone();
            le.source_label = label.clone();
            re.source_label = label.clone();
            let (min, max) = (pos2(l.a.x, t0), pos2(r.a.x, b0));
            shapes.push(Shape { color: l.color, extra: Some(Box::new(le)), pair: Some(Box::new(re)), label, ..Shape::new(min, max, l.kind) });
        }
        for (i, s) in sides.into_iter().enumerate() {
            if !used[i] {
                self.keep_raw(s.raw);
            }
        }
        for g in std::mem::take(&mut self.groups) {
            let owner = shapes.iter_mut().find(|s| {
                let ids: Vec<u32> = s.extra.iter().chain(s.pair.iter()).map(|e| e.id).collect();
                !g.graphics.is_empty() && g.graphics.iter().all(|id| ids.contains(id))
            });
            match owner {
                Some(s) => {
                    if s.label.is_none() {
                        // The index lives only in the record: show it, and keep the record as it is.
                        s.label = g.label.clone();
                        for e in s.extra.iter_mut().chain(s.pair.iter_mut()) {
                            e.source_label = s.label.clone();
                        }
                    }
                    let mut e = Extra { id: g.id, source_label: s.label.clone(), ..Default::default() };
                    match g.raw {
                        Raw::Cdx(d) => e.cdx_children.push(d),
                        Raw::Xml(x) => e.xml_children.push(x),
                    }
                    s.group = Some(Box::new(e));
                }
                None => self.keep_raw(g.raw),
            }
        }
        self.doc.shapes.extend(shapes);
    }

    fn keep_raw(&mut self, r: Raw) {
        match r {
            Raw::Cdx(d) => self.fx.cdx_page_children.push(d),
            Raw::Xml(x) => self.fx.xml_page_children.push(x),
        }
    }
}

fn label_unchanged(e: Option<&Extra>, label: Option<&str>) -> bool {
    e.map(|e| e.source_label.as_deref() == label).unwrap_or(false)
}

/// Write a pair of brackets (binary).
#[allow(clippy::too_many_arguments)]
fn write_bracket_cdx(body: &mut W, cx: &mut Ctx, doc: &Doc, s: &Shape, atom_ids: &[u32], bond_ids: &[u32], font_id: u16) {
    let r = egui::Rect::from_two_pos(cx.p(s.min), cx.p(s.max));
    let label = s.label.as_deref().map(str::trim).filter(|l| !l.is_empty());
    let has = |e: Option<&Extra>, t: u16| e.map(|e| e.cdx_props.iter().any(|p| p.0 == t)).unwrap_or(false);
    let left = s.extra.as_deref();
    let right = s.pair.as_deref();
    let color = cx.color_index(s.color);
    // A pair stored as one graphic stays one graphic while its index is unchanged.
    let one_graphic = right.is_none()
        && left.map(|e| e.cdx_props.iter().any(|p| p.0 == P_BRACKETTYPE && p.1.len() >= 2 && i16::from_le_bytes([p.1[0], p.1[1]]) <= 2)).unwrap_or(false)
        && label_unchanged(left, label);
    if one_graphic {
        let id = cx.id(left.map(|e| e.id));
        body.obj(T_GRAPHIC, id);
        body.prop(P_BBOX, &rect4(r.min.y, r.min.x, r.max.y, r.max.x));
        body.i16(P_GRAPHICTYPE, 6);
        if let Some(ci) = color {
            body.i16(P_FGCOLOR, ci as i16);
        }
        if let Some(e) = left {
            body.props(&e.cdx_props);
            for k in &e.cdx_children {
                body.raw(k);
            }
        }
        body.end();
        return;
    }
    let side_type: i16 = if s.kind == ShapeKind::Parens { 5 } else { 3 };
    let left_id = cx.id(left.map(|e| e.id));
    body.obj(T_GRAPHIC, left_id);
    body.prop(P_BBOX, &rect4(r.max.y, r.min.x, r.min.y, r.min.x)); // bottom to top: lips right
    body.i16(P_GRAPHICTYPE, 6);
    if let Some(e) = left.filter(|e| has(Some(e), P_BRACKETTYPE) && s.pair.is_some()) {
        body.props(&e.cdx_props);
    } else {
        body.i16(P_BRACKETTYPE, side_type);
        if let Some(e) = left {
            body.props(&e.cdx_props.iter().filter(|p| p.0 != P_BRACKETTYPE).cloned().collect::<Vec<_>>());
        }
    }
    if let Some(ci) = color {
        body.i16(P_FGCOLOR, ci as i16);
    }
    if let Some(e) = left {
        for k in &e.cdx_children {
            body.raw(k);
        }
    }
    body.end();

    // Kept parts are reused only when they came from a file of this format.
    let same = label_unchanged(right, label) && right.is_some_and(|e| e.xml_attrs.is_empty());
    let right_id = cx.id(right.map(|e| e.id));
    body.obj(T_GRAPHIC, right_id);
    body.prop(P_BBOX, &rect4(r.min.y, r.max.x, r.max.y, r.max.x)); // top to bottom: lips left
    body.i16(P_GRAPHICTYPE, 6);
    if !has(right, P_BRACKETTYPE) {
        body.i16(P_BRACKETTYPE, side_type);
    }
    if let Some(e) = right {
        body.props(&e.cdx_props.iter().filter(|p| same || p.0 != P_BRACKETUSAGE).cloned().collect::<Vec<_>>());
    }
    if let Some(l) = label.filter(|_| !same || !has(right, P_BRACKETUSAGE)) {
        body.prop(P_BRACKETUSAGE, &[bracket_usage(l).0]);
    }
    if let Some(ci) = color {
        body.i16(P_FGCOLOR, ci as i16);
    }
    if let Some(e) = right {
        for k in e.cdx_children.iter().filter(|k| same || !is_cdx_bracket_tag(k)) {
            body.raw(k);
        }
    }
    if let Some(l) = label.filter(|_| !same) {
        let sz = doc.style.label_size / cx.scale * 0.75;
        let tag_id = cx.id(None);
        body.obj(T_OBJECTTAG, tag_id);
        body.i16(P_TAGTYPE, 3);
        body.prop(P_NAME, BRACKET_TAG.as_bytes());
        body.prop(P_TAGVALUE, &plain_cdx_string(l));
        let text_id = cx.id(None);
        body.obj(T_TEXT, text_id);
        body.prop(P_POS2D, &point2d(pos2(r.max.x + sz * 0.45, r.max.y + sz * 0.35)));
        body.prop(P_TEXT, &make_cdx_string(l, font_id, 0, sz, 0));
        body.end();
        body.end();
    }
    body.end();

    // The record of what repeats.
    let group = s.group.as_deref();
    if let (Some(g), true) = (group, same && label_unchanged(group, label)) {
        if let Some(raw) = g.cdx_children.first() {
            body.raw(raw);
            return;
        }
    }
    let Some(l) = label else { return };
    let (inside, crossing) = bracket_contents(doc, s);
    if inside.is_empty() {
        return;
    }
    let (usage, _, count) = bracket_usage(l);
    let gid = cx.id(group.map(|g| g.id));
    body.obj(T_BRACKETGROUP, gid);
    let ids: Vec<u8> = inside.iter().flat_map(|&i| atom_ids[i].to_le_bytes()).collect();
    body.prop(P_BRACKETED, &ids);
    body.i16(P_BRACKETUSAGE, usage as i16);
    match count {
        Some(n) => body.prop(P_REPEATCOUNT, &(n as f64).to_le_bytes()),
        None => body.prop(P_SRULABEL, &plain_cdx_string(l)),
    }
    for (graphic, right_side) in [(left_id, false), (right_id, true)] {
        let aid = cx.id(None);
        body.obj(T_BRACKETATTACH, aid);
        body.u32(P_GRAPHICID, graphic);
        for &(b, inner, rs) in &crossing {
            if rs == right_side {
                let cid = cx.id(None);
                body.obj(T_CROSSINGBOND, cid);
                body.u32(P_BONDID, bond_ids[b]);
                body.u32(P_INNERATOM, atom_ids[inner]);
                body.end();
            }
        }
        body.end();
    }
    body.end();
}

/// Write a pair of brackets (XML). Returns the graphics, then the group record if any.
fn write_bracket_xml(cx: &mut Ctx, doc: &Doc, s: &Shape, atom_ids: &[u32], bond_ids: &[u32], font: &str) -> (Vec<Node>, Option<Node>) {
    let r = egui::Rect::from_two_pos(cx.p(s.min), cx.p(s.max));
    let label = s.label.as_deref().map(str::trim).filter(|l| !l.is_empty());
    let left = s.extra.as_deref();
    let right = s.pair.as_deref();
    let attr = |e: Option<&Extra>, k: &str| e.and_then(|e| e.xml_attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone()));
    let color = cx.color_index(s.color);
    let bbox = |a: Pos2, b: Pos2| format!("{} {} {} {}", f2(a.x), f2(a.y), f2(b.x), f2(b.y));
    let finish = |n: &mut Node, e: Option<&Extra>, skip_tag: bool, skip: &[&str]| {
        if let Some(ci) = color {
            n.attrs.push(("color".into(), ci.to_string()));
        }
        if let Some(e) = e {
            n.attrs.extend(clean_attrs(&e.xml_attrs).filter(|(k, _)| !skip.contains(&k.as_str())));
            let kids: Vec<String> = e.xml_children.iter().filter(|k| !(skip_tag && is_xml_bracket_tag(k))).cloned().collect();
            push_raw(&mut n.children, &kids);
        }
    };
    let one_graphic = right.is_none() && attr(left, "BracketType").map(|t| t.ends_with("Pair")).unwrap_or(false) && label_unchanged(left, label);
    if one_graphic {
        let id = cx.id(left.map(|e| e.id));
        let mut n = xml::el("graphic", vec![("id", id.to_string()), ("BoundingBox", bbox(r.min, r.max)), ("GraphicType", "Bracket".into())]);
        finish(&mut n, left, false, &[]);
        return (vec![n], None);
    }
    let side_type = if s.kind == ShapeKind::Parens { "Round" } else { "Square" };
    let keep_type = |e: Option<&Extra>| attr(e, "BracketType").filter(|t| !t.ends_with("Pair") && s.pair.is_some());
    let left_id = cx.id(left.map(|e| e.id));
    let mut ln = xml::el("graphic", vec![("id", left_id.to_string()), ("BoundingBox", bbox(pos2(r.min.x, r.max.y), pos2(r.min.x, r.min.y))), ("GraphicType", "Bracket".into())]);
    if keep_type(left).is_none() {
        ln.attrs.push(("BracketType".into(), side_type.into()));
        finish(&mut ln, left, false, &["BracketType"]);
    } else {
        finish(&mut ln, left, false, &[]);
    }
    let same = label_unchanged(right, label) && right.is_some_and(|e| !e.xml_attrs.is_empty());
    let right_id = cx.id(right.map(|e| e.id));
    let mut rn = xml::el("graphic", vec![("id", right_id.to_string()), ("BoundingBox", bbox(pos2(r.max.x, r.min.y), pos2(r.max.x, r.max.y))), ("GraphicType", "Bracket".into())]);
    let mut skip: Vec<&str> = Vec::new();
    if keep_type(right).is_none() {
        rn.attrs.push(("BracketType".into(), side_type.into()));
        skip.push("BracketType");
    }
    if let Some(l) = label.filter(|_| !same || attr(right, "BracketUsage").is_none()) {
        rn.attrs.push(("BracketUsage".into(), bracket_usage(l).1.into()));
        skip.push("BracketUsage");
    }
    finish(&mut rn, right, !same, &skip);
    if let Some(l) = label.filter(|_| !same) {
        let sz = doc.style.label_size / cx.scale * 0.75;
        let mut tag = xml::el("objecttag", vec![("TagType", "String".into()), ("Name", BRACKET_TAG.into()), ("Value", l.to_string())]);
        let mut t = xml::el("t", vec![("p", format!("{} {}", f2(r.max.x + sz * 0.45), f2(r.max.y + sz * 0.35)))]);
        let mut run = xml::el("s", vec![("font", font.to_string()), ("size", f2(sz)), ("color", "0".into())]);
        run.children.push(Child::Text(l.to_string()));
        t.children.push(Child::El(run));
        tag.children.push(Child::El(t));
        rn.children.push(Child::El(tag));
    }
    let graphics = vec![ln, rn];
    let group = s.group.as_deref();
    if let (Some(g), true) = (group, same && label_unchanged(group, label)) {
        if let Some(n) = g.xml_children.first().and_then(|x| xml::parse(x).ok()) {
            return (graphics, Some(n));
        }
    }
    let Some(l) = label else { return (graphics, None) };
    let (inside, crossing) = bracket_contents(doc, s);
    if inside.is_empty() {
        return (graphics, None);
    }
    let (_, usage, count) = bracket_usage(l);
    let gid = cx.id(group.map(|g| g.id));
    let ids: Vec<String> = inside.iter().map(|&i| atom_ids[i].to_string()).collect();
    let mut gn = xml::el("bracketedgroup", vec![("id", gid.to_string()), ("BracketedObjectIDs", ids.join(" ")), ("BracketUsage", usage.into())]);
    match count {
        Some(n) => gn.attrs.push(("RepeatCount".into(), n.to_string())),
        None => gn.attrs.push(("SRULabel".into(), l.to_string())),
    }
    for (graphic, right_side) in [(left_id, false), (right_id, true)] {
        let aid = cx.id(None);
        let mut an = xml::el("bracketattachment", vec![("id", aid.to_string()), ("GraphicID", graphic.to_string())]);
        for &(b, inner, rs) in &crossing {
            if rs == right_side {
                let cid = cx.id(None);
                an.children.push(Child::El(xml::el("crossingbond", vec![("id", cid.to_string()), ("BondID", bond_ids[b].to_string()), ("InnerAtomID", atom_ids[inner].to_string())])));
            }
        }
        gn.children.push(Child::El(an));
    }
    (graphics, Some(gn))
}
