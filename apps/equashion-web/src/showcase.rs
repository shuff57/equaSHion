//! The showcase app: Editor, Gallery, OMML and Selftest tabs.

use std::collections::HashMap;
use std::sync::Arc;

use egui::{FontId, RichText, TextureHandle, TextureOptions, Ui, Vec2, pos2, vec2};
use equashion_egui::EquationEditor;
use equashion_layout::{DEFAULT_FAMILY, FontDb, Rgb, layout_equation_wrapped};
use equashion_math::from_linear;

use crate::corpus::{self, Row};
use crate::selftest::{self, Report};

/// The tabs, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Editor,
    Gallery,
    Omml,
    Selftest,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Editor, Tab::Gallery, Tab::Omml, Tab::Selftest];

    /// The name the test hooks use (`editor`, `gallery`, `omml`, `selftest`).
    pub fn name(self) -> &'static str {
        match self {
            Tab::Editor => "editor",
            Tab::Gallery => "gallery",
            Tab::Omml => "omml",
            Tab::Selftest => "selftest",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Tab::Editor => "Editor",
            Tab::Gallery => "Gallery",
            Tab::Omml => "OMML",
            Tab::Selftest => "Selftest",
        }
    }

    /// A tab by name (any case).
    pub fn parse(name: &str) -> Option<Tab> {
        let n = name.trim().to_ascii_lowercase();
        Tab::ALL.into_iter().find(|t| t.name() == n)
    }
}

/// A rendered gallery card.
struct Thumb {
    tex: TextureHandle,
    /// Size in points.
    size: Vec2,
}

/// The whole page.
pub struct Showcase {
    pub editor: EquationEditor,
    tab: Tab,
    fonts: Arc<FontDb>,
    rows: Vec<Row>,
    thumbs: HashMap<&'static str, Thumb>,
    omml_in: String,
    omml_sample: usize,
    report: Option<Report>,
}

impl Default for Showcase {
    fn default() -> Self {
        Self::new()
    }
}

/// Gallery equation size, points.
const THUMB_PT: f64 = 20.0;
/// Gallery card width, points.
const CARD_W: f32 = 250.0;
/// Gap between cards, points.
const CARD_GAP: f32 = 10.0;
/// A wide card spans two columns.
const WIDE_W: f32 = 2.0 * CARD_W + CARD_GAP;

impl Showcase {
    pub fn new() -> Self {
        let first = corpus::OMML_SAMPLES.first().map(|s| s.1).unwrap_or_default();
        Showcase {
            editor: EquationEditor::with_linear(r"x=\frac{-b\pm\sqrt{b^2-4ac}}{2a}").id_salt("showcase-editor"),
            tab: Tab::Editor,
            fonts: equashion_egui::bundled_fonts(),
            rows: corpus::rows(),
            thumbs: HashMap::new(),
            omml_in: first.to_string(),
            omml_sample: 0,
            report: None,
        }
    }

    pub fn tab(&self) -> Tab {
        self.tab
    }

    pub fn set_tab(&mut self, t: Tab) {
        self.tab = t;
    }

    /// Number of gallery cards (the rows of `corpus/equations.tsv`).
    pub fn gallery_count(&self) -> usize {
        self.rows.len()
    }

    /// Load a gallery row into the editor and show the editor.
    pub fn load_row(&mut self, i: usize) {
        if let Some(r) = self.rows.get(i) {
            self.editor.set_linear(r.linear);
            self.tab = Tab::Editor;
        }
    }

    /// Run (or re-run) the selftest and keep the report for the Selftest tab.
    pub fn run_selftest(&mut self) -> &Report {
        self.report.insert(selftest::run(&self.fonts))
    }

    pub fn report(&self) -> Option<&Report> {
        self.report.as_ref()
    }

    /// The OMML page: (input, normalised output, idempotent).
    pub fn omml_page(&self) -> (&str, String, bool) {
        let (out, same) = corpus::omml_round_trip(&self.omml_in);
        (&self.omml_in, out, same)
    }

    pub fn set_omml_input(&mut self, x: &str) {
        self.omml_in = x.to_string();
    }

