//! The NMR tab: predicted ¹H and ¹³C spectra of the drawn (or selected) structures, a peak
//! table, the shifts written next to the atoms, and the peak list to copy or paste.

use super::*;
use crate::nmr::{self, Peak, Prediction};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

/// Prediction for one structure (connected piece) of the drawing.
pub struct NmrResult {
    pub atoms: Vec<usize>,
    pub p: Prediction,
}

/// Solvent name, residual ¹H and ¹³C peaks, and whether OH/NH protons exchange away.
const SOLVENTS: &[(&str, Option<f64>, Option<f64>, bool)] = &[
    ("CDCl₃", Some(7.26), Some(77.16), false),
    ("DMSO-d₆", Some(2.50), Some(39.52), false),
    ("CD₃OD", Some(3.31), Some(49.00), true),
    ("D₂O", Some(4.79), None, true),
    ("C₆D₆", Some(7.16), Some(128.06), false),
    ("Acetone-d₆", Some(2.05), Some(29.84), false),
];

const NMR_BLUE: Color32 = Color32::from_rgb(20, 90, 200);

impl App {
    /// Predictions for the selected structures (or all), recomputed when the drawing changes.
    pub(super) fn nmr_results(&mut self) -> Rc<Vec<NmrResult>> {
        let mut h = DefaultHasher::new();
        self.topology_key().hash(&mut h);
        // Alkene geometry (cis/trans) depends on where atoms are.
        for a in &self.doc.atoms {
            ((a.pos.x / 4.0).round() as i32, (a.pos.y / 4.0).round() as i32).hash(&mut h);
        }
        (self.nmr_solvent, (self.nmr_enrich * 10.0).round() as i64).hash(&mut h);
        let key = h.finish();
        if let Some((k, r)) = &self.nmr_cache {
            if *k == key {
                return r.clone();
            }
        }
        let chosen: HashSet<usize> = self.selected_or_all_atoms().into_iter().collect();
        let mut out = Vec::new();
        for frag in self.doc.fragments() {
            let f: Vec<usize> = frag.into_iter().filter(|i| chosen.contains(i)).collect();
            // Anything with an NMR nucleus counts, carbon-free ones too (PF6⁻, BF4⁻, H2O...).
            let nmr_atom = |i: usize| {
                let a = &self.doc.atoms[i];
                a.label.is_some() || matches!(a.el, 1 | 6 | 9 | 15) || self.doc.implicit_h(i) > 0
            };
            if f.is_empty() || f.len() > 400 || !f.iter().any(|&i| nmr_atom(i)) {
                continue;
            }
            let o = nmr::Options { exchange: SOLVENTS[self.nmr_solvent].3, enrich: self.nmr_enrich / 100.0, ..Default::default() };
            let p = nmr::predict_with(&self.doc, &f, o);
            if p.h.is_empty() && p.c.is_empty() && p.f.is_empty() && p.p.is_empty() && p.n.is_empty() {
                continue;
            }
            out.push(NmrResult { atoms: f, p });
            if out.len() >= 6 {
                break;
            }
        }
        let r = Rc::new(out);
        self.nmr_cache = Some((key, r.clone()));
        r
    }

