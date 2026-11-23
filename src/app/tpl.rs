//! Templates: the group list that drops down from the stamp button, the window of pictures
//! for one group, and placing a template in the drawing.

use super::*;
use crate::sugars::{self, Choice, Drawing, Ring};
use crate::templates;

/// The sugar builder's current choice, and the drawing made from it for the stamp tool.
pub(super) struct SugarPick {
    pub idx: usize,
    pub l_form: bool,
    pub ring: Option<Ring>,
    pub beta: bool,
    pub drawing: Drawing,
    pub made: Option<templates::Template>,
}

impl Default for SugarPick {
    fn default() -> Self {
        SugarPick { idx: 0, l_form: false, ring: Some(Ring::Pyranose), beta: true, drawing: Drawing::Chair, made: None }
    }
}

/// Marks the sugar builder's drawing as the current template.
const BUILT: usize = usize::MAX;

fn all_sugars() -> Vec<sugars::Sugar> {
    sugars::HEXOSES.iter().chain(sugars::PENTOSES.iter()).copied().collect()
}

const CELL: Vec2 = vec2(128.0, 118.0);

/// Draw a small picture of `d` filling `rect`.
pub(super) fn thumbnail(painter: &egui::Painter, rect: Rect, d: &Doc) {
    let Some(b) = d.bounds_of(None) else { return };
    let pad = 8.0;
    let zoom = ((rect.width() - pad * 2.0) / b.width().max(1.0)).min((rect.height() - pad * 2.0) / b.height().max(1.0)).min(0.9);
    let origin = rect.center() - b.center().to_vec2() * zoom;
    let clip = painter.with_clip_rect(rect);
    let mut cv = EguiCanvas { painter: &clip, origin, zoom };
    render::draw_doc(d, &mut cv, &RenderOpts { color_atoms: false, warnings: false });
}

impl App {
    pub(super) fn current_template(&self) -> Option<&templates::Template> {
        let (c, i) = self.template?;
        if c == BUILT {
            return self.sugar.made.as_ref();
        }
        templates::all().get(c)?.items.get(i)
    }