    /// Draw the page.
    pub fn ui(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("equaSHion").size(18.0).strong());
            ui.add_space(12.0);
            for t in Tab::ALL {
                if ui.selectable_label(self.tab == t, t.title()).clicked() {
                    self.tab = t;
                }
            }
        });
        ui.separator();
        match self.tab {
            Tab::Editor => self.editor_tab(ui),
            Tab::Gallery => self.gallery_tab(ui),
            Tab::Omml => self.omml_tab(ui),
            Tab::Selftest => self.selftest_tab(ui),
        }
    }

    fn editor_tab(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(900.0);
            let r = self.editor.show(ui);
            if r.cancelled {
                self.editor.revert();
            }
            ui.separator();
            readout(ui, "linear()", &self.editor.linear(), false);
            readout(ui, "omml()", &self.editor.omml(), true);
            let unknown = self.editor.unknown_commands();
            ui.horizontal(|ui| {
                ui.label(RichText::new("Unknown commands").strong());
                if unknown.is_empty() {
                    ui.weak("none");
                } else {
                    ui.colored_label(ui.visuals().warn_fg_color, unknown.join(", "));
                }
            });
        });
    }

    fn thumb(&mut self, ui: &Ui, row: &Row) -> Option<&Thumb> {
        if !self.thumbs.contains_key(row.id) {
            let m = from_linear(row.linear);
            // Long equations wrap at operators to the width of a wide card.
            let l = layout_equation_wrapped(&*self.fonts, &m, DEFAULT_FAMILY, THUMB_PT, row.display, Rgb::WHITE, 2.0, f64::from(WIDE_W - 20.0));
            let scale = f64::from(ui.ctx().pixels_per_point().max(1.0)) * 1.5;
            let img = equashion_render::render_scene(&l.scene, &*self.fonts, scale, None);
            let ci = egui::ColorImage::from_rgba_premultiplied([img.width as usize, img.height as usize], &img.pixels);
            let tex = ui.ctx().load_texture(format!("equashion/gallery/{}", row.id), ci, TextureOptions::LINEAR);
            self.thumbs.insert(row.id, Thumb { tex, size: vec2(l.width() as f32, l.height() as f32) });
        }
        self.thumbs.get(row.id)
    }

    fn gallery_tab(&mut self, ui: &mut Ui) {
        ui.label(format!("{} equations from corpus/equations.tsv. Click one to edit it.", self.rows.len()));
        let mut clicked = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(CARD_GAP, CARD_GAP);
                let rows = self.rows.clone();
                for (i, row) in rows.iter().enumerate() {
                    let Some(th) = self.thumb(ui, row) else { continue };
                    if card(ui, th, row).clicked() {
                        clicked = Some(i);
                    }
                }
            });
        });
        if let Some(i) = clicked {
            self.load_row(i);
        }
    }

    fn omml_tab(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label("Sample");
            let names: Vec<&str> = corpus::OMML_SAMPLES.iter().map(|s| s.0).collect();
            let current = names.get(self.omml_sample).copied().unwrap_or("custom");
            egui::ComboBox::from_id_salt("equashion-omml-sample").selected_text(current).width(320.0).show_ui(ui, |ui| {
                for (i, n) in names.iter().enumerate() {
                    if ui.selectable_label(self.omml_sample == i, *n).clicked() {
                        self.omml_sample = i;
                        if let Some(s) = corpus::OMML_SAMPLES.get(i) {
                            self.omml_in = s.1.to_string();
                        }
                    }
                }
            });
        });
        let (_, out, same) = self.omml_page();
        ui.horizontal(|ui| {
            let (text, color) = if same {
                ("round-trip idempotent: yes", egui::Color32::from_rgb(40, 150, 70))
            } else {
                ("round-trip idempotent: no", ui.visuals().error_fg_color)
            };
            egui::Frame::new().fill(color).corner_radius(4.0).inner_margin(egui::Margin::symmetric(8, 3)).show(ui, |ui| {
                ui.label(RichText::new(text).color(egui::Color32::WHITE).strong());
            });
            if ui.button("Load into editor").clicked() {
                self.editor.set_omml(&self.omml_in);
                self.tab = Tab::Editor;
            }
        });
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.label(RichText::new("OMML in").strong());
            if ui
                .add(egui::TextEdit::multiline(&mut self.omml_in).font(FontId::monospace(12.0)).desired_width(f32::INFINITY).desired_rows(8))
                .changed()
            {
                self.omml_sample = usize::MAX;
            }
            readout(ui, "Normalised OMML out", &out, true);
        });
    }

    fn selftest_tab(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui.button("Run selftest").clicked() {
                self.run_selftest();
            }
            ui.label("parse -> layout -> render (and an OMML round trip) for every corpus item");
        });
        let Some(rep) = &self.report else {
            ui.weak("Not run yet.");
            return;
        };
        let fails = rep.failures().count();
        let missing: usize = rep.checks.iter().map(|c| c.missing_glyphs).sum();
        let summary = format!("{} items, {} passed, {} failed, {} missing glyphs", rep.checks.len(), rep.checks.len() - fails, fails, missing);
        ui.label(RichText::new(summary).strong());
        let ok_col = egui::Color32::from_rgb(40, 150, 70);
        let bad_col = ui.visuals().error_fg_color;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("equashion-selftest").striped(true).num_columns(4).show(ui, |ui| {
                for c in &rep.checks {
                    ui.monospace(&c.id);
                    if c.ok {
                        ui.colored_label(ok_col, "pass");
                    } else {
                        ui.colored_label(bad_col, "FAIL");
                    }
                    ui.label(format!("{:.0} x {:.0} pt", c.size.0, c.size.1));
                    let mut note = c.reason.clone().unwrap_or_default();
                    if c.missing_glyphs > 0 {
                        note = format!("{note} {} missing glyph(s)", c.missing_glyphs).trim().to_string();
                    }
                    ui.label(note);
                    ui.end_row();
                }
            });
        });
    }
}

