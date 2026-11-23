//! The docked side panel: Analysis, Chemical Properties, Info and the other tabs.

use super::*;
use crate::props;

/// Rows of the Chemical Properties tab.
pub(super) const PROP_ROWS: usize = 13;

/// Rows that need a licensed calculator this app doesn't include.
const NOT_INCLUDED: &[(&str, &str)] = &[("Henry's Law", ""), ("CLogP", ""), ("CMR", "cm³/mol")];

const AN_LABELS: [&str; 5] = ["Formula:", "Exact Mass:", "Mol. Wt.:", "m/z:", "Elem. Anal.:"];
const AN_PASTE: [&str; 5] = ["Chemical Formula: ", "Exact Mass: ", "Molecular Weight: ", "m/z: ", "Elemental Analysis: "];

/// Name, value (None = not available) and unit of every property row, in display order.
fn property_rows(p: Option<&props::ChemProps>) -> Vec<(&'static str, Option<String>, &'static str)> {
    let mut rows = match p {
        Some(p) => props::rows(p),
        None => props::rows(&props::ChemProps::default()),
    };
    let at = rows.iter().position(|r| r.0 == "Heat of Form").unwrap_or(rows.len());
    rows.insert(at, (NOT_INCLUDED[0].0, None, NOT_INCLUDED[0].1));
    for (n, u) in &NOT_INCLUDED[1..] {
        rows.push((n, None, u));
    }
    rows
}

