//! Isotope labels: mark atoms as ¹³C, ¹⁵N, ¹⁸O, ²H… to show enrichment. Set from the atom's
//! right-click menu, Structure ▸ Isotope, or by typing the mass number in a label ("13C").

use super::*;

impl App {
    /// Atoms an isotope command applies to: the selection if it holds the atom (or no atom is
    /// given), else the atom alone.
    fn isotope_targets(&self, atom: Option<usize>) -> Vec<usize> {
        match atom {
            Some(a) if !self.sel.atoms.contains(&a) => vec![a],
            _ => self.sel.atoms.iter().copied().collect(),
        }
    }

    /// Set (or with 0 remove) the mass number on atoms. Atoms of another element keep theirs.
    pub(super) fn set_isotope(&mut self, atoms: &[usize], el: u8, mass: u16) {
        let atoms: Vec<usize> = atoms.iter().copied().filter(|&i| i < self.doc.atoms.len() && self.doc.atoms[i].el == el).collect();
        if atoms.is_empty() {
            return;
        }
        self.checkpoint();
        for &i in &atoms {
            self.doc.atoms[i].isotope = mass;
        }
        self.dirty = true;
        let sym = elements::symbol(el);
        self.status(if mass == 0 {
            format!("{} atom(s) set to natural {}", atoms.len(), sym)
        } else {
            format!("{} atom(s) labelled as {}{}", atoms.len(), elements::superscript_number(mass), sym)
        });
    }

    /// The Isotope menu for the element of the given atoms.
    pub(super) fn isotope_menu(&mut self, ui: &mut egui::Ui, atom: Option<usize>) {
        let targets = self.isotope_targets(atom);
        let Some(&first) = targets.first() else {
            ui.label(egui::RichText::new("Select atoms, or right-click one.").color(Color32::GRAY));
            return;
        };
        // The element of the clicked atom, or the most common one in the selection.
        let el = atom.map(|a| self.doc.atoms[a].el).unwrap_or_else(|| {
            let mut count: Vec<(u8, usize)> = Vec::new();
            for &i in &targets {
                let z = self.doc.atoms[i].el;
                match count.iter_mut().find(|c| c.0 == z) {
                    Some(c) => c.1 += 1,
                    None => count.push((z, 1)),
                }
            }
            count.iter().max_by_key(|c| c.1).map(|c| c.0).unwrap_or(self.doc.atoms[first].el)
        });
        let sym = elements::symbol(el);
        let current = self.doc.atoms[first].isotope;
        let n = targets.iter().filter(|&&i| self.doc.atoms[i].el == el).count();
        if targets.len() > 1 {
            ui.label(egui::RichText::new(format!("{} {} atom(s)", n, sym)).color(Color32::GRAY));
        }
        if ui.radio(current == 0, format!("Natural {}", sym)).clicked() {
            self.set_isotope(&targets, el, 0);
            ui.close();
        }
        for a in elements::known_isotopes(el) {
            let name = match (el, a) {
                (1, 2) => "²H (D, deuterium)".to_string(),
                (1, 3) => "³H (T, tritium)".to_string(),
                _ => format!("{}{}", elements::superscript_number(a), sym),
            };
            let mass = elements::isotope_mass(el, a).map(|m| format!("  {:.4}", m)).unwrap_or_default();
            if ui.radio(current == a, format!("{}{}", name, mass)).clicked() {
                self.set_isotope(&targets, el, a);
                ui.close();
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Mass number:");
            ui.add(egui::DragValue::new(&mut self.iso_other).range(1..=300));
            if ui.button("Set").clicked() {
                let m = self.iso_other;
                self.set_isotope(&targets, el, m);
                ui.close();
            }
        });
        if elements::isotope_mass(el, self.iso_other).is_none() {
            ui.label(egui::RichText::new("Not in the isotope table: shown, but masses use the natural element.").small().color(Color32::GRAY));
        }
    }

    /// Right-click menu on an atom.
    pub(super) fn atom_context_menu(&mut self, ui: &mut egui::Ui) {
        let Some(a) = self.ctx_atom.filter(|a| *a < self.doc.atoms.len()) else {
            ui.close();
            return;
        };
        let atom = &self.doc.atoms[a];
        let title = match &atom.label {
            Some(l) => l.clone(),
            None if atom.isotope != 0 => format!("{}{}", elements::superscript_number(atom.isotope), elements::symbol(atom.el)),
            None => elements::symbol(atom.el).to_string(),
        };
        ui.label(egui::RichText::new(format!("Atom: {}", title)).strong());
        ui.menu_button("Isotope", |ui| self.isotope_menu(ui, Some(a)));
        ui.separator();
        if ui.button("Charge +1").clicked() {
            self.checkpoint();
            let c = &mut self.doc.atoms[a].charge;
            *c = (*c + 1).min(9);
            self.dirty = true;
            ui.close();
        }
        if ui.button("Charge −1").clicked() {
            self.checkpoint();
            let c = &mut self.doc.atoms[a].charge;
            *c = (*c - 1).max(-9);
            self.dirty = true;
            ui.close();
        }
        if ui.button("Edit Label…").clicked() {
            let text = current_label(&self.doc, a);
            self.editing = Some(Editing::Label { atom: a, text, fresh: false });
            self.edit_focus = true;
            ui.close();
        }
    }
}
