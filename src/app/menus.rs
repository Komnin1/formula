//! The menu bar, laid out like a desktop chemistry drawing program, and the commands that only
//! the menus reach (recent files, printing, frames, stacking order, text styles).

use super::*;

/// Every menu command that has a key, for the Shortcuts cheat sheet.
pub(super) const SHORTCUTS: &[(&str, &str)] = &[
    ("New Document", "⌘N"),
    ("Open…", "⌘O"),
    ("Close", "⌘W"),
    ("Save", "⌘S"),
    ("Save As…", "⇧⌘S"),
    ("Print…", "⌘P"),
    ("Undo", "⌘Z"),
    ("Redo", "⇧⌘Z"),
    ("Cut / Copy / Paste", "⌘X / ⌘C / ⌘V"),
    ("Copy for LibreOffice / for Word (the other one from ⌘C, Windows)", "⇧⌘C"),
    ("Select All", "⌘A"),
    ("Invert Selection", "⇧⌘I"),
    ("Show Crosshair", "⌥⌘X"),
    ("Show Rulers", "⌘;"),
    ("Show Document", "⌘/"),
    ("Magnify / Reduce", "⇧⌘> / ⇧⌘<"),
    ("Zoom In / Out", "⌘+ / ⌘−"),
    ("Fit to Window", "⌘0"),
    ("Fixed Lengths", "⌘L"),
    ("Fixed Angles", "⌘E"),
    ("Scale…", "⌘K"),
    ("Bring to Front / Send to Back", "⌘[ / ⌘]"),
    ("Align Left / L-R Centers / Right", "⌥⇧⌘L / ⌥⇧⌘C / ⌥⇧⌘R"),
    ("Align Top / T-B Centers / Bottom", "⌥⇧⌘T / ⌥⇧⌘M / ⌥⇧⌘B"),
    ("Distribute Horizontally / Vertically", "⌥⇧⌘H / ⌥⇧⌘V"),
    ("Rotate…", "⌘R"),
    ("Rotate 180° Horizontal / Vertical", "⇧⌘H / ⇧⌘V"),
    ("Clean Up Structure", "⇧⌘K"),
    ("Convert Name to Structure", "⇧⌘N"),
    ("Convert Structure to Name", "⌥⌘N"),
    ("Plain / Bold / Italic / Formula text", "⌘T / ⌘B / ⌘I / ⌘F"),
];

const TEXT_SIZES: [f32; 10] = [5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 12.0, 14.0, 18.0, 24.0];

/// Colours of the Colors menu: name and colour (None = the foreground colour).
const MENU_COLORS: &[(&str, Option<Rgb>)] = &[
    ("Red", Some([230, 0, 0])),
    ("Yellow", Some([230, 200, 0])),
    ("Green", Some([0, 160, 0])),
    ("Cyan", Some([0, 170, 200])),
    ("Blue", Some([0, 0, 230])),
    ("Magenta", Some([200, 0, 200])),
];

#[derive(Clone, Copy)]
pub(super) enum TextStyle {
    Plain,
    Bold,
    Italic,
    Formula,
}

fn recent_file() -> Option<PathBuf> {
    Some(crate::recover::data_dir()?.join("recent.json"))
}