/// A read-only text box with a Copy button.
fn readout(ui: &mut Ui, title: &str, text: &str, multiline: bool) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).strong());
        if ui.small_button("Copy").clicked() {
            ui.ctx().copy_text(text.to_string());
        }
    });
    let mut t = text.to_string();
    let edit = if multiline { egui::TextEdit::multiline(&mut t).desired_rows(4) } else { egui::TextEdit::singleline(&mut t) };
    ui.add(edit.interactive(false).font(FontId::monospace(12.0)).desired_width(f32::INFINITY));
}

/// A gallery card: the equation (shrunk to fit), its id and source.
fn card(ui: &mut Ui, th: &Thumb, row: &Row) -> egui::Response {
    let pic_h = 80.0;
    // An equation that would shrink below 60% in one column gets two.
    let fit = |w: f32| ((w - 16.0) / th.size.x.max(1.0)).min(pic_h / th.size.y.max(1.0)).min(1.0);
    let card_w = if fit(CARD_W) < 0.6 { WIDE_W } else { CARD_W };
    let size = th.size * fit(card_w);
    let (rect, resp) = ui.allocate_exact_size(vec2(card_w, pic_h + 44.0), egui::Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, row.id));
    if ui.is_rect_visible(rect) {
        let v = ui.visuals();
        let fill = if resp.hovered() { v.widgets.hovered.weak_bg_fill } else { v.extreme_bg_color };
        ui.painter().rect(rect, 6.0, fill, egui::Stroke::new(1.0, v.widgets.noninteractive.bg_stroke.color), egui::StrokeKind::Inside);
        let pic = egui::Rect::from_min_size(rect.min + vec2(8.0, 6.0), vec2(card_w - 16.0, pic_h));
        let r = egui::Rect::from_center_size(pic.center(), size);
        ui.painter().image(th.tex.id(), r, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), v.strong_text_color());
        let text_x = rect.min.x + 8.0;
        // One line each, elided with "…" at the card edge.
        let line = |text: &str, font: FontId, color: egui::Color32| {
            let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
            job.wrap = egui::text::TextWrapping::truncate_at_width(card_w - 16.0);
            ui.fonts_mut(|f| f.layout_job(job))
        };
        let (id_color, src_color) = (v.strong_text_color(), v.weak_text_color());
        let id = line(row.id, FontId::proportional(13.0), id_color);
        let src = line(row.linear, FontId::monospace(10.5), src_color);
        ui.painter().galley(pos2(text_x, rect.min.y + pic_h + 12.0), id, id_color);
        ui.painter().galley(pos2(text_x, rect.min.y + pic_h + 28.0), src, src_color);
    }
    resp.on_hover_text(row.linear)
}
