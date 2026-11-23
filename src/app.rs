//! The main window: menus, tool palette, drawing sheet and the docked side panel.

use crate::chem;
use crate::doc::*;
use crate::elements;
use crate::icons::{self, Tool};
use crate::render::{self, Canvas, EguiCanvas, RenderOpts};
use crate::arrange::{self, Edge};
use crate::{cdx, clip, emf, export, formats, names, styles};
use egui::{Color32, Key, Modifiers, Pos2, Rect, Sense, Stroke, Vec2, pos2, vec2};
use std::collections::{HashSet, hash_map::DefaultHasher};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

mod arrowpick;
mod isotope;
mod menus;
mod nmrui;
mod panels;
mod seq;
mod tpl;

pub const APP_NAME: &str = "Formula";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_CREDIT: &str = "Developed by Arkadiy Bazhin with the help of CraftAnything and Claude Code.";
pub const APP_LICENSE: &str = "Free software under the MIT License.";
pub const APP_DISCLAIMER: &str = "Independent project, not affiliated with or endorsed by any chemistry software company or publisher. File-format and journal names are used only to describe compatibility.";

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Analysis,
    Properties,
    Info,
    Elements,
    Names,
    Sequence,
    Nmr,
    Style,
    Help,
}

enum Drag {
    Bond { from_atom: Option<usize>, start: Pos2, cur: Pos2 },
    Chain { from_atom: Option<usize>, start: Pos2, cur: Pos2 },
    Move { last: Pos2 },
    Marquee { start: Pos2, cur: Pos2 },
    Lasso { pts: Vec<Pos2> },
    Arrow { start: Pos2, cur: Pos2 },
    Shape { start: Pos2, cur: Pos2 },
    Erase,
    Pan,
    /// Turning the selection with the round handle (angles in radians).
    Rotate { pivot: Pos2, start_ang: f32, orig: Box<Doc>, angle: f32 },
}

/// Small windows that ask for a number.
#[derive(Clone, Copy, PartialEq)]
enum Dialog {
    Rotate,
    Gap { horizontal: bool },
    Scale,
    Color,
}

enum Editing {
    Label { atom: usize, text: String, fresh: bool },
    Caption { idx: usize, text: String, fresh: bool },
}

#[derive(Clone, Copy, PartialEq)]
enum CloseAsk {
    None,
    Asking,
}

struct Pending {
    path: PathBuf,
    saved_view: (f32, Vec2),
    stage: u8,
}

#[derive(Default, Clone)]
struct Analysis {
    key: u64,
    lines: Vec<FragmentInfo>,
    total_formula: String,
    warnings: usize,
    smiles: String,
    /// Estimated chemical properties, when exactly one structure is analysed.
    chem: Option<crate::props::ChemProps>,
}

#[derive(Default, Clone)]
struct FragmentInfo {
    name: Option<String>,
    formula: chem::Formula,
    charge: i32,
    exact: f64,
    mw: f64,
    /// Isotope peaks of the measured ion (see `ion`).
    mz: Vec<(f64, f64)>,
    /// The ion a mass spectrometer sees ([M+H]⁺ for a neutral molecule), its monoisotopic m/z
    /// and its predicted spectrum.
    ion: String,
    ion_formula: String,
    ion_mz: f64,
    spectrum: Vec<(f64, f64)>,
    ea: Vec<(u8, f64)>,
    props: chem::Properties,
}

