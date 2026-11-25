//! The Sequence panel: type or paste a protein, DNA or RNA sequence, see its length, formula
//! and mass, put it in the drawing as a numbered sequence block, and edit it there later.
//! FASTA text can be opened, saved, copied and pasted.

use super::*;
use crate::sequence::{self, Record, SeqKind};

#[derive(Default)]
pub(super) struct SeqEditor {
    pub kind: SeqKind,
    pub header: String,
    pub text: String,
    /// The sequence block in the drawing this editor is changing, if any.
    pub target: Option<usize>,
    /// An undo step was taken for the current round of live edits.
    pub undo_taken: bool,
}

impl App {
    /// Load a sequence block from the drawing into the editor.
    pub(super) fn open_sequence(&mut self, i: usize) {
        let Some(c) = self.doc.captions.get(i) else { return };
        let rec = sequence::parse_fasta(&c.text).into_iter().next().unwrap_or_default();
        self.seq = seq::SeqEditor { kind: c.seq.unwrap_or_default(), header: rec.header, text: rec.seq, target: Some(i), undo_taken: false };
        self.tab = Tab::Sequence;
        self.show_panel = true;
        self.sel = Selection { captions: [i].into_iter().collect(), ..Default::default() };
    }

    fn seq_record(&self) -> Record {
        let (seq, _) = sequence::clean(&self.seq.text, self.seq.kind);
        Record { header: self.seq.header.trim().to_string(), seq }
    }

    /// Where a new sequence block goes: under everything else.
    fn seq_place(&self) -> Pos2 {
        match self.doc.bounds_of(None) {
            Some(b) => pos2(b.min.x, b.max.y + self.doc.style.caption_size * 2.0),
            None => self.view_center() - vec2(150.0, 40.0),
        }
    }

    fn add_sequences(&mut self, recs: &[Record], kind: Option<SeqKind>) -> Option<usize> {
        if recs.is_empty() {
            return None;
        }
        self.checkpoint();
        let size = self.doc.style.caption_size.max(8.0);
        let mut pos = self.seq_place();
        let mut first = None;
        for r in recs {
            let k = kind.unwrap_or_else(|| sequence::guess_kind(&r.seq));
            let c = sequence::caption(r, k, pos, size);
            pos.y = crate::doc::caption_rect(&c).max.y + size * 1.5;
            self.doc.captions.push(c);
            first.get_or_insert(self.doc.captions.len() - 1);
        }
        self.dirty = true;
        first
    }

    /// Keep the sequence block in the drawing in step with the editor.
    fn push_live_edit(&mut self) {
        let Some(i) = self.seq.target else { return };
        if i >= self.doc.captions.len() || self.doc.captions[i].seq.is_none() {
            self.seq.target = None;
            return;
        }
        if !self.seq.undo_taken {
            self.checkpoint();
            self.seq.undo_taken = true;
        }
        let rec = self.seq_record();
        let c = &mut self.doc.captions[i];
        c.text = sequence::to_fasta(std::slice::from_ref(&rec), 60);
        c.seq = Some(self.seq.kind);
        self.dirty = true;
    }

    fn load_fasta_text(&mut self, text: &str) {
        let recs: Vec<Record> = sequence::parse_fasta(text).into_iter().filter(|r| !r.seq.is_empty()).collect();
        match recs.first() {
            Some(r) => {
                self.seq.kind = sequence::guess_kind(&r.seq);
                self.seq.header = r.header.clone();
                self.seq.text = r.seq.clone();
                self.push_live_edit();
                if recs.len() > 1 {
                    self.status(format!("{} sequences found; the first is in the editor. Use Open FASTA to add them all.", recs.len()));
                }
            }
            None => self.status("No sequence found in that text"),
        }
    }