    /// Peaks shown for the current nucleus and solvent.
    fn nmr_peaks<'a>(&self, r: &'a NmrResult) -> Vec<&'a Peak> {
        let exchanges = SOLVENTS[self.nmr_solvent].3;
        match self.nmr_nucleus {
            13 => r.p.c.iter().collect(),
            19 => r.p.f.iter().collect(),
            31 => r.p.p.iter().collect(),
            15 => r.p.n.iter().collect(),
            _ => r.p.h.iter().filter(|p| !(exchanges && p.mult == "br s")).collect(),
        }
    }

    fn nmr_text(&self, r: &NmrResult) -> String {
        let solvent = SOLVENTS[self.nmr_solvent].0;
        let peaks: Vec<Peak> = self.nmr_peaks(r).into_iter().cloned().collect();
        match self.nmr_nucleus {
            13 => nmr::c_text(&peaks, self.nmr_mhz, solvent),
            15 | 19 | 31 => nmr::x_text(&peaks, self.nmr_nucleus, self.nmr_mhz, solvent),
            _ => nmr::h_text(&peaks, self.nmr_mhz, solvent),
        }
    }

    pub(super) fn nmr_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.label(egui::RichText::new("NMR Spectrum (predicted)").strong());
        // ¹⁵N is offered only when a nitrogen is marked as ¹⁵N (Isotope menu or "15N" label).
        let has_15n = self.doc.atoms.iter().any(|a| a.isotope == 15 && (a.el == 7 || a.label.as_deref().is_some_and(|l| chem::label_main_element(l) == 7)));
        if self.nmr_nucleus == 15 && !has_15n {
            self.nmr_nucleus = 1;
        }
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.nmr_nucleus, 1, "¹H");
            ui.selectable_value(&mut self.nmr_nucleus, 13, "¹³C");
            ui.selectable_value(&mut self.nmr_nucleus, 19, "¹⁹F");
            ui.selectable_value(&mut self.nmr_nucleus, 31, "³¹P");
            // ¹⁵N only for structures with a nitrogen marked as ¹⁵N.
            if has_15n {
                ui.selectable_value(&mut self.nmr_nucleus, 15, "¹⁵N").on_hover_text("¹⁵N spectrum of the ¹⁵N-labelled nitrogens");
            }
            ui.separator();
            egui::ComboBox::from_id_salt("nmr-mhz").selected_text(format!("{} MHz", self.nmr_mhz)).width(80.0).show_ui(ui, |ui| {
                for f in [300, 400, 500, 600, 800] {
                    ui.selectable_value(&mut self.nmr_mhz, f, format!("{} MHz", f));
                }
            });
            egui::ComboBox::from_id_salt("nmr-solvent").selected_text(SOLVENTS[self.nmr_solvent].0).width(90.0).show_ui(ui, |ui| {
                for (k, s) in SOLVENTS.iter().enumerate() {
                    ui.selectable_value(&mut self.nmr_solvent, k, s.0);
                }
            });
        });
        ui.checkbox(&mut self.nmr_on_structure, "Show shifts on the structure");
        // Partly labelled compounds: the labelled and unlabelled molecules give separate peaks.
        if self.doc.atoms.iter().any(|a| a.isotope != 0) {
            ui.horizontal(|ui| {
                ui.label("Isotope enrichment:");
                ui.add(egui::DragValue::new(&mut self.nmr_enrich).range(1.0..=100.0).speed(0.5).max_decimals(1).suffix(" %"))
                    .on_hover_text("Share of molecules carrying the label. Below 100 %, a ¹⁵N–H shows as a doublet (labelled) over a singlet (unlabelled).");
            });
        }
        let res = self.nmr_results();
        if res.is_empty() {
            ui.add_space(8.0);
            ui.label("Draw a structure to see its predicted ¹H and ¹³C NMR spectra here.");
            return;
        }
        let mut hover: Vec<usize> = Vec::new();
        let hetero = matches!(self.nmr_nucleus, 15 | 19 | 31);
        if hetero && res.iter().all(|r| self.nmr_peaks(r).is_empty()) {
            ui.add_space(8.0);
            ui.label(if self.nmr_nucleus == 19 { "There is no fluorine in the drawing (or the selection)." } else { "There is no phosphorus in the drawing (or the selection)." });
            return;
        }
        for (k, r) in res.iter().enumerate() {
            let peaks = self.nmr_peaks(r);
            // Structures without the element are left out of ¹⁹F and ³¹P.
            if hetero && peaks.is_empty() {
                continue;
            }
            egui::Frame::group(ui.style()).fill(Color32::WHITE).show(ui, |ui| {
                ui.set_width(ui.available_width());
                if res.len() > 1 {
                    ui.label(egui::RichText::new(format!("Structure {}", k + 1)).color(Color32::GRAY));
                }
                if peaks.is_empty() && matches!(self.nmr_nucleus, 15 | 19 | 31) {
                    ui.label(egui::RichText::new(if self.nmr_nucleus == 19 { "No fluorine in this structure." } else { "No phosphorus in this structure." }).color(Color32::GRAY));
                    return;
                }
                if let Some(a) = nmr_plot(ui, &peaks, self.nmr_nucleus, self.nmr_mhz as f64, SOLVENTS[self.nmr_solvent]) {
                    hover = a;
                }
                match self.nmr_nucleus {
                    19 => {
                        ui.label(egui::RichText::new("δ from CFCl₃; couplings to H and P shown, F–F couplings not modelled.").small().color(Color32::GRAY));
                    }
                    15 => {
                        ui.label(egui::RichText::new("δ from liquid NH₃ (subtract 380.5 for the CH₃NO₂ scale); proton-decoupled (¹⁵N{¹H}); only ¹⁵N-labelled nitrogens are shown.").small().color(Color32::GRAY));
                    }
                    31 => {
                        ui.label(egui::RichText::new("δ from 85% H₃PO₄; proton-decoupled (³¹P{¹H}), couplings to F and P shown.").small().color(Color32::GRAY));
                    }
                    _ => {}
                }
                ui.add_space(4.0);
                let coarse = self.nmr_nucleus != 1;
                let nucleus_sym = match self.nmr_nucleus {
                    13 => "C",
                    19 => "F",
                    31 => "P",
                    15 => "N",
                    _ => "H",
                };
                egui::Grid::new(("nmr-peaks", k)).num_columns(5).striped(true).spacing(vec2(12.0, 2.0)).show(ui, |ui| {
                    for h in ["δ (ppm)", "Mult.", "J (Hz)", nucleus_sym, "Group"] {
                        ui.label(egui::RichText::new(h).strong());
                    }
                    ui.end_row();
                    for p in &peaks {
                        let r1 = ui.label(if coarse { format!("{:.1}", p.shift) } else { format!("{:.2}", p.shift) });
                        let r2 = ui.label(&p.mult);
                        let r3 = ui.label(if p.j.is_empty() || p.mult == "m" { "—".to_string() } else { p.j.iter().map(|j| format!("{:.1}", j)).collect::<Vec<_>>().join(", ") });
                        let r4 = ui.label(nmr::count_text(p));
                        let r5 = if p.learned > 0 {
                            ui.label(format!("{} ✓", p.what)).on_hover_text(format!("Corrected with measured spectra: {} published compounds with the same surroundings", p.learned))
                        } else {
                            ui.label(&p.what)
                        };
                        if [r1, r2, r3, r4, r5].iter().any(|r| r.hovered()) {
                            hover = p.atoms.clone();
                        }
                        ui.end_row();
                    }
                });
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("Copy peak list").on_hover_text("Copy the peaks as written in an experimental section").clicked() {
                        ctx.copy_text(self.nmr_text(r));
                        self.status("Copied the predicted NMR peaks");
                    }
                    if ui.button("Paste").on_hover_text("Add the peak list as text under the structure").clicked() {
                        let text = self.nmr_text(r);
                        let sel = Selection { atoms: r.atoms.iter().copied().collect(), ..Default::default() };
                        let old = std::mem::replace(&mut self.sel, sel);
                        self.caption_below(text, false, ctx);
                        self.sel = old;
                        self.status("Pasted the predicted NMR peaks under the structure");
                    }
                });
                if !r.p.skipped.is_empty() {
                    ui.colored_label(Color32::from_rgb(170, 90, 0), format!("Not included (label not understood): {}", r.p.skipped.join(", ")));
                }
            });
            ui.add_space(4.0);
        }
        self.nmr_hover = hover;
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Estimated offline from additive increment rules, corrected (✓) with measured shifts from about 3,500 published compounds with the same surroundings (typical error ±0.2 ppm for ¹H, ±2–5 ppm for ¹³C). \
                 Equivalent atoms share a peak; diastereotopic protons, long-range couplings and second-order effects are not modelled. \
                 OH and NH shifts depend on concentration and solvent.",
            )
            .small()
            .color(Color32::GRAY),
        );
    }

    /// Shifts written next to the atoms on the canvas while the NMR tab is open.
    pub(super) fn draw_nmr_labels(&mut self, painter: &egui::Painter) {
        if !(self.show_panel && self.tab == Tab::Nmr && self.nmr_on_structure) {
            return;
        }
        let res = self.nmr_results();
        let mut by_atom: HashMap<usize, Vec<String>> = HashMap::new();
        for r in res.iter() {
            for p in self.nmr_peaks(r) {
                let t = if self.nmr_nucleus != 1 { format!("{:.1}", p.shift) } else { format!("{:.2}", p.shift) };
                for &a in &p.atoms {
                    let v = by_atom.entry(a).or_default();
                    if !v.contains(&t) {
                        v.push(t.clone());
                    }
                }
            }
        }
        let size = (10.0 * self.zoom.sqrt()).clamp(9.0, 15.0);
        let font = egui::FontId::proportional(size);
        for (a, texts) in by_atom {
            let Some(at) = self.doc.atoms.get(a) else { continue };
            let p = self.to_screen(at.pos);
            // Put the number on the open side of the atom.
            let dir = if self.doc.degree(a) > 0 { self.doc.free_direction(a) } else { vec2(0.0, -1.0) };
            let q = p + dir * (BOND_LEN * 0.42 * self.zoom).max(14.0);
            let hot = self.nmr_hover.contains(&a);
            let col = if hot { Color32::from_rgb(220, 90, 30) } else { NMR_BLUE };
            if hot {
                painter.circle_stroke(p, (BOND_LEN * 0.22 * self.zoom).max(7.0), egui::Stroke::new(2.0, col));
            }
            painter.text(q, egui::Align2::CENTER_CENTER, texts.join(" / "), font.clone(), col);
        }
    }
}