pub struct App {
    doc: Doc,
    undo: Vec<Doc>,
    redo: Vec<Doc>,
    sel: Selection,
    tool: Tool,
    arrow_kind: ArrowKind,
    arrow_style: ArrowStyle,
    arrow_menu: bool,
    /// Set on the frame the grid opens, so the click that opened it doesn't close it again.
    arrow_menu_fresh: bool,
    arrow_menu_at: Pos2,
    shape_kind: ShapeKind,
    element: u8,
    zoom: f32,
    pan: Vec2,
    drag: Option<Drag>,
    hover: Option<Hit>,
    hover_pos: Option<Pos2>,
    editing: Option<Editing>,
    edit_focus: bool,
    path: Option<PathBuf>,
    dirty: bool,
    status: String,
    tab: Tab,
    show_panel: bool,
    name_input: String,
    smiles_input: String,
    color_atoms: bool,
    clipboard: Option<(Doc, String)>,
    canvas: Rect,
    analysis: Analysis,
    shot_path: Option<PathBuf>,
    frame: u64,
    shot: Arc<Mutex<Option<Arc<egui::ColorImage>>>>,
    shot_requested: bool,
    export_pending: Option<Pending>,
    close_ask: CloseAsk,
    show_about: bool,
    allow_close: bool,
    title: String,
    caption_size: f32,
    caption_bold: bool,
    caption_italic: bool,
    caption_formula: bool,
    view_initialized: bool,
    init_frames: u32,
    /// The atom hot keys act on when the pointer isn't over the structure (shown as a green dot).
    hotspot: Option<usize>,
    /// Index typed next to new brackets (number of repeats, or a label like "n").
    bracket_label: String,
    show_rulers: bool,
    show_grid: bool,
    /// The page and its drawing area on the canvas.
    show_page: bool,
    dialog: Option<Dialog>,
    /// Last angle typed in the Rotate dialog (degrees) and its direction.
    rotate_deg: f32,
    rotate_ccw: bool,
    /// Last gap typed for Distribute (millimetres).
    gap_mm: f32,
    dialog_focus: bool,
    /// Analysis panel: which lines Paste adds (formula, exact mass, mol. wt., m/z, elem. anal.).
    an_show: [bool; 5],
    an_decimals: usize,
    /// NMR panel: nucleus (1 or 13), spectrometer frequency, solvent, shifts on the structure.
    nmr_nucleus: u8,
    nmr_mhz: u32,
    nmr_solvent: usize,
    nmr_on_structure: bool,
    /// Enrichment of isotope-labelled atoms, %.
    nmr_enrich: f64,
    /// Atoms of the NMR peak under the pointer (marked on the structure).
    nmr_hover: Vec<usize>,
    /// Atom the right-click menu was opened on, and the mass number typed in it.
    ctx_atom: Option<usize>,
    /// Sequence panel: draw peptides head-to-tail cyclic.
    seq_cyclic: bool,
    iso_other: u16,
    nmr_cache: Option<(u64, std::rc::Rc<Vec<nmrui::NmrResult>>)>,
    /// Chemical Properties panel: which rows Paste adds.
    prop_show: Vec<bool>,
    show_report: bool,
    show_crosshair: bool,
    fixed_lengths: bool,
    fixed_angles: bool,
    scale_pct: f32,
    custom_color: [u8; 3],
    recent: Vec<PathBuf>,
    /// Windows: Copy (⌘C) puts only the editable object on the clipboard, so LibreOffice
    /// pastes an object rather than a picture. ⇧⌘C then copies the other way. Remembered.
    copy_object: bool,
    show_nicknames: bool,
    show_shortcuts: bool,
    /// Where the current drag began (drawing units), for the Info panel.
    drag_origin: Option<Pos2>,
    /// Counts edits, so the recovery copy is only rewritten after a change.
    edit_gen: u64,
    saved_gen: u64,
    last_autosave: f64,
    recovery_written: bool,
    /// A drawing rescued from a run that stopped unexpectedly: the copy, its time, a crash log.
    recovery_offer: Option<(PathBuf, std::time::SystemTime, Option<PathBuf>)>,
    /// The template the stamp tool places (group, item), the open group window and the
    /// drop-down list of groups.
    template: Option<(usize, usize)>,
    template_window: Option<usize>,
    template_menu: bool,
    template_menu_at: Pos2,
    seq: seq::SeqEditor,
    sugar: tpl::SugarPick,
    /// Editing an object in a document (Windows; started by double-clicking it in Word).
    embedded: bool,
    /// The document holding the object, for the title.
    host: Option<String>,
    /// `edit_gen` when the object last got the structure.
    ole_gen: u64,
    /// The container closed the object, so the window closes without storing again.
    ole_quit: bool,
    /// Closing an object's window: the store count before asking, when it was asked, and
    /// when the store finished.
    close_store: Option<(u64, f64, Option<f64>)>,
    /// The last thing the eraser removed during the current stroke.
    erase_last: Option<Hit>,
    /// Automatic edit-cycle test: when it started, and whether the edit was made.
    auto_edit: Option<f64>,
    auto_edit_done: bool,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, shot_path: Option<PathBuf>, open: Option<PathBuf>) -> Self {
        setup_fonts(&cc.egui_ctx);
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        let mut style = (*cc.egui_ctx.global_style()).clone();
        style.spacing.item_spacing = vec2(6.0, 4.0);
        style.spacing.button_padding = vec2(6.0, 2.0);
        cc.egui_ctx.set_global_style(style);
        let mut app = App {
            doc: new_drawing(),
            undo: vec![],
            redo: vec![],
            sel: Selection::default(),
            tool: Tool::Bond(1, Stereo::None),
            arrow_kind: ArrowKind::Reaction,
            arrow_style: ArrowStyle::default(),
            arrow_menu: false,
            arrow_menu_fresh: false,
            arrow_menu_at: Pos2::ZERO,
            shape_kind: ShapeKind::Rect,
            element: elements::N,
            zoom: 1.5,
            pan: vec2(0.0, 0.0),
            drag: None,
            hover: None,
            hover_pos: None,
            editing: None,
            edit_focus: false,
            path: None,
            dirty: false,
            status: "Ready".into(),
            tab: Tab::Analysis,
            show_panel: true,
            name_input: String::new(),
            smiles_input: String::new(),
            color_atoms: false,
            clipboard: None,
            canvas: Rect::NOTHING,
            analysis: Analysis { key: u64::MAX, ..Default::default() },
            shot_path,
            frame: 0,
            shot: Arc::new(Mutex::new(None)),
            shot_requested: false,
            export_pending: None,
            close_ask: CloseAsk::None,
            show_about: false,
            allow_close: false,
            title: String::new(),
            caption_size: 12.0,
            caption_bold: false,
            caption_italic: false,
            caption_formula: false,
            view_initialized: false,
            hotspot: None,
            bracket_label: "n".into(),
            init_frames: 0,
            show_rulers: false,
            show_grid: false,
            show_page: true,
            dialog: None,
            rotate_deg: 90.0,
            rotate_ccw: false,
            gap_mm: 5.0,
            dialog_focus: false,
            an_show: [true, true, false, false, false],
            an_decimals: 4,
            nmr_nucleus: 1,
            nmr_mhz: 400,
            nmr_solvent: 0,
            nmr_on_structure: true,
            nmr_enrich: 99.0,
            nmr_hover: Vec::new(),
            ctx_atom: None,
            seq_cyclic: false,
            iso_other: 13,
            nmr_cache: None,
            prop_show: vec![true; panels::PROP_ROWS],
            show_report: false,
            show_crosshair: false,
            fixed_lengths: true,
            fixed_angles: true,
            scale_pct: 100.0,
            custom_color: [0, 128, 128],
            recent: menus::load_recent(),
            copy_object: cfg!(windows) && menus::load_settings().copy_object,
            show_nicknames: false,
            show_shortcuts: false,
            drag_origin: None,
            edit_gen: 0,
            saved_gen: 0,
            last_autosave: 0.0,
            recovery_written: false,
            recovery_offer: None,
            template: None,
            template_window: None,
            template_menu: false,
            template_menu_at: Pos2::ZERO,
            seq: Default::default(),
            sugar: Default::default(),
            embedded: crate::ole::embedding(),
            host: None,
            ole_gen: 0,
            ole_quit: false,
            auto_edit: None,
            auto_edit_done: false,
            close_store: None,
            erase_last: None,
        };
        crate::ole::set_context(&cc.egui_ctx);
        if app.embedded {
            app.status("Waiting for the document to send the structure…");
        } else if std::env::args().skip(1).any(|a| crate::ole::is_embedding_arg(&a)) {
            let log = crate::ole::log_path().map(|p| p.display().to_string()).unwrap_or_default();
            app.status(format!("Started by a document, but could not connect to it; details are in {}", log));
        }
        // `--templates "Amino Acids"` starts with that template group open.
        if let Some(g) = std::env::args().skip_while(|a| a != "--templates").nth(1) {
            app.template_window = crate::templates::all().iter().position(|c| c.name.eq_ignore_ascii_case(&g)).or(Some(0));
            app.tool = Tool::Template;
            app.template = app.template_window.map(|k| (k, 0));
        }
        // `--arrows` starts with the arrow tool and its grid of styles open.
        if std::env::args().any(|a| a == "--arrows") {
            app.tool = Tool::Arrow;
            app.arrow_menu = true;
            app.arrow_menu_at = pos2(90.0, 120.0);
        }
        if shot_path_is_none(&app) && !app.embedded {
            if let Some((f, t)) = crate::recover::pending() {
                let log = crate::recover::crash_log_since(t - std::time::Duration::from_secs(600));
                app.recovery_offer = Some((f, t, log));
            }
        }
        if let Some(p) = open {
            app.open_path(p, &cc.egui_ctx);
        }
        // Optional drawing settings given at start-up (`--style "ACS Document 1996"`).
        // `--rulers` and `--grid` start with those switched on.
        app.show_rulers = std::env::args().any(|a| a == "--rulers");
        app.show_grid = std::env::args().any(|a| a == "--grid");
        app.show_page = !std::env::args().any(|a| a == "--no-page");
        app.show_crosshair = std::env::args().any(|a| a == "--crosshair");
        // `--tab properties` starts with that side-panel tab showing.
        if let Some(t) = std::env::args().skip_while(|a| a != "--tab").nth(1) {
            app.tab = match t.to_ascii_lowercase().as_str() {
                "properties" => Tab::Properties,
                "info" => Tab::Info,
                "elements" | "table" => Tab::Elements,
                "names" => Tab::Names,
                "sequence" | "fasta" => Tab::Sequence,
                "nmr" | "1h" => Tab::Nmr,
                "15n" => {
                    app.nmr_nucleus = 15;
                    Tab::Nmr
                }
                "19f" => {
                    app.nmr_nucleus = 19;
                    Tab::Nmr
                }
                "31p" => {
                    app.nmr_nucleus = 31;
                    Tab::Nmr
                }
                "13c" => {
                    app.nmr_nucleus = 13;
                    Tab::Nmr
                }
                "document" | "style" => Tab::Style,
                "help" => Tab::Help,
                _ => Tab::Analysis,
            };
        }
        // `--nmr-enrich 50`: isotope enrichment (%) used by the NMR prediction.
        if let Some(v) = std::env::args().skip_while(|a| a != "--nmr-enrich").nth(1).and_then(|v| v.parse::<f64>().ok()) {
            app.nmr_enrich = v.clamp(1.0, 100.0);
        }
        if std::env::args().any(|a| a == "--select-all") {
            app.sel = app.doc.select_all();
            app.tool = Tool::Marquee;
        }
        if let Some(p) = std::env::args().skip_while(|a| a != "--style").nth(1).and_then(|n| styles::find(&n)) {
            styles::apply(&mut app.doc, styles::to_style(p, true));
            app.tab = Tab::Style;
        }
        app
    }

    // -----------------------------------------------------------------------
    // Document helpers

    fn checkpoint(&mut self) {
        self.edit_gen += 1;
        self.undo.push(self.doc.clone());
        if self.undo.len() > 300 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    fn do_undo(&mut self) {
        if let Some(d) = self.undo.pop() {
            self.edit_gen += 1;
            self.redo.push(std::mem::replace(&mut self.doc, d));
            self.hotspot = None;
            self.sel.clear();
            self.editing = None;
            self.dirty = true;
        }
    }

    fn do_redo(&mut self) {
        if let Some(d) = self.redo.pop() {
            self.edit_gen += 1;
            self.undo.push(std::mem::replace(&mut self.doc, d));
            self.hotspot = None;
            self.sel.clear();
            self.editing = None;
            self.dirty = true;
        }
    }

    fn to_doc(&self, p: Pos2) -> Pos2 {
        ((p - self.canvas.min - self.pan) / self.zoom).to_pos2()
    }

    fn to_screen(&self, p: Pos2) -> Pos2 {
        self.canvas.min + self.pan + p.to_vec2() * self.zoom
    }

    fn tol(&self) -> f32 {
        (7.0 / self.zoom).max(3.0)
    }

    fn view_center(&self) -> Pos2 {
        if self.canvas.is_positive() { self.to_doc(self.canvas.center()) } else { Pos2::ZERO }
    }

    /// Document settings ▸ Page: paper, orientation, margins, and what fits in the drawing area.
    fn page_settings_ui(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.label(egui::RichText::new("Page").strong());
        let mut changed = false;
        let pg = &mut self.doc.style.page;
        egui::Grid::new("page").num_columns(2).show(ui, |ui| {
            ui.label("Paper");
            let name = pg.paper_name();
            ui.horizontal(|ui| {
                egui::ComboBox::from_id_salt("paper").selected_text(name).width(90.0).show_ui(ui, |ui| {
                    for (n, size) in [("Letter", LETTER), ("A4", A4)] {
                        if ui.selectable_label(name == n, n).clicked() {
                            let (w, h) = if pg.landscape() { (size.1, size.0) } else { size };
                            pg.paper_w = w;
                            pg.paper_h = h;
                            changed = true;
                        }
                    }
                    let _ = ui.selectable_label(name == "Custom", "Custom (type the size)");
                });
                let mut land = pg.landscape();
                if ui.radio_value(&mut land, false, "Portrait").clicked() | ui.radio_value(&mut land, true, "Landscape").clicked() && land != pg.landscape() {
                    std::mem::swap(&mut pg.paper_w, &mut pg.paper_h);
                    std::mem::swap(&mut pg.left, &mut pg.top);
                    std::mem::swap(&mut pg.right, &mut pg.bottom);
                    changed = true;
                }
            });
            ui.end_row();
            let cm = |ui: &mut egui::Ui, v: &mut f32, lo: f32, hi: f32| ui.add(egui::DragValue::new(v).range(lo..=hi).speed(0.01).max_decimals(3).suffix(" cm")).changed();
            ui.label("Paper size");
            ui.horizontal(|ui| {
                changed |= cm(ui, &mut pg.paper_w, 3.0, 200.0);
                ui.label("×");
                changed |= cm(ui, &mut pg.paper_h, 3.0, 200.0);
            });
            ui.end_row();
            ui.label("Margins");
            ui.horizontal(|ui| {
                ui.label("left");
                changed |= cm(ui, &mut pg.left, 0.0, 20.0);
                ui.label("right");
                changed |= cm(ui, &mut pg.right, 0.0, 20.0);
            });
            ui.end_row();
            ui.label("");
            ui.horizontal(|ui| {
                ui.label("top");
                changed |= cm(ui, &mut pg.top, 0.0, 20.0);
                ui.label("bottom");
                changed |= cm(ui, &mut pg.bottom, 0.0, 20.0);
            });
            ui.end_row();
        });
        if changed {
            self.dirty = true;
        }
        let pg = self.doc.style.page;
        let (aw, ah) = pg.area_cm();
        let bond_cm = self.doc.style.bond_length * 2.54 / 72.0;
        ui.label(format!(
            "Drawing area: {:.2} × {:.2} cm per page, about {:.0} × {:.0} bond lengths ({:.2} cm each).",
            aw,
            ah,
            aw / bond_cm,
            ah / bond_cm,
            bond_cm
        ));
        if let Some(b) = self.doc.bounds_of(None) {
            let upcm = arrange::units_per_mm(&self.doc) * 10.0;
            let (dw, dh) = (b.width() / upcm, b.height() / upcm);
            // Copies side by side with a gap of one bond length.
            let fit = |area: f32, size: f32| (((area + bond_cm) / (size + bond_cm)).floor() as i32).max(0);
            let n = fit(aw, dw) * fit(ah, dh);
            let (_, _, nx, ny) = arrange::pages_used(&self.doc);
            let pages = if nx * ny > 1 { format!(" It spreads over {} pages ({} across, {} down).", nx * ny, nx, ny) } else { String::new() };
            let fits = match n {
                0 => "it is larger than one page".to_string(),
                1 => "one of this size fits on a page".to_string(),
                n => format!("{} of this size fit on one page", n),
            };
            ui.label(format!("This drawing: {:.2} × {:.2} cm, {:.0} % of the page width and {:.0} % of its height; {}.{}", dw, dh, dw / aw * 100.0, dh / ah * 100.0, fits, pages));
        }
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.show_page, "Show the page");
            if ui.button("Show Whole Page").clicked() {
                self.show_page = true;
                self.fit_pages();
            }
        });
    }

    /// Zoom so every page the drawing uses is in view.
    pub fn fit_pages(&mut self) {
        if !self.canvas.is_positive() {
            return;
        }
        let s = arrange::page_size(&self.doc);
        let (c0, r0, nx, ny) = arrange::pages_used(&self.doc);
        let sheet = Rect::from_min_size(pos2(c0 as f32 * s.x, r0 as f32 * s.y), vec2(nx as f32 * s.x, ny as f32 * s.y));
        let room = self.canvas.size() - vec2(40.0, 40.0);
        self.zoom = (room.x / sheet.width()).min(room.y / sheet.height()).clamp(0.05, 8.0);
        self.pan = self.canvas.size() / 2.0 - sheet.center().to_vec2() * self.zoom;
    }

    fn fit_view(&mut self) {
        if !self.canvas.is_positive() {
            return;
        }
        let Some(b) = self.doc.bounds_of(None) else {
            self.zoom = 1.5;
            self.pan = self.canvas.size() / 2.0;
            return;
        };
        let b = b.expand(BOND_LEN);
        let z = (self.canvas.width() / b.width()).min(self.canvas.height() / b.height()).clamp(0.2, 4.0);
        self.zoom = z;
        self.pan = self.canvas.size() / 2.0 - b.center().to_vec2() * z;
    }

    fn set_zoom(&mut self, z: f32, around: Pos2) {
        let z = z.clamp(0.1, 16.0);
        let d = self.to_doc(around);
        self.zoom = z;
        self.pan = around - self.canvas.min - d.to_vec2() * z;
    }

    fn selected_or_all_atoms(&self) -> Vec<usize> {
        if self.sel.atoms.is_empty() { (0..self.doc.atoms.len()).collect() } else { self.sel.atoms.iter().copied().collect() }
    }

    fn selection_or_all(&self) -> Selection {
        if self.sel.is_empty() { self.doc.select_all() } else { self.sel.clone() }
    }

    fn status(&mut self, s: impl Into<String>) {
        self.status = s.into();
    }

    // -----------------------------------------------------------------------
    // Files

    fn new_doc(&mut self) {
        self.doc = new_drawing();
        self.hotspot = None;
        self.undo.clear();
        self.redo.clear();
        self.sel.clear();
        self.path = None;
        self.dirty = false;
        self.editing = None;
        self.view_initialized = false;
        self.init_frames = 0;
        self.status("New document");
    }

    fn open_dialog(&mut self, ctx: &egui::Context) {
        let f = rfd::FileDialog::new()
            .add_filter("All chemistry documents", &["cdx", "cdxml", "ctp", "mol", "mdl", "sdf", "sd", "rxn", "cml", "smi", "smiles", "skc", "tgf", "chm", "fasta", "fa", "fas", "faa", "fna", "ffn"])
            .add_filter("CDX document (*.cdx)", &["cdx"])
            .add_filter("CDXML document (*.cdxml)", &["cdxml"])
            .add_filter("CDX stationery (*.ctp)", &["ctp"])
            .add_filter("MDL Molfile (*.mol)", &["mol", "mdl"])
            .add_filter("SDfile (*.sdf)", &["sdf", "sd"])
            .add_filter("Reaction Molfile (*.rxn)", &["rxn"])
            .add_filter("Chemical Markup Language (*.cml)", &["cml"])
            .add_filter("SMILES (*.smi)", &["smi", "smiles", "txt"])
            .add_filter("FASTA sequences (*.fasta)", &["fasta", "fa", "fas", "faa", "fna", "ffn"])
            .pick_file();
        if let Some(p) = f {
            self.open_path(p, ctx);
        }
    }

    pub fn open_path(&mut self, p: PathBuf, _ctx: &egui::Context) {
        match read_any(&p) {
            Ok(d) => {
                self.doc = d;
                // Formats without page coordinates (SMILES, Molfile...) are centred on the
                // origin: move such drawings onto the first page.
                if self.doc.file.is_none() {
                    if let Some(b) = self.doc.bounds_of(None) {
                        if b.min.x < 0.0 || b.min.y < 0.0 {
                            let m = arrange::units_per_mm(&self.doc) * 15.0;
                            let all = self.doc.select_all();
                            self.doc.translate(&all, vec2(m - b.min.x, m - b.min.y));
                        }
                    }
                }
                self.hotspot = None;
                self.undo.clear();
                self.redo.clear();
                self.sel.clear();
                self.editing = None;
                self.dirty = false;
                let kept = self.doc.file.as_ref().map(|f| f.cdx_page_children.len() + f.xml_page_children.len()).unwrap_or(0);
                self.status(if kept > 0 {
                    format!("Opened {} — {} item(s) this app can't show are kept and will be saved back unchanged", file_name(&p), kept)
                } else {
                    format!("Opened {}", file_name(&p))
                });
                if self.shot_path.is_none() {
                    self.remember_recent(&p);
                }
                self.path = Some(p);
                self.view_initialized = false;
        self.init_frames = 0;
            }
            Err(e) => self.status(format!("Could not open {}: {}", file_name(&p), e)),
        }
    }

    fn save(&mut self, ctx: &egui::Context) {
        if self.embedded {
            self.ole_publish(ctx, self.edit_gen != self.ole_gen);
            if crate::ole::update_container() {
                self.dirty = false;
                let host = self.host.clone().unwrap_or("the document".into());
                self.status(format!("Updated the structure in {}", host));
            } else {
                self.status("The document holding this structure isn't connected any more; use Save As to keep a copy");
            }
            return;
        }
        match self.path.clone() {
            Some(p) if is_document_ext(&p) => self.save_to(p, ctx),
            _ => self.save_as(ctx),
        }
    }

    fn save_as(&mut self, ctx: &egui::Context) {
        let name = self.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or("Untitled".into());
        let f = rfd::FileDialog::new()
            .set_file_name(format!("{}.cdx", name))
            .add_filter("CDX document (*.cdx)", &["cdx"])
            .add_filter("CDXML document (*.cdxml)", &["cdxml"])
            .add_filter("CDX stationery (*.ctp)", &["ctp"])
            .add_filter("MDL Molfile V2000 (*.mol)", &["mol"])
            .add_filter("SDfile (*.sdf)", &["sdf"])
            .add_filter("Reaction Molfile V2000 (*.rxn)", &["rxn"])
            .add_filter("Chemical Markup Language (*.cml)", &["cml"])
            .add_filter("SMILES (*.smi)", &["smi"])
            .add_filter("FASTA sequences (*.fasta)", &["fasta", "fa"])
            .add_filter("PDF (*.pdf)", &["pdf"])
            .add_filter("Encapsulated PostScript (*.eps)", &["eps"])
            .add_filter("SVG picture (*.svg)", &["svg"])
            .add_filter("PNG (*.png)", &["png"])
            .add_filter("JPEG (*.jpg)", &["jpg", "jpeg"])
            .add_filter("GIF (*.gif)", &["gif"])
            .add_filter("BMP (*.bmp)", &["bmp"])
            .add_filter("TIFF (*.tif)", &["tif", "tiff"])
            .save_file();
        if let Some(p) = f {
            self.save_to(p, ctx);
        }
    }

    fn export_variant(&mut self, ctx: &egui::Context, label: &str, ext: &str, v3000: bool) {
        let name = self.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().to_string()).unwrap_or("Untitled".into());
        let f = rfd::FileDialog::new().set_file_name(format!("{}.{}", name, ext)).add_filter(label, &[ext]).save_file();
        let Some(p) = f else { return };
        if v3000 {
            let text = if ext == "rxn" { formats::write_rxn_v3000(&self.doc) } else { formats::write_molfile_v3000(&self.doc, &name) };
            match std::fs::write(&p, text) {
                Ok(_) => self.status(format!("Saved {}", file_name(&p))),
                Err(e) => self.status(format!("Could not save: {}", e)),
            }
        } else {
            self.save_to(p, ctx);
        }
    }

    fn save_to(&mut self, p: PathBuf, ctx: &egui::Context) {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("cdx").to_ascii_lowercase();
        let result: Result<(), String> = match ext.as_str() {
            "cdx" | "ctp" | "cdxml" | "mol" | "mdl" | "sdf" | "sd" | "rxn" | "cml" | "smi" | "smiles" | "fasta" | "fa" | "fas" | "faa" | "fna" | "ffn" => write_doc_file(&self.doc, &p),
            "pdf" => std::fs::write(&p, export::to_pdf(&self.doc, ctx, self.color_atoms)).map_err(|e| e.to_string()),
            "eps" | "ps" => std::fs::write(&p, export::to_eps(&self.doc, ctx, self.color_atoms)).map_err(|e| e.to_string()),
            "svg" => std::fs::write(&p, render::to_svg(&self.doc, ctx, self.color_atoms)).map_err(|e| e.to_string()),
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "tif" | "tiff" => {
                if self.doc.is_empty() {
                    Err("the drawing is empty".into())
                } else {
                    self.export_pending = Some(Pending { path: p.clone(), saved_view: (self.zoom, self.pan), stage: 0 });
                    self.status(format!("Saving picture {}…", file_name(&p)));
                    return;
                }
            }
            "skc" | "tgf" | "chm" => Err("this format is closed and undocumented, so it can't be written here; use .cdx or .cdxml instead".into()),
            other => Err(format!("unknown file type .{}", other)),
        };
        match result {
            Ok(_) => {
                if is_document_ext(&p) {
                    self.remember_recent(&p);
                    self.path = Some(p.clone());
                    self.dirty = false;
                }
                self.status(format!("Saved {}", file_name(&p)));
            }
            Err(e) => self.status(format!("Could not save {}: {}", file_name(&p), e)),
        }
    }

    // -----------------------------------------------------------------------
    // Clipboard

    fn copy(&mut self, ctx: &egui::Context, cut: bool) {
        self.copy_as(ctx, cut, self.copy_object);
    }

    /// Copy. `object_only` leaves the pictures off the clipboard, so programs that would
    /// otherwise paste a picture (LibreOffice) paste an object that opens here when
    /// double-clicked (Windows).
    pub(super) fn copy_as(&mut self, ctx: &egui::Context, cut: bool, object_only: bool) {
        if self.sel.is_empty() {
            return;
        }
        let part = self.doc.extract(&self.sel);
        let mut msg = if cut { "Cut" } else { "Copied" }.to_string();
        if clip::NATIVE {
            // A picture of the structure with the structure itself stored inside it.
            let mut p = part.clone();
            p.file = None;
            let cdxml = cdx::write_cdxml(&p);
            let rich = clip::Rich {
                pdf: if cfg!(target_os = "macos") { export::to_pdf_with_data(&p, ctx, self.color_atoms, Some(&cdxml)) } else { Vec::new() },
                emf: if cfg!(windows) { emf::to_emf(&p, ctx, self.color_atoms, Some(cdxml.as_bytes())) } else { Vec::new() },
                cdxml: cdxml.clone(),
                object_only: object_only && cfg!(windows),
            };
            match clip::write(&rich) {
                Ok(()) if rich.object_only => msg.push_str(" for LibreOffice: paste it there, then double-click it to edit it here (for Word, use the other copy)"),
                Ok(()) => msg.push_str(" as a picture with the structure inside: paste it into Word or PowerPoint, or copy it back from there to edit it here"),
                Err(e) => {
                    ctx.copy_text(formats::write_smiles(&part));
                    msg = format!("Couldn't put a picture on the clipboard ({}); copied SMILES text instead", e);
                }
            }
            self.clipboard = Some((part, cdxml));
        } else {
            let smi = formats::write_smiles(&part);
            ctx.copy_text(smi.clone());
            self.clipboard = Some((part, smi));
            msg.push_str(" (as a drawing, and as SMILES text for other apps)");
        }
        if cut {
            self.checkpoint();
            let s = self.sel.clone();
            self.doc.delete_selection(&s);
            self.sel.clear();
        }
        self.status(msg);
    }

    /// Paste from the system clipboard: a structure (also one inside a picture copied from a
    /// document), else text (SMILES, molfile, CDXML or a name).
    fn paste(&mut self) {
        if !clip::NATIVE {
            if let Some((_, s)) = self.clipboard.clone() {
                self.paste_text(&s);
            }
            return;
        }
        if let Some(found) = clip::read_structure() {
            let own = match (&self.clipboard, &found) {
                (Some((d, key)), clip::Found::Cdxml(s)) if key.trim() == s.trim() => Some(d.clone()),
                _ => None,
            };
            let is_own = own.is_some();
            if let Some(d) = own.or_else(|| found.to_doc()) {
                self.paste_doc(d, is_own);
                return;
            }
        }
        match clip::read_text() {
            Some(t) if !t.trim().is_empty() => self.paste_text(&t),
            _ => self.status("The clipboard has no structure or text this app understands"),
        }
    }

    fn paste_doc(&mut self, mut d: Doc, own: bool) {
        d.file = None;
        let at = self.view_center();
        let b = d.bounds_of(None).map(|r| r.center()).unwrap_or(Pos2::ZERO);
        let offset = if own && self.doc.bounds_of(None).is_some() { vec2(BOND_LEN * 0.6, BOND_LEN * 0.6) } else { at - b };
        self.checkpoint();
        self.sel = self.doc.merge(&d, offset);
        self.status(if own { "Pasted" } else { "Pasted the structure from the clipboard; edit it, then Copy and paste it back" });
    }

    fn paste_text(&mut self, text: &str) {
        let at = self.view_center();
        let part = match &self.clipboard {
            Some((d, s)) if s.trim() == text.trim() => Some(d.clone()),
            _ => None,
        };
        let part = part.or_else(|| {
            if text.contains("M  END") {
                formats::read_molfile(text).ok()
            } else if text.trim_start().starts_with("<?xml") || text.contains("<CDXML") {
                cdx::read_cdxml(text).ok()
            } else {
                formats::parse_smiles(text).ok().or_else(|| names::name_to_structure(text))
            }
        });
        match part {
            Some(mut d) => {
                d.file = None;
                let b = d.bounds_of(None).map(|r| r.center()).unwrap_or(Pos2::ZERO);
                let offset = if self.clipboard.is_some() && self.doc.bounds_of(None).is_some() {
                    // Paste next to the original, shifted a little.
                    vec2(BOND_LEN * 0.6, BOND_LEN * 0.6) + (at - b) * 0.0
                } else {
                    at - b
                };
                self.checkpoint();
                self.sel = self.doc.merge(&d, offset);
                self.status("Pasted");
            }
            None => self.status("Clipboard text isn't a structure this app understands"),
        }
    }

    fn insert_doc(&mut self, mut d: Doc, msg: &str) {
        d.file = None;
        let at = self.view_center();
        let b = d.bounds_of(None).map(|r| r.center()).unwrap_or(Pos2::ZERO);
        self.checkpoint();
        self.sel = self.doc.merge(&d, at - b);
        self.status(msg.to_string());
    }

    // -----------------------------------------------------------------------
    // Edits

    fn apply_label(&mut self, atom: usize, text: &str) {
        if atom >= self.doc.atoms.len() {
            return;
        }
        let mut t = text.trim().to_string();
        // A mass number in front sets the isotope: "13C", "15NH2", "¹⁸O".
        let mut isotope = 0u16;
        let lead: String = t.chars().take_while(|c| c.is_ascii_digit() || "⁰¹²³⁴⁵⁶⁷⁸⁹".contains(*c)).collect();
        if !lead.is_empty() && t[lead.len()..].starts_with(|c: char| c.is_ascii_uppercase()) {
            let digits: String = lead.chars().map(|c| match "⁰¹²³⁴⁵⁶⁷⁸⁹".chars().position(|s| s == c) {
                Some(k) => char::from(b'0' + k as u8),
                None => c,
            }).collect();
            isotope = digits.parse().unwrap_or(0);
            t = t[lead.len()..].to_string();
        }
        let mut charge = 0i8;
        // Trailing charge such as "NH4+" or "O-" or "Fe3+".
        if t.len() > 1 && (t.ends_with('+') || t.ends_with('-') || t.ends_with('−')) {
            let sign = if t.ends_with('+') { 1 } else { -1 };
            t.pop();
            let mut digits = String::new();
            while t.len() > 1 && t.ends_with(|c: char| c.is_ascii_digit()) && chem::tokenize_label(&t[..t.len() - 1]).is_some() {
                // Only treat a digit as a charge count if what's left still reads as a label
                // and the digit isn't a subscript of an H ("NH4+" keeps 4 as count of H).
                if t[..t.len() - 1].ends_with('H') {
                    break;
                }
                digits.insert(0, t.pop().unwrap());
            }
            charge = sign * digits.parse::<i8>().unwrap_or(1);
        }
        let a = &mut self.doc.atoms[atom];
        a.isotope = isotope;
        if t.is_empty() {
            a.el = elements::C;
            a.label = None;
            a.charge = 0;
            a.show = false;
            return;
        }
        let (z, label) = cdx::interpret_label(&t);
        a.el = if z == 0 && label.is_none() { elements::C } else { z };
        // "CH3", "CH4", "NH2"…: the element, shown, its hydrogens following the bonds.
        a.show = label.is_none() && (t != "C" || isotope != 0);
        a.label = label;
        if charge != 0 || text.trim().ends_with(['+', '-', '−']) {
            a.charge = charge;
        }
    }

    fn commit_edit(&mut self) {
        let Some(e) = self.editing.take() else { return };
        match e {
            Editing::Label { atom, text, fresh } => {
                if fresh && text.trim().is_empty() {
                    return;
                }
                self.checkpoint();
                self.apply_label(atom, &text);
            }
            Editing::Caption { idx, text, fresh } => {
                if idx >= self.doc.captions.len() {
                    return;
                }
                if text.trim().is_empty() {
                    if !fresh {
                        self.checkpoint();
                    }
                    self.doc.captions.remove(idx);
                    return;
                }
                if !fresh {
                    self.checkpoint();
                }
                self.doc.captions[idx].text = text;
                self.dirty = true;
            }
        }
    }

    fn set_element_on(&mut self, i: usize, z: u8) {
        let a = &mut self.doc.atoms[i];
        a.el = z;
        a.label = None;
    }

    fn click_bond_tool(&mut self, order: u8, stereo: Stereo, hit: Option<Hit>, p: Pos2) {
        match hit {
            Some(Hit::Atom(i)) => {
                self.checkpoint();
                let dir = self.doc.free_direction(i);
                let q = self.doc.atoms[i].pos + dir * BOND_LEN;
                let j = self.doc.atom_or_new(q);
                if let Some(bi) = self.doc.find_bond(i, j) {
                    self.bump_bond(bi, order, stereo);
                } else {
                    self.doc.add_bond(i, j, order, stereo);
                }
            }
            Some(Hit::Bond(bi)) => {
                self.checkpoint();
                self.bump_bond(bi, order, stereo);
            }
            _ => {
                self.checkpoint();
                let a = self.doc.atom_or_new(p);
                let d = vec2(30f32.to_radians().cos(), -30f32.to_radians().sin());
                let b = self.doc.atom_or_new(p + d * BOND_LEN);
                self.doc.add_bond(a, b, order, stereo);
            }
        }
    }

    /// Clicking an existing bond with a bond tool changes it the way the tool suggests.
    fn bump_bond(&mut self, bi: usize, order: u8, stereo: Stereo) {
        let b = &mut self.doc.bonds[bi];
        if stereo != Stereo::None {
            if b.stereo == stereo {
                std::mem::swap(&mut b.a, &mut b.b);
            } else {
                b.stereo = stereo;
                b.order = 1;
            }
            return;
        }
        b.stereo = Stereo::None;
        match order {
            1 => b.order = if b.order >= 3 { 1 } else { b.order + 1 },
            2 => {
                if b.order != 2 {
                    b.order = 2;
                    b.dbl = DblPos::Auto;
                } else {
                    b.dbl = match b.dbl {
                        DblPos::Auto => DblPos::Center,
                        DblPos::Center => DblPos::Right,
                        DblPos::Right => DblPos::Left,
                        DblPos::Left => DblPos::Auto,
                    };
                }
            }
            _ => b.order = order,
        }
    }

    fn bond_drag_end(&self, from_atom: Option<usize>, start: Pos2, cur: Pos2) -> (Pos2, Option<usize>) {
        if let Some(j) = self.doc.atom_at(cur, self.tol() * 2.0, from_atom) {
            return (self.doc.atoms[j].pos, Some(j));
        }
        let d = cur - start;
        let mut ang = d.y.atan2(d.x);
        if self.fixed_angles {
            let step = 15f32.to_radians();
            ang = (ang / step).round() * step;
        }
        let len = if self.fixed_lengths { BOND_LEN } else { d.length().max(BOND_LEN * 0.25) };
        (start + vec2(ang.cos(), ang.sin()) * len, None)
    }

    /// A charged (or radical) atom whose + / − sign is under `p`: the sign sits just beside the
    /// atom, a little outside the atom's own hit area.
    fn charge_sign_at(&self, p: Pos2) -> Option<usize> {
        self.doc
            .atoms
            .iter()
            .enumerate()
            .filter(|(_, a)| a.charge != 0 || a.radical != 0)
            .map(|(i, a)| (i, (a.pos - p).length()))
            .filter(|(_, d)| *d < BOND_LEN * 0.7)
            .min_by(|x, y| x.1.total_cmp(&y.1))
            .map(|x| x.0)
    }

    fn erase_hit(&mut self, hit: Hit) {
        match hit {
            Hit::Atom(i) => {
                // A charge or radical goes first; erasing again removes the atom.
                let a = &mut self.doc.atoms[i];
                if a.charge != 0 || a.radical != 0 {
                    let what = if a.charge != 0 { "charge" } else { "radical" };
                    a.charge = 0;
                    a.radical = 0;
                    self.status(format!("Removed the {} (erase again to remove the atom)", what));
                    return;
                }
                let a = &self.doc.atoms[i];
                if a.el != elements::C || a.label.is_some() {
                    if self.doc.degree(i) > 0 {
                        let a = &mut self.doc.atoms[i];
                        a.el = elements::C;
                        a.label = None;
                        a.charge = 0;
                        return;
                    }
                }
                // Remove the atom and any carbons left on their own.
                let nb = self.doc.neighbors(i);
                let mut set = HashSet::from([i]);
                for n in nb {
                    if self.doc.degree(n) == 1 && self.doc.atoms[n].el == elements::C && self.doc.atoms[n].label.is_none() {
                        set.insert(n);
                    }
                }
                self.doc.remove_atoms(&set);
            }
            Hit::Bond(b) => self.doc.remove_bond(b),
            Hit::Arrow(i) => {
                self.doc.arrows.remove(i);
            }
            Hit::Caption(i) => {
                self.doc.captions.remove(i);
            }
            Hit::Shape(i) => {
                self.doc.shapes.remove(i);
            }
        }
        self.sel.clear();
    }

    fn select_hit(&mut self, hit: Hit, add: bool) {
        if !add {
            self.sel.clear();
        }
        match hit {
            Hit::Atom(i) => {
                if add && self.sel.atoms.contains(&i) {
                    self.sel.atoms.remove(&i);
                } else {
                    self.sel.atoms.insert(i);
                }
            }
            Hit::Bond(b) => {
                let bb = &self.doc.bonds[b];
                self.sel.atoms.insert(bb.a);
                self.sel.atoms.insert(bb.b);
            }
            Hit::Arrow(i) => {
                self.sel.arrows.insert(i);
            }
            Hit::Caption(i) => {
                self.sel.captions.insert(i);
            }
            Hit::Shape(i) => {
                self.sel.shapes.insert(i);
            }
        }
    }

    fn hit_selected(&self, hit: Hit) -> bool {
        match hit {
            Hit::Atom(i) => self.sel.atoms.contains(&i),
            Hit::Bond(b) => self.sel.atoms.contains(&self.doc.bonds[b].a) && self.sel.atoms.contains(&self.doc.bonds[b].b),
            Hit::Arrow(i) => self.sel.arrows.contains(&i),
            Hit::Caption(i) => self.sel.captions.contains(&i),
            Hit::Shape(i) => self.sel.shapes.contains(&i),
        }
    }

    fn select_in(&mut self, inside: impl Fn(Pos2) -> bool, add: bool) {
        if !add {
            self.sel.clear();
        }
        for (i, a) in self.doc.atoms.iter().enumerate() {
            if inside(a.pos) {
                self.sel.atoms.insert(i);
            }
        }
        for (i, a) in self.doc.arrows.iter().enumerate() {
            if inside(a.from) && inside(a.to) {
                self.sel.arrows.insert(i);
            }
        }
        for (i, c) in self.doc.captions.iter().enumerate() {
            if inside(caption_rect(c).center()) {
                self.sel.captions.insert(i);
            }
        }
        for (i, s) in self.doc.shapes.iter().enumerate() {
            if inside(s.min) && inside(s.max) {
                self.sel.shapes.insert(i);
            }
        }
    }

    fn transform_selection(&mut self, f: impl Fn(Vec2) -> Vec2, msg: &str) {
        let sel = self.selection_or_all();
        let Some(b) = self.doc.bounds_of(Some(&sel)) else { return };
        let c = b.center();
        self.checkpoint();
        self.doc.transform(&sel, |p| c + f(p - c));
        // Wedges keep pointing the same way; double-bond sides flip with a mirror.
        self.status(msg.to_string());
    }

    fn flip(&mut self, horizontal: bool) {
        self.transform_selection(|v| if horizontal { vec2(-v.x, v.y) } else { vec2(v.x, -v.y) }, if horizontal { "Flipped horizontally" } else { "Flipped vertically" });
        let sel = self.selection_or_all();
        for b in &mut self.doc.bonds {
            if sel.atoms.contains(&b.a) && sel.atoms.contains(&b.b) {
                b.dbl = match b.dbl {
                    DblPos::Left => DblPos::Right,
                    DblPos::Right => DblPos::Left,
                    x => x,
                };
            }
        }
    }

    /// Turn the selection (or everything) by `deg` degrees, clockwise on screen. A branch of a
    /// larger structure turns around the atom it hangs from.
    fn rotate_by(&mut self, deg: f32) {
        let sel = self.selection_or_all();
        let Some(pivot) = arrange::rotation_pivot(&self.doc, &sel) else { return };
        self.checkpoint();
        arrange::rotate(&mut self.doc, &sel, pivot, deg);
        self.status(format!("Rotated {}° {}", fmt_deg(deg.abs()), if deg >= 0.0 { "clockwise" } else { "counter-clockwise" }));
    }

    /// Turn the selection over (180° about a vertical or horizontal axis in space). Unlike a
    /// flip, which mirrors, this keeps the stereochemistry: wedges become hashes and back.
    fn rotate_180(&mut self, horizontal: bool) {
        let sel = self.selection_or_all();
        if self.doc.bounds_of(Some(&sel)).is_none() {
            return;
        }
        self.flip(horizontal);
        for b in &mut self.doc.bonds {
            if sel.atoms.contains(&b.a) && sel.atoms.contains(&b.b) {
                b.stereo = match b.stereo {
                    Stereo::Wedge => Stereo::Hash,
                    Stereo::Hash => Stereo::Wedge,
                    x => x,
                };
            }
        }
        self.status(if horizontal { "Rotated 180° about the vertical axis" } else { "Rotated 180° about the horizontal axis" });
    }

    fn align(&mut self, edge: Edge, ctx: &egui::Context) {
        let sel = self.sel.clone();
        if arrange::objects(&self.doc, &sel).len() < 2 {
            self.status("Select two or more objects to align");
            return;
        }
        self.checkpoint();
        let n = arrange::align(&mut self.doc, &sel, edge, ctx);
        self.status(format!("Aligned {} objects by {}", n, edge.label().to_lowercase()));
    }

    /// Spread the selected objects evenly (`gap_mm` None) or with a fixed gap in millimetres.
    fn distribute(&mut self, horizontal: bool, gap_mm: Option<f32>, ctx: &egui::Context) {
        let sel = self.sel.clone();
        let need = if gap_mm.is_some() { 2 } else { 3 };
        if arrange::objects(&self.doc, &sel).len() < need {
            self.status(format!("Select {} or more objects to distribute", if need == 2 { "two" } else { "three" }));
            return;
        }
        self.checkpoint();
        let gap = gap_mm.map(|mm| mm * arrange::units_per_mm(&self.doc));
        let n = arrange::distribute(&mut self.doc, &sel, horizontal, gap, ctx);
        let axis = if horizontal { "horizontally" } else { "vertically" };
        self.status(match gap_mm {
            Some(mm) => format!("Distributed {} objects {} with a {} mm gap", n, axis, fmt_deg(mm)),
            None => format!("Distributed {} objects evenly {}", n, axis),
        });
    }

    /// Where the round rotation handle sits on screen (top-right of the selection box).
    fn rotate_handle(&self) -> Option<Pos2> {
        if self.sel.is_empty() || !matches!(self.tool, Tool::Lasso | Tool::Marquee) {
            return None;
        }
        let b = self.doc.bounds_of(Some(&self.sel))?;
        let r = Rect::from_two_pos(self.to_screen(b.min), self.to_screen(b.max)).expand(8.0);
        Some(pos2(r.max.x + 10.0, r.min.y - 10.0))
    }

    fn open_dialog_box(&mut self, d: Dialog) {
        self.dialog = Some(d);
        self.dialog_focus = true;
    }

    fn dialog_ui(&mut self, ctx: &egui::Context) {
        let Some(d) = self.dialog else { return };
        let title = match d {
            Dialog::Rotate => "Rotate",
            Dialog::Gap { horizontal: true } => "Distribute Horizontally with Gap",
            Dialog::Gap { horizontal: false } => "Distribute Vertically with Gap",
            Dialog::Scale => "Scale",
            Dialog::Color => "Other Color",
        };
        let mut open = true;
        let mut apply = false;
        let mut cancel = false;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, vec2(0.0, -60.0))
            .open(&mut open)
            .show(ctx, |ui| {
                match d {
                    Dialog::Rotate => {
                        ui.horizontal(|ui| {
                            ui.label("Angle:");
                            let r = ui.add(egui::DragValue::new(&mut self.rotate_deg).range(0.0..=360.0).speed(1.0).max_decimals(2).suffix("°"));
                            if self.frame_requested_focus(&r) {
                                r.request_focus();
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.radio_value(&mut self.rotate_ccw, false, "Clockwise");
                            ui.radio_value(&mut self.rotate_ccw, true, "Counter-clockwise");
                        });
                        ui.label(egui::RichText::new("A selected branch turns around the atom it is attached to.").size(11.0).color(Color32::GRAY));
                    }
                    Dialog::Gap { .. } => {
                        ui.horizontal(|ui| {
                            ui.label("Gap between objects:");
                            let r = ui.add(egui::DragValue::new(&mut self.gap_mm).range(-500.0..=500.0).speed(0.1).max_decimals(2).suffix(" mm"));
                            if self.frame_requested_focus(&r) {
                                r.request_focus();
                            }
                        });
                        ui.label(egui::RichText::new("Measured from the edge of one object to the edge of the next; the first object stays in place.").size(11.0).color(Color32::GRAY));
                    }
                    Dialog::Scale => {
                        ui.horizontal(|ui| {
                            ui.label("Scale to:");
                            let r = ui.add(egui::DragValue::new(&mut self.scale_pct).range(1.0..=1000.0).speed(1.0).max_decimals(1).suffix(" %"));
                            if self.frame_requested_focus(&r) {
                                r.request_focus();
                            }
                        });
                        ui.label(egui::RichText::new("The selection (or the whole drawing) grows or shrinks around its centre.").size(11.0).color(Color32::GRAY));
                    }
                    Dialog::Color => {
                        ui.horizontal(|ui| {
                            ui.label("Colour:");
                            ui.color_edit_button_srgb(&mut self.custom_color);
                        });
                        ui.label(egui::RichText::new("Applied to the selected objects.").size(11.0).color(Color32::GRAY));
                    }
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.button("OK").clicked() {
                        apply = true;
                    }
                    if ui.button("Cancel").clicked() {
                        cancel = true;
                    }
                });
                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    apply = true;
                }
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    cancel = true;
                }
            });
        if apply {
            self.dialog = None;
            match d {
                Dialog::Rotate => {
                    let deg = if self.rotate_ccw { -self.rotate_deg } else { self.rotate_deg };
                    self.rotate_by(deg);
                }
                Dialog::Gap { horizontal } => {
                    let mm = self.gap_mm;
                    self.distribute(horizontal, Some(mm), ctx);
                }
                Dialog::Scale => {
                    let k = self.scale_pct / 100.0;
                    if k > 0.0 && k != 1.0 {
                        self.transform_selection(|v| v * k, &format!("Scaled to {}%", fmt_deg(self.scale_pct)));
                    }
                }
                Dialog::Color => {
                    let c = self.custom_color;
                    self.apply_color(Some(c));
                }
            }
        } else if cancel || !open {
            self.dialog = None;
        }
    }

    /// True on the first frame a dialog is shown, so its number field gets the keyboard.
    fn frame_requested_focus(&mut self, _r: &egui::Response) -> bool {
        std::mem::take(&mut self.dialog_focus)
    }

    fn clean_up(&mut self) {
        let atoms = self.selected_or_all_atoms();
        if atoms.len() < 2 {
            return;
        }
        self.checkpoint();
        chem::clean(&mut self.doc, &atoms);
        self.status("Cleaned up structure");
    }

    fn expand_labels(&mut self) {
        let atoms = self.selected_or_all_atoms();
        let targets: Vec<usize> = atoms
            .into_iter()
            .filter(|&i| self.doc.atoms[i].label.as_ref().map(|l| chem::expansion_smiles(l).is_some()).unwrap_or(false))
            .collect();
        if targets.is_empty() {
            self.status("No group labels to expand (known: Me, Et, Ph, OMe, CO2H, NO2, Boc, …)");
            return;
        }
        self.checkpoint();
        for &i in targets.iter().rev() {
            formats::expand_atom(&mut self.doc, i);
        }
        self.sel.clear();
        self.status(format!("Expanded {} label(s)", targets.len()));
    }

    fn apply_color(&mut self, c: Option<Rgb>) {
        if self.sel.is_empty() {
            self.status("Select something first, then pick a colour");
            return;
        }
        self.checkpoint();
        let s = self.sel.clone();
        for &i in &s.atoms {
            self.doc.atoms[i].color = c;
        }
        for b in &mut self.doc.bonds {
            if s.atoms.contains(&b.a) && s.atoms.contains(&b.b) {
                b.color = c;
            }
        }
        for &i in &s.arrows {
            self.doc.arrows[i].color = c;
        }
        for &i in &s.captions {
            self.doc.captions[i].color = c;
        }
        for &i in &s.shapes {
            self.doc.shapes[i].color = c;
        }
    }

    // -----------------------------------------------------------------------
    // Analysis

    fn topology_key(&self) -> u64 {
        let mut h = DefaultHasher::new();
        for a in &self.doc.atoms {
            (a.el, a.charge, a.isotope, a.radical, &a.label).hash(&mut h);
        }
        for b in &self.doc.bonds {
            (b.a, b.b, b.order).hash(&mut h);
        }
        for a in &self.sel.atoms {
            a.hash(&mut h);
        }
        h.finish()
    }

    fn update_analysis(&mut self) {
        let key = self.topology_key();
        if key == self.analysis.key {
            return;
        }
        let atoms: HashSet<usize> = self.selected_or_all_atoms().into_iter().collect();
        let mut lines = Vec::new();
        let mut parts = Vec::new();
        for frag in self.doc.fragments() {
            let f: Vec<usize> = frag.into_iter().filter(|i| atoms.contains(i)).collect();
            if f.is_empty() {
                continue;
            }
            let (formula, charge) = chem::atoms_formula(&self.doc, &f);
            let sub = self.doc.extract(&Selection { atoms: f.iter().copied().collect(), ..Default::default() });
            parts.push(chem::formula_string(&formula, charge));
            let ion = chem::ms_ion(&formula, charge);
            lines.push(FragmentInfo {
                mz: chem::isotope_pattern(&ion.formula, ion.charge),
                ion_formula: chem::formula_string(&ion.formula, ion.charge),
                ion_mz: ion.mz(),
                spectrum: ion.spectrum(),
                ion: ion.label,
                name: if f.len() <= 60 { names::structure_to_name(&sub) } else { None },
                exact: chem::exact_mass(&formula, charge),
                mw: chem::mol_weight(&formula),
                ea: chem::elemental_analysis(&formula),
                props: chem::properties(&self.doc, &f),
                formula,
                charge,
            });
        }
        let warnings = atoms.iter().filter(|&&i| self.doc.valence_error(i)).count();
        let frags: Vec<Vec<usize>> = self.doc.fragments().into_iter().map(|f| f.into_iter().filter(|i| atoms.contains(i)).collect::<Vec<_>>()).filter(|f| !f.is_empty()).collect();
        let chem = if frags.len() == 1 { Some(crate::props::compute(&self.doc, &frags[0])) } else { None };
        let sub = self.doc.extract(&Selection { atoms: atoms.iter().copied().collect(), ..Default::default() });
        self.analysis = Analysis { key, lines, total_formula: parts.join(" · "), warnings, smiles: if sub.atoms.is_empty() { String::new() } else { formats::write_smiles(&sub) }, chem };
    }
}