impl App {
    pub(super) fn side_panel(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal_wrapped(|ui| {
            for (t, name) in [
                (Tab::Analysis, "Analysis"),
                (Tab::Properties, "Properties"),
                (Tab::Info, "Info"),
                (Tab::Elements, "Periodic Table"),
                (Tab::Names, "Name ↔ Structure"),
                (Tab::Sequence, "Sequence"),
                (Tab::Nmr, "NMR"),
                (Tab::Style, "Document"),
                (Tab::Help, "Help"),
            ] {
                if ui.selectable_label(self.tab == t, name).clicked() {
                    self.tab = t;
                }
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match self.tab {
            Tab::Analysis => self.analysis_ui(ui, ctx),
            Tab::Properties => self.properties_ui(ui, ctx),
            Tab::Info => self.info_ui(ui, ctx),
            Tab::Elements => self.elements_ui(ui),
            Tab::Names => self.names_ui(ui, ctx),
            Tab::Sequence => self.sequence_ui(ui, ctx),
            Tab::Nmr => self.nmr_ui(ui, ctx),
            Tab::Style => self.style_ui(ui),
            Tab::Help => help_ui(ui),
        });
    }

    /// The five Analysis values as text, each joined over the analysed structures.
    fn analysis_values(&self) -> [String; 5] {
        let d = self.an_decimals;
        let lines = &self.analysis.lines;
        let join = |f: &dyn Fn(&FragmentInfo) -> String, sep: &str| lines.iter().map(f).collect::<Vec<_>>().join(sep);
        [
            join(&|f| chem::formula_string(&f.formula, f.charge), ", "),
            join(&|f| format!("{:.*}", d, f.exact), ", "),
            join(&|f| format!("{:.*}", d, f.mw), ", "),
            join(&|f| format!("{} {}", f.ion, f.mz.iter().map(|(m, i)| format!("{:.*} ({:.1}%)", d, m, i)).collect::<Vec<_>>().join(", ")), "; "),
            join(&|f| f.ea.iter().map(|(z, p)| format!("{}, {:.2}", elements::symbol(*z), p)).collect::<Vec<_>>().join("; "), " / "),
        ]
    }

    fn analysis_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let a = self.analysis.clone();
        ui.label(egui::RichText::new(if self.sel.atoms.is_empty() { "Whole drawing" } else { "Selection" }).color(Color32::GRAY));
        let has = !a.lines.is_empty();
        let vals = self.analysis_values();
        egui::Frame::group(ui.style()).fill(Color32::WHITE).show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Grid::new("an-main").num_columns(2).spacing(vec2(8.0, 5.0)).show(ui, |ui| {
                for k in 0..5 {
                    let r = ui.checkbox(&mut self.an_show[k], AN_LABELS[k]);
                    match k {
                        1 => {
                            r.on_hover_text("Monoisotopic mass: the most abundant isotope of every element (an ion's charge is included)");
                        }
                        3 => {
                            r.on_hover_text("Isotope peaks of the ion a mass spectrometer sees: [M+H]⁺ for a neutral molecule, the molecule itself if it is a cation or an anion");
                        }
                        _ => {}
                    }
                    if !has {
                        ui.label("");
                    } else if k == 0 && a.lines.len() == 1 {
                        ui.label(formula_job(&a.lines[0].formula, a.lines[0].charge));
                    } else {
                        ui.add(egui::Label::new(&vals[k]).wrap());
                    }
                    ui.end_row();
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let any = has && self.an_show.iter().any(|&b| b);
                if ui.add_enabled(any, egui::Button::new("Paste")).on_hover_text("Add the ticked lines as text under the structure").clicked() {
                    let text = (0..5).filter(|&k| self.an_show[k]).map(|k| format!("{}{}", AN_PASTE[k], vals[k])).collect::<Vec<_>>().join("\n");
                    self.caption_below(text, true, ctx);
                    self.status("Pasted the analysis under the structure");
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(egui::DragValue::new(&mut self.an_decimals).range(0..=8));
                    ui.label("Decimals:");
                });
            });
        });
        if !has {
            ui.add_space(8.0);
            ui.label("Draw a structure to see its formula, masses and properties here.");
            return;
        }
        ui.add_space(8.0);
        self.spectrum_ui(ui, ctx, &a);
        ui.add_space(8.0);
        for (k, f) in a.lines.iter().enumerate() {
            ui.label(egui::RichText::new(if a.lines.len() > 1 { format!("Structure {}", k + 1) } else { "Details".into() }).strong());
            egui::Grid::new(format!("an{}", k)).num_columns(2).spacing(vec2(10.0, 4.0)).striped(true).show(ui, |ui| {
                ui.label("Name:");
                ui.add(egui::Label::new(f.name.clone().unwrap_or_else(|| "—".into())).wrap()).on_hover_text("Names come from a built-in list and systematic rules for simple compounds; no online service is used.");
                ui.end_row();
                for (n, v) in [
                    ("Heavy atoms:", f.props.heavy_atoms.to_string()),
                    ("Rings:", f.props.rings.to_string()),
                    ("H-bond donors:", f.props.donors.to_string()),
                    ("H-bond acceptors:", f.props.acceptors.to_string()),
                    ("Rotatable bonds:", f.props.rotatable.to_string()),
                ] {
                    ui.label(n);
                    ui.label(v);
                    ui.end_row();
                }
            });
            ui.add_space(4.0);
        }
        if a.warnings > 0 {
            ui.colored_label(Color32::from_rgb(200, 0, 0), format!("⚠ {} atom(s) have more bonds than allowed", a.warnings));
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new("SMILES").strong());
        let mut s = a.smiles.clone();
        ui.add(egui::TextEdit::multiline(&mut s).desired_rows(2).desired_width(f32::INFINITY));
        ui.horizontal(|ui| {
            if ui.button("Copy SMILES").clicked() {
                ctx.copy_text(a.smiles.clone());
                self.status("Copied SMILES");
            }
            if ui.button("Copy formula").clicked() {
                ctx.copy_text(a.total_formula.clone());
                self.status("Copied formula");
            }
        });
    }

    /// Predicted mass spectrum of each analysed structure: the ion, its monoisotopic m/z and the
    /// isotope peaks drawn as a stick spectrum.
    fn spectrum_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, a: &Analysis) {
        let d = self.an_decimals;
        ui.label(egui::RichText::new("Mass Spectrum (predicted)").strong());
        for (k, f) in a.lines.iter().enumerate().take(4) {
            if f.spectrum.is_empty() {
                continue;
            }
            egui::Frame::group(ui.style()).fill(Color32::WHITE).show(ui, |ui| {
                ui.set_width(ui.available_width());
                if a.lines.len() > 1 {
                    ui.label(egui::RichText::new(format!("Structure {}", k + 1)).color(Color32::GRAY));
                }
                let top = f.spectrum.iter().cloned().fold((0.0, -1.0), |b, p| if p.1 > b.1 { p } else { b });
                egui::Grid::new(format!("ms{}", k)).num_columns(2).spacing(vec2(8.0, 3.0)).show(ui, |ui| {
                    ui.label("Ion:");
                    ui.horizontal(|ui| {
                        ui.label(&f.ion);
                        let ion = chem::ms_ion(&f.formula, f.charge);
                        ui.label(formula_job(&ion.formula, ion.charge));
                    });
                    ui.end_row();
                    ui.label("Monoisotopic m/z:");
                    ui.label(format!("{:.*}", d, f.ion_mz));
                    ui.end_row();
                    if (top.0 - f.ion_mz).abs() > 0.3 / (f.charge.abs().max(1) as f64) {
                        ui.label("Most abundant m/z:");
                        ui.label(format!("{:.*}", d, top.0));
                        ui.end_row();
                    }
                });
                ui.add_space(4.0);
                spectrum_plot(ui, &f.spectrum);
                ui.add_space(2.0);
                egui::CollapsingHeader::new(format!("Peaks ({})", f.spectrum.len())).id_salt(("peaks", k)).show(ui, |ui| {
                    egui::Grid::new(format!("pk{}", k)).num_columns(2).striped(true).spacing(vec2(16.0, 2.0)).show(ui, |ui| {
                        ui.label(egui::RichText::new("m/z").strong());
                        ui.label(egui::RichText::new("Intensity").strong());
                        ui.end_row();
                        for (m, i) in &f.spectrum {
                            ui.label(format!("{:.*}", d, m));
                            ui.label(format!("{:.1}%", i));
                            ui.end_row();
                        }
                    });
                });
                ui.horizontal(|ui| {
                    if ui.button("Copy peaks").on_hover_text("Copy the peak list as text (m/z and intensity, tab separated)").clicked() {
                        let mut t = format!("{} {}\nm/z\tIntensity (%)\n", f.ion, f.ion_formula);
                        for (m, i) in &f.spectrum {
                            t += &format!("{:.*}\t{:.1}\n", d, m, i);
                        }
                        ctx.copy_text(t);
                        self.status("Copied the predicted peaks");
                    }
                });
            });
            ui.add_space(4.0);
        }
    }

    /// The Chemical Properties report as plain text.
    pub(super) fn properties_report(&self) -> String {
        let Some(p) = self.analysis.chem.as_ref() else {
            return "Select a single structure to see its properties.".into();
        };
        let mut out = String::new();
        if let Some(f) = self.analysis.lines.first() {
            if let Some(n) = &f.name {
                out += &format!("Name: {}\n", n);
            }
            out += &format!("Formula: {}\n\n", chem::formula_string(&f.formula, f.charge));
        }
        for (n, v, u) in property_rows(Some(p)) {
            let unit = if u.is_empty() { String::new() } else { format!(" [{}]", u) };
            out += &format!("{:<14}{}{}\n", format!("{}:", n), v.unwrap_or_else(|| "—".into()), unit);
        }
        out += "\nBoiling and melting points, critical values and energies: Joback group contributions.\n";
        out += "Log P: Moriguchi method. MR: atom refractivities. tPSA: Ertl fragment method.\n";
        out += "Henry's Law, CLogP and CMR need licensed calculators that are not included.\n";
        if !p.unknown_groups.is_empty() {
            out += &format!("Groups without Joback values: {}\n", p.unknown_groups.join(", "));
        }
        out
    }

    fn properties_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let p = self.analysis.chem.clone();
        let rows = property_rows(p.as_ref());
        if self.prop_show.len() != rows.len() {
            self.prop_show = vec![true; rows.len()];
        }
        ui.label(egui::RichText::new("Chemical Properties").strong());
        if p.is_none() {
            ui.label(egui::RichText::new(if self.analysis.lines.is_empty() { "Draw a structure to estimate its properties." } else { "Select a single structure." }).color(Color32::GRAY));
        }
        egui::Frame::group(ui.style()).fill(Color32::WHITE).show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Grid::new("props").num_columns(3).striped(true).spacing(vec2(8.0, 4.0)).show(ui, |ui| {
                ui.label(egui::RichText::new("Property").strong());
                ui.label(egui::RichText::new("Value").strong());
                ui.label(egui::RichText::new("Unit").strong());
                ui.end_row();
                for (k, (n, v, u)) in rows.iter().enumerate() {
                    let na = NOT_INCLUDED.iter().any(|x| x.0 == *n);
                    if na {
                        ui.add_enabled(false, egui::Checkbox::new(&mut false, *n));
                    } else {
                        ui.checkbox(&mut self.prop_show[k], *n);
                    }
                    let txt = match (v, p.is_some()) {
                        (Some(v), _) => egui::RichText::new(v.clone()),
                        (None, true) if na => egui::RichText::new("—").color(Color32::GRAY),
                        (None, true) => egui::RichText::new("n/a").color(Color32::GRAY),
                        (None, false) => egui::RichText::new(""),
                    };
                    let r = ui.label(txt);
                    if na {
                        r.on_hover_text("Needs a licensed calculator that is not included");
                    } else if v.is_none() && p.is_some() {
                        r.on_hover_text("This structure has a group the estimation method has no value for");
                    }
                    ui.label(*u);
                    ui.end_row();
                }
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let ok = p.is_some();
            if ui.add_enabled(ok, egui::Button::new("Paste")).on_hover_text("Add the ticked properties as text under the structure").clicked() {
                let text = rows
                    .iter()
                    .enumerate()
                    .filter(|(k, r)| self.prop_show[*k] && r.1.is_some())
                    .map(|(_, (n, v, u))| if u.is_empty() { format!("{}: {}", n, v.clone().unwrap_or_default()) } else { format!("{}: {} [{}]", n, v.clone().unwrap_or_default(), u) })
                    .collect::<Vec<_>>()
                    .join("\n");
                if !text.is_empty() {
                    self.caption_below(text, false, ctx);
                    self.status("Pasted the properties under the structure");
                }
            }
            if ui.add_enabled(ok, egui::Button::new("Report…")).clicked() {
                self.show_report = true;
            }
        });
        ui.add_space(6.0);
        ui.label(
            egui::RichText::new("Estimates calculated on this computer: Joback group contributions (temperatures, critical values, energies), Moriguchi Log P, atom-refractivity MR and Ertl tPSA.")
                .size(11.0)
                .color(Color32::GRAY),
        );
    }

    fn info_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let upm = arrange::units_per_mm(&self.doc);
        let cm = |u: f32| format!("{:.2} cm", u / (upm * 10.0));
        ui.label(egui::RichText::new("Info").strong());
        egui::Frame::group(ui.style()).fill(Color32::WHITE).show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Grid::new("info").num_columns(4).spacing(vec2(8.0, 5.0)).show(ui, |ui| {
                let (x, y) = self.hover_pos.map(|p| (cm(p.x), cm(p.y))).unwrap_or_default();
                ui.label("X:");
                ui.label(x);
                ui.label("Y:");
                ui.label(y);
                ui.end_row();
                let (dx, dy) = match (self.drag_origin, self.hover_pos.or(ctx.pointer_interact_pos().map(|p| self.to_doc(p)))) {
                    (Some(a), Some(b)) if self.drag.is_some() => (cm(b.x - a.x), cm(b.y - a.y)),
                    _ => Default::default(),
                };
                ui.label("ΔX:");
                ui.label(dx);
                ui.label("ΔY:");
                ui.label(dy);
                ui.end_row();
                let b = if self.sel.is_empty() { None } else { arrange::visual_bounds(&self.doc, &self.sel, ctx).or_else(|| self.doc.bounds_of(Some(&self.sel))) };
                let [sx, sy, sw, sh] = b.map(|b| [cm(b.min.x), cm(b.min.y), cm(b.width()), cm(b.height())]).unwrap_or_default();
                ui.label("Sel X:");
                ui.label(sx);
                ui.label("Sel Y:");
                ui.label(sy);
                ui.end_row();
                ui.label("Width:");
                ui.label(sw);
                ui.label("Height:");
                ui.label(sh);
                ui.end_row();
            });
        });
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Tracking").strong());
        let track = match &self.drag {
            Some(Drag::Bond { start, cur, .. } | Drag::Chain { start, cur, .. } | Drag::Arrow { start, cur }) => {
                let d = *cur - *start;
                Some((-d.y.atan2(d.x).to_degrees(), d.length()))
            }
            Some(Drag::Rotate { angle, .. }) => Some((-angle.to_degrees(), 0.0)),
            Some(Drag::Move { last }) => self.drag_origin.map(|o| {
                let d = *last - o;
                (-d.y.atan2(d.x).to_degrees(), d.length())
            }),
            _ => None,
        };
        egui::Grid::new("track").num_columns(2).spacing(vec2(8.0, 4.0)).show(ui, |ui| {
            ui.label("Angle:");
            ui.label(track.map(|t| format!("{:.1}°", t.0)).unwrap_or_default());
            ui.end_row();
            ui.label("Distance:");
            ui.label(track.filter(|t| t.1 > 0.0).map(|t| cm(t.1)).unwrap_or_default());
            ui.end_row();
        });
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Positions are measured from the ruler origin; Y grows downwards. Angles are counter-clockwise from the right.").size(11.0).color(Color32::GRAY));
    }
}