    pub(super) fn sequence_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.label(egui::RichText::new("Sequence (FASTA)").strong());
        if let Some(i) = self.seq.target {
            if self.doc.captions.get(i).is_none_or(|c| c.seq.is_none()) {
                self.seq.target = None;
            }
        }
        // A sequence block selected in the drawing can be picked up here.
        let picked: Vec<usize> = self.sel.captions.iter().copied().filter(|&i| self.doc.captions.get(i).is_some_and(|c| c.seq.is_some())).collect();
        match self.seq.target {
            Some(_) => {
                ui.horizontal(|ui| {
                    ui.colored_label(ACCENT, "Editing the sequence in the drawing");
                    if ui.small_button("Done").clicked() {
                        self.seq.target = None;
                        self.seq.undo_taken = false;
                    }
                });
            }
            None if picked.len() == 1 => {
                if ui.button("Edit the selected sequence").clicked() {
                    self.open_sequence(picked[0]);
                }
            }
            None => {
                ui.label(egui::RichText::new("Double-click a sequence in the drawing to edit it here.").size(11.0).color(Color32::GRAY));
            }
        }
        ui.add_space(4.0);
        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Type:");
            for k in [SeqKind::Protein, SeqKind::Dna, SeqKind::Rna] {
                changed |= ui.radio_value(&mut self.seq.kind, k, k.name()).changed();
            }
        });
        ui.horizontal(|ui| {
            ui.label("Name:");
            changed |= ui.add(egui::TextEdit::singleline(&mut self.seq.header).hint_text("e.g. sp|P69905|HBA_HUMAN").desired_width(f32::INFINITY)).changed();
        });
        let r = ui.add(
            egui::TextEdit::multiline(&mut self.seq.text)
                .font(egui::TextStyle::Monospace)
                .desired_rows(7)
                .desired_width(f32::INFINITY)
                .hint_text("Type or paste letters (MKTAYIAKQR…, ATGGCC…) or a whole FASTA record"),
        );
        if r.changed() {
            if self.seq.text.trim_start().starts_with('>') {
                // A pasted FASTA record: take its name and letters.
                let t = self.seq.text.clone();
                self.load_fasta_text(&t);
            } else {
                changed = true;
            }
        }
        let (clean, bad) = sequence::clean(&self.seq.text, self.seq.kind);
        if !bad.is_empty() {
            let list: Vec<String> = bad.iter().map(|c| c.to_string()).collect();
            let hint = if self.seq.kind != SeqKind::Protein && sequence::guess_kind(&self.seq.text) == SeqKind::Protein { " — is this a protein?" } else { "" };
            ui.colored_label(Color32::from_rgb(200, 0, 0), format!("Not {} letters: {}{}", self.seq.kind.name(), list.join(", "), hint));
        }
        if changed {
            self.push_live_edit();
        }

        // Peptides: D-amino acids (lower-case letters) and head-to-tail cyclic peptides.
        if self.seq.kind == SeqKind::Protein {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.seq_cyclic, "Cyclic (head to tail)");
                let nd = sequence::peptide_residues(&self.seq.text).iter().filter(|r| r.d).count();
                ui.label(egui::RichText::new(if nd > 0 { format!("{} D-amino acid(s)", nd) } else { "lower case = D-amino acid".into() }).size(11.0).color(Color32::GRAY));
            });
        }
        // Numbers.
        let mut st = sequence::stats(&clean, self.seq.kind);
        if self.seq.kind == SeqKind::Protein && self.seq_cyclic {
            let res = sequence::peptide_residues(&clean);
            st.formula = sequence::peptide_formula(&res, true);
        }
        egui::Grid::new("seq-stats").num_columns(2).striped(true).spacing(vec2(10.0, 4.0)).show(ui, |ui| {
            ui.label("Length:");
            ui.label(format!("{} {}", st.length, if self.seq.kind == SeqKind::Protein { "residues" } else { "nucleotides" }));
            ui.end_row();
            if let Some(f) = &st.formula {
                ui.label("Formula:");
                ui.label(formula_job(f, 0));
                ui.end_row();
                ui.label("Exact Mass:");
                ui.label(format!("{:.4}", chem::exact_mass(f, 0)));
                ui.end_row();
                ui.label("Mol. Wt.:");
                ui.label(format!("{:.2}", chem::mol_weight(f)));
                ui.end_row();
            }
            if let Some(gc) = st.gc {
                ui.label("GC content:");
                ui.label(format!("{:.1}%", gc));
                ui.end_row();
            }
            if !st.counts.is_empty() {
                ui.label("Composition:");
                let s: Vec<String> = st.counts.iter().map(|(c, n)| format!("{} {}", c, n)).collect();
                ui.add(egui::Label::new(s.join(", ")).wrap());
                ui.end_row();
            }
        });
        if st.formula.is_none() && !clean.is_empty() {
            ui.label(egui::RichText::new("No formula: the sequence has letters without a fixed composition (X, N, B, Z, gaps or stops).").size(11.0).color(Color32::GRAY));
        } else if self.seq.kind != SeqKind::Protein && !clean.is_empty() {
            ui.label(egui::RichText::new("Single strand with 5′-OH and 3′-OH ends.").size(11.0).color(Color32::GRAY));
        }

        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if self.seq.target.is_none() && ui.add_enabled(!clean.is_empty(), egui::Button::new("Add to Drawing")).clicked() {
                let rec = self.seq_record();
                if let Some(i) = self.add_sequences(&[rec], Some(self.seq.kind)) {
                    self.seq.target = Some(i);
                    self.seq.undo_taken = false;
                    self.sel = Selection { captions: [i].into_iter().collect(), ..Default::default() };
                    self.status("Added the sequence to the drawing");
                }
            }
            if ui.button("New").clicked() {
                self.seq = seq::SeqEditor { kind: self.seq.kind, ..Default::default() };
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button("Open FASTA…").clicked() {
                let f = rfd::FileDialog::new().add_filter("FASTA", &["fasta", "fa", "fas", "faa", "fna", "ffn", "seq", "txt"]).pick_file();
                if let Some(p) = f {
                    match std::fs::read_to_string(&p) {
                        Ok(t) => {
                            let recs: Vec<Record> = sequence::parse_fasta(&t).into_iter().filter(|r| !r.seq.is_empty()).collect();
                            match self.add_sequences(&recs, None) {
                                Some(i) => {
                                    self.open_sequence(i);
                                    self.status(format!("Added {} sequence(s) from {}", recs.len(), file_name(&p)));
                                }
                                None => self.status(format!("No sequences in {}", file_name(&p))),
                            }
                        }
                        Err(e) => self.status(format!("Could not read {}: {}", file_name(&p), e)),
                    }
                }
            }
            if ui.add_enabled(!clean.is_empty(), egui::Button::new("Save FASTA…")).clicked() {
                let name = if self.seq.header.trim().is_empty() { "sequence".to_string() } else { self.seq.header.split_whitespace().next().unwrap_or("sequence").replace(['|', '/', ':'], "_") };
                if let Some(p) = rfd::FileDialog::new().add_filter("FASTA", &["fasta", "fa"]).set_file_name(format!("{}.fasta", name)).save_file() {
                    let text = sequence::to_fasta(&[self.seq_record()], 60);
                    match std::fs::write(&p, text) {
                        Ok(_) => self.status(format!("Saved {}", file_name(&p))),
                        Err(e) => self.status(format!("Could not save {}: {}", file_name(&p), e)),
                    }
                }
            }
            if ui.add_enabled(!clean.is_empty(), egui::Button::new("Copy FASTA")).clicked() {
                ctx.copy_text(sequence::to_fasta(&[self.seq_record()], 60));
                self.status("Copied the sequence as FASTA");
            }
            if clip::NATIVE && ui.button("Paste FASTA").clicked() {
                match clip::read_text() {
                    Some(t) => self.load_fasta_text(&t),
                    None => self.status("There is no text on the clipboard"),
                }
            }
        });

        ui.add_space(6.0);
        ui.label(egui::RichText::new("Tools").strong());
        ui.horizontal_wrapped(|ui| {
            let nuc = self.seq.kind != SeqKind::Protein;
            let mut replace: Option<(String, SeqKind, String)> = None;
            if nuc {
                if ui.button("Reverse Complement").clicked() {
                    replace = Some((sequence::reverse_complement(&clean, self.seq.kind), self.seq.kind, "reverse complement".into()));
                }
                if ui.button("Complement").clicked() {
                    replace = Some((sequence::complement(&clean, self.seq.kind), self.seq.kind, "complement".into()));
                }
                if self.seq.kind == SeqKind::Dna {
                    if ui.button("Transcribe to RNA").clicked() {
                        replace = Some((sequence::transcribe(&clean), SeqKind::Rna, "RNA".into()));
                    }
                } else if ui.button("Back-transcribe to DNA").clicked() {
                    replace = Some((sequence::back_transcribe(&clean), SeqKind::Dna, "DNA".into()));
                }
                if ui.button("Translate to Protein").on_hover_text("Standard genetic code, first reading frame; * marks a stop").clicked() {
                    replace = Some((sequence::translate(&clean), SeqKind::Protein, "translation".into()));
                }
                let r = ui.add_enabled(!clean.is_empty(), egui::Button::new("Draw as Monomers (HELM)"));
                if r.on_hover_text("Draw the strand as HELM monomers: sugars (R or dR) joined by phosphates (P), each carrying its base").clicked() {
                    let d = sequence::nucleic_monomers(&clean, self.seq.kind);
                    self.insert_doc(d, if self.seq.kind == SeqKind::Rna { "Drew the RNA as monomers" } else { "Drew the DNA as monomers" });
                }
            } else {
                let res: Vec<sequence::Residue> = sequence::peptide_residues(&self.seq.text).into_iter().filter(|r| SeqKind::Protein.alphabet().contains(r.code)).collect();
                let cyc = self.seq_cyclic;
                let short = res.len() <= 40;
                let r = ui.add_enabled(!res.is_empty() && short, egui::Button::new("Draw Structure"));
                if r.on_hover_text("Draw the peptide atom by atom (up to 40 residues). Lower-case letters are D-amino acids.").clicked() {
                    match sequence::peptide_smiles_of(&res, cyc).and_then(|s| formats::parse_smiles(&s).ok()) {
                        Some(d) => self.insert_doc(d, if cyc { "Drew the cyclic peptide" } else { "Drew the peptide structure" }),
                        None => self.status("Only the 20 standard amino acids (and U) can be drawn as a structure"),
                    }
                }
                let r = ui.add_enabled(!res.is_empty(), egui::Button::new("Draw as Monomers (HELM)"));
                if r.on_hover_text("Draw the peptide as a chain of amino-acid circles, as the HELM editor does (dA, dF… for D-amino acids)").clicked() {
                    let d = sequence::peptide_monomers(&res, cyc);
                    self.insert_doc(d, "Drew the peptide as monomers");
                }
            }
            if let Some((s, k, what)) = replace {
                // The result becomes a new sequence (the original block stays as it was).
                self.seq = seq::SeqEditor {
                    kind: k,
                    header: if self.seq.header.trim().is_empty() { what } else { format!("{} {}", self.seq.header.trim(), what) },
                    text: s,
                    target: None,
                    undo_taken: false,
                };
            }
        });
    }
}