/// A number without needless decimals (90, 12.5, 0.25).
fn fmt_deg(v: f32) -> String {
    let s = format!("{:.2}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn file_name(p: &std::path::Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

fn is_document_ext(p: &std::path::Path) -> bool {
    matches!(p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(), Some("cdx" | "cdxml" | "ctp"))
}

/// Write a document in a chemistry format chosen by the file extension (not pictures).
pub fn write_doc_file(doc: &Doc, p: &std::path::Path) -> Result<(), String> {
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("cdx").to_ascii_lowercase();
    let title = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let data: Vec<u8> = match ext.as_str() {
        "cdx" | "ctp" => cdx::write_cdx(doc),
        "cdxml" => cdx::write_cdxml(doc).into_bytes(),
        "mol" | "mdl" => formats::write_molfile_v2000(doc, &title).into_bytes(),
        "sdf" | "sd" => formats::write_sdf(doc).into_bytes(),
        "rxn" => formats::write_rxn_v2000(doc).into_bytes(),
        "cml" => formats::write_cml(doc).into_bytes(),
        "smi" | "smiles" => format!("{}\n", formats::write_smiles(doc)).into_bytes(),
        "fasta" | "fa" | "fas" | "faa" | "fna" | "ffn" => crate::sequence::doc_to_fasta(doc)?.into_bytes(),
        other => return Err(format!("can't write .{} files from the command line", other)),
    };
    std::fs::write(p, data).map_err(|e| e.to_string())
}

pub fn read_any(p: &std::path::Path) -> Result<Doc, String> {
    let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
    let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    if bytes.starts_with(cdx::HEADER) {
        return cdx::read_cdx(&bytes);
    }
    let text = String::from_utf8_lossy(&bytes).to_string();
    // Formats without drawing settings open in the style new drawings start with.
    let styled = |r: Result<Doc, String>| {
        r.map(|mut d| {
            styles::apply(&mut d, new_drawing().style);
            d
        })
    };
    match ext.as_str() {
        "cdx" | "ctp" => cdx::read_cdx(&bytes),
        "cdxml" => cdx::read_cdxml(&text),
        "mol" | "mdl" => styled(formats::read_molfile(&text)),
        "sdf" | "sd" => styled(formats::read_sdf(&text)),
        "rxn" => styled(formats::read_rxn(&text)),
        "cml" => styled(formats::read_cml(&text)),
        "smi" | "smiles" => styled(formats::parse_smiles(text.lines().next().unwrap_or(""))),
        "fasta" | "fa" | "fas" | "faa" | "fna" | "ffn" | "seq" => styled(crate::sequence::doc_from_fasta(&text)),
        "skc" => Err("ISIS/Sketch files use a closed, undocumented format, so this app can't read them. Save them as .mol or .cdxml from the program that made them.".into()),
        "tgf" => Err("Transportable Graphics files use a closed, undocumented format, so this app can't read them.".into()),
        "chm" => Err("Version 3.x .chm files use an old undocumented format, so this app can't read them.".into()),
        _ => {
            if text.contains("<CDXML") {
                cdx::read_cdxml(&text)
            } else if text.starts_with("$RXN") {
                styled(formats::read_rxn(&text))
            } else if text.contains("$$$$") {
                styled(formats::read_sdf(&text))
            } else if text.contains("M  END") {
                styled(formats::read_molfile(&text))
            } else if text.contains("<cml") || text.contains("<molecule") {
                styled(formats::read_cml(&text))
            } else if text.trim_start().starts_with('>') {
                styled(crate::sequence::doc_from_fasta(&text))
            } else {
                styled(formats::parse_smiles(text.lines().next().unwrap_or("")))
            }
        }
    }
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let base: Vec<String> = fonts.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default();
    // The same fonts as installed with macOS and with Windows (file names differ).
    let candidates = [
        ("chem", "Arial.ttf", "arial.ttf"),
        ("chem-bold", "Arial Bold.ttf", "arialbd.ttf"),
        ("chem-italic", "Arial Italic.ttf", "ariali.ttf"),
        ("chem-serif", "Times New Roman.ttf", "times.ttf"),
        ("chem-serif-bold", "Times New Roman Bold.ttf", "timesbd.ttf"),
    ];
    let win_fonts = std::path::PathBuf::from(std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into())).join("Fonts");
    for (name, mac, win) in candidates {
        let mut fam = Vec::new();
        let bytes = std::fs::read(std::path::Path::new("/System/Library/Fonts/Supplemental").join(mac))
            .or_else(|_| std::fs::read(win_fonts.join(win)));
        if let Ok(bytes) = bytes {
            fonts.font_data.insert(name.to_string(), Arc::new(egui::FontData::from_owned(bytes)));
            fam.push(name.to_string());
        }
        fam.extend(base.iter().cloned());
        fonts.families.insert(egui::FontFamily::Name(name.into()), fam);
    }
    ctx.set_fonts(fonts);
}

// ---------------------------------------------------------------------------
// UI

const PANEL_BG: Color32 = Color32::from_rgb(236, 236, 238);
const ACCENT: Color32 = Color32::from_rgb(40, 110, 220);
const SEL_FILL: Color32 = Color32::from_rgba_premultiplied(60, 130, 240, 70);

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.frame += 1;
        self.ole_frame(&ctx);
        self.handle_close(&ctx);
        self.tidy_selection();
        self.update_title(&ctx);
        self.shortcuts(&ctx);
        self.update_analysis();

        egui::Panel::top("menu").frame(egui::Frame::default().fill(PANEL_BG).inner_margin(egui::Margin::symmetric(6, 2))).show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| self.menus(ui, &ctx));
        });
        egui::Panel::top("toolbar").frame(egui::Frame::default().fill(PANEL_BG).inner_margin(egui::Margin::symmetric(8, 4))).show(ui, |ui| {
            self.toolbar(ui, &ctx);
        });
        egui::Panel::bottom("status").frame(egui::Frame::default().fill(PANEL_BG).inner_margin(egui::Margin::symmetric(8, 3))).show(ui, |ui| {
            self.status_bar(ui);
        });
        egui::Panel::left("tools").exact_size(82.0).frame(egui::Frame::default().fill(PANEL_BG).inner_margin(egui::Margin::same(4))).show(ui, |ui| {
            self.palette(ui);
        });
        if self.show_panel {
            egui::Panel::right("side").default_size(300.0).size_range(240.0..=480.0).frame(egui::Frame::default().fill(Color32::from_rgb(246, 246, 247)).inner_margin(egui::Margin::same(8))).show(ui, |ui| {
                self.side_panel(ui, &ctx);
            });
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(self.doc.style.paper())).show(ui, |ui| {
            self.canvas_ui(ui, &ctx);
        });
        if self.close_ask == CloseAsk::Asking {
            self.close_bar(&ctx);
        }
        if self.show_about {
            self.about_box(&ctx);
        }
        self.dialog_ui(&ctx);
        self.extra_windows(&ctx);
        self.template_window_ui(&ctx);
        self.template_menu_ui(&ctx);
        self.arrow_menu_ui(&ctx);
        self.recovery_bar(&ctx);
        self.autosave(&ctx);
        self.handle_export(&ctx);
        self.handle_screenshot(&ctx);
    }
}