/// A stick spectrum: relative intensity against m/z, with the stronger peaks labelled and the
/// value of the nearest peak shown under the pointer.
fn spectrum_plot(ui: &mut egui::Ui, peaks: &[(f64, f64)]) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 150.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    let grey = Color32::from_gray(150);
    let font = egui::FontId::proportional(9.5);
    let plot = Rect::from_min_max(rect.min + vec2(30.0, 16.0), rect.max - vec2(10.0, 18.0));
    let lo_m = peaks.iter().map(|x| x.0).fold(f64::MAX, f64::min);
    let hi_m = peaks.iter().map(|x| x.0).fold(f64::MIN, f64::max);
    let pad = ((hi_m - lo_m) * 0.12).max(0.6);
    let (lo, hi) = (lo_m - pad, hi_m + pad);
    let x = |m: f64| plot.left() + ((m - lo) / (hi - lo)) as f32 * plot.width();
    let y = |pct: f64| plot.bottom() - (pct / 100.0) as f32 * plot.height();
    // Axes, intensity ticks and m/z ticks.
    p.line_segment([plot.left_bottom(), plot.right_bottom()], egui::Stroke::new(1.0, grey));
    p.line_segment([plot.left_bottom(), plot.left_top()], egui::Stroke::new(1.0, grey));
    for pct in [0.0, 50.0, 100.0] {
        p.line_segment([pos2(plot.left() - 3.0, y(pct)), pos2(plot.left(), y(pct))], egui::Stroke::new(1.0, grey));
        p.text(pos2(plot.left() - 5.0, y(pct)), egui::Align2::RIGHT_CENTER, format!("{}", pct as i32), font.clone(), grey);
    }
    let range = hi - lo;
    let step = [0.1, 0.2, 0.5, 1.0, 2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0, 500.0].into_iter().find(|s| range / s <= 6.0).unwrap_or(1000.0);
    let mut t = (lo / step).ceil() * step;
    while t <= hi {
        let px = x(t);
        p.line_segment([pos2(px, plot.bottom()), pos2(px, plot.bottom() + 3.0)], egui::Stroke::new(1.0, grey));
        let dec = if step < 1.0 { 1 } else { 0 };
        p.text(pos2(px, plot.bottom() + 4.0), egui::Align2::CENTER_TOP, format!("{:.*}", dec, t), font.clone(), grey);
        t += step;
    }
    p.text(pos2(plot.right(), rect.bottom() - 1.0), egui::Align2::RIGHT_BOTTOM, "m/z", font.clone(), grey);
    // Sticks, then labels for the stronger peaks (strongest first, without overlaps).
    for (m, i) in peaks {
        p.line_segment([pos2(x(*m), plot.bottom()), pos2(x(*m), y(*i))], egui::Stroke::new(2.0, ACCENT));
    }
    let mut order: Vec<&(f64, f64)> = peaks.iter().filter(|pk| pk.1 >= 2.0).collect();
    order.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
    let mut used: Vec<Rect> = Vec::new();
    for (m, i) in order {
        let text = format!("{:.4}", m);
        let g = ui.painter().layout_no_wrap(text, font.clone(), Color32::from_gray(60));
        let r = Rect::from_center_size(pos2(x(*m), y(*i) - 2.0 - g.size().y / 2.0), g.size() + vec2(4.0, 0.0));
        if used.iter().any(|u| u.intersects(r)) || r.left() < rect.left() || r.right() > rect.right() {
            continue;
        }
        p.galley(r.min + vec2(2.0, 0.0), g, Color32::from_gray(60));
        used.push(r);
    }
    // Hover: the nearest peak.
    if let Some(h) = resp.hover_pos() {
        if let Some((m, i)) = peaks.iter().min_by(|a, b| (x(a.0) - h.x).abs().partial_cmp(&(x(b.0) - h.x).abs()).unwrap()) {
            if (x(*m) - h.x).abs() < 8.0 {
                p.line_segment([pos2(x(*m), plot.bottom()), pos2(x(*m), y(*i))], egui::Stroke::new(3.0, Color32::from_rgb(220, 90, 30)));
                resp.on_hover_text(format!("m/z {:.4}  ·  {:.1}%", m, i));
            }
        }
    }
}