pub(super) fn load_recent() -> Vec<PathBuf> {
    recent_file().and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save_recent(list: &[PathBuf]) {
    if let Some(f) = recent_file() {
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(f, serde_json::to_string(list).unwrap_or_default());
    }
}

/// Choices remembered between runs (settings.json next to the recent files).
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub(super) struct Settings {
    /// Copy puts only the editable object on the clipboard (Windows).
    #[serde(default)]
    pub copy_object: bool,
}

fn settings_file() -> Option<PathBuf> {
    Some(crate::recover::data_dir()?.join("settings.json"))
}

pub(super) fn load_settings() -> Settings {
    settings_file().and_then(|f| std::fs::read_to_string(f).ok()).and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save_settings(s: &Settings) {
    if let Some(f) = settings_file() {
        if let Some(d) = f.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let _ = std::fs::write(f, serde_json::to_string_pretty(s).unwrap_or_default());
    }
}

/// A menu item with its key shown on the right.
fn item(ui: &mut egui::Ui, label: &str, keys: &str) -> bool {
    ui.add(egui::Button::new(label).shortcut_text(super::keys(keys))).clicked()
}

fn item_on(ui: &mut egui::Ui, enabled: bool, label: &str, keys: &str) -> bool {
    ui.add_enabled(enabled, egui::Button::new(label).shortcut_text(super::keys(keys))).clicked()
}

/// A tick-able menu item.
fn check(ui: &mut egui::Ui, on: bool, label: &str, keys: &str) -> bool {
    ui.add(egui::Button::selectable(on, label).shortcut_text(super::keys(keys))).clicked()
}

impl App {
    pub(super) fn remember_recent(&mut self, p: &std::path::Path) {
        let p = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
        self.recent.retain(|q| *q != p);
        self.recent.insert(0, p);
        self.recent.truncate(10);
        save_recent(&self.recent);
    }

    pub(super) fn invert_selection(&mut self) {
        let all = self.doc.select_all();
        let inv = |a: &std::collections::BTreeSet<usize>, b: &std::collections::BTreeSet<usize>| a.difference(b).copied().collect();
        self.sel = Selection { atoms: inv(&all.atoms, &self.sel.atoms), arrows: inv(&all.arrows, &self.sel.arrows), captions: inv(&all.captions, &self.sel.captions), shapes: inv(&all.shapes, &self.sel.shapes) };
        if !matches!(self.tool, Tool::Lasso | Tool::Marquee) {
            self.tool = Tool::Marquee;
        }
    }

    /// Insert a structure or drawing from a file into this document.
    pub(super) fn insert_file(&mut self) {
        let f = rfd::FileDialog::new()
            .add_filter("Chemical structures", &["cdx", "cdxml", "ctp", "mol", "sdf", "rxn", "cml", "smi", "json"])
            .pick_file();
        if let Some(p) = f {
            match read_any(&p) {
                Ok(d) => self.insert_doc(d, &format!("Inserted {}", file_name(&p))),
                Err(e) => self.status(format!("Could not read {}: {}", file_name(&p), e)),
            }
        }
    }

    /// Draw a frame (box, brackets…) around the selection.
    pub(super) fn add_frame(&mut self, kind: ShapeKind, ctx: &egui::Context) {
        let sel = self.selection_or_all();
        let Some(b) = arrange::visual_bounds(&self.doc, &sel, ctx).or_else(|| self.doc.bounds_of(Some(&sel))) else {
            self.status("Draw or select something to frame");
            return;
        };
        let m = BOND_LEN * 0.3;
        let r = b.expand(m);
        self.checkpoint();
        self.doc.shapes.push(Shape::new(r.min, r.max, kind));
        self.sel.shapes.insert(self.doc.shapes.len() - 1);
        self.status("Added a frame");
    }

    /// Bring the selected shapes, arrows and captions in front of the others (or send them behind).
    pub(super) fn restack(&mut self, front: bool) {
        fn reorder<T>(v: &mut Vec<T>, picked: &std::collections::BTreeSet<usize>, front: bool) -> std::collections::BTreeSet<usize> {
            let mut a = Vec::new();
            let mut b = Vec::new();
            for (i, x) in std::mem::take(v).into_iter().enumerate() {
                if picked.contains(&i) { a.push(x) } else { b.push(x) }
            }
            let n = a.len();
            let start = if front { b.len() } else { 0 };
            if front {
                b.extend(a);
                *v = b;
            } else {
                a.extend(b);
                *v = a;
            }
            (start..start + n).collect()
        }
        if self.sel.shapes.is_empty() && self.sel.arrows.is_empty() && self.sel.captions.is_empty() {
            self.status("Select shapes, arrows or text to change their stacking order");
            return;
        }
        self.checkpoint();
        self.sel.shapes = reorder(&mut self.doc.shapes, &self.sel.shapes, front);
        self.sel.arrows = reorder(&mut self.doc.arrows, &self.sel.arrows, front);
        self.sel.captions = reorder(&mut self.doc.captions, &self.sel.captions, front);
        self.status(if front { "Brought to front" } else { "Sent to back" });
    }

    /// Text ▸ Style: change the selected captions and the style of new ones.
    pub(super) fn set_text_style(&mut self, st: TextStyle) {
        match st {
            TextStyle::Plain => {
                self.caption_bold = false;
                self.caption_italic = false;
                self.caption_formula = false;
            }
            TextStyle::Bold => self.caption_bold = !self.caption_bold,
            TextStyle::Italic => self.caption_italic = !self.caption_italic,
            TextStyle::Formula => self.caption_formula = !self.caption_formula,
        }
        let s = self.sel.captions.clone();
        if s.is_empty() {
            return;
        }
        self.checkpoint();
        for i in s {
            let c = &mut self.doc.captions[i];
            match st {
                TextStyle::Plain => {
                    c.bold = false;
                    c.italic = false;
                    c.formula = false;
                }
                TextStyle::Bold => c.bold = self.caption_bold,
                TextStyle::Italic => c.italic = self.caption_italic,
                TextStyle::Formula => c.formula = self.caption_formula,
            }
        }
    }

    /// Text ▸ Size in points (as printed).
    fn set_text_size(&mut self, pt: f32) {
        let units = pt * BOND_LEN / self.doc.style.bond_length.max(1.0);
        self.caption_size = units;
        let s = self.sel.captions.clone();
        if !s.is_empty() {
            self.checkpoint();
            for i in s {
                self.doc.captions[i].size = units;
            }
        }
    }

    fn caption_pt(&self) -> f32 {
        self.caption_size * self.doc.style.bond_length.max(1.0) / BOND_LEN
    }

    /// Pick the arrow drawn next; selected arrows change to it too.
    pub(super) fn set_arrow_kind(&mut self, k: ArrowKind, style: ArrowStyle) {
        self.arrow_kind = k;
        self.arrow_style = style;
        let s = self.sel.arrows.clone();
        if s.is_empty() {
            self.tool = Tool::Arrow;
            return;
        }
        self.checkpoint();
        for i in s {
            let a = &mut self.doc.arrows[i];
            a.kind = k;
            a.style = style;
        }
        self.dirty = true;
    }

    /// Add a caption under the selected structure (or the whole drawing).
    pub(super) fn caption_below(&mut self, text: String, formula: bool, ctx: &egui::Context) {
        let sel = if self.sel.atoms.is_empty() { self.doc.select_all() } else { Selection { atoms: self.sel.atoms.clone(), ..Default::default() } };
        let b = arrange::visual_bounds(&self.doc, &sel, ctx).or_else(|| self.doc.bounds_of(Some(&sel)));
        let size = self.doc.style.caption_size.max(4.0);
        let pos = match b {
            Some(b) => pos2(b.min.x, b.max.y + size * 0.8),
            None => self.view_center(),
        };
        self.checkpoint();
        self.doc.captions.push(Caption { pos, text, size, bold: false, italic: false, serif: self.doc.style.caption_font == crate::doc::LabelFont::Serif, formula, color: None, extra: None, seq: None });
        self.sel = Selection { captions: [self.doc.captions.len() - 1].into_iter().collect(), ..Default::default() };
        self.dirty = true;
    }

    pub(super) fn structure_to_name(&mut self, ctx: &egui::Context) {
        let names: Vec<String> = self.analysis.lines.iter().filter_map(|f| f.name.clone()).collect();
        if names.is_empty() {
            self.status("No name found for this structure (names come from a built-in list and rules for simple compounds)");
            return;
        }
        self.caption_below(names.join("\n"), false, ctx);
        self.status("Added the name under the structure");
    }

    /// Print: the drawing goes to a PDF that opens in the system viewer, which prints it.
    pub(super) fn print(&mut self, ctx: &egui::Context) {
        if self.doc.is_empty() {
            self.status("Nothing to print");
            return;
        }
        let path = std::env::temp_dir().join(format!("{} print.pdf", APP_NAME));
        if let Err(e) = std::fs::write(&path, export::to_pdf(&self.doc, ctx, self.color_atoms)) {
            self.status(format!("Could not prepare printing: {}", e));
            return;
        }
        let r = if cfg!(target_os = "macos") {
            std::process::Command::new("open").arg(&path).spawn()
        } else if cfg!(windows) {
            std::process::Command::new("cmd").args(["/C", "start", ""]).arg(&path).spawn()
        } else {
            std::process::Command::new("xdg-open").arg(&path).spawn()
        };
        self.status(match r {
            Ok(_) => "Opened the drawing in the system viewer; print from there".to_string(),
            Err(e) => format!("Could not open the viewer: {}", e),
        });
    }

    fn panel_on(&self, t: Tab) -> bool {
        self.show_panel && self.tab == t
    }

    fn toggle_panel(&mut self, t: Tab) {
        if self.panel_on(t) {
            self.show_panel = false;
        } else {
            self.show_panel = true;
            self.tab = t;
        }
    }

    fn zoom_by(&mut self, k: f32) {
        let c = self.canvas.center();
        self.set_zoom(self.zoom * k, c);
    }

    pub(super) fn menus(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.menu_button("File", |ui| {
            if item(ui, "New Document", "⌘N") {
                self.new_doc();
                ui.close();
            }
            ui.menu_button("New from Stationery", |ui| {
                for p in styles::PRESETS {
                    if ui.button(p.name).clicked() {
                        ui.close();
                        let w = self.doc.style.show_warnings;
                        self.new_doc();
                        styles::apply(&mut self.doc, styles::to_style(p, w));
                        self.status(format!("New document with the “{}” settings", p.name));
                    }
                }
            });
            ui.menu_button("Open Templates", |ui| {
                for (k, c) in crate::templates::all().iter().enumerate() {
                    if ui.button(c.name).clicked() {
                        self.template_window = Some(k);
                        self.tool = Tool::Template;
                        ui.close();
                    }
                }
            });
            if item(ui, "Open…", "⌘O") {
                ui.close();
                self.open_dialog(ctx);
            }
            ui.menu_button("Open Recent", |ui| {
                if self.recent.is_empty() {
                    ui.add_enabled(false, egui::Button::new("No recent documents"));
                }
                for p in self.recent.clone() {
                    if ui.button(file_name(&p)).on_hover_text(p.display().to_string()).clicked() {
                        ui.close();
                        self.open_path(p, ctx);
                    }
                }
                ui.separator();
                if ui.add_enabled(!self.recent.is_empty(), egui::Button::new("Clear Menu")).clicked() {
                    self.recent.clear();
                    save_recent(&self.recent);
                    ui.close();
                }
            });
            ui.separator();
            if item(ui, "Close", "⌘W") {
                ui.close();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if item(ui, "Save", "⌘S") {
                ui.close();
                self.save(ctx);
            }
            if item(ui, "Save As…", "⇧⌘S") {
                ui.close();
                self.save_as(ctx);
            }
            ui.menu_button("Export", |ui| {
                let items: [(&str, &str, bool); 18] = [
                    ("CDXML document (*.cdxml)", "cdxml", false),
                    ("CDX stationery (*.ctp)", "ctp", false),
                    ("MDL Molfile V2000 (*.mol)", "mol", false),
                    ("MDL Molfile V3000 (*.mol)", "mol", true),
                    ("SDfile (*.sdf)", "sdf", false),
                    ("Reaction Molfile V2000 (*.rxn)", "rxn", false),
                    ("Reaction Molfile V3000 (*.rxn)", "rxn", true),
                    ("Chemical Markup Language (*.cml)", "cml", false),
                    ("SMILES (*.smi)", "smi", false),
                    ("FASTA sequences (*.fasta)", "fasta", false),
                    ("PDF (*.pdf)", "pdf", false),
                    ("Encapsulated PostScript (*.eps)", "eps", false),
                    ("SVG (*.svg)", "svg", false),
                    ("PNG (*.png)", "png", false),
                    ("JPEG (*.jpg)", "jpg", false),
                    ("GIF (*.gif)", "gif", false),
                    ("TIFF (*.tif)", "tif", false),
                    ("BMP (*.bmp)", "bmp", false),
                ];
                for (label, ext, v3) in items {
                    if ui.button(label).clicked() {
                        ui.close();
                        self.export_variant(ctx, label, ext, v3);
                    }
                }
            });
            ui.separator();
            if ui.button("Document Settings…").clicked() {
                ui.close();
                self.show_panel = true;
                self.tab = Tab::Style;
            }
            ui.menu_button("Apply Document Settings from", |ui| {
                self.style_menu(ui);
            });
            ui.separator();
            if ui.button("List Nicknames…").clicked() {
                self.show_nicknames = true;
                ui.close();
            }
            ui.separator();
            if item(ui, "Print…", "⌘P") {
                ui.close();
                self.print(ctx);
            }
            ui.separator();
            if item(ui, "Quit", "⌘Q") {
                ui.close();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
        ui.menu_button("Edit", |ui| {
            if item_on(ui, !self.undo.is_empty(), "Undo", "⌘Z") {
                self.do_undo();
                ui.close();
            }
            if item_on(ui, !self.redo.is_empty(), "Redo", "⇧⌘Z") {
                self.do_redo();
                ui.close();
            }
            ui.separator();
            let has = !self.sel.is_empty();
            if item_on(ui, has, "Cut", "⌘X") {
                self.copy(ctx, true);
                ui.close();
            }
            if item_on(ui, has, "Copy", "⌘C") {
                self.copy(ctx, false);
                ui.close();
            }
            if cfg!(windows) {
                // ⇧⌘C copies the other way from ⌘C.
                let (label, tip) = if self.copy_object {
                    ("Copy for Word and Other Programs", "The object with its pictures: Word, PowerPoint and Excel paste an object you can double-click to edit here; programs that can't hold objects (WordCraft, DeckCraft, image editors) paste the picture. LibreOffice pastes a picture.")
                } else {
                    ("Copy for LibreOffice", "Only the object, no pictures: LibreOffice (Writer, Calc, Impress, Draw) pastes an object you can double-click to edit here. Word can't paste this copy; use the normal Copy for Word.")
                };
                if ui.add_enabled(has, egui::Button::new(label).shortcut_text(super::keys("⇧⌘C"))).on_hover_text(tip).clicked() {
                    self.copy_as(ctx, false, !self.copy_object);
                    ui.close();
                }
                if ui
                    .add(egui::Button::selectable(self.copy_object, "Copy for LibreOffice by Default"))
                    .on_hover_text("On: Copy (Ctrl+C) makes the copy for LibreOffice (an object you can double-click to edit here), and Ctrl+Shift+C the copy for Word and other programs. Off: the other way round. Word can't paste the LibreOffice copy. Remembered.")
                    .clicked()
                {
                    self.copy_object = !self.copy_object;
                    save_settings(&Settings { copy_object: self.copy_object });
                    self.status(if self.copy_object {
                        "Copy now makes the copy for LibreOffice (Ctrl+Shift+C makes the one for Word and other programs)"
                    } else {
                        "Copy now makes the copy for Word and other programs (Ctrl+Shift+C makes the one for LibreOffice)"
                    });
                    ui.close();
                }
            }
            ui.menu_button("Copy As", |ui| {
                let part = if self.sel.is_empty() { self.doc.clone() } else { self.doc.extract(&self.sel) };
                if ui.button("SMILES").clicked() {
                    ctx.copy_text(formats::write_smiles(&part));
                    self.status("Copied SMILES");
                    ui.close();
                }
                if ui.button("Molfile").clicked() {
                    ctx.copy_text(formats::write_molfile_v2000(&part, ""));
                    self.status("Copied molfile text");
                    ui.close();
                }
                if ui.button("CDXML Text").clicked() {
                    let mut p = part.clone();
                    p.file = None;
                    ctx.copy_text(cdx::write_cdxml(&p));
                    self.status("Copied CDXML text");
                    ui.close();
                }
                if ui.button("Name").clicked() {
                    let n: Vec<String> = self.analysis.lines.iter().filter_map(|f| f.name.clone()).collect();
                    if n.is_empty() {
                        self.status("No name found for this structure");
                    } else {
                        ctx.copy_text(n.join("\n"));
                        self.status("Copied the name");
                    }
                    ui.close();
                }
            });
            if item_on(ui, clip::NATIVE || self.clipboard.is_some(), "Paste", "⌘V") {
                self.paste();
                ui.close();
            }
            ui.menu_button("Paste Special", |ui| {
                let text = || if clip::NATIVE { clip::read_text() } else { None };
                if ui.button("Structure from Text (SMILES, Molfile, CDXML or Name)").clicked() {
                    ui.close();
                    match text() {
                        Some(t) => self.paste_text(&t),
                        None => self.status("There is no text on the clipboard"),
                    }
                }
                if ui.button("Text as Caption").clicked() {
                    ui.close();
                    match text() {
                        Some(t) if !t.trim().is_empty() => {
                            let size = self.caption_size;
                            self.checkpoint();
                            self.doc.captions.push(Caption { pos: self.view_center(), text: t.trim().to_string(), size, bold: false, italic: false, serif: self.doc.style.caption_font == crate::doc::LabelFont::Serif, formula: false, color: None, extra: None, seq: None });
                            self.sel = Selection { captions: [self.doc.captions.len() - 1].into_iter().collect(), ..Default::default() };
                        }
                        _ => self.status("There is no text on the clipboard"),
                    }
                }
            });
            if item_on(ui, has, "Clear", "⌫") {
                self.checkpoint();
                let s = self.sel.clone();
                self.doc.delete_selection(&s);
                self.sel.clear();
                ui.close();
            }
            ui.separator();
            if item(ui, "Select All", "⌘A") {
                self.sel = self.doc.select_all();
                ui.close();
            }
            if item(ui, "Invert Selection", "⇧⌘I") {
                self.invert_selection();
                ui.close();
            }
            ui.separator();
            if ui.button("Insert File…").clicked() {
                ui.close();
                self.insert_file();
            }
        });
        ui.menu_button("View", |ui| {
            if check(ui, self.show_crosshair, "Show Crosshair", "⌥⌘X") {
                self.show_crosshair = !self.show_crosshair;
                ui.close();
            }
            if check(ui, self.show_rulers, "Show Rulers", "⌘;") {
                self.show_rulers = !self.show_rulers;
                ui.close();
            }
            if check(ui, self.show_page, "Show Page", "") {
                self.show_page = !self.show_page;
                ui.close();
            }
            if check(ui, self.show_grid, "Show Grid", "") {
                self.show_grid = !self.show_grid;
                ui.close();
            }
            ui.separator();
            if item(ui, "Actual Size", "") {
                let c = self.canvas.center();
                self.set_zoom(1.0, c);
                ui.close();
            }
            if item(ui, "Show Document", "⌘/") {
                if self.show_page {
                    self.fit_pages();
                } else {
                    self.fit_view();
                }
                ui.close();
            }
            if item(ui, "Magnify", "⇧⌘>") {
                self.zoom_by(2.0);
                ui.close();
            }
            if item(ui, "Reduce", "⇧⌘<") {
                self.zoom_by(0.5);
                ui.close();
            }
            if item(ui, "Zoom In", "⌘+") {
                self.zoom_by(1.25);
                ui.close();
            }
            if item(ui, "Zoom Out", "⌘−") {
                self.zoom_by(0.8);
                ui.close();
            }
            ui.separator();
            for (t, name) in [
                (Tab::Analysis, "Show Analysis Window"),
                (Tab::Properties, "Show Chemical Properties Window"),
                (Tab::Info, "Show Info Window"),
                (Tab::Elements, "Show Periodic Table Window"),
                (Tab::Names, "Show Name ↔ Structure Window"),
                (Tab::Sequence, "Show Sequence Editor"),
                (Tab::Nmr, "Show NMR Window"),
                (Tab::Style, "Show Document Settings"),
            ] {
                if check(ui, self.panel_on(t), name, "") {
                    self.toggle_panel(t);
                    ui.close();
                }
            }
            ui.separator();
            ui.checkbox(&mut self.color_atoms, "Colour Atoms by Element");
            ui.checkbox(&mut self.doc.style.show_carbons, "Show Carbon Labels");
            ui.checkbox(&mut self.doc.style.show_warnings, "Show Chemical Warnings");
        });
        ui.menu_button("Object", |ui| {
            if check(ui, self.fixed_lengths, "Fixed Lengths", "⌘L") {
                self.fixed_lengths = !self.fixed_lengths;
                ui.close();
            }
            if check(ui, self.fixed_angles, "Fixed Angles", "⌘E") {
                self.fixed_angles = !self.fixed_angles;
                ui.close();
            }
            ui.separator();
            if item(ui, "Scale…", "⌘K") {
                self.open_dialog_box(Dialog::Scale);
                ui.close();
            }
            if ui.button("Center on Page").clicked() {
                let sel = self.selection_or_all();
                if let Some(b) = self.doc.bounds_of(Some(&sel)) {
                    self.checkpoint();
                    let d = self.view_center() - b.center();
                    self.doc.translate(&sel, d);
                }
                ui.close();
            }
            ui.menu_button("Add Frame", |ui| {
                for (k, name) in [(ShapeKind::Brackets, "Brackets"), (ShapeKind::Parens, "Parentheses"), (ShapeKind::Rect, "Rectangle"), (ShapeKind::RoundRect, "Rounded Rectangle"), (ShapeKind::Ellipse, "Oval")] {
                    if ui.button(name).clicked() {
                        self.add_frame(k, ctx);
                        ui.close();
                    }
                }
            });
            ui.separator();
            if item(ui, "Bring to Front", "⌘[") {
                self.restack(true);
                ui.close();
            }
            if item(ui, "Send to Back", "⌘]") {
                self.restack(false);
                ui.close();
            }
            ui.separator();
            ui.menu_button("Align", |ui| {
                for (e, keys) in [(Edge::Left, "⌥⇧⌘L"), (Edge::CentreX, "⌥⇧⌘C"), (Edge::Right, "⌥⇧⌘R")] {
                    if item(ui, e.label(), keys) {
                        self.align(e, ctx);
                        ui.close();
                    }
                }
                ui.separator();
                for (e, keys) in [(Edge::Top, "⌥⇧⌘T"), (Edge::CentreY, "⌥⇧⌘M"), (Edge::Bottom, "⌥⇧⌘B")] {
                    if item(ui, e.label(), keys) {
                        self.align(e, ctx);
                        ui.close();
                    }
                }
            });
            ui.menu_button("Distribute", |ui| {
                for (axis, horizontal, keys) in [("Horizontally", true, "⌥⇧⌘H"), ("Vertically", false, "⌥⇧⌘V")] {
                    ui.menu_button(axis, |ui| {
                        if item(ui, "Evenly", keys) {
                            self.distribute(horizontal, None, ctx);
                            ui.close();
                        }
                        if ui.button("Gap…").on_hover_text("A fixed distance in millimetres between objects").clicked() {
                            self.open_dialog_box(Dialog::Gap { horizontal });
                            ui.close();
                        }
                    });
                }
            });
            ui.separator();
            if ui.button("Flip Horizontal").clicked() {
                self.flip(true);
                ui.close();
            }
            if ui.button("Flip Vertical").clicked() {
                self.flip(false);
                ui.close();
            }
            if ui.add(egui::Button::new("Rotate 180° Horizontal").shortcut_text(super::keys("⇧⌘H"))).on_hover_text("Turn over left-to-right, keeping the stereochemistry").clicked() {
                self.rotate_180(true);
                ui.close();
            }
            if ui.add(egui::Button::new("Rotate 180° Vertical").shortcut_text(super::keys("⇧⌘V"))).on_hover_text("Turn over top-to-bottom, keeping the stereochemistry").clicked() {
                self.rotate_180(false);
                ui.close();
            }
            if item(ui, "Rotate…", "⌘R") {
                self.open_dialog_box(Dialog::Rotate);
                ui.close();
            }
            ui.menu_button("Rotate by", |ui| {
                for (label, deg) in [("90° Clockwise", 90.0f32), ("90° Counter-clockwise", -90.0), ("15° Clockwise", 15.0), ("15° Counter-clockwise", -15.0), ("180°", 180.0)] {
                    if ui.button(label).clicked() {
                        self.rotate_by(deg);
                        ui.close();
                    }
                }
            });
        });
        ui.menu_button("Structure", |ui| {
            if item(ui, "Clean Up Structure", "⇧⌘K") {
                self.clean_up();
                ui.close();
            }
            ui.menu_button("Isotope", |ui| self.isotope_menu(ui, None));
            if ui.button("Expand Label").clicked() {
                self.expand_labels();
                ui.close();
            }
            ui.separator();
            if item(ui, "Convert Name to Structure", "⇧⌘N") {
                self.tab = Tab::Names;
                self.show_panel = true;
                ui.close();
            }
            if item(ui, "Convert Structure to Name", "⌥⌘N") {
                self.structure_to_name(ctx);
                ui.close();
            }
            if ui.button("Insert SMILES…").clicked() {
                self.tab = Tab::Names;
                self.show_panel = true;
                ui.close();
            }
            ui.separator();
            if ui.button("Check Structure").clicked() {
                let n = self.analysis.warnings;
                self.status(if n == 0 { "No problems found".to_string() } else { format!("{} atom(s) have too many bonds (marked with red boxes)", n) });
                ui.close();
            }
            if ui.button("Analysis").clicked() {
                self.tab = Tab::Analysis;
                self.show_panel = true;
                ui.close();
            }
            ui.separator();
            if ui.button("Predict ¹H-NMR Shifts").clicked() {
                self.nmr_nucleus = 1;
                self.tab = Tab::Nmr;
                self.show_panel = true;
                ui.close();
            }
            if ui.button("Predict ¹³C-NMR Shifts").clicked() {
                self.nmr_nucleus = 13;
                self.tab = Tab::Nmr;
                self.show_panel = true;
                ui.close();
            }
            // ¹⁵N only when a nitrogen is marked as ¹⁵N.
            let has_15n = self.doc.atoms.iter().any(|a| a.isotope == 15 && a.el == 7);
            if ui.add_enabled(has_15n, egui::Button::new("Predict ¹⁵N-NMR Shifts")).on_disabled_hover_text("Mark a nitrogen as ¹⁵N first (right-click ▸ Isotope, or type 15N)").clicked() {
                self.nmr_nucleus = 15;
                self.tab = Tab::Nmr;
                self.show_panel = true;
                ui.close();
            }
            for (n, label) in [(19u8, "Predict ¹⁹F-NMR Shifts"), (31, "Predict ³¹P-NMR Shifts")] {
                if ui.button(label).clicked() {
                    self.nmr_nucleus = n;
                    self.tab = Tab::Nmr;
                    self.show_panel = true;
                    ui.close();
                }
            }
            if ui.button("Chemical Properties").clicked() {
                self.tab = Tab::Properties;
                self.show_panel = true;
                ui.close();
            }
        });
        ui.menu_button("Text", |ui| {
            ui.menu_button("Style", |ui| {
                let plain = !self.caption_bold && !self.caption_italic && !self.caption_formula;
                for (st, on, name, keys) in [
                    (TextStyle::Plain, plain, "Plain", "⌘T"),
                    (TextStyle::Bold, self.caption_bold, "Bold", "⌘B"),
                    (TextStyle::Italic, self.caption_italic, "Italic", "⌘I"),
                    (TextStyle::Formula, self.caption_formula, "Formula", "⌘F"),
                ] {
                    if check(ui, on, name, keys) {
                        self.set_text_style(st);
                        ui.close();
                    }
                }
            });
            ui.menu_button("Size", |ui| {
                let cur = self.caption_pt();
                for pt in TEXT_SIZES {
                    if check(ui, (cur - pt).abs() < 0.05, &format!("{} pt", pt), "") {
                        self.set_text_size(pt);
                        ui.close();
                    }
                }
            });
            ui.separator();
            ui.label(egui::RichText::new("Applies to the selected captions and to new ones.").size(11.0).color(Color32::GRAY));
        });
        ui.menu_button("Curves", |ui| {
            for (k, name) in [
                (ArrowKind::Reaction, "Reaction Arrow"),
                (ArrowKind::Equilibrium, "Equilibrium Arrow"),
                (ArrowKind::Retro, "Retrosynthetic Arrow"),
                (ArrowKind::Resonance, "Resonance Arrow"),
                (ArrowKind::Curved, "Curved Arrow"),
                (ArrowKind::Dashed, "Dashed Arrow"),
                (ArrowKind::NoGo, "No-Go Arrow"),
                (ArrowKind::Line, "Plain Line"),
            ] {
                if check(ui, self.arrow_kind == k && self.arrow_style.is_default(), name, "") {
                    self.set_arrow_kind(k, ArrowStyle::default());
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("All Arrow Styles…").clicked() {
                self.arrow_menu = true;
                self.arrow_menu_fresh = true;
                self.arrow_menu_at = self.canvas.min + vec2(8.0, 8.0);
                ui.close();
            }
            ui.separator();
            ui.label(egui::RichText::new("Changes the selected arrows, or picks the arrow drawn next.").size(11.0).color(Color32::GRAY));
        });
        ui.menu_button("Colors", |ui| {
            if ui.button("Foreground (Black)").clicked() {
                self.apply_color(None);
                ui.close();
            }
            ui.separator();
            for (k, (name, c)) in MENU_COLORS.iter().enumerate() {
                let (rect, resp) = ui.allocate_exact_size(vec2(200.0, 20.0), Sense::click());
                let col = c.map(|c| Color32::from_rgb(c[0], c[1], c[2])).unwrap_or(Color32::BLACK);
                if resp.hovered() {
                    ui.painter().rect_filled(rect, 3.0, ui.visuals().widgets.hovered.weak_bg_fill);
                }
                ui.painter().rect_filled(Rect::from_min_size(rect.min + vec2(4.0, 4.0), vec2(22.0, 12.0)), 2.0, col);
                ui.painter().text(rect.min + vec2(34.0, 10.0), egui::Align2::LEFT_CENTER, format!("Other #{} — {}", k + 1, name), egui::FontId::proportional(13.0), ui.visuals().text_color());
                if resp.clicked() {
                    self.apply_color(*c);
                    ui.close();
                }
            }
            ui.separator();
            if ui.button("Other…").clicked() {
                self.open_dialog_box(Dialog::Color);
                ui.close();
            }
            if check(ui, self.color_atoms, "By Element", "") {
                self.color_atoms = !self.color_atoms;
                ui.close();
            }
        });
        ui.menu_button("Help", |ui| {
            if ui.button("Hotkeys Cheat Sheet").clicked() {
                self.tab = Tab::Help;
                self.show_panel = true;
                ui.close();
            }
            if ui.button("Shortcuts Cheat Sheet").clicked() {
                self.show_shortcuts = true;
                ui.close();
            }
            ui.separator();
            if ui.button(format!("About {}", APP_NAME)).clicked() {
                self.show_about = true;
                ui.close();
            }
        });
    }

    /// Small windows opened from the menus: shortcuts, nicknames, property report.
    pub(super) fn extra_windows(&mut self, ctx: &egui::Context) {
        if self.show_shortcuts {
            let mut open = true;
            egui::Window::new("Shortcuts Cheat Sheet").open(&mut open).default_width(420.0).show(ctx, |ui| {
                egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                    egui::Grid::new("sc").striped(true).num_columns(2).show(ui, |ui| {
                        for (a, k) in SHORTCUTS {
                            ui.label(super::keys(a));
                            ui.label(egui::RichText::new(super::keys(k)).monospace());
                            ui.end_row();
                        }
                    });
                });
            });
            self.show_shortcuts = open;
        }
        if self.show_nicknames {
            let mut open = true;
            let mut insert = None;
            egui::Window::new("Nicknames").open(&mut open).default_width(340.0).show(ctx, |ui| {
                ui.label("Type a nickname as an atom label (or press its key over an atom). Click one to add it to the drawing.");
                egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                    egui::Grid::new("nn").striped(true).num_columns(2).show(ui, |ui| {
                        ui.label(egui::RichText::new("Nickname").strong());
                        ui.label(egui::RichText::new("Formula").strong());
                        ui.end_row();
                        for (n, f, smi) in chem::ABBREVIATIONS {
                            if ui.link(*n).clicked() {
                                insert = Some((*n, *smi));
                            }
                            ui.label(*f);
                            ui.end_row();
                        }
                    });
                });
            });
            self.show_nicknames = open;
            if let Some((n, smi)) = insert {
                if let Ok(d) = formats::parse_smiles(smi) {
                    self.insert_doc(d, &format!("Inserted {}", n));
                }
            }
        }
        if self.show_report {
            let text = self.properties_report();
            let mut open = true;
            egui::Window::new("Chemical Properties Report").open(&mut open).default_width(420.0).show(ctx, |ui| {
                let mut t = text.clone();
                ui.add(egui::TextEdit::multiline(&mut t).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY).desired_rows(16));
                if ui.button("Copy").clicked() {
                    ctx.copy_text(text.clone());
                    self.status("Copied the report");
                }
            });
            self.show_report = open;
        }
    }
}