impl App {
    /// Give the object in the document the current structure and picture.
    fn ole_publish(&mut self, ctx: &egui::Context, changed: bool) {
        let cdxml = cdx::write_cdxml(&self.doc);
        let pic = emf::to_emf(&self.doc, ctx, self.color_atoms, Some(cdxml.as_bytes()));
        crate::ole::publish(cdxml, pic, changed);
        self.ole_gen = self.edit_gen;
    }

    /// Requests from the program holding the object (Windows), and changes sent back to it.
    fn ole_frame(&mut self, ctx: &egui::Context) {
        if !self.embedded {
            return;
        }
        for e in crate::ole::take_events() {
            match e {
                crate::ole::Event::Load(text) => {
                    let read = if text.trim().is_empty() { None } else { Some(cdx::read_cdxml(&text)) };
                    let (doc, note) = match read {
                        None => (new_drawing(), "The document sent no structure: draw one here, then close this window to put it into the document".to_string()),
                        Some(Ok(d)) => {
                            let n = d.atoms.len();
                            (d, format!("Editing the structure from the document ({} atoms): close this window (or use Save) to put your changes into it", n))
                        }
                        Some(Err(e)) => (new_drawing(), format!("The structure from the document could not be read ({})", e)),
                    };
                    crate::ole::log(format!("Window: {}", note));
                    self.status(note);
                    self.doc = doc;
                    self.hotspot = None;
                    self.undo.clear();
                    self.redo.clear();
                    self.sel.clear();
                    self.editing = None;
                    self.dirty = false;
                    self.view_initialized = false;
                    self.init_frames = 0;
                    self.ole_publish(ctx, false);
                }
                crate::ole::Event::Show => {
                    if self.auto_edit.is_none() && crate::ole::auto_edit_requested() {
                        self.auto_edit = Some(ctx.input(|i| i.time));
                    }
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                crate::ole::Event::Hide => ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true)),
                crate::ole::Event::Host(name) => self.host = Some(name),
                crate::ole::Event::Quit => {
                    if !self.ole_quit {
                        self.ole_quit = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
            }
        }
        // Automatic test of the edit cycle (see `ole::auto_edit_requested`): add one atom after
        // 2 s, close the window after 4 s, as a person editing the object would.
        if let Some(t0) = self.auto_edit {
            let t = ctx.input(|i| i.time) - t0;
            if t >= 2.0 && !self.auto_edit_done && !self.doc.atoms.is_empty() {
                self.auto_edit_done = true;
                self.checkpoint();
                let p = self.doc.atoms[0].pos + vec2(0.0, -BOND_LEN);
                let n = self.doc.atom_or_new(p);
                self.doc.add_bond(0, n, 1, Stereo::None);
                self.dirty = true;
                crate::ole::log("Automatic test: added an atom");
            }
            if t >= 4.0 && !self.ole_quit {
                crate::ole::log("Automatic test: closing the window");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }
        // Each finished change updates the picture in the document straight away.
        if self.edit_gen != self.ole_gen && self.drag.is_none() && self.editing.is_none() && !self.ole_quit {
            self.ole_publish(ctx, true);
        }
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let name = if self.embedded {
            format!("Structure in {}", self.host.as_deref().unwrap_or("a document"))
        } else {
            self.path.as_ref().map(|p| file_name(p)).unwrap_or("Untitled".into())
        };
        let t = format!("{}{} — {}", if self.dirty { "• " } else { "" }, name, APP_NAME);
        if t != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(t.clone()));
            self.title = t;
        }
    }