    /// Pick any sugar and draw it as a Fischer or Haworth projection or as a ring conformer.
    fn sugar_builder_ui(&mut self, ui: &mut egui::Ui) {
        let list = all_sugars();
        let p = &mut self.sugar;
        p.idx = p.idx.min(list.len() - 1);
        ui.horizontal(|ui| {
            ui.label("Sugar:");
            egui::ComboBox::from_id_salt("sugar-pick").selected_text(list[p.idx].name).width(190.0).show_ui(ui, |ui| {
                for (k, sg) in list.iter().enumerate() {
                    let kind = if k < sugars::HEXOSES.len() { "hexose" } else { "pentose" };
                    ui.selectable_value(&mut p.idx, k, format!("{}  ({})", sg.name, kind));
                }
            });
            ui.radio_value(&mut p.l_form, false, "D");
            ui.radio_value(&mut p.l_form, true, "L");
        });
        let sg = list[p.idx];
        ui.horizontal(|ui| {
            ui.label("Form:");
            ui.radio_value(&mut p.ring, None, "Open chain");
            for (r, name) in [(Ring::Pyranose, "Pyranose"), (Ring::Furanose, "Furanose")] {
                let ok = sugars::ring_possible(&sg, r);
                if ui.add_enabled(ok, egui::RadioButton::new(p.ring == Some(r), name)).clicked() {
                    p.ring = Some(r);
                }
            }
            if p.ring.is_some() {
                ui.separator();
                ui.radio_value(&mut p.beta, false, "α");
                ui.radio_value(&mut p.beta, true, "β");
            }
        });
        if p.ring.is_some_and(|r| !sugars::ring_possible(&sg, r)) {
            p.ring = [Ring::Pyranose, Ring::Furanose].into_iter().find(|r| sugars::ring_possible(&sg, *r));
        }
        let choices = Drawing::for_ring(p.ring);
        if !choices.contains(&p.drawing) {
            p.drawing = choices[0];
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Drawing:");
            for &dr in choices {
                ui.radio_value(&mut p.drawing, dr, dr.name());
            }
        });
        let c = Choice { sugar: sg, l_form: p.l_form, ring: p.ring, beta: p.beta, drawing: p.drawing };
        let doc = sugars::draw(&c);
        let title = c.title();
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(220.0, 130.0), Sense::hover());
            ui.painter().rect_filled(rect, 4.0, Color32::WHITE);
            ui.painter().rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_gray(215)), egui::StrokeKind::Inside);
            if let Some(d) = &doc {
                thumbnail(&ui.painter_at(rect), rect, d);
            }
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(&title).strong());
                ui.label(egui::RichText::new(c.drawing.name()).color(Color32::GRAY));
                if ui.add_enabled(doc.is_some(), egui::Button::new("Use This Drawing")).clicked() {
                    let about = format!("{}, {}", title, c.drawing.name().to_lowercase());
                    self.sugar.made = Some(templates::Template { name: title.clone(), about, doc: doc.clone().unwrap() });
                    self.template = Some((BUILT, 0));
                    self.tool = Tool::Template;
                    self.status(format!("{}: click in the drawing to place it", title));
                }
            });
        });
    }

    /// Open the template window on the group last used (the first group the first time) and
    /// switch to the stamp tool.
    pub(super) fn open_template_window(&mut self) {
        let group = match self.template {
            Some((c, _)) if c != BUILT => c,
            // The sugar builder's drawing lives in the Hexoses group.
            Some(_) => templates::all().iter().position(|c| c.name == "Hexoses").unwrap_or(0),
            None => self.template_window.unwrap_or(0),
        };
        self.template_window = Some(group);
        self.template_menu = false;
        self.tool = Tool::Template;
    }

    /// Put the chosen template into the drawing, centred on `p`.
    pub(super) fn place_template(&mut self, p: Pos2) {
        let Some((mut d, name)) = self.current_template().map(|t| (t.doc.clone(), t.name.clone())) else {
            self.open_template_window();
            self.status("Pick a template first");
            return;
        };
        d.file = None;
        let c = d.bounds_of(None).map(|b| b.center()).unwrap_or(Pos2::ZERO);
        self.checkpoint();
        self.sel = self.doc.merge(&d, p - c);
        self.dirty = true;
        self.status(format!("Placed {}", name));
    }

    /// The outline of the template that a click would place, following the pointer.
    pub(super) fn draw_template_ghost(&self, painter: &egui::Painter) {
        if self.tool != Tool::Template || self.drag.is_some() {
            return;
        }
        let (Some(t), Some(p)) = (self.current_template(), self.hover_pos) else { return };
        let Some(b) = t.doc.bounds_of(None) else { return };
        let c = self.to_screen(p);
        let r = Rect::from_center_size(c, b.size() * self.zoom).expand(4.0);
        painter.rect_stroke(r, 3.0, Stroke::new(1.0, ACCENT.gamma_multiply(0.7)), egui::StrokeKind::Outside);
    }

    /// The list of template groups under the stamp button (like the original's pop-up list).
    pub(super) fn template_menu_ui(&mut self, ctx: &egui::Context) {
        if !self.template_menu {
            return;
        }
        let at = self.template_menu_at;
        let r = egui::Area::new(egui::Id::new("template-groups")).order(egui::Order::Foreground).fixed_pos(at).show(ctx, |ui| {
            egui::Frame::menu(ui.style()).show(ui, |ui| {
                ui.set_min_width(200.0);
                for (k, c) in templates::all().iter().enumerate() {
                    let on = self.template_window == Some(k);
                    if ui.add(egui::Button::selectable(on, c.name).right_text("›")).clicked() {
                        self.template_window = Some(k);
                        self.template_menu = false;
                        self.tool = Tool::Template;
                    }
                }
            });
        });
        if r.response.clicked_elsewhere() || ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.template_menu = false;
        }
    }

    /// The pictures of one template group; clicking one picks it for the stamp tool.
    pub(super) fn template_window_ui(&mut self, ctx: &egui::Context) {
        let Some(k) = self.template_window else { return };
        let Some(cat) = templates::all().get(k) else {
            self.template_window = None;
            return;
        };
        let mut open = true;
        egui::Window::new(cat.name)
            .id(egui::Id::new("template-window"))
            .open(&mut open)
            .default_pos(self.canvas.min + vec2(24.0, 24.0))
            .default_width(CELL.x * 4.0 + 24.0)
            .default_height(640.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (j, c) in templates::all().iter().enumerate() {
                        if ui.selectable_label(j == k, c.name).clicked() {
                            self.template_window = Some(j);
                        }
                    }
                });
                ui.separator();
                if matches!(cat.name, "Hexoses" | "Pentoses" | "Conformers") {
                    egui::CollapsingHeader::new("Sugar Builder: any sugar, any projection or conformer").default_open(true).show(ui, |ui| {
                        self.sugar_builder_ui(ui);
                    });
                    ui.separator();
                }
                egui::ScrollArea::vertical().auto_shrink([false, false]).min_scrolled_height(240.0).show(ui, |ui| {
                    let cols = ((ui.available_width() / (CELL.x + 4.0)).floor() as usize).max(1);
                    egui::Grid::new(("tpl-grid", k)).spacing(vec2(4.0, 4.0)).show(ui, |ui| {
                        for (i, t) in cat.items.iter().enumerate() {
                            let (rect, resp) = ui.allocate_exact_size(CELL, Sense::click());
                            let on = self.template == Some((k, i));
                            let bg = if on { Color32::from_rgb(214, 228, 252) } else if resp.hovered() { Color32::from_gray(238) } else { Color32::WHITE };
                            let painter = ui.painter_at(rect);
                            painter.rect_filled(rect, 4.0, bg);
                            painter.rect_stroke(rect, 4.0, Stroke::new(1.0, if on { ACCENT } else { Color32::from_gray(215) }), egui::StrokeKind::Inside);
                            let pic = Rect::from_min_max(rect.min, pos2(rect.max.x, rect.max.y - 30.0));
                            thumbnail(&painter, pic, &t.doc);
                            // Long names go on two lines: the name, then the form in brackets.
                            let short = |s: &str| if s.chars().count() > 22 { s.chars().take(21).collect::<String>() + "…" } else { s.to_string() };
                            let (l1, l2) = match t.name.split_once(" (") {
                                Some((a, b)) => (short(a), short(&format!("({}", b))),
                                None => (short(&t.name), String::new()),
                            };
                            let font = egui::FontId::proportional(11.0);
                            let y1 = if l2.is_empty() { rect.max.y - 9.0 } else { rect.max.y - 22.0 };
                            painter.text(pos2(rect.center().x, y1), egui::Align2::CENTER_CENTER, l1, font.clone(), Color32::from_gray(40));
                            if !l2.is_empty() {
                                painter.text(pos2(rect.center().x, rect.max.y - 9.0), egui::Align2::CENTER_CENTER, l2, font, Color32::from_gray(100));
                            }
                            if resp.on_hover_text(&t.about).clicked() {
                                self.template = Some((k, i));
                                self.tool = Tool::Template;
                                self.sel.clear();
                                self.status(format!("{}: click in the drawing to place it", t.name));
                            }
                            if (i + 1) % cols == 0 {
                                ui.end_row();
                            }
                        }
                    });
                });
            });
        if !open {
            self.template_window = None;
        }
    }
}
