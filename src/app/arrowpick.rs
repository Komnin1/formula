//! The grid of arrow styles that drops down from the arrow button, and the arrow tool's
//! toolbar.

use super::*;
use crate::arrows;

const CELL: Vec2 = vec2(46.0, 30.0);

impl App {
    fn arrow_cell(&mut self, ui: &mut egui::Ui, kind: ArrowKind, style: ArrowStyle, size: Vec2) -> bool {
        let (resp, painter) = ui.allocate_painter(size, Sense::click());
        let on = self.arrow_kind == kind && self.arrow_style == style;
        let bg = if on {
            Color32::from_rgb(200, 220, 250)
        } else if resp.hovered() {
            Color32::from_rgb(225, 225, 228)
        } else {
            Color32::TRANSPARENT
        };
        painter.rect_filled(resp.rect, 4.0, bg);
        if on {
            painter.rect_stroke(resp.rect, 4.0, Stroke::new(1.0, ACCENT), egui::StrokeKind::Inside);
        }
        arrows::icon(&painter, resp.rect.shrink(2.0), kind, style, Color32::from_gray(20));
        let clicked = resp.on_hover_text(arrows::describe(kind, &style)).clicked();
        if clicked {
            self.set_arrow_kind(kind, style);
        }
        clicked
    }

    /// The arrow tool's options in the toolbar: a few common arrows, the current one and the full grid.
    pub(super) fn arrow_toolbar_ui(&mut self, ui: &mut egui::Ui) {
        for k in [ArrowKind::Reaction, ArrowKind::Equilibrium, ArrowKind::Retro, ArrowKind::Resonance, ArrowKind::Curved, ArrowKind::Dashed, ArrowKind::NoGo, ArrowKind::Line] {
            self.arrow_cell(ui, k, ArrowStyle::default(), vec2(46.0, 22.0));
        }
        if !self.arrow_style.is_default() {
            ui.separator();
            let (k, s) = (self.arrow_kind, self.arrow_style);
            self.arrow_cell(ui, k, s, vec2(46.0, 22.0));
        }
        let r = ui.button("More Styles ▾").on_hover_text("Every arrow style: head sizes, half heads, curved, block, no-go, equilibrium and rotation arrows");
        if r.clicked() {
            self.arrow_menu = !self.arrow_menu;
            self.arrow_menu_fresh = true;
            self.arrow_menu_at = r.rect.left_bottom() + vec2(0.0, 4.0);
        }
    }

    /// The grid of every arrow style, like the original's pop-up palette.
    pub(super) fn arrow_menu_ui(&mut self, ctx: &egui::Context) {
        if !self.arrow_menu {
            return;
        }
        let at = self.arrow_menu_at;
        let mut picked = false;
        let r = egui::Area::new(egui::Id::new("arrow-styles")).order(egui::Order::Foreground).fixed_pos(at).show(ctx, |ui| {
            egui::Frame::menu(ui.style()).inner_margin(8.0).show(ui, |ui| {
                ui.label(egui::RichText::new("Arrow styles").strong());
                ui.add_space(2.0);
                let list = arrows::presets();
                egui::Grid::new("arrow-style-grid").spacing(vec2(2.0, 2.0)).show(ui, |ui| {
                    for (i, p) in list.iter().enumerate() {
                        picked |= self.arrow_cell(ui, p.kind, p.style, CELL);
                        if (i + 1) % arrows::COLS == 0 {
                            ui.end_row();
                        }
                    }
                });
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    picked |= self.arrow_cell(ui, ArrowKind::Line, ArrowStyle::default(), CELL);
                    ui.label(egui::RichText::new("Plain line. Picking a style also changes the selected arrows.").size(11.0).color(Color32::GRAY));
                });
            });
        });
        // The click that opened the grid lands outside it; only later clicks close it.
        let fresh = std::mem::take(&mut self.arrow_menu_fresh);
        if picked || (!fresh && r.response.clicked_elsewhere()) || ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.arrow_menu = false;
        }
    }
}