    fn handle_close(&mut self, ctx: &egui::Context) {
        if self.embedded {
            // Closing puts the changes into the document; nothing to ask. First the changes are
            // stored exactly as Save does, while the window still exists: LibreOffice collects
            // them (and redraws its picture) then, but not once the window has gone.
            let now = ctx.input(|i| i.time);
            if ctx.input(|i| i.viewport().close_requested()) && !self.ole_quit && self.close_store.is_none() {
                self.ole_publish(ctx, self.edit_gen != self.ole_gen);
                let before = crate::ole::updates_done();
                if crate::ole::changes_to_store() && crate::ole::update_container() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    self.close_store = Some((before, now, None));
                    let host = self.host.clone().unwrap_or("the document".into());
                    self.status(format!("Putting the changes into {}…", host));
                } else {
                    crate::ole::closing();
                    self.ole_quit = true;
                }
            }
            if let Some((before, asked, stored)) = self.close_store {
                if ctx.input(|i| i.viewport().close_requested()) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                }
                match stored {
                    // Give the document a moment to redraw its picture, then close.
                    Some(t) if now - t >= 0.5 => {
                        self.close_store = None;
                        crate::ole::closing();
                        self.ole_quit = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    Some(_) => {}
                    None if crate::ole::updates_done() > before || now - asked > 8.0 => self.close_store = Some((before, asked, Some(now))),
                    None => {}
                }
                ctx.request_repaint_after(std::time::Duration::from_millis(50));
            }
            return;
        }
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.dirty && !self.allow_close && self.shot_path.is_none() {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.close_ask = CloseAsk::Asking;
            } else if self.shot_path.is_none() && self.recovery_offer.is_none() {
                // A normal close: the recovery copy isn't needed any more.
                crate::recover::clear();
            }
        }
    }

    /// Copy the drawing to the recovery file a few seconds after each change, and drop the
    /// copy once the work is saved.
    fn autosave(&mut self, ctx: &egui::Context) {
        if self.shot_path.is_some() || self.recovery_offer.is_some() || self.embedded {
            return;
        }
        if !self.dirty {
            if self.recovery_written {
                crate::recover::clear();
                self.recovery_written = false;
            }
            self.saved_gen = self.edit_gen;
            return;
        }
        if self.edit_gen == self.saved_gen || self.drag.is_some() {
            return;
        }
        let now = ctx.input(|i| i.time);
        let wait = 4.0 - (now - self.last_autosave);
        if wait > 0.0 {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait));
            return;
        }
        self.last_autosave = now;
        self.saved_gen = self.edit_gen;
        if crate::recover::save(&self.doc).is_ok() {
            self.recovery_written = true;
        }
    }

    /// Offer back the drawing rescued from a run that stopped unexpectedly.
    fn recovery_bar(&mut self, ctx: &egui::Context) {
        let Some((file, time, log)) = self.recovery_offer.clone() else { return };
        egui::Modal::new(egui::Id::new("recover")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading(format!("{} closed unexpectedly", APP_NAME));
            ui.label(format!("Your unsaved drawing was kept ({}). Restore it?", crate::recover::ago(time)));
            if log.is_some() {
                ui.label(egui::RichText::new("A crash log with the details was saved.").size(11.0).color(Color32::GRAY));
            }
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Restore").clicked() {
                    match read_any(&file) {
                        Ok(d) => {
                            self.doc = d;
                            self.doc.file = None;
                            self.path = None;
                            self.dirty = true;
                            self.undo.clear();
                            self.redo.clear();
                            self.sel.clear();
                            self.view_initialized = false;
                            self.init_frames = 0;
                            self.edit_gen += 1;
                            self.recovery_written = true;
                            self.status("Restored the drawing. Save it to keep it.");
                        }
                        Err(e) => self.status(format!("Could not restore the drawing: {}", e)),
                    }
                    self.recovery_offer = None;
                }
                if ui.button("Discard").clicked() {
                    crate::recover::clear();
                    self.recovery_offer = None;
                }
                if let Some(l) = &log {
                    if ui.button("Show Crash Log").clicked() {
                        open_in_system(l);
                    }
                }
            });
        });
    }

    /// Drop selected or edited items that no longer exist (after undo, open, paste…), so
    /// nothing reaches past the end of the drawing.
    fn tidy_selection(&mut self) {
        let d = &self.doc;
        self.sel.atoms.retain(|&i| i < d.atoms.len());
        self.sel.arrows.retain(|&i| i < d.arrows.len());
        self.sel.captions.retain(|&i| i < d.captions.len());
        self.sel.shapes.retain(|&i| i < d.shapes.len());
        let stale = match &self.editing {
            Some(Editing::Label { atom, .. }) => *atom >= d.atoms.len(),
            Some(Editing::Caption { idx, .. }) => *idx >= d.captions.len(),
            None => false,
        };
        if stale {
            self.editing = None;
        }
        if self.hotspot.is_some_and(|h| h >= d.atoms.len()) {
            self.hotspot = None;
        }
        let gone = match self.hover {
            Some(Hit::Atom(i)) => i >= d.atoms.len(),
            Some(Hit::Bond(i)) => i >= d.bonds.len(),
            Some(Hit::Arrow(i)) => i >= d.arrows.len(),
            Some(Hit::Caption(i)) => i >= d.captions.len(),
            Some(Hit::Shape(i)) => i >= d.shapes.len(),
            None => false,
        };
        if gone {
            self.hover = None;
        }
    }

    fn about_box(&mut self, ctx: &egui::Context) {
        let r = egui::Modal::new(egui::Id::new("about")).show(ctx, |ui| {
            ui.set_width(340.0);
            ui.vertical_centered(|ui| {
                ui.heading(APP_NAME);
                ui.label(format!("Version {}", APP_VERSION));
                ui.add_space(6.0);
                ui.label("A desktop editor for chemical structures and reactions.");
                ui.add_space(6.0);
                ui.label(APP_CREDIT);
                ui.add_space(6.0);
                ui.label(APP_LICENSE);
                ui.label(egui::RichText::new(APP_DISCLAIMER).small().weak());
                ui.add_space(10.0);
                if ui.button("OK").clicked() {
                    self.show_about = false;
                }
            });
        });
        if r.should_close() {
            self.show_about = false;
        }
    }

    fn close_bar(&mut self, ctx: &egui::Context) {
        egui::Modal::new(egui::Id::new("close-ask")).show(ctx, |ui| {
            ui.set_width(320.0);
            ui.heading("Save changes before closing?");
            ui.label("Your changes will be lost if you don't save them.");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Save").clicked() {
                    self.save(ctx);
                    if !self.dirty {
                        self.allow_close = true;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    self.close_ask = CloseAsk::None;
                }
                if ui.button("Don't Save").clicked() {
                    self.allow_close = true;
                    self.close_ask = CloseAsk::None;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if ui.button("Cancel").clicked() {
                    self.close_ask = CloseAsk::None;
                }
            });
        });
    }

    /// The index new brackets get (None for boxes and ellipses, or an empty field).
    fn new_bracket_label(&self) -> Option<String> {
        let l = self.bracket_label.trim();
        (matches!(self.shape_kind, ShapeKind::Brackets | ShapeKind::Parens) && !l.is_empty()).then(|| l.to_string())
    }

    /// The "Index" field: the repeat count or label at the lower right of brackets. It sets the
    /// index of new brackets and of any selected brackets.
    fn bracket_index_ui(&mut self, ui: &mut egui::Ui) {
        let selected: Vec<usize> = self.sel.shapes.iter().copied().filter(|&i| self.doc.shapes.get(i).is_some_and(|s| s.is_bracket())).collect();
        if !matches!(self.shape_kind, ShapeKind::Brackets | ShapeKind::Parens) && selected.is_empty() {
            return;
        }
        ui.separator();
        ui.label("Index:");
        if let Some(&first) = selected.first() {
            if !ui.memory(|m| m.has_focus(egui::Id::new("bracket_index"))) {
                self.bracket_label = self.doc.shapes[first].label.clone().unwrap_or_default();
            }
        }
        let r = ui
            .add(egui::TextEdit::singleline(&mut self.bracket_label).id(egui::Id::new("bracket_index")).desired_width(40.0))
            .on_hover_text("Number of repeats (e.g. 3) or a label such as n, shown at the lower right of the brackets. Leave empty for none.");
        if r.changed() && !selected.is_empty() {
            self.checkpoint();
            let l = self.bracket_label.trim().to_string();
            for i in selected {
                self.doc.shapes[i].label = (!l.is_empty()).then(|| l.clone());
            }
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        // Only a focused text field counts as typing; a clicked canvas or button must not
        // swallow the hot keys.
        let text_focus = ctx
            .memory(|m| m.focused())
            .is_some_and(|id| egui::TextEdit::load_state(ctx, id).is_some());
        let typing = text_focus || self.editing.is_some() || self.dialog.is_some();
        // ⌘ plus exactly these extra keys (so ⌘C, ⇧⌘C and ⌥⇧⌘C stay apart).
        let key = |alt: bool, shift: bool, k: Key| {
            ctx.input_mut(|i| {
                let m = i.modifiers;
                if !m.command || m.alt != alt || m.shift != shift {
                    return false;
                }
                let mut want = Modifiers::COMMAND;
                if alt {
                    want = want | Modifiers::ALT;
                }
                if shift {
                    want = want | Modifiers::SHIFT;
                }
                i.consume_key(want, k)
            })
        };
        let cmd = |k: Key| key(false, false, k);
        let cmd_shift = |k: Key| key(false, true, k);
        let cmd_alt = |k: Key| key(true, false, k);
        let cmd_alt_shift = |k: Key| key(true, true, k);
        if cmd(Key::N) {
            self.new_doc();
        }
        if cmd(Key::O) {
            self.open_dialog(ctx);
        }
        if cmd_shift(Key::S) {
            self.save_as(ctx);
        }
        if cmd(Key::S) {
            self.save(ctx);
        }
        if cmd(Key::W) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if cmd(Key::P) {
            self.print(ctx);
        }
        if typing {
            // Text fields still copy and paste text.
            if clip::NATIVE {
                if cmd(Key::C) {
                    ctx.input_mut(|i| i.events.push(egui::Event::Copy));
                }
                if cmd(Key::X) {
                    ctx.input_mut(|i| i.events.push(egui::Event::Cut));
                }
                if cmd(Key::V) {
                    if let Some(t) = clip::read_text() {
                        ctx.input_mut(|i| i.events.push(egui::Event::Paste(t)));
                    }
                }
            }
            return;
        }
        if cmd_shift(Key::Z) || cmd(Key::Y) {
            self.do_redo();
        }
        if cmd(Key::Z) {
            self.do_undo();
        }
        if cmd(Key::A) {
            self.sel = self.doc.select_all();
            self.tool = if matches!(self.tool, Tool::Lasso) { Tool::Lasso } else { Tool::Marquee };
        }
        if cmd_shift(Key::I) {
            self.invert_selection();
        }
        if cmd_shift(Key::K) {
            self.clean_up();
        }
        if cmd_shift(Key::H) {
            self.rotate_180(true);
        }
        if cmd_shift(Key::V) {
            self.rotate_180(false);
        }
        if cmd(Key::R) {
            self.open_dialog_box(Dialog::Rotate);
        }
        if cmd(Key::K) {
            self.open_dialog_box(Dialog::Scale);
        }
        if cmd(Key::L) {
            self.fixed_lengths = !self.fixed_lengths;
            self.status(if self.fixed_lengths { "Fixed lengths on" } else { "Fixed lengths off" });
        }
        if cmd(Key::E) {
            self.fixed_angles = !self.fixed_angles;
            self.status(if self.fixed_angles { "Fixed angles on" } else { "Fixed angles off" });
        }
        if cmd(Key::OpenBracket) {
            self.restack(true);
        }
        if cmd(Key::CloseBracket) {
            self.restack(false);
        }
        for (k, e) in [(Key::L, Edge::Left), (Key::C, Edge::CentreX), (Key::R, Edge::Right), (Key::T, Edge::Top), (Key::M, Edge::CentreY), (Key::B, Edge::Bottom)] {
            if cmd_alt_shift(k) {
                self.align(e, ctx);
            }
        }
        if cmd_alt_shift(Key::H) {
            self.distribute(true, None, ctx);
        }
        if cmd_alt_shift(Key::V) {
            self.distribute(false, None, ctx);
        }
        if cmd_shift(Key::N) {
            self.tab = Tab::Names;
            self.show_panel = true;
        }
        if cmd_alt(Key::N) {
            self.structure_to_name(ctx);
        }
        if cmd_alt(Key::X) {
            self.show_crosshair = !self.show_crosshair;
        }
        if cmd(Key::Semicolon) {
            self.show_rulers = !self.show_rulers;
        }
        if cmd(Key::Slash) || cmd(Key::Num0) {
            self.fit_view();
        }
        if cmd_shift(Key::Period) {
            let c = self.canvas.center();
            self.set_zoom(self.zoom * 2.0, c);
        }
        if cmd_shift(Key::Comma) {
            let c = self.canvas.center();
            self.set_zoom(self.zoom / 2.0, c);
        }
        for (k, st) in [(Key::T, menus::TextStyle::Plain), (Key::B, menus::TextStyle::Bold), (Key::I, menus::TextStyle::Italic), (Key::F, menus::TextStyle::Formula)] {
            if cmd(k) {
                self.set_text_style(st);
            }
        }
        if clip::NATIVE {
            if cmd(Key::C) {
                self.copy(ctx, false);
            }
            if cfg!(windows) && cmd_shift(Key::C) {
                // The other kind of copy from the one ⌘C makes.
                self.copy_as(ctx, false, !self.copy_object);
            }
            if cmd(Key::X) {
                self.copy(ctx, true);
            }
            if cmd(Key::V) {
                self.paste();
            }
        }
        if cmd(Key::Plus) || cmd(Key::Equals) || cmd_shift(Key::Equals) || cmd_shift(Key::Plus) {
            let c = self.canvas.center();
            self.set_zoom(self.zoom * 1.25, c);
        }
        if cmd(Key::Minus) {
            let c = self.canvas.center();
            self.set_zoom(self.zoom / 1.25, c);
        }
        // Clipboard events.
        let events = ctx.input(|i| i.events.clone());
        for e in &events {
            match e {
                egui::Event::Copy => self.copy(ctx, false),
                egui::Event::Cut => self.copy(ctx, true),
                egui::Event::Paste(t) => self.paste_text(t),
                _ => {}
            }
        }
        // Hot keys: a key over an atom or bond (or at the hotspot) changes or grows the structure.
        let mods = ctx.input(|i| i.modifiers);
        if mods.command || mods.ctrl {
            return;
        }
        if let Some(h) = self.hotspot {
            if h >= self.doc.atoms.len() {
                self.hotspot = None;
            }
        }
        for e in &events {
            // An earlier key in this frame may have removed what the pointer was over.
            self.tidy_selection();
            match e {
                egui::Event::Text(t) => self.hover_key(t),
                egui::Event::Key { key, pressed: true, modifiers, .. } => match key {
                    Key::Delete | Key::Backspace => {
                        if !self.sel.is_empty() {
                            self.checkpoint();
                            let s = self.sel.clone();
                            self.doc.delete_selection(&s);
                            self.sel.clear();
                            self.hotspot = None;
                        } else if let Some(h) = self.hover {
                            self.checkpoint();
                            self.erase_hit(h);
                            self.hover = None;
                            self.hotspot = None;
                        }
                    }
                    Key::Escape => {
                        self.sel.clear();
                        self.drag = None;
                        self.hotspot = None;
                    }
                    Key::Enter => {
                        let target = match self.hover {
                            Some(Hit::Atom(i)) => Some(i),
                            None => self.hotspot,
                            _ => None,
                        };
                        if let Some(i) = target {
                            let text = current_label(&self.doc, i);
                            self.editing = Some(Editing::Label { atom: i, text, fresh: false });
                            self.edit_focus = true;
                        }
                    }
                    Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown if !modifiers.alt => {
                        let d = match key {
                            Key::ArrowLeft => vec2(-1.0, 0.0),
                            Key::ArrowRight => vec2(1.0, 0.0),
                            Key::ArrowUp => vec2(0.0, -1.0),
                            _ => vec2(0.0, 1.0),
                        };
                        let from = self.hotspot.or(match self.hover {
                            Some(Hit::Atom(i)) => Some(i),
                            _ => None,
                        });
                        if let Some(i) = from {
                            if let Some(n) = crate::hotkeys::neighbor_towards(&self.doc, i, d) {
                                self.hotspot = Some(n);
                            } else {
                                self.hotspot = Some(i);
                            }
                        }
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }

    fn hover_key(&mut self, t: &str) {
        let t = t.trim();
        if t.is_empty() {
            return;
        }
        let target = match self.hover {
            Some(h) => Some(h),
            None => self.hotspot.map(Hit::Atom),
        };
        let Some(h) = target else { return };
        let mut trial = self.doc.clone();
        let result = match h {
            Hit::Atom(i) => crate::hotkeys::atom_key(&mut trial, i, t).map(|d| (d, Some(i))),
            Hit::Bond(b) => crate::hotkeys::bond_key(&mut trial, b, t).map(|d| (d, None)),
            _ => None,
        };
        if let Some((d, atom)) = result {
            self.checkpoint();
            self.doc = trial;
            self.hotspot = d.hotspot.or(atom).or(self.hotspot);
            self.status = format!("Hot key {}: {}", t, d.status);
            self.dirty = true;
        }
    }

    // -------------------------------------------------------------------
    // Menus and bars

    fn toolbar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.horizontal(|ui| {
            if ui.button("New").on_hover_text("New document (⌘N)").clicked() {
                self.new_doc();
            }
            if ui.button("Open").on_hover_text("Open a file (⌘O)").clicked() {
                self.open_dialog(ctx);
            }
            if ui.button("Save").on_hover_text("Save (⌘S)").clicked() {
                self.save(ctx);
            }
            ui.separator();
            if ui.add_enabled(!self.undo.is_empty(), egui::Button::new("↶ Undo")).clicked() {
                self.do_undo();
            }
            if ui.add_enabled(!self.redo.is_empty(), egui::Button::new("↷ Redo")).clicked() {
                self.do_redo();
            }
            ui.separator();
            if ui.button("−").on_hover_text("Zoom out").clicked() {
                let c = self.canvas.center();
                self.set_zoom(self.zoom / 1.25, c);
            }
            ui.label(format!("{:.0}%", self.zoom * 100.0 / 1.5 * 100.0 / 100.0));
            if ui.button("+").on_hover_text("Zoom in").clicked() {
                let c = self.canvas.center();
                self.set_zoom(self.zoom * 1.25, c);
            }
            if ui.button("Fit").on_hover_text("Fit drawing to window (⌘0)").clicked() {
                self.fit_view();
            }
            ui.separator();
            if ui.button("Clean Up").on_hover_text("Tidy bond lengths and angles (⇧⌘K)").clicked() {
                self.clean_up();
            }
            ui.separator();
            // Options for the current tool.
            match self.tool {
                Tool::Arrow => self.arrow_toolbar_ui(ui),
                Tool::Shape => {
                    for k in [ShapeKind::Rect, ShapeKind::RoundRect, ShapeKind::Ellipse, ShapeKind::Brackets, ShapeKind::Parens] {
                        let (resp, painter) = ui.allocate_painter(vec2(34.0, 22.0), Sense::click());
                        let on = self.shape_kind == k;
                        painter.rect_filled(resp.rect, 4.0, if on { Color32::from_rgb(200, 220, 250) } else if resp.hovered() { Color32::from_rgb(225, 225, 228) } else { Color32::TRANSPARENT });
                        icons::shape_icon(&painter, resp.rect, k);
                        if resp.on_hover_text(format!("{:?}", k)).clicked() {
                            self.shape_kind = k;
                        }
                    }
                    self.bracket_index_ui(ui);
                }
                Tool::Element => {
                    for z in [6u8, 7, 8, 16, 15, 9, 17, 35, 53, 1, 14, 5] {
                        if ui.selectable_label(self.element == z, elements::symbol(z)).clicked() {
                            self.element = z;
                        }
                    }
                    if ui.button("Table…").clicked() {
                        self.tab = Tab::Elements;
                        self.show_panel = true;
                    }
                }
                Tool::Template => {
                    ui.label("Template:");
                    let name = self.current_template().map(|t| t.name.clone()).unwrap_or_else(|| "none chosen".into());
                    ui.label(egui::RichText::new(name).strong());
                    if ui.button("Choose…").clicked() {
                        self.template_window = Some(self.template.map(|t| t.0).unwrap_or(0));
                    }
                }
                Tool::Text => {
                    ui.label("Caption:");
                    ui.add(egui::DragValue::new(&mut self.caption_size).range(6.0..=48.0).suffix(" pt"));
                    ui.toggle_value(&mut self.caption_bold, egui::RichText::new("B").strong());
                    ui.toggle_value(&mut self.caption_italic, egui::RichText::new("I").italics());
                    ui.toggle_value(&mut self.caption_formula, "Formula").on_hover_text("Numbers after letters become subscripts");
                }
                _ => {
                    ui.label(egui::RichText::new(self.tool.name()).color(Color32::DARK_GRAY));
                    if self.sel.shapes.iter().any(|&i| self.doc.shapes.get(i).is_some_and(|s| s.is_bracket())) {
                        self.bracket_index_ui(ui);
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.selectable_label(self.show_panel, "Side Panel").clicked() {
                    self.show_panel = !self.show_panel;
                }
                ui.separator();
                for (name, c) in COLORS.iter().rev() {
                    let (resp, painter) = ui.allocate_painter(vec2(16.0, 16.0), Sense::click());
                    let col = c.map(|c| Color32::from_rgb(c[0], c[1], c[2])).unwrap_or(Color32::BLACK);
                    painter.rect_filled(resp.rect.shrink(1.0), 3.0, col);
                    if resp.on_hover_text(format!("Colour selection: {}", name)).clicked() {
                        self.apply_color(*c);
                    }
                }
                ui.label("Colour:");
            });
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&self.status).size(12.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(p) = self.hover_pos {
                    ui.label(egui::RichText::new(format!("{:.0}, {:.0}", p.x, p.y)).size(11.0).color(Color32::GRAY));
                }
                ui.label(egui::RichText::new(self.tool.hint()).size(11.0).color(Color32::DARK_GRAY));
            });
        });
    }

    fn palette(&mut self, ui: &mut egui::Ui) {
        let size = vec2(32.0, 30.0);
        let sym = elements::symbol(self.element).to_string();
        egui::Grid::new("palette").spacing(vec2(2.0, 2.0)).show(ui, |ui| {
            for (k, t) in icons::PALETTE.iter().enumerate() {
                let (resp, painter) = ui.allocate_painter(size, Sense::click());
                let on = self.tool == *t;
                let bg = if on {
                    Color32::from_rgb(200, 220, 250)
                } else if resp.hovered() {
                    Color32::from_rgb(222, 222, 226)
                } else {
                    Color32::from_rgb(248, 248, 249)
                };
                painter.rect_filled(resp.rect, 4.0, bg);
                if on {
                    painter.rect_stroke(resp.rect, 4.0, Stroke::new(1.0, ACCENT), egui::StrokeKind::Inside);
                }
                icons::draw(&painter, resp.rect, *t, &sym, Color32::from_rgb(30, 30, 30));
                if matches!(*t, Tool::Template | Tool::Arrow) {
                    // A small corner mark: this button opens a list.
                    let c = resp.rect.right_bottom() - vec2(3.0, 3.0);
                    painter.add(egui::Shape::convex_polygon(vec![c, c - vec2(5.0, 0.0), c - vec2(0.0, 5.0)], Color32::from_gray(60), Stroke::NONE));
                }
                // The Templates button always opens the template window (on the last group used).
                if *t == Tool::Template && resp.clicked() {
                    self.open_template_window();
                }
                // A second click on the active arrow button opens the grid of styles.
                if *t == Tool::Arrow && resp.clicked() && self.tool == Tool::Arrow {
                    self.arrow_menu = !self.arrow_menu;
                    self.arrow_menu_fresh = true;
                    self.arrow_menu_at = resp.rect.right_top() + vec2(4.0, 0.0);
                }
                if resp.on_hover_text(t.name()).clicked() {
                    self.commit_edit();
                    self.tool = *t;
                    self.hotspot = None;
                    if !matches!(t, Tool::Lasso | Tool::Marquee) {
                        self.sel.clear();
                    }
                }
                if k % 2 == 1 {
                    ui.end_row();
                }
            }
        });
    }

    fn elements_ui(&mut self, ui: &mut egui::Ui) {
        ui.label("Click an element to use it with the Atom Label tool. If atoms are selected, they change to it.");
        ui.add_space(6.0);
        let cell = ((ui.available_width() - 4.0) / 18.0).clamp(12.0, 26.0);
        let (rect, _) = ui.allocate_exact_size(vec2(cell * 18.0, cell * 9.6), Sense::hover());
        let painter = ui.painter_at(rect);
        let mut clicked = None;
        for z in 1..=118u8 {
            let (r, c) = elements::grid_pos(z);
            let y = if r >= 7 { rect.min.y + (r as f32 + 0.5) * cell } else { rect.min.y + r as f32 * cell };
            let cr = Rect::from_min_size(pos2(rect.min.x + c as f32 * cell, y), vec2(cell, cell)).shrink(1.0);
            let resp = ui.interact(cr, ui.id().with(("pt", z)), Sense::click());
            let base = match elements::category(z) {
                0 => Color32::from_rgb(200, 235, 200),
                1 => Color32::from_rgb(220, 210, 245),
                2 => Color32::from_rgb(250, 210, 200),
                3 => Color32::from_rgb(250, 230, 200),
                4 => Color32::from_rgb(220, 230, 200),
                5 => Color32::from_rgb(250, 245, 190),
                6 => Color32::from_rgb(240, 215, 235),
                7 => Color32::from_rgb(235, 205, 225),
                8 => Color32::from_rgb(215, 225, 235),
                _ => Color32::from_rgb(230, 225, 215),
            };
            let fill = if self.element == z { Color32::from_rgb(120, 170, 250) } else if resp.hovered() { Color32::from_rgb(255, 255, 255) } else { base };
            painter.rect_filled(cr, 2.0, fill);
            painter.text(cr.center(), egui::Align2::CENTER_CENTER, elements::symbol(z), egui::FontId::proportional(cell * 0.45), Color32::BLACK);
            let e = &elements::ELEMENTS[z as usize];
            if resp.on_hover_text(format!("{} ({})\nAtomic number {}\nAtomic weight {:.3}", e.name, e.sym, z, e.mass)).clicked() {
                clicked = Some(z);
            }
        }
        if let Some(z) = clicked {
            self.element = z;
            self.tool = Tool::Element;
            if !self.sel.atoms.is_empty() {
                self.checkpoint();
                let s: Vec<usize> = self.sel.atoms.iter().copied().collect();
                for i in s {
                    self.set_element_on(i, z);
                }
            }
        }
        ui.add_space(6.0);
        let e = &elements::ELEMENTS[self.element as usize];
        ui.label(egui::RichText::new(format!("{} — {}", e.sym, e.name)).strong());
        ui.label(format!("Atomic number: {}\nAtomic weight: {:.4}\nMost abundant isotope: {:.6}", self.element, e.mass, e.mono));
    }

    fn names_ui(&mut self, ui: &mut egui::Ui, _ctx: &egui::Context) {
        ui.label(egui::RichText::new("Name to Structure").strong());
        ui.label("Type a name (e.g. caffeine, 2-methylbutane, cyclohexane) or SMILES.");
        let r = ui.add(egui::TextEdit::singleline(&mut self.name_input).hint_text("name").desired_width(f32::INFINITY));
        let go = ui.button("Insert Structure").clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)));
        if go && !self.name_input.trim().is_empty() {
            match names::name_to_structure(&self.name_input).or_else(|| parse_alkane_name(&self.name_input)) {
                Some(d) => {
                    let n = self.name_input.clone();
                    self.insert_doc(d, &format!("Inserted {}", n));
                }
                None => self.status("Couldn't read that name: it isn't in the built-in list of known compounds and isn't a systematic name Formula can read (chains, single rings, benzene and common groups). Try the systematic name or SMILES."),
            }
        }
        ui.add_space(12.0);
        ui.label(egui::RichText::new("SMILES").strong());
        ui.add(egui::TextEdit::multiline(&mut self.smiles_input).hint_text("e.g. CC(=O)Oc1ccccc1C(O)=O").desired_rows(2).desired_width(f32::INFINITY));
        if ui.button("Insert SMILES").clicked() && !self.smiles_input.trim().is_empty() {
            match formats::parse_smiles(&self.smiles_input) {
                Ok(d) => self.insert_doc(d, "Inserted structure from SMILES"),
                Err(e) => self.status(format!("Couldn't read that SMILES: {}", e)),
            }
        }
        ui.add_space(12.0);
        ui.label(egui::RichText::new("Structure to Name").strong());
        let name = self.analysis.lines.iter().map(|f| f.name.clone().unwrap_or("(no name found)".into())).collect::<Vec<_>>().join("\n");
        ui.label(if name.is_empty() { "Draw or select a structure.".to_string() } else { name });
        ui.add_space(12.0);
        ui.label(egui::RichText::new("Templates").strong());
        ui.horizontal_wrapped(|ui| {
            for (n, _) in names::COMMON.iter().filter(|(n, _)| {
                ["benzene", "naphthalene", "indole", "pyridine", "cyclohexanone", "glucose", "alanine", "caffeine", "aspirin", "cholesterol", "anthracene", "purine"].contains(n)
            }) {
                if ui.small_button(*n).clicked() {
                    if let Some(d) = names::name_to_structure(n) {
                        self.insert_doc(d, &format!("Inserted {}", n));
                    }
                }
            }
        });
    }

    /// The list of ready-made drawing settings, plus "Other…" to copy them from a file.
    fn style_menu(&mut self, ui: &mut egui::Ui) {
        if ui.button("Other…").on_hover_text("Use the drawing settings of another document or stationery file").clicked() {
            ui.close();
            let f = rfd::FileDialog::new()
                .add_filter("CDX and CDXML documents and stationery", &["cdx", "cdxml", "ctp"])
                .pick_file();
            if let Some(p) = f {
                match read_any(&p) {
                    Ok(d) => {
                        let mut st = d.style.clone();
                        st.name = file_name(&p);
                        self.apply_style(st);
                    }
                    Err(e) => self.status(format!("Could not read {}: {}", file_name(&p), e)),
                }
            }
        }
        ui.separator();
        for p in styles::PRESETS {
            let on = self.doc.style.name == p.name;
            if ui.add(egui::Button::selectable(on, p.name)).clicked() {
                ui.close();
                self.apply_style(styles::to_style(p, self.doc.style.show_warnings));
            }
        }
    }

    fn apply_style(&mut self, st: Style) {
        self.checkpoint();
        let name = st.name.clone();
        styles::apply(&mut self.doc, st);
        self.status(format!("Applied the “{}” drawing settings", name));
    }

    fn style_ui(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new("Drawing settings").strong());
        ui.horizontal(|ui| {
            ui.label("Style:");
            let current = self.doc.style.name.clone();
            egui::ComboBox::from_id_salt("style-preset").selected_text(current).width(200.0).show_ui(ui, |ui| {
                for p in styles::PRESETS {
                    if ui.selectable_label(self.doc.style.name == p.name, p.name).clicked() {
                        self.apply_style(styles::to_style(p, self.doc.style.show_warnings));
                    }
                }
            });
        });
        ui.add_space(4.0);
        let st = &mut self.doc.style;
        let mut changed = false;
        // Shown in printed points; stored in drawing units.
        let k = st.bond_length.max(1.0) / BOND_LEN;
        let pt = |ui: &mut egui::Ui, label: &str, v: &mut f32, lo: f32, hi: f32, changed: &mut bool| {
            ui.label(label);
            let mut x = *v * k;
            if ui.add(egui::DragValue::new(&mut x).range(lo..=hi).speed(0.05).max_decimals(2).suffix(" pt")).changed() {
                *v = x / k;
                *changed = true;
            }
            ui.end_row();
        };
        egui::Grid::new("style").num_columns(2).show(ui, |ui| {
            ui.label("Bond length");
            let mut bl = st.bond_length;
            if ui.add(egui::DragValue::new(&mut bl).range(5.0..=100.0).speed(0.1).max_decimals(2).suffix(" pt")).changed() {
                // Keep printed widths and fonts the same size on paper.
                let r = st.bond_length / bl;
                for v in [&mut st.line_width, &mut st.bold_width, &mut st.hash_spacing, &mut st.margin, &mut st.wedge_width, &mut st.label_size, &mut st.caption_size] {
                    *v *= r;
                }
                st.bond_length = bl;
                changed = true;
            }
            ui.end_row();
            pt(ui, "Line width", &mut st.line_width, 0.1, 6.0, &mut changed);
            pt(ui, "Bold width", &mut st.bold_width, 0.2, 10.0, &mut changed);
            pt(ui, "Hash spacing", &mut st.hash_spacing, 0.5, 10.0, &mut changed);
            pt(ui, "Margin width", &mut st.margin, 0.0, 10.0, &mut changed);
            ui.label("Double bond spacing");
            let mut pc = st.dbl_spacing * 100.0;
            if ui.add(egui::DragValue::new(&mut pc).range(8.0..=40.0).speed(0.2).max_decimals(1).suffix(" % of length")).changed() {
                st.dbl_spacing = pc / 100.0;
                changed = true;
            }
            ui.end_row();
            pt(ui, "Atom label size", &mut st.label_size, 3.0, 40.0, &mut changed);
            pt(ui, "Caption size", &mut st.caption_size, 3.0, 40.0, &mut changed);
            ui.label("Label font");
            ui.horizontal(|ui| {
                changed |= ui.radio_value(&mut st.label_font, LabelFont::Sans, "Sans-serif").changed();
                changed |= ui.radio_value(&mut st.label_font, LabelFont::Serif, "Serif").changed();
            });
            ui.end_row();
            ui.label("Caption font");
            ui.horizontal(|ui| {
                changed |= ui.radio_value(&mut st.caption_font, LabelFont::Sans, "Sans-serif").changed();
                changed |= ui.radio_value(&mut st.caption_font, LabelFont::Serif, "Serif").changed();
            });
            ui.end_row();
        });
        changed |= ui.checkbox(&mut st.label_bold, "Bold atom labels").changed();
        changed |= ui.checkbox(&mut st.hide_implicit_h, "Hide implicit hydrogens").changed();
        changed |= ui.checkbox(&mut st.show_carbons, "Show carbon labels").changed();
        changed |= ui.checkbox(&mut st.show_terminal_carbons, "Show terminal carbon labels").changed();
        changed |= ui.checkbox(&mut st.dark, "Dark background").changed();
        changed |= ui.checkbox(&mut st.show_warnings, "Show chemical warnings").changed();
        if changed {
            st.name = "Custom".into();
            self.dirty = true;
        }
        self.page_settings_ui(ui);
        ui.add_space(10.0);
        ui.label(egui::RichText::new("This document").strong());
        ui.label(format!("{} atoms, {} bonds, {} arrows, {} captions, {} shapes", self.doc.atoms.len(), self.doc.bonds.len(), self.doc.arrows.len(), self.doc.captions.len(), self.doc.shapes.len()));
        if let Some(f) = &self.doc.file {
            let kept = f.cdx_page_children.len() + f.xml_page_children.len() + f.cdx_doc_children.len() + f.xml_doc_children.len();
            ui.label(format!("{} item(s) from the opened file are kept unchanged for saving (e.g. tables, spectra or plates this app doesn't show).", kept));
        }
        ui.add_space(10.0);
        ui.label(egui::RichText::new("Not included").strong());
        ui.label("Online features run by the original vendor (cloud storage and sharing, account sign-in, online name and property services) are left out. Everything here works offline.");
    }

    // -------------------------------------------------------------------
    // Canvas

    fn canvas_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let full = ui.available_rect_before_wrap();
        let exporting = self.export_pending.is_some();
        let rulers = self.show_rulers && !exporting;
        // Rulers take a strip along the top and left; the sheet is what is left.
        let rect = if rulers { Rect::from_min_max(full.min + vec2(arrange::RULER_W, arrange::RULER_H), full.max) } else { full };
        self.canvas = rect;
        // Frame the drawing during the first few frames, while panel sizes settle.
        if !self.view_initialized && rect.is_positive() {
            self.init_frames += 1;
            if self.init_frames >= 4 {
                self.view_initialized = true;
            }
            if self.doc.is_empty() {
                self.zoom = 1.5;
                // The page's top left corner near the window's, as on paper.
                self.pan = if self.show_page { vec2(40.0, 40.0) } else { rect.size() / 2.0 };
            } else {
                self.fit_view();
                if self.zoom > 2.0 {
                    let c = rect.center();
                    self.set_zoom(2.0, c);
                }
            }
        }
        let resp = ui.allocate_rect(rect, Sense::click_and_drag());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, self.doc.style.paper());
        if self.show_page && !exporting {
            arrange::draw_pages(&painter, rect, &self.doc, rect.min + self.pan, self.zoom);
        }
        let upm = arrange::units_per_mm(&self.doc);
        if self.show_grid && !exporting {
            arrange::draw_grid(&painter, rect, rect.min + self.pan, self.zoom, upm);
        }
        if self.show_crosshair && !exporting {
            arrange::draw_crosshair(&painter, rect, rect.min + self.pan);
        }

        // Pan and zoom.
        if resp.hovered() {
            let (scroll, zoom_d, mods) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta(), i.modifiers));
            if zoom_d != 1.0 {
                if let Some(p) = resp.hover_pos() {
                    self.set_zoom(self.zoom * zoom_d, p);
                }
            } else if scroll != Vec2::ZERO && !mods.command {
                self.pan += scroll;
            }
        }

        let pointer = resp.hover_pos().or(ctx.pointer_interact_pos());
        let doc_p = pointer.map(|p| self.to_doc(p));
        self.hover_pos = if resp.hovered() { doc_p } else { None };
        if self.drag.is_none() {
            self.hover = if resp.hovered() { doc_p.and_then(|p| self.doc.hit(p, self.tol())) } else { None };
        }

        // Any click or drag in the drawing ends hot-key typing: the green hotspot dot goes away
        // (only hot keys set it).
        if resp.clicked() || resp.drag_started() || resp.secondary_clicked() || resp.double_clicked() {
            self.hotspot = None;
        }
        // Right-click on an atom: its menu (isotope, charge, label).
        if resp.secondary_clicked() {
            self.ctx_atom = match self.hover {
                Some(Hit::Atom(i)) => Some(i),
                _ => None,
            };
        }
        if self.ctx_atom.is_some() {
            resp.context_menu(|ui| self.atom_context_menu(ui));
        }
        self.handle_pointer(&resp, ctx);
        // A click may have just removed what the pointer was over (eraser, undo, a template...).
        if self.drag.is_none() && self.hover.is_some() {
            self.hover = if resp.hovered() { doc_p.and_then(|p| self.doc.hit(p, self.tol())) } else { None };
        }
        if self.drag.is_none() {
            self.drag_origin = None;
        } else if self.drag_origin.is_none() {
            self.drag_origin = ctx.input(|i| i.pointer.press_origin()).map(|p| self.to_doc(p));
        }

        // Draw.
        let mut cv = EguiCanvas { painter: &painter, origin: rect.min + self.pan, zoom: self.zoom };
        if !exporting {
            self.draw_selection(&mut cv);
        }
        let opts = RenderOpts { color_atoms: self.color_atoms, warnings: !exporting };
        render::draw_doc(&self.doc, &mut cv, &opts);
        if !exporting {
            self.draw_hover(&mut cv);
            self.draw_drag_preview(&mut cv);
        }
        if !exporting {
            self.draw_rotate_handle(&painter, pointer.filter(|_| resp.hovered()));
            self.draw_template_ghost(&painter);
            self.draw_nmr_labels(&painter);
        }
        if rulers {
            let top = Rect::from_min_max(pos2(rect.min.x, full.min.y), pos2(full.max.x, rect.min.y));
            let left = Rect::from_min_max(pos2(full.min.x, rect.min.y), pos2(rect.min.x, full.max.y));
            let ruler_painter = ui.painter_at(full);
            arrange::draw_rulers(&ruler_painter, top, left, rect.min + self.pan, self.zoom, upm, pointer.filter(|_| resp.hovered() || self.drag.is_some()));
        }
        if self.doc.is_empty() && !exporting {
            painter.text(
                rect.center() + vec2(0.0, -rect.height() * 0.3),
                egui::Align2::CENTER_CENTER,
                "Pick a tool on the left and click or drag here to draw.",
                egui::FontId::proportional(14.0),
                Color32::from_gray(170),
            );
        }
        self.edit_overlay(ui, ctx);
    }

    fn handle_pointer(&mut self, resp: &egui::Response, ctx: &egui::Context) {
        let mods = ctx.input(|i| i.modifiers);
        let tol = self.tol();
        // Secondary/middle drag pans.
        if resp.dragged_by(egui::PointerButton::Middle) || resp.dragged_by(egui::PointerButton::Secondary) {
            self.pan += resp.drag_delta();
            return;
        }
        if resp.drag_started_by(egui::PointerButton::Primary) {
            if self.editing.is_some() {
                self.commit_edit();
            }
            let origin = ctx.input(|i| i.pointer.press_origin()).unwrap_or(resp.interact_pointer_pos().unwrap_or_default());
            let start = self.to_doc(origin);
            let hit = self.doc.hit(start, tol);
            let on_handle = self.rotate_handle().is_some_and(|h| (h - origin).length() <= 10.0);
            let pivot = if on_handle { arrange::rotation_pivot(&self.doc, &self.sel) } else { None };
            self.drag = if let Some(pivot) = pivot {
                self.checkpoint();
                Some(Drag::Rotate { pivot, start_ang: angle_of(start - pivot), orig: Box::new(self.doc.clone()), angle: 0.0 })
            } else { match self.tool {
                Tool::Lasso | Tool::Marquee => match hit {
                    Some(h) if self.hit_selected(h) => {
                        self.checkpoint();
                        Some(Drag::Move { last: start })
                    }
                    Some(h) => {
                        self.select_hit(h, mods.shift);
                        self.checkpoint();
                        Some(Drag::Move { last: start })
                    }
                    None => {
                        if self.tool == Tool::Lasso {
                            Some(Drag::Lasso { pts: vec![start] })
                        } else {
                            Some(Drag::Marquee { start, cur: start })
                        }
                    }
                },
                Tool::Bond(..) => {
                    let from = self.doc.atom_at(start, tol * 1.4, None);
                    let s = from.map(|i| self.doc.atoms[i].pos).unwrap_or(start);
                    Some(Drag::Bond { from_atom: from, start: s, cur: start })
                }
                Tool::Chain => {
                    let from = self.doc.atom_at(start, tol * 1.4, None);
                    let s = from.map(|i| self.doc.atoms[i].pos).unwrap_or(start);
                    Some(Drag::Chain { from_atom: from, start: s, cur: start })
                }
                Tool::Arrow => Some(Drag::Arrow { start, cur: start }),
                Tool::Shape => Some(Drag::Shape { start, cur: start }),
                Tool::Eraser => {
                    self.checkpoint();
                    self.erase_last = None;
                    Some(Drag::Erase)
                }
                _ => match hit {
                    Some(h) if self.hit_selected(h) => {
                        self.checkpoint();
                        Some(Drag::Move { last: start })
                    }
                    _ => Some(Drag::Pan),
                },
            } };
        }
        if resp.dragged_by(egui::PointerButton::Primary) {
            if let Some(p) = resp.interact_pointer_pos() {
                let cur = self.to_doc(p);
                let delta = resp.drag_delta();
                let mut erase = None;
                let mut turn = None;
                match &mut self.drag {
                    Some(Drag::Rotate { pivot, start_ang, angle, .. }) => {
                        let mut a = norm_angle(angle_of(cur - *pivot) - *start_ang);
                        if a > std::f32::consts::PI {
                            a -= std::f32::consts::TAU;
                        }
                        // Shift snaps to 15 degree steps.
                        if mods.shift {
                            let step = 15f32.to_radians();
                            a = (a / step).round() * step;
                        }
                        *angle = a;
                        turn = Some(a);
                    }
                    Some(Drag::Bond { cur: c, .. }) | Some(Drag::Chain { cur: c, .. }) | Some(Drag::Marquee { cur: c, .. }) | Some(Drag::Arrow { cur: c, .. }) | Some(Drag::Shape { cur: c, .. }) => *c = cur,
                    Some(Drag::Lasso { pts }) => {
                        if pts.last().map(|l| (*l - cur).length() > 2.0 / self.zoom).unwrap_or(true) {
                            pts.push(cur);
                        }
                    }
                    Some(Drag::Move { last }) => {
                        let d = cur - *last;
                        *last = cur;
                        let s = self.sel.clone();
                        self.doc.translate(&s, d);
                    }
                    Some(Drag::Erase) => {
                        let h = self.doc.hit(cur, tol);
                        // Erase each thing once per stroke: an atom whose charge was just
                        // removed isn't deleted by the same drag.
                        if h.is_some() && h != self.erase_last {
                            erase = h;
                            self.erase_last = h;
                        }
                    }
                    Some(Drag::Pan) => self.pan += delta,
                    None => {}
                }
                if let Some(h) = erase {
                    self.erase_hit(h);
                }
                if let (Some(a), Some(Drag::Rotate { pivot, orig, .. })) = (turn, &self.drag) {
                    let (pivot, mut d) = (*pivot, (**orig).clone());
                    arrange::rotate(&mut d, &self.sel, pivot, a.to_degrees());
                    self.doc = d;
                    self.status = format!("Rotating {}°  (hold Shift for 15° steps)", fmt_deg(a.to_degrees()));
                }
            }
        }
        if resp.drag_stopped() {
            if let Some(d) = self.drag.take() {
                self.finish_drag(d, mods.shift);
            }
        }
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                if self.editing.is_some() {
                    self.commit_edit();
                    return;
                }
                let dp = self.to_doc(p);
                let hit = self.doc.hit(dp, tol);
                self.click(hit, dp, mods.shift);
            }
        }
        if resp.double_clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                let dp = self.to_doc(p);
                match self.doc.hit(dp, tol) {
                    Some(Hit::Atom(i)) if matches!(self.tool, Tool::Lasso | Tool::Marquee) => {
                        self.sel.clear();
                        self.sel.atoms = self.doc.fragment_of(i).into_iter().collect();
                    }
                    Some(Hit::Bond(b)) if matches!(self.tool, Tool::Lasso | Tool::Marquee) => {
                        self.sel.clear();
                        self.sel.atoms = self.doc.fragment_of(self.doc.bonds[b].a).into_iter().collect();
                    }
                    Some(Hit::Caption(i)) if self.doc.captions[i].seq.is_some() => self.open_sequence(i),
                    Some(Hit::Caption(i)) => {
                        let text = self.doc.captions[i].text.clone();
                        self.editing = Some(Editing::Caption { idx: i, text, fresh: false });
                        self.edit_focus = true;
                    }
                    Some(Hit::Atom(i)) => {
                        let text = current_label(&self.doc, i);
                        self.editing = Some(Editing::Label { atom: i, text, fresh: false });
                        self.edit_focus = true;
                    }
                    _ => {}
                }
            }
        }
    }

    fn click(&mut self, hit: Option<Hit>, p: Pos2, shift: bool) {
        match self.tool {
            Tool::Lasso | Tool::Marquee => match hit {
                Some(h) => self.select_hit(h, shift),
                None => self.sel.clear(),
            },
            Tool::Bond(o, s) => self.click_bond_tool(o, s, hit, p),
            Tool::Template => self.place_template(p),
            Tool::Chain => {
                if let Some(Hit::Atom(_)) = hit {
                    self.click_bond_tool(1, Stereo::None, hit, p);
                }
            }
            Tool::Ring(n, arom) => {
                self.checkpoint();
                let anchor = match hit {
                    Some(Hit::Atom(i)) => RingAnchor::Atom(i),
                    Some(Hit::Bond(b)) => RingAnchor::Bond(b),
                    _ => RingAnchor::Point(p),
                };
                self.doc.add_ring(n, arom, anchor);
            }
            Tool::Eraser => {
                // A click on a + / − sign beside an atom removes that charge.
                let hit = match hit {
                    Some(Hit::Atom(i)) => Some(Hit::Atom(i)),
                    other => self.charge_sign_at(p).map(Hit::Atom).or(other),
                };
                if let Some(h) = hit {
                    self.checkpoint();
                    self.erase_hit(h);
                }
            }
            Tool::Text => match hit {
                Some(Hit::Atom(i)) => {
                    let text = current_label(&self.doc, i);
                    self.editing = Some(Editing::Label { atom: i, text, fresh: false });
                    self.edit_focus = true;
                }
                Some(Hit::Caption(i)) if self.doc.captions[i].seq.is_some() => self.open_sequence(i),
                Some(Hit::Caption(i)) => {
                    let text = self.doc.captions[i].text.clone();
                    self.editing = Some(Editing::Caption { idx: i, text, fresh: false });
                    self.edit_focus = true;
                }
                _ => {
                    self.checkpoint();
                    self.doc.captions.push(Caption {
                        pos: p - vec2(0.0, self.caption_size * 0.6),
                        text: String::new(),
                        size: self.caption_size,
                        bold: self.caption_bold,
                        italic: self.caption_italic,
                        serif: self.doc.style.caption_font == LabelFont::Serif,
                        formula: self.caption_formula,
                        color: None,
                        extra: None,
                        seq: None,
                    });
                    let idx = self.doc.captions.len() - 1;
                    self.editing = Some(Editing::Caption { idx, text: String::new(), fresh: true });
                    self.edit_focus = true;
                }
            },
            Tool::Charge(c) => {
                if let Some(Hit::Atom(i)) = hit {
                    self.checkpoint();
                    let a = &mut self.doc.atoms[i];
                    a.charge = (a.charge + c).clamp(-9, 9);
                }
            }
            Tool::Radical => {
                if let Some(Hit::Atom(i)) = hit {
                    self.checkpoint();
                    let a = &mut self.doc.atoms[i];
                    a.radical = (a.radical + 1) % 3;
                }
            }
            Tool::Element => match hit {
                Some(Hit::Atom(i)) => {
                    self.checkpoint();
                    let z = self.element;
                    self.set_element_on(i, z);
                }
                None => {
                    self.checkpoint();
                    self.doc.add_atom(p, self.element);
                }
                _ => {}
            },
            Tool::Arrow => {
                self.checkpoint();
                self.doc.arrows.push(Arrow { from: p, to: p + vec2(BOND_LEN * 2.5, 0.0), kind: self.arrow_kind, color: None, extra: None, style: self.arrow_style });
            }
            Tool::Shape => {}
        }
    }

    fn finish_drag(&mut self, d: Drag, shift: bool) {
        match d {
            Drag::Bond { from_atom, start, cur } => {
                if (cur - start).length() < 4.0 / self.zoom {
                    return;
                }
                let Tool::Bond(order, stereo) = self.tool else { return };
                let (end, end_atom) = self.bond_drag_end(from_atom, start, cur);
                self.checkpoint();
                let a = from_atom.unwrap_or_else(|| self.doc.atom_or_new(start));
                let b = end_atom.unwrap_or_else(|| self.doc.atom_or_new(end));
                if let Some(bi) = self.doc.find_bond(a, b) {
                    self.bump_bond(bi, order, stereo);
                } else {
                    self.doc.add_bond(a, b, order, stereo);
                }
            }
            Drag::Chain { from_atom, start, cur } => {
                if (cur - start).length() < BOND_LEN * 0.5 {
                    return;
                }
                self.checkpoint();
                let pts = Doc::chain_points(start, cur);
                let mut prev = from_atom.unwrap_or_else(|| self.doc.atom_or_new(pts[0]));
                for p in &pts[1..] {
                    let n = self.doc.atom_or_new(*p);
                    self.doc.add_bond(prev, n, 1, Stereo::None);
                    prev = n;
                }
            }
            Drag::Marquee { start, cur } => {
                let r = Rect::from_two_pos(start, cur);
                self.select_in(|p| r.contains(p), shift);
            }
            Drag::Lasso { pts } => {
                if pts.len() > 2 {
                    self.select_in(|p| point_in_poly(p, &pts), shift);
                }
            }
            Drag::Arrow { start, cur } => {
                if (cur - start).length() > 4.0 / self.zoom {
                    self.checkpoint();
                    // With Fixed Angles on, arrows snap to 15° steps.
                    let d = cur - start;
                    let step = 15f32.to_radians();
                    let end = if self.fixed_angles {
                        let snapped = (d.y.atan2(d.x) / step).round() * step;
                        start + vec2(snapped.cos(), snapped.sin()) * d.length()
                    } else {
                        cur
                    };
                    self.doc.arrows.push(Arrow { from: start, to: end, kind: self.arrow_kind, color: None, extra: None, style: self.arrow_style });
                }
            }
            Drag::Shape { start, cur } => {
                let r = Rect::from_two_pos(start, cur);
                if r.width() > 3.0 && r.height() > 3.0 {
                    self.checkpoint();
                    self.doc.shapes.push(Shape { label: self.new_bracket_label(), ..Shape::new(r.min, r.max, self.shape_kind) });
                }
            }
            Drag::Move { .. } => {
                // Merge moved atoms that landed on others (e.g. joining two structures).
                self.merge_overlaps();
            }
            Drag::Rotate { angle, .. } => {
                let deg = angle.to_degrees();
                self.status(format!("Rotated {}° {}", fmt_deg(deg.abs()), if deg >= 0.0 { "clockwise" } else { "counter-clockwise" }));
            }
            Drag::Erase | Drag::Pan => {}
        }
    }

    fn merge_overlaps(&mut self) {
        let moved: Vec<usize> = self.sel.atoms.iter().copied().collect();
        let mut pairs = Vec::new();
        for &i in &moved {
            for j in 0..self.doc.atoms.len() {
                if self.sel.atoms.contains(&j) {
                    continue;
                }
                if (self.doc.atoms[i].pos - self.doc.atoms[j].pos).length() < BOND_LEN * 0.12 {
                    pairs.push((i, j));
                    break;
                }
            }
        }
        if pairs.is_empty() {
            return;
        }
        for &(i, j) in &pairs {
            // Move i's bonds to j.
            for b in &mut self.doc.bonds {
                if b.a == i {
                    b.a = j;
                }
                if b.b == i {
                    b.b = j;
                }
            }
        }
        // Remove duplicates and self bonds.
        let mut seen = HashSet::new();
        self.doc.bonds.retain(|b| b.a != b.b && seen.insert(if b.a < b.b { (b.a, b.b) } else { (b.b, b.a) }));
        let set: HashSet<usize> = pairs.iter().map(|p| p.0).collect();
        self.doc.remove_atoms(&set);
        self.sel.clear();
        self.status("Joined atoms");
    }

    fn draw_selection(&self, cv: &mut EguiCanvas) {
        if self.sel.is_empty() {
            return;
        }
        let p = cv.painter;
        for b in &self.doc.bonds {
            if self.sel.atoms.contains(&b.a) && self.sel.atoms.contains(&b.b) {
                p.line_segment([cv.to_screen(self.doc.atoms[b.a].pos), cv.to_screen(self.doc.atoms[b.b].pos)], Stroke::new(7.0 * self.zoom.sqrt(), SEL_FILL));
            }
        }
        for &i in &self.sel.atoms {
            p.circle_filled(cv.to_screen(self.doc.atoms[i].pos), 5.0 * self.zoom.sqrt(), SEL_FILL);
        }
        for &i in &self.sel.arrows {
            let a = &self.doc.arrows[i];
            p.line_segment([cv.to_screen(a.from), cv.to_screen(a.to)], Stroke::new(8.0, SEL_FILL));
        }
        for &i in &self.sel.captions {
            let c = &self.doc.captions[i];
            let size = render::caption_size(c, cv);
            p.rect_filled(Rect::from_min_size(cv.to_screen(c.pos), size * self.zoom).expand(2.0), 2.0, SEL_FILL);
        }
        for &i in &self.sel.shapes {
            let s = &self.doc.shapes[i];
            p.rect_stroke(Rect::from_two_pos(cv.to_screen(s.min), cv.to_screen(s.max)).expand(3.0), 2.0, Stroke::new(4.0, SEL_FILL), egui::StrokeKind::Middle);
        }
        if let Some(b) = self.doc.bounds_of(Some(&self.sel)) {
            let r = Rect::from_two_pos(cv.to_screen(b.min), cv.to_screen(b.max)).expand(8.0);
            let pts = vec![r.min, pos2(r.max.x, r.min.y), r.max, pos2(r.min.x, r.max.y), r.min];
            p.add(egui::Shape::dashed_line(&pts, Stroke::new(1.0, ACCENT), 4.0, 3.0));
        }
    }

    /// The round handle used to turn a selection, and the centre it turns around.
    fn draw_rotate_handle(&self, painter: &egui::Painter, pointer: Option<Pos2>) {
        if let Some(Drag::Rotate { pivot, .. }) = &self.drag {
            let c = self.to_screen(*pivot);
            let s = Stroke::new(1.5, ACCENT);
            painter.line_segment([c - vec2(6.0, 0.0), c + vec2(6.0, 0.0)], s);
            painter.line_segment([c - vec2(0.0, 6.0), c + vec2(0.0, 6.0)], s);
            painter.circle_stroke(c, 3.0, s);
        }
        let Some(h) = self.rotate_handle() else { return };
        let hot = pointer.is_some_and(|p| (p - h).length() <= 10.0) || matches!(self.drag, Some(Drag::Rotate { .. }));
        painter.circle_filled(h, 7.0, if hot { Color32::from_rgb(200, 220, 250) } else { Color32::WHITE });
        painter.circle_stroke(h, 7.0, Stroke::new(1.2, ACCENT));
        // A small curved arrow inside.
        let pts: Vec<Pos2> = (0..=10)
            .map(|k| {
                let a = (-200.0 + 22.0 * k as f32).to_radians();
                h + vec2(a.cos(), a.sin()) * 3.8
            })
            .collect();
        let tip = *pts.last().unwrap();
        painter.add(egui::Shape::line(pts, Stroke::new(1.2, ACCENT)));
        painter.line_segment([tip, tip + vec2(-2.5, -0.5)], Stroke::new(1.2, ACCENT));
        painter.line_segment([tip, tip + vec2(0.5, -2.5)], Stroke::new(1.2, ACCENT));
        if hot && self.drag.is_none() {
            painter.text(h + vec2(10.0, -4.0), egui::Align2::LEFT_BOTTOM, "drag to rotate", egui::FontId::proportional(11.0), Color32::GRAY);
        }
    }

    fn draw_hover(&self, cv: &mut EguiCanvas) {
        // The hotspot: hot keys act here when the pointer is away from the structure.
        if let Some(a) = self.hotspot.and_then(|i| self.doc.atoms.get(i)) {
            let c = cv.to_screen(a.pos);
            cv.painter.circle_filled(c, 3.0 * self.zoom.sqrt(), Color32::from_rgb(40, 180, 70));
        }
        let Some(h) = self.hover else { return };
        if self.drag.is_some() {
            return;
        }
        let p = cv.painter;
        let col = Color32::from_rgb(40, 110, 220);
        match h {
            Hit::Atom(i) => {
                let Some(at) = self.doc.atoms.get(i) else { return };
                let c = cv.to_screen(at.pos);
                let s = 5.0 * self.zoom.sqrt();
                p.rect_stroke(Rect::from_center_size(c, vec2(s * 2.0, s * 2.0)), 1.0, Stroke::new(1.5, col), egui::StrokeKind::Middle);
            }
            Hit::Bond(b) => {
                let Some(bb) = self.doc.bonds.get(b) else { return };
                let (Some(aa), Some(ab)) = (self.doc.atoms.get(bb.a), self.doc.atoms.get(bb.b)) else { return };
                let a = cv.to_screen(aa.pos);
                let e = cv.to_screen(ab.pos);
                let m = a + (e - a) * 0.5;
                let d = (e - a).normalized();
                let n = vec2(-d.y, d.x) * 4.0;
                let l = d * ((e - a).length() * 0.3);
                let pts = vec![m - l + n, m + l + n, m + l - n, m - l - n, m - l + n];
                p.add(egui::Shape::line(pts, Stroke::new(1.5, col)));
            }
            Hit::Caption(i) => {
                let Some(c) = self.doc.captions.get(i) else { return };
                let size = render::caption_size(c, cv);
                p.rect_stroke(Rect::from_min_size(cv.to_screen(c.pos), size * self.zoom).expand(2.0), 2.0, Stroke::new(1.0, col), egui::StrokeKind::Middle);
            }
            Hit::Arrow(i) => {
                let Some(a) = self.doc.arrows.get(i) else { return };
                p.line_segment([cv.to_screen(a.from), cv.to_screen(a.to)], Stroke::new(6.0, Color32::from_rgba_premultiplied(40, 110, 220, 50)));
            }
            Hit::Shape(i) => {
                let Some(s) = self.doc.shapes.get(i) else { return };
                p.rect_stroke(Rect::from_two_pos(cv.to_screen(s.min), cv.to_screen(s.max)).expand(2.0), 2.0, Stroke::new(1.0, col), egui::StrokeKind::Middle);
            }
        }
    }

    fn draw_drag_preview(&self, cv: &mut EguiCanvas) {
        let gray = Color32::from_rgb(90, 140, 230);
        match &self.drag {
            Some(Drag::Bond { from_atom, start, cur }) => {
                let (end, _) = self.bond_drag_end(*from_atom, *start, *cur);
                cv.line(*start, end, 1.5, gray);
                cv.circle(end, 2.0, Some(gray), None);
            }
            Some(Drag::Chain { start, cur, .. }) => {
                let pts = Doc::chain_points(*start, *cur);
                cv.polyline(&pts, 1.5, gray);
                let n = pts.len() - 1;
                let last = pts[n];
                cv.painter.text(cv.to_screen(last) + vec2(8.0, -8.0), egui::Align2::LEFT_BOTTOM, n.to_string(), egui::FontId::proportional(12.0), gray);
            }
            Some(Drag::Marquee { start, cur }) => {
                let r = Rect::from_two_pos(cv.to_screen(*start), cv.to_screen(*cur));
                cv.painter.rect_filled(r, 0.0, Color32::from_rgba_premultiplied(60, 130, 240, 25));
                let pts = vec![r.min, pos2(r.max.x, r.min.y), r.max, pos2(r.min.x, r.max.y), r.min];
                cv.painter.add(egui::Shape::dashed_line(&pts, Stroke::new(1.0, ACCENT), 4.0, 3.0));
            }
            Some(Drag::Lasso { pts }) => {
                let mut s: Vec<Pos2> = pts.iter().map(|p| cv.to_screen(*p)).collect();
                if let Some(f) = s.first().copied() {
                    s.push(f);
                }
                cv.painter.add(egui::Shape::dashed_line(&s, Stroke::new(1.0, ACCENT), 4.0, 3.0));
            }
            Some(Drag::Arrow { start, cur }) => {
                let a = Arrow { from: *start, to: *cur, kind: self.arrow_kind, color: Some([90, 140, 230]), extra: None, style: self.arrow_style };
                render::draw_arrow(&a, cv, self.doc.style.line_width, self.doc.style.ink());
            }
            Some(Drag::Shape { start, cur }) => {
                let r = Rect::from_two_pos(*start, *cur);
                let s = Shape { color: Some([90, 140, 230]), label: self.new_bracket_label(), ..Shape::new(r.min, r.max, self.shape_kind) };
                render::draw_shape(&s, cv, self.doc.style.line_width, self.doc.style.ink(), self.doc.style.label_size);
            }
            _ => {}
        }
    }

    fn edit_overlay(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(ed) = &mut self.editing else { return };
        let (pos, is_label) = match ed {
            Editing::Label { atom, .. } => {
                if *atom >= self.doc.atoms.len() {
                    self.editing = None;
                    return;
                }
                (self.canvas.min + self.pan + self.doc.atoms[*atom].pos.to_vec2() * self.zoom - vec2(10.0, 12.0), true)
            }
            Editing::Caption { idx, .. } => {
                if *idx >= self.doc.captions.len() {
                    self.editing = None;
                    return;
                }
                (self.canvas.min + self.pan + self.doc.captions[*idx].pos.to_vec2() * self.zoom - vec2(4.0, 4.0), false)
            }
        };
        let text = match ed {
            Editing::Label { text, .. } | Editing::Caption { text, .. } => text,
        };
        let mut commit = false;
        let mut cancel = false;
        let focus = std::mem::take(&mut self.edit_focus);
        egui::Area::new(egui::Id::new("editor")).fixed_pos(pos).order(egui::Order::Foreground).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).inner_margin(egui::Margin::same(3)).show(ui, |ui| {
                let te = if is_label {
                    egui::TextEdit::singleline(text).desired_width(90.0).hint_text("e.g. OH")
                } else {
                    egui::TextEdit::multiline(text).desired_width(220.0).desired_rows(2).hint_text("caption text")
                };
                let r = ui.add(te);
                if focus {
                    r.request_focus();
                }
                let enter = ui.input(|i| i.key_pressed(Key::Enter));
                let cmd_enter = ui.input(|i| i.key_pressed(Key::Enter) && i.modifiers.command);
                if ui.input(|i| i.key_pressed(Key::Escape)) {
                    cancel = true;
                } else if is_label && r.lost_focus() && enter {
                    commit = true;
                } else if !is_label && cmd_enter {
                    commit = true;
                }
                if is_label {
                    ui.label(egui::RichText::new("Enter to apply").size(10.0).color(Color32::GRAY));
                } else {
                    ui.label(egui::RichText::new("⌘Enter or click outside to finish").size(10.0).color(Color32::GRAY));
                }
            });
        });
        let _ = ui;
        if cancel {
            if let Some(Editing::Caption { idx, fresh: true, .. }) = &self.editing {
                let i = *idx;
                self.doc.captions.remove(i);
                self.undo.pop();
            }
            self.editing = None;
        } else if commit {
            self.commit_edit();
        }
    }

    // -------------------------------------------------------------------
    // Picture export and screenshots

    fn handle_export(&mut self, ctx: &egui::Context) {
        let Some(p) = &mut self.export_pending else { return };
        match p.stage {
            0 => {
                // Frame the drawing, then capture on the next frame.
                p.stage = 1;
                self.fit_view();
                ctx.request_repaint();
            }
            1 => {
                p.stage = 2;
                let slot = self.shot.clone();
                ctx.request_screenshot(move |img| {
                    *slot.lock().unwrap() = Some(img);
                });
                ctx.request_repaint();
            }
            _ => {
                let img = self.shot.lock().unwrap().take();
                if let Some(img) = img {
                    let ppp = ctx.pixels_per_point();
                    let b = self.doc.bounds_of(None).unwrap_or(Rect::ZERO).expand(BOND_LEN * 0.6);
                    let r = Rect::from_two_pos(self.to_screen(b.min), self.to_screen(b.max)).intersect(self.canvas);
                    let crop = Rect::from_min_max((r.min.to_vec2() * ppp).to_pos2(), (r.max.to_vec2() * ppp).to_pos2());
                    let p = self.export_pending.take().unwrap();
                    self.zoom = p.saved_view.0;
                    self.pan = p.saved_view.1;
                    match export::save_image(&img, Some(crop), &p.path) {
                        Ok(_) => self.status(format!("Saved picture {}", file_name(&p.path))),
                        Err(e) => self.status(format!("Could not save picture: {}", e)),
                    }
                } else {
                    ctx.request_repaint();
                }
            }
        }
    }

    fn handle_screenshot(&mut self, ctx: &egui::Context) {
        let Some(path) = self.shot_path.clone() else { return };
        ctx.request_repaint();
        if self.frame >= 120 && !self.shot_requested {
            self.shot_requested = true;
            let slot = self.shot.clone();
            ctx.request_screenshot(move |img| {
                *slot.lock().unwrap() = Some(img);
            });
        }
        if self.shot_requested {
            let img = self.shot.lock().unwrap().take();
            if let Some(img) = img {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Err(e) = export::save_image(&img, None, &path) {
                    eprintln!("could not save screenshot: {}", e);
                }
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
}

const COLORS: &[(&str, Option<Rgb>)] = &[
    ("Black", None),
    ("Red", Some([220, 30, 30])),
    ("Orange", Some([235, 130, 0])),
    ("Green", Some([20, 150, 50])),
    ("Blue", Some([30, 80, 220])),
    ("Purple", Some([130, 40, 180])),
    ("Grey", Some([120, 120, 120])),
];

fn current_label(doc: &Doc, i: usize) -> String {
    let a = &doc.atoms[i];
    // A labelled atom is edited with its mass number in front ("13C", "15NH2").
    let iso = if a.isotope != 0 { a.isotope.to_string() } else { String::new() };
    if let Some(l) = &a.label {
        return format!("{}{}", iso, l);
    }
    if a.el == elements::C && doc.degree(i) > 0 && a.isotope == 0 {
        return String::new();
    }
    if a.el == elements::C && doc.degree(i) > 0 {
        return format!("{}C", iso);
    }
    let h = doc.implicit_h(i);
    let mut s = format!("{}{}", iso, elements::symbol(a.el));
    if h > 0 {
        s.push('H');
        if h > 1 {
            s.push_str(&h.to_string());
        }
    }
    s
}

fn point_in_poly(p: Pos2, poly: &[Pos2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn formula_job(f: &chem::Formula, charge: i32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let normal = egui::TextFormat { font_id: egui::FontId::proportional(14.0), color: Color32::BLACK, ..Default::default() };
    let sub = egui::TextFormat { font_id: egui::FontId::proportional(10.0), color: Color32::BLACK, valign: egui::Align::BOTTOM, ..Default::default() };
    let sup = egui::TextFormat { font_id: egui::FontId::proportional(10.0), color: Color32::BLACK, valign: egui::Align::TOP, ..Default::default() };
    for (z, n) in chem::hill_order(f) {
        // A labelled isotope: mass number raised before the symbol (¹³C).
        match elements::iso_of_key(z) {
            Some((1, 2, _)) => job.append("D", 0.0, normal.clone()),
            Some((1, 3, _)) => job.append("T", 0.0, normal.clone()),
            Some((el, a, _)) => {
                job.append(&a.to_string(), 0.0, sup.clone());
                job.append(elements::symbol(el), 0.0, normal.clone());
            }
            None => job.append(elements::symbol(z), 0.0, normal.clone()),
        }
        if n != 1 {
            job.append(&n.to_string(), 0.0, sub.clone());
        }
    }
    if charge != 0 {
        let s = format!("{}{}", if charge.abs() > 1 { charge.abs().to_string() } else { String::new() }, if charge > 0 { "+" } else { "−" });
        job.append(&s, 0.0, sup);
    }
    job
}

fn parse_alkane_name(s: &str) -> Option<Doc> {
    // Branched alkanes such as "2-methylbutane" or "2,2-dimethylpropane".
    let s = s.trim().to_lowercase();
    let stems = ["meth", "eth", "prop", "but", "pent", "hex", "hept", "oct", "non", "dec"];
    let parent_len = stems.iter().enumerate().filter(|(_, st)| s.ends_with(&format!("{}ane", st))).map(|(k, st)| (k + 1, st.len() + 3)).max_by_key(|x| x.1)?;
    let (n, tail) = parent_len;
    let prefix = &s[..s.len() - tail];
    let mut smiles_atoms: Vec<Vec<usize>> = vec![vec![]; n + 1];
    for part in prefix.split('-').collect::<Vec<_>>().chunks(2) {
        if part.len() < 2 {
            if part[0].is_empty() {
                continue;
            }
            return None;
        }
        let locs: Vec<usize> = part[0].split(',').filter_map(|x| x.parse().ok()).collect();
        let name = part[1].trim_start_matches("di").trim_start_matches("tri").trim_start_matches("tetra");
        let len = stems.iter().position(|st| name == format!("{}yl", st))? + 1;
        for l in locs {
            if l == 0 || l > n {
                return None;
            }
            smiles_atoms[l].push(len);
        }
    }
    let mut smi = String::new();
    for k in 1..=n {
        smi.push('C');
        for &len in &smiles_atoms[k] {
            smi.push('(');
            smi.push_str(&"C".repeat(len));
            smi.push(')');
        }
    }
    formats::parse_smiles(&smi).ok()
}

fn help_ui(ui: &mut egui::Ui) {
    ui.label(egui::RichText::new("About").strong());
    ui.label(format!("{} version {}\n{}\n{}", APP_NAME, APP_VERSION, APP_CREDIT, APP_LICENSE));
    ui.label(egui::RichText::new(APP_DISCLAIMER).small().weak());
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Drawing").strong());
    ui.label("• Bond tools: click empty space for a bond, click an atom to grow from it, drag to aim. Click a bond to raise its order (single → double → triple) or flip a wedge.\n• Rings: click empty space, an atom (spiro) or a bond (fused).\n• Chain: drag to draw a zig-zag chain; the count shows while dragging.\n• Text tool: click an atom and type a label (N, OH, NH2, CO2Me, OMe, Ph, NH4+). Click empty space for a caption.\n• Arrows and shapes: drag; pick the type in the toolbar.\n• Rotate: select, then drag the round handle at the top-right of the selection (Shift = 15° steps), or Object ▸ Rotate… (⌘R). A selected branch turns around the atom it hangs from. ⇧⌘H / ⇧⌘V turn the selection over by 180° and keep its stereochemistry.\n• Arrange: Object ▸ Align lines up edges or centres; Object ▸ Distribute spreads objects evenly or with a gap in millimetres.\n• View ▸ Show Rulers / Show Grid: centimetre rulers and a 1 cm grid.");
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Hot keys").strong());
    ui.label("Point at an atom or a bond and type a key. When the pointer is away from the structure, keys act on the hotspot (green dot): the atom you last changed. Arrow keys move the hotspot along bonds; Esc clears it; Delete removes what you point at.");
    ui.label(egui::RichText::new("On an atom").italics());
    let labels: Vec<String> = crate::hotkeys::ATOM_LABELS.iter().map(|(k, l)| format!("{} {}", k, l)).collect();
    ui.label(format!("Labels: {}", labels.join(" · ")));
    for (k, d) in crate::hotkeys::HELP_ATOM {
        ui.label(format!("{}  —  {}", k, d));
    }
    ui.label(egui::RichText::new("On a bond").italics());
    for (k, d) in crate::hotkeys::HELP_BOND {
        ui.label(format!("{}  —  {}", k, d));
    }
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Selecting").strong());
    ui.label("Marquee or Lasso: drag around things; Shift adds. Double-click selects a whole structure. Drag a selection to move it; dropping an atom onto another joins them.");
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Keys").strong());
    ui.label("⌘N new · ⌘O open · ⌘S save · ⇧⌘S save as\n⌘Z undo · ⇧⌘Z redo · ⌘X ⌘C ⌘V · ⌘A select all\n⇧⌘K clean up structure · ⌘+ ⌘− zoom · ⌘0 fit\nScroll to move around, pinch or ⌘-scroll to zoom, right-drag to pan.");
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Files").strong());
    ui.label("Opens and saves CDX and CDXML documents (.cdx, .cdxml, .ctp), molfiles V2000/V3000, SD files, reaction files V2000/V3000, CML and SMILES. Exports PDF, EPS, SVG, PNG, JPEG, GIF, BMP and TIFF. Parts of a CDX or CDXML document this app can't show are kept and written back when you save in the same format.");
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Word, PowerPoint and other documents").strong());
    ui.label("Copy (⌘C) puts the drawing on the clipboard as a picture with the structure stored inside it. Paste it into Word, PowerPoint or Excel: it shows as a picture. Later, copy that picture there and paste it here (⌘V) to get the editable structure back.");
    if cfg!(windows) {
        ui.label("On Windows, Word, PowerPoint and Excel paste it as a Formula Structure object. Double-click the object to edit it here; closing this window (or Save) puts the changes back into the document or slide. Their Insert ▸ Object also lists Formula Structure.");
        ui.label("LibreOffice (Writer, Calc, Impress, Draw) pastes a picture whenever the copy includes one, while Word needs the picture to paste an object. So for LibreOffice use Edit ▸ Copy for LibreOffice (Ctrl+Shift+C), or turn on Edit ▸ Copy for LibreOffice by Default; the normal Copy is for Word and other programs. In LibreOffice, Paste Special ▸ Formula Structure also works after a normal Copy.");
        ui.label("Programs that can't hold embedded objects at all (WordCraft, DeckCraft, image editors) always paste a sharp picture with a see-through background. The structure is stored inside that picture, so if the program keeps the picture as it is, copying it back and pasting here gives the editable structure.");
        if let Some(p) = crate::ole::log_path().filter(|p| p.exists()) {
            if ui.button("Open the embedded objects log").on_hover_text("What Word or PowerPoint asked Formula to do, step by step (useful when something goes wrong)").clicked() {
                open_in_system(&p);
            }
        }
    }
}

fn shot_path_is_none(app: &App) -> bool {
    app.shot_path.is_none()
}

/// Open a file in the program the system uses for it.
fn open_in_system(p: &std::path::Path) {
    let _ = if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(p).spawn()
    } else if cfg!(windows) {
        std::process::Command::new("cmd").args(["/C", "start", ""]).arg(p).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(p).spawn()
    };
}

/// An empty drawing in the style new documents start with: ACS Document 1996, as in the
/// original's "Untitled ACS Document 1996" windows (bonds 14.4 pt, labels 10 pt), so labels
/// are in proportion to bonds and an N is taller than a triple bond.
pub fn new_drawing() -> Doc {
    let mut d = Doc::default();
    if let Some(p) = styles::find("ACS Document 1996") {
        d.style = styles::to_style(p, d.style.show_warnings);
    }
    d
}
