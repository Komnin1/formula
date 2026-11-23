//! Ready-made drawing settings ("Apply Document Settings from").
//!
//! Each preset is written in printed points, the way journal author guidelines
//! give them, and converted to drawing units where a standard bond is `BOND_LEN`.

use crate::doc::*;

pub struct Preset {
    pub name: &'static str,
    /// Bond length, line width, bold width, hash spacing, margin (points).
    pub bond: f32,
    pub line: f32,
    pub bold: f32,
    pub hash: f32,
    pub margin: f32,
    /// Double-bond spacing as a fraction of the bond length.
    pub spacing: f32,
    /// Atom label and caption font sizes (points).
    pub label: f32,
    pub caption: f32,
    pub font: LabelFont,
    pub label_bold: bool,
    pub terminal_carbons: bool,
    pub dark: bool,
    pub caption_font: LabelFont,
    /// Hide implicit hydrogens on atom labels.
    pub hide_h: bool,
    pub page: Page,
}

const LETTER_PAGE: Page = Page { paper_w: LETTER.0, paper_h: LETTER.1, left: 1.27, right: 1.27, top: 1.27, bottom: 1.27 };

/// Paper (width, height in cm) and margins (left, right, top, bottom), from the original's
/// Layout page for the style.
const fn page(mut x: Preset, paper: (f32, f32), l: f32, r: f32, t: f32, b: f32) -> Preset {
    x.page = Page { paper_w: paper.0, paper_h: paper.1, left: l, right: r, top: t, bottom: b };
    x
}

const LETTER_WIDE: (f32, f32) = (LETTER.1, LETTER.0);
const A4_WIDE: (f32, f32) = (A4.1, A4.0);

const fn p(name: &'static str, bond: f32, line: f32, bold: f32, hash: f32, margin: f32, label: f32, caption: f32) -> Preset {
    Preset {
        name,
        bond,
        line,
        bold,
        hash,
        margin,
        spacing: 0.18,
        label,
        caption,
        font: LabelFont::Sans,
        label_bold: false,
        terminal_carbons: false,
        dark: false,
        caption_font: LabelFont::Sans,
        hide_h: false,
        page: LETTER_PAGE,
    }
}

const fn serif(mut x: Preset) -> Preset {
    x.font = LabelFont::Serif;
    x
}

const fn spacing(mut x: Preset, s: f32) -> Preset {
    x.spacing = s;
    x
}

const fn dark(mut x: Preset) -> Preset {
    x.dark = true;
    x
}

const fn terminal(mut x: Preset) -> Preset {
    x.terminal_carbons = true;
    x
}

const fn bold_labels(mut x: Preset) -> Preset {
    x.label_bold = true;
    x
}

/// Centimetres, as the original's Document Settings show them, to points.
const fn cm(v: f32) -> f32 {
    v * 72.0 / 2.54
}

/// The original's built-in settings, read from its Document Settings pages (Drawing, Text
/// Captions, Atom Labels): fixed length, line width, bold width, hash spacing and margin
/// width in cm, bond spacing in % of the length, caption and label sizes in points.
#[allow(clippy::too_many_arguments)]
const fn cd(name: &'static str, len: f32, line: f32, bold: f32, hash: f32, margin: f32, pct: f32, caption: f32, label: f32) -> Preset {
    spacing(p(name, cm(len), cm(line), cm(bold), cm(hash), cm(margin), label, caption), pct / 100.0)
}

const fn times_captions(mut x: Preset) -> Preset {
    x.caption_font = LabelFont::Serif;
    x
}

const fn no_h(mut x: Preset) -> Preset {
    x.hide_h = true;
    x
}