/// The spectrum: ppm axis running right to left, sticks for every line of every multiplet,
/// shift labels and the solvent's residual peak. Returns the atoms of the peak under the
/// pointer.
fn nmr_plot(ui: &mut egui::Ui, peaks: &[&Peak], nucleus: u8, mhz: f64, solvent: (&str, Option<f64>, Option<f64>, bool)) -> Option<Vec<usize>> {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 182.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    let grey = Color32::from_gray(150);
    let font = egui::FontId::proportional(9.5);
    let plot = Rect::from_min_max(rect.min + vec2(10.0, 18.0), rect.max - vec2(10.0, 30.0));
    let carbon = nucleus == 13;
    // ¹⁹F and ³¹P: wide, signed ppm ranges around the peaks.
    let hetero = matches!(nucleus, 15 | 19 | 31);
    let max_s = peaks.iter().map(|x| x.shift).fold(f64::MIN, f64::max);
    let min_s = peaks.iter().map(|x| x.shift).fold(f64::MAX, f64::min);
    let (lo, hi, step) = if hetero {
        let (a, b) = if peaks.is_empty() { (-50.0, 50.0) } else { (min_s - 20.0, max_s + 20.0) };
        let span = (b - a).max(80.0);
        let step = [5.0, 10.0, 20.0, 50.0, 100.0].into_iter().find(|s| span / s <= 8.0).unwrap_or(200.0);
        let mid = (a + b) / 2.0;
        (((mid - span / 2.0) / step).floor() * step, ((mid + span / 2.0) / step).ceil() * step, step)
    } else if carbon {
        ((min_s - 10.0).min(0.0).floor(), if max_s > 190.0 { 220.0 } else { 200.0 }, 20.0)
    } else {
        ((min_s - 0.5).min(0.0).floor(), (max_s + 0.5).max(10.0).ceil(), 1.0)
    };
    // Spectrometer frequency for this nucleus (couplings in Hz → ppm).
    let mhz = mhz * match nucleus {
        19 => 0.9408,
        31 => 0.4048,
        15 => 0.1013,
        13 => 0.2515,
        _ => 1.0,
    };
    let carbon = carbon || hetero;
    let x = |ppm: f64| plot.right() - ((ppm - lo) / (hi - lo)) as f32 * plot.width();
    p.line_segment([plot.left_bottom(), plot.right_bottom()], egui::Stroke::new(1.0, grey));
    let mut t = (lo / step).ceil() * step;
    while t <= hi + 1e-9 {
        let px = x(t);
        p.line_segment([pos2(px, plot.bottom()), pos2(px, plot.bottom() + 3.0)], egui::Stroke::new(1.0, grey));
        p.text(pos2(px, plot.bottom() + 4.0), egui::Align2::CENTER_TOP, format!("{}", t as i32), font.clone(), grey);
        t += step;
    }
    p.text(pos2(plot.center().x, rect.bottom() - 1.0), egui::Align2::CENTER_BOTTOM, "δ (ppm)", font.clone(), grey);
    // Residual solvent peak.
    let residual = match nucleus {
        1 => solvent.1,
        13 => solvent.2,
        _ => None,
    };
    if let Some(s) = residual {
        if s > lo && s < hi {
            let px = x(s);
            let h = plot.height() * 0.25;
            p.line_segment([pos2(px, plot.bottom()), pos2(px, plot.bottom() - h)], egui::Stroke::new(1.0, Color32::from_gray(190)));
            p.text(pos2(px, plot.bottom() - h - 1.0), egui::Align2::CENTER_BOTTOM, solvent.0, egui::FontId::proportional(8.5), Color32::from_gray(170));
        }
    }
    if peaks.is_empty() {
        return None;
    }
    // Lines of each multiplet, scaled so the tallest line reaches the top.
    let all: Vec<(usize, Vec<(f64, f64)>)> = peaks
        .iter()
        .enumerate()
        .map(|(k, pk)| {
            let mut l = if nucleus == 13 && pk.pattern.is_empty() { vec![(pk.shift, pk.count as f64)] } else { nmr::lines(pk, mhz) };
            // A ¹³C-enriched carbon stands far above natural-abundance peaks.
            if nucleus == 13 && pk.what.contains("(¹³C)") {
                for x in &mut l {
                    x.1 *= 5.0;
                }
            }
            (k, l)
        })
        .collect();
    let top = all.iter().flat_map(|(_, l)| l.iter().map(|x| x.1)).fold(0.0f64, f64::max).max(1e-6);
    let hover_x = resp.hover_pos().map(|h| h.x);
    let near = hover_x.and_then(|hx| peaks.iter().enumerate().map(|(k, pk)| (k, (x(pk.shift) - hx).abs())).filter(|(_, d)| *d < 8.0).min_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).map(|x| x.0));
    for (k, lines) in &all {
        let col = if Some(*k) == near { Color32::from_rgb(220, 90, 30) } else { ACCENT };
        for (ppm, h) in lines {
            let y = plot.bottom() - (h / top) as f32 * plot.height() * 0.92;
            p.line_segment([pos2(x(*ppm), plot.bottom()), pos2(x(*ppm), y)], egui::Stroke::new(1.5, col));
        }
    }
    // Shift labels, strongest first, without overlaps.
    let mut order: Vec<usize> = (0..peaks.len()).collect();
    order.sort_by(|a, b| peaks[*b].count.cmp(&peaks[*a].count));
    let mut used: Vec<Rect> = Vec::new();
    for k in order {
        let pk = peaks[k];
        let text = if carbon { format!("{:.1}", pk.shift) } else { format!("{:.2}", pk.shift) };
        let g = ui.painter().layout_no_wrap(text, font.clone(), Color32::from_gray(60));
        let r = Rect::from_center_size(pos2(x(pk.shift), rect.top() + 2.0 + g.size().y / 2.0), g.size() + vec2(4.0, 0.0));
        if used.iter().any(|u| u.intersects(r)) || r.left() < rect.left() || r.right() > rect.right() {
            continue;
        }
        p.galley(r.min + vec2(2.0, 0.0), g, Color32::from_gray(60));
        used.push(r);
    }
    let k = near?;
    let pk = peaks[k];
    let j = if pk.j.is_empty() || pk.mult == "m" { String::new() } else { format!(", J = {} Hz", pk.j.iter().map(|j| format!("{:.1}", j)).collect::<Vec<_>>().join(", ")) };
    let n = format!("{} {}", pk.count, match nucleus {
        13 => "C",
        19 => "F",
        31 => "P",
        15 => "N",
        _ => "H",
    });
    resp.on_hover_text(format!("δ {:.2} ppm  ·  {}{}  ·  {}  ·  {}", pk.shift, pk.mult, j, n, pk.what));
    Some(pk.atoms.clone())
}