pub const PRESETS: &[Preset] = &[
    cd("ACS Document 1996", 0.508, 0.021, 0.071, 0.088, 0.056, 18.0, 10.0, 10.0),
    page(times_captions(cd("Adv. Synth. Catal. Document", 0.6, 0.035, 0.071, 0.088, 0.056, 18.0, 12.0, 10.0)), LETTER, 4.293, 4.293, 1.27, 1.27),
    page(dark(cd("Dark Mode", 0.508, 0.057, 0.096, 0.088, 0.056, 18.0, 10.0, 10.0)), LETTER_WIDE, 1.27, 1.27, 1.27, 1.27),
    no_h(times_captions(cd("I Draw Styles", 1.058, 0.035, 0.141, 0.095, 0.071, 12.0, 12.0, 10.0))),
    page(cd("J. Chin. Chem. Soc. Doc", 0.635, 0.035, 0.088, 0.088, 0.071, 20.0, 12.0, 12.0), (26.539, 44.002), 1.27, 1.27, 1.27, 1.27),
    // Bond spacing given as an absolute 0.061 cm.
    serif(times_captions(cd("J. Het. Chem", 0.381, 0.021, 0.073, 0.046, 0.038, 0.061 / 0.381 * 100.0, 7.0, 7.0))),
    page(serif(times_captions(cd("J. Mol. Mod. (1 Column)", 0.508, 0.021, 0.071, 0.088, 0.056, 18.0, 10.0, 10.0))), LETTER, 6.546, 6.546, 1.27, 1.27),
    page(serif(times_captions(cd("J. Mol. Mod. (2 Column)", 0.508, 0.021, 0.071, 0.088, 0.056, 18.0, 10.0, 10.0))), LETTER, 1.894, 1.894, 1.27, 1.27),
    cd("Labeled Monomers", 0.508, 0.021, 0.071, 0.088, 0.056, 18.0, 10.0, 10.0),
    page(cd("Nature Research Document", 0.381, 0.021, 0.055, 0.06, 0.042, 18.0, 6.0, 6.0), LETTER, 1.795, 1.795, 1.97, 1.97),
    page(times_captions(cd("New A4 Document", 1.058, 0.035, 0.141, 0.095, 0.071, 12.0, 12.0, 10.0)), A4, 1.27, 1.27, 1.27, 1.27),
    times_captions(cd("New Document", 1.058, 0.035, 0.141, 0.095, 0.071, 12.0, 12.0, 10.0)),
    page(times_captions(cd("New Slide", 1.058, 0.056, 0.141, 0.095, 0.071, 15.0, 16.0, 12.0)), (41.835, 25.4), 1.27, 1.27, 1.27, 1.27),
    page(bold_labels(cd("Phytomedicine Document", 0.706, 0.035, 0.047, 0.106, 0.044, 8.0, 12.0, 12.0)), (20.955, 27.305), 1.27, 1.27, 1.27, 1.27),
    page(cd("RSC (1 Column) Document", 0.43, 0.016, 0.056, 0.062, 0.044, 20.0, 7.0, 7.0), LETTER, 5.715, 5.715, 1.27, 1.27),
    page(cd("RSC (2 Column) Document", 0.43, 0.016, 0.056, 0.062, 0.044, 20.0, 7.0, 7.0), A4, 1.0, 1.0, 1.0, 1.0),
    page(cd("Science of Synthesis", 0.6, 0.028, 0.071, 0.088, 0.079, 18.0, 10.0, 10.0), A4, 0.88, 0.33, 1.27, 1.27),
    terminal(times_captions(cd("Show Terminal Carbon Labels", 1.058, 0.035, 0.141, 0.095, 0.071, 12.0, 12.0, 10.0))),
    page(cd("SYNTHESIS, SYNLETT Document", 0.6, 0.028, 0.071, 0.088, 0.046, 18.0, 10.0, 10.0), A4, 4.501, 4.501, 1.501, 1.501),
    page(cd("Verlag Helv. Chim. Acta Doc", 0.6, 0.021, 0.071, 0.071, 0.071, 15.0, 12.0, 10.0), A4, 2.094, 2.094, 2.137, 2.137),
    page(cd("Wiley Document", 0.6, 0.026, 0.092, 0.092, 0.071, 18.0, 12.0, 12.0), A4_WIDE, 1.27, 1.27, 1.27, 1.27),
];

pub fn find(name: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.name == name)
}

/// The preset as a style in drawing units.
pub fn to_style(p: &Preset, keep_warnings: bool) -> Style {
    let k = BOND_LEN / p.bond;
    Style {
        line_width: p.line * k,
        bold_width: p.bold * k,
        dbl_spacing: p.spacing,
        label_size: p.label * k,
        caption_size: p.caption * k,
        show_carbons: false,
        show_warnings: keep_warnings,
        name: p.name.into(),
        bond_length: p.bond,
        hash_spacing: p.hash * k,
        margin: p.margin * k,
        wedge_width: p.bold * k * 1.5,
        label_font: p.font,
        label_bold: p.label_bold,
        show_terminal_carbons: p.terminal_carbons,
        dark: p.dark,
        caption_font: p.caption_font,
        hide_implicit_h: p.hide_h,
        page: p.page,
    }
}

/// Use `new` as the document's style. Captions in the old default size and font take the
/// new ones.
pub fn apply(doc: &mut Doc, new: Style) {
    let old = doc.style.caption_size;
    let old_serif = doc.style.caption_font == LabelFont::Serif;
    for c in &mut doc.captions {
        if (c.size - old).abs() < 0.05 && c.serif == old_serif && c.seq.is_none() {
            c.size = new.caption_size;
            c.serif = new.caption_font == LabelFont::Serif;
        }
    }
    doc.style = new;
}
