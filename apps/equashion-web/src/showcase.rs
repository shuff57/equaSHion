//! The showcase app: Editor, Gallery, OMML, Selftest and About tabs, a header and a credit
//! footer. Plain egui, so the native tests drive it with egui_kittest.

use std::collections::HashMap;
use std::sync::Arc;

use egui::{
    Align, Color32, CornerRadius, FontId, Frame, Layout, Margin, Rect, RichText, Sense, Stroke, StrokeKind, TextureHandle, TextureOptions, Theme, Ui,
    Vec2, pos2, vec2,
};
use equashion_egui::EquationEditor;
use equashion_layout::{DEFAULT_FAMILY, FontDb, Rgb, layout_equation_wrapped};
use equashion_math::from_linear;

use crate::corpus::{self, Row};
use crate::selftest::{self, Report};
use crate::theme::{self, Palette, R_CONTROL, R_PANEL, S4, S8, S12, S16, S24, T_BODY, T_CAPTION, T_H2, T_MONO, ThemePref};

/// The tabs, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Editor,
    Gallery,
    Omml,
    Selftest,
    About,
}

impl Tab {
    pub const ALL: [Tab; 5] = [Tab::Editor, Tab::Gallery, Tab::Omml, Tab::Selftest, Tab::About];

    /// The name the test hooks use (`editor`, `gallery`, `omml`, `selftest`, `about`).
    pub fn name(self) -> &'static str {
        match self {
            Tab::Editor => "editor",
            Tab::Gallery => "gallery",
            Tab::Omml => "omml",
            Tab::Selftest => "selftest",
            Tab::About => "about",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Tab::Editor => "Editor",
            Tab::Gallery => "Gallery",
            Tab::Omml => "OMML",
            Tab::Selftest => "Selftest",
            Tab::About => "About",
        }
    }

    /// A tab by name (any case).
    pub fn parse(name: &str) -> Option<Tab> {
        let n = name.trim().to_ascii_lowercase();
        Tab::ALL.into_iter().find(|t| t.name() == n)
    }
}

/// The one-line description under the wordmark.
pub const TAGLINE: &str = "A pure-Rust equation editor.";

/// The credit line shown in the footer of every tab, in the About tab and in README.md.
pub const CREDIT: &str = "Typing behaviour is modelled on MathLive (cortexjs.io/mathlive). equaSHion is an independent Rust implementation, not a port and not affiliated with MathLive; no MathLive code is used.";

const MATHLIVE_URL: &str = "https://cortexjs.io/mathlive";
/// How the MathLive address appears inside [`CREDIT`].
const MATHLIVE_SHOWN: &str = "cortexjs.io/mathlive";

/// The gallery's groups: a heading and the corpus ids under it, wide equations last so they
/// start a fresh grid row. Rows not listed here go under "Other".
pub const GALLERY_GROUPS: &[(&str, &[&str])] = &[
    ("Fractions and roots", &["frac_simple", "frac_nested", "frac_nested_inline", "cfrac", "sqrt", "sqrt_degree", "sqrt_nested", "binom"]),
    ("Scripts and symbols", &["script_sup", "script_sub", "script_both", "script_nested", "greek", "func", "floor_ceil", "angle"]),
    ("Operators and limits", &["sum_display", "sum_inline", "prod_display", "int_display", "int_inline", "oint", "lim_display", "lim_inline"]),
    ("Brackets, matrices and cases", &["delims_tall", "abs_tall", "pmatrix", "bmatrix", "cases", "wide_matrix"]),
    ("Accents and braces", &["accent_hat", "accent_vec", "accent_bar", "overline", "overrightarrow", "boxed", "overbrace", "underbrace"]),
    ("Physics and long forms", &["euler", "quadratic", "maxwell_gauss", "maxwell_faraday", "maxwell_ampere", "long_wrap"]),
];

/// The layout switches to one column below this width, points.
const TWO_COLUMNS_FROM: f32 = 800.0;

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
    /// Gallery groups as row indices.
    groups: Vec<(&'static str, Vec<usize>)>,
    thumbs: HashMap<&'static str, Thumb>,
    omml_in: String,
    omml_sample: usize,
    report: Option<Report>,
    theme: ThemePref,
    reduced_motion: bool,
    /// Fonts and styles must be (re)installed on the context.
    style_dirty: bool,
    /// The Geist fonts are registered (they take effect from the frame after `set_fonts`).
    fonts_ready: bool,
    /// Which Copy button was pressed, and when (for the "Copied" note).
    copied: Option<(&'static str, f64)>,
}

impl Default for Showcase {
    fn default() -> Self {
        Self::new()
    }
}

/// Gallery equation size, points.
const THUMB_PT: f64 = 20.0;
/// Narrowest gallery card, points.
const CARD_MIN_W: f32 = 260.0;
/// Gap between cards, points.
const CARD_GAP: f32 = S12;
/// Height of the equation area of a card, points.
const PIC_H: f32 = 84.0;
/// Card height: padding, picture, caption of two mono lines.
const CARD_H: f32 = S12 + PIC_H + S8 + 1.0 + S8 + 16.0 + S4 + 16.0 + S12;
/// Width long equations wrap to before they are shrunk into a card.
const WRAP_W: f64 = 480.0;

impl Showcase {
    pub fn new() -> Self {
        let first = corpus::OMML_SAMPLES.first().map(|s| s.1).unwrap_or_default();
        let rows = corpus::rows();
        Showcase {
            editor: EquationEditor::with_linear(r"x=\frac{-b\pm\sqrt{b^2-4ac}}{2a}").id_salt("showcase-editor"),
            tab: Tab::Editor,
            fonts: equashion_egui::bundled_fonts(),
            groups: group_rows(&rows),
            rows,
            thumbs: HashMap::new(),
            omml_in: first.to_string(),
            omml_sample: 0,
            report: None,
            theme: ThemePref::System,
            reduced_motion: false,
            style_dirty: true,
            fonts_ready: false,
            copied: None,
        }
    }

    pub fn tab(&self) -> Tab {
        self.tab
    }

    pub fn set_tab(&mut self, t: Tab) {
        self.tab = t;
    }

    /// The theme preference (System follows `prefers-color-scheme`).
    pub fn theme_pref(&self) -> ThemePref {
        self.theme
    }

    pub fn set_theme_pref(&mut self, t: ThemePref) {
        if self.theme != t {
            self.theme = t;
            self.style_dirty = true;
        }
    }

    /// The theme in effect on `ctx` once the preference is applied.
    pub fn effective_theme(&self, ctx: &egui::Context) -> Theme {
        match self.theme {
            ThemePref::Light => Theme::Light,
            ThemePref::Dark => Theme::Dark,
            ThemePref::System => ctx.system_theme().unwrap_or(Theme::Light),
        }
    }

    /// Honour `prefers-reduced-motion`: no egui animations, no caret blink.
    pub fn set_reduced_motion(&mut self, on: bool) {
        self.reduced_motion = on;
        self.editor.set_caret_blink(!on);
        self.style_dirty = true;
    }

    pub fn reduced_motion(&self) -> bool {
        self.reduced_motion
    }

    /// Number of gallery cards (the rows of `corpus/equations.tsv`).
    pub fn gallery_count(&self) -> usize {
        self.rows.len()
    }

    /// The gallery groups: heading and the ids under it, in display order.
    pub fn gallery_groups(&self) -> Vec<(&'static str, Vec<&'static str>)> {
        self.groups.iter().map(|(t, ix)| (*t, ix.iter().filter_map(|&i| self.rows.get(i).map(|r| r.id)).collect())).collect()
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

    /// The OMML page's status line.
    pub fn omml_status(&self) -> OmmlStatus {
        omml_status(&self.omml_in)
    }

    pub fn set_omml_input(&mut self, x: &str) {
        self.omml_in = x.to_string();
        self.omml_sample = usize::MAX;
    }

    /// Install the fonts and the theme on `ctx` now. [`Showcase::ui`] does it on its first
    /// frame otherwise (and then skips drawing that frame, because new fonts only apply from
    /// the next one).
    pub fn install(&mut self, ctx: &egui::Context) {
        theme::install(ctx, self.theme, self.reduced_motion);
        self.style_dirty = false;
        self.fonts_ready = true;
    }

    /// Draw the page.
    pub fn ui(&mut self, ui: &mut Ui) {
        if !self.fonts_ready {
            self.install(ui.ctx());
            ui.ctx().request_discard("equashion: fonts installed");
            return;
        }
        if self.style_dirty {
            theme::install(ui.ctx(), self.theme, self.reduced_motion);
            self.style_dirty = false;
            ui.ctx().request_repaint();
        }
        let p = *theme::palette(ui);
        egui::Panel::top("equashion-header")
            .exact_size(56.0)
            .frame(Frame::new().fill(p.surface).inner_margin(Margin::symmetric(S16 as i8, 0)))
            .show(ui, |ui| self.header(ui, &p));
        egui::Panel::bottom("equashion-footer")
            .frame(Frame::new().fill(p.bg).inner_margin(Margin::symmetric(S16 as i8, S8 as i8)))
            .show(ui, |ui| footer(ui, &p));
        egui::CentralPanel::default().frame(Frame::new().fill(p.bg).inner_margin(Margin::symmetric(S16 as i8, S16 as i8))).show(ui, |ui| match self
            .tab
        {
            Tab::Editor => self.editor_tab(ui, &p),
            Tab::Gallery => self.gallery_tab(ui, &p),
            Tab::Omml => self.omml_tab(ui, &p),
            Tab::Selftest => self.selftest_tab(ui, &p),
            Tab::About => about_tab(ui, &p),
        });
    }

    fn header(&mut self, ui: &mut Ui, p: &Palette) {
        let w = ui.available_width();
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.label(RichText::new("equaSHion").font(theme::semibold(20.0)).color(p.strong));
            if w >= 1060.0 {
                ui.add_space(S4);
                ui.label(RichText::new(TAGLINE).color(p.muted));
            }
            ui.add_space(if w >= 760.0 { S24 } else { S8 });
            ui.spacing_mut().item_spacing.x = S4;
            for t in Tab::ALL {
                if segment(ui, p, self.tab == t, t.title(), None).clicked() {
                    self.tab = t;
                }
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = S4;
                // Right to left: the last segment first.
                let mut chosen = None;
                for t in ThemePref::ALL.into_iter().rev() {
                    let hint = match t {
                        ThemePref::System => "Follow the system light or dark setting",
                        ThemePref::Light => "Always use the light theme",
                        ThemePref::Dark => "Always use the dark theme",
                    };
                    if segment(ui, p, self.theme == t, t.title(), Some(hint)).clicked() {
                        chosen = Some(t);
                    }
                }
                if w >= 1160.0 {
                    ui.add_space(S8);
                    ui.label(RichText::new("Theme").color(p.muted));
                }
                ui.add_space(S4);
                ui.separator();
                if let Some(t) = chosen {
                    self.set_theme_pref(t);
                }
            });
        });
    }

    fn editor_tab(&mut self, ui: &mut Ui, p: &Palette) {
        egui::ScrollArea::vertical().id_salt("equashion-editor-scroll").auto_shrink([false, false]).show(ui, |ui| {
            columns(ui, 0.58, |ui, col| match col {
                0 => panel(ui, p, |ui| {
                    h2(ui, p, "Type an equation");
                    caption(
                        ui,
                        p,
                        "Type into the box: x^2 raises, / makes a fraction, sqrt a root, alpha a Greek letter. Tab moves to the next place, Ctrl+Z undoes. The buttons insert a structure at the caret.",
                    );
                    ui.add_space(S8);
                    let r = self.editor.show(ui);
                    if r.cancelled {
                        self.editor.revert();
                    }
                }),
                _ => panel(ui, p, |ui| {
                    let now = ui.input(|i| i.time);
                    self.readout(ui, p, now, "Linear form", "editor.linear()", &self.editor.linear(), 1);
                    ui.add_space(S16);
                    self.readout(ui, p, now, "OMML markup", "editor.omml()", &self.editor.omml(), 14);
                    ui.add_space(S16);
                    h2(ui, p, "Unknown commands");
                    let unknown = self.editor.unknown_commands();
                    if unknown.is_empty() {
                        ui.label(RichText::new("None. Every backslash command in the linear field is recognised.").color(p.muted));
                    } else {
                        ui.label(
                            RichText::new(format!("Not recognised, so kept as plain text: {}", unknown.join(", ")))
                                .color(p.error),
                        );
                    }
                }),
            });
        });
    }

    /// A titled read-only text field with a Copy button.
    #[allow(clippy::too_many_arguments)]
    fn readout(&mut self, ui: &mut Ui, p: &Palette, now: f64, title: &'static str, call: &str, text: &str, rows: usize) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(title).font(theme::semibold(T_H2)).color(p.strong));
            ui.label(RichText::new(call).font(FontId::monospace(T_CAPTION)).color(p.muted));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Copy").on_hover_text("Copy to the clipboard").clicked() {
                    ui.ctx().copy_text(text.to_string());
                    self.copied = Some((title, now));
                }
                if let Some((t, at)) = self.copied
                    && t == title
                {
                    if now - at < 1.5 {
                        ui.label(RichText::new("Copied").color(p.muted));
                        ui.ctx().request_repaint_after(std::time::Duration::from_millis(300));
                    } else {
                        self.copied = None;
                    }
                }
            });
        });
        ui.add_space(S4);
        let mut t = text;
        if rows <= 1 {
            ui.add(
                egui::TextEdit::singleline(&mut t)
                    .font(FontId::monospace(T_MONO))
                    .desired_width(f32::INFINITY)
                    .margin(Margin::symmetric(S8 as i8, 6)),
            );
        } else {
            let h = rows as f32 * 18.0 + 16.0;
            egui::ScrollArea::vertical().id_salt(("equashion-readout", title)).max_height(h).min_scrolled_height(h).show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut t)
                        .font(FontId::monospace(T_MONO))
                        .desired_width(f32::INFINITY)
                        .desired_rows(4)
                        .margin(Margin::symmetric(S8 as i8, 6)),
                );
            });
        }
    }

    fn thumb(&mut self, ui: &Ui, row: &Row) -> Option<&Thumb> {
        if !self.thumbs.contains_key(row.id) {
            let m = from_linear(row.linear);
            // Long equations wrap at operators before they are shrunk into a card.
            let l = layout_equation_wrapped(&*self.fonts, &m, DEFAULT_FAMILY, THUMB_PT, row.display, Rgb::WHITE, 2.0, WRAP_W);
            let scale = f64::from(ui.ctx().pixels_per_point().max(1.0)) * 1.5;
            let img = equashion_render::render_scene(&l.scene, &*self.fonts, scale, None);
            let ci = egui::ColorImage::from_rgba_premultiplied([img.width as usize, img.height as usize], &img.pixels);
            let tex = ui.ctx().load_texture(format!("equashion/gallery/{}", row.id), ci, TextureOptions::LINEAR);
            self.thumbs.insert(row.id, Thumb { tex, size: vec2(l.width() as f32, l.height() as f32) });
        }
        self.thumbs.get(row.id)
    }

    fn gallery_tab(&mut self, ui: &mut Ui, p: &Palette) {
        let mut clicked = None;
        egui::ScrollArea::vertical().id_salt("equashion-gallery-scroll").auto_shrink([false, false]).show(ui, |ui| {
            caption(
                ui,
                p,
                &format!(
                    "{} equations from corpus/equations.tsv, laid out and rendered by equashion-render. Select one to open it in the editor.",
                    self.rows.len()
                ),
            );
            ui.add_space(S16);
            let groups = self.groups.clone();
            let rows = self.rows.clone();
            for (title, ix) in &groups {
                ui.horizontal(|ui| {
                    h2(ui, p, title);
                    ui.label(RichText::new(ix.len().to_string()).font(FontId::monospace(T_CAPTION)).color(p.muted));
                });
                ui.add_space(S8);
                let w = ui.available_width();
                let cols = (((w + CARD_GAP) / (CARD_MIN_W + CARD_GAP)).floor() as usize).max(1);
                let card_w = (w - (cols as f32 - 1.0) * CARD_GAP) / cols as f32;
                let origin = ui.cursor().min;
                let (mut col, mut line) = (0usize, 0usize);
                for &i in ix {
                    let Some(row) = rows.get(i) else { continue };
                    let Some(th) = self.thumb(ui, row) else { continue };
                    // An equation that would shrink below 60% in one column gets two.
                    let span = if cols > 1 && fit(th.size, card_w) < 0.6 { 2 } else { 1 };
                    if col + span > cols {
                        col = 0;
                        line += 1;
                    }
                    let min = origin + vec2(col as f32 * (card_w + CARD_GAP), line as f32 * (CARD_H + CARD_GAP));
                    let rect = Rect::from_min_size(min, vec2(span as f32 * card_w + (span as f32 - 1.0) * CARD_GAP, CARD_H));
                    if card(ui, p, th, row, rect).clicked() {
                        clicked = Some(i);
                    }
                    col += span;
                }
                let lines = if ix.is_empty() { 0 } else { line + 1 };
                let h = lines as f32 * CARD_H + lines.saturating_sub(1) as f32 * CARD_GAP;
                ui.advance_cursor_after_rect(Rect::from_min_size(origin, vec2(w, h)));
                ui.add_space(S24);
            }
        });
        if let Some(i) = clicked {
            self.load_row(i);
        }
    }

    fn omml_tab(&mut self, ui: &mut Ui, p: &Palette) {
        let (_, out, _) = self.omml_page();
        let status = self.omml_status();
        let mut load = false;
        egui::ScrollArea::vertical().id_salt("equashion-omml-scroll").auto_shrink([false, false]).show(ui, |ui| {
            columns(ui, 0.5, |ui, col| match col {
                0 => panel(ui, p, |ui| {
                    h2(ui, p, "OMML input");
                    caption(ui, p, "Paste an m:oMath or m:oMathPara element from Word or PowerPoint, or pick a sample from the corpus.");
                    ui.add_space(S8);
                    ui.horizontal(|ui| {
                        ui.label("Sample");
                        let names: Vec<&str> = corpus::OMML_SAMPLES.iter().map(|s| s.0).collect();
                        let current = names.get(self.omml_sample).copied().unwrap_or("Your input");
                        egui::ComboBox::from_id_salt("equashion-omml-sample").selected_text(current).width(ui.available_width().min(320.0)).show_ui(
                            ui,
                            |ui| {
                                for (i, n) in names.iter().enumerate() {
                                    if ui.selectable_label(self.omml_sample == i, *n).clicked() {
                                        self.omml_sample = i;
                                        if let Some(s) = corpus::OMML_SAMPLES.get(i) {
                                            self.omml_in = s.1.to_string();
                                        }
                                    }
                                }
                            },
                        );
                    });
                    ui.add_space(S8);
                    egui::ScrollArea::vertical().id_salt("equashion-omml-in").max_height(380.0).min_scrolled_height(260.0).show(ui, |ui| {
                        let r = ui.add(
                            egui::TextEdit::multiline(&mut self.omml_in)
                                .font(FontId::monospace(T_MONO))
                                .desired_width(f32::INFINITY)
                                .desired_rows(14)
                                .margin(Margin::symmetric(S8 as i8, 6))
                                .hint_text("<m:oMath>...</m:oMath>"),
                        );
                        if r.changed() {
                            self.omml_sample = usize::MAX;
                        }
                    });
                }),
                _ => panel(ui, p, |ui| {
                    ui.horizontal(|ui| {
                        h2(ui, p, "Normalised output");
                        ui.label(RichText::new("to_omml(from_omml(input))").font(FontId::monospace(T_CAPTION)).color(p.muted));
                    });
                    ui.add_space(S8);
                    ui.horizontal(|ui| {
                        badge(ui, p, status.badge(), status.is_error());
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let loadable = matches!(status, OmmlStatus::Stable | OmmlStatus::Changed);
                            if primary_button(ui, p, "Load into editor", loadable).clicked() {
                                load = true;
                            }
                        });
                    });
                    ui.add_space(S4);
                    caption(ui, p, status.explanation());
                    ui.add_space(S8);
                    if status != OmmlStatus::Empty {
                        egui::ScrollArea::vertical().id_salt("equashion-omml-out").max_height(380.0).min_scrolled_height(260.0).show(ui, |ui| {
                            let mut t = out.as_str();
                            ui.add(
                                egui::TextEdit::multiline(&mut t)
                                    .font(FontId::monospace(T_MONO))
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(14)
                                    .margin(Margin::symmetric(S8 as i8, 6)),
                            );
                        });
                    }
                }),
            });
        });
        if load {
            self.editor.set_omml(&self.omml_in);
            self.tab = Tab::Editor;
        }
    }

    fn selftest_tab(&mut self, ui: &mut Ui, p: &Palette) {
        egui::ScrollArea::vertical().id_salt("equashion-selftest-scroll").auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(ui.available_width().min(820.0));
            panel(ui, p, |ui| {
                ui.horizontal(|ui| {
                    h2(ui, p, "Selftest");
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if primary_button(ui, p, "Run selftest", true).clicked() {
                            self.run_selftest();
                        }
                    });
                });
                caption(ui, p, "Parses, lays out and renders every corpus item in this browser, and checks that its OMML round trip is stable.");
                ui.add_space(S12);
                let Some(rep) = &self.report else {
                    ui.label(RichText::new("Not run yet. Press Run selftest to check all corpus items.").color(p.muted));
                    return;
                };
                let fails = rep.failures().count();
                let missing: usize = rep.checks.iter().map(|c| c.missing_glyphs).sum();
                let summary =
                    format!("{} checks: {} passed, {} failed, {} missing glyphs.", rep.checks.len(), rep.checks.len() - fails, fails, missing);
                ui.label(RichText::new(summary).font(theme::semibold(T_BODY)).color(if fails > 0 { p.error } else { p.strong }));
                ui.add_space(S8);
                let mono = |s: String, c: Color32| RichText::new(s).font(FontId::monospace(T_MONO)).color(c);
                let head = |s: &str| RichText::new(s).font(theme::semibold(T_CAPTION)).color(p.muted);
                egui::Grid::new("equashion-selftest").striped(true).num_columns(5).spacing(vec2(S16, S4)).min_row_height(22.0).show(ui, |ui| {
                    ui.label(head("Item"));
                    ui.label(head("Result"));
                    num_cell(ui, head("Width pt"));
                    num_cell(ui, head("Height pt"));
                    ui.label(head("Note"));
                    ui.end_row();
                    for c in &rep.checks {
                        ui.label(mono(c.id.clone(), p.text));
                        if c.ok {
                            ui.label(RichText::new("Pass").color(p.text));
                        } else {
                            ui.label(RichText::new("Fail").font(theme::semibold(T_BODY)).color(p.error));
                        }
                        num_cell(ui, mono(format!("{:.0}", c.size.0), p.text));
                        num_cell(ui, mono(format!("{:.0}", c.size.1), p.text));
                        let mut note = c.reason.clone().unwrap_or_default();
                        if c.missing_glyphs > 0 && !note.contains("glyph") {
                            note = format!("{note} {} missing glyph(s)", c.missing_glyphs).trim().to_string();
                        }
                        ui.label(RichText::new(note).color(p.muted));
                        ui.end_row();
                    }
                });
            });
        });
    }
}

/// What the OMML page says about its input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OmmlStatus {
    /// Nothing pasted.
    Empty,
    /// Nothing that could be read as an equation.
    NoEquation,
    /// Exporting the output again gives the same OMML.
    Stable,
    /// A second import and export gives different OMML.
    Changed,
}

impl OmmlStatus {
    /// The badge text: says the result in words, not only in colour.
    pub fn badge(self) -> &'static str {
        match self {
            OmmlStatus::Empty => "No input",
            OmmlStatus::NoEquation => "No equation found",
            OmmlStatus::Stable => "Round trip stable",
            OmmlStatus::Changed => "Round trip changed",
        }
    }

    pub fn explanation(self) -> &'static str {
        match self {
            OmmlStatus::Empty => "Paste OMML on the left. The normalised form and the round-trip result appear here.",
            OmmlStatus::NoEquation => {
                "No m:oMath content could be read from this input. Import never fails, so markup that is not OMML, or is cut off, gives an empty equation. Check that the element and its m: namespace are complete."
            }
            OmmlStatus::Stable => "Importing this output and exporting it again gives the same OMML.",
            OmmlStatus::Changed => {
                "Importing this output and exporting it again gives different OMML. This is a bug in equaSHion. The output below is the first pass."
            }
        }
    }

    fn is_error(self) -> bool {
        matches!(self, OmmlStatus::NoEquation | OmmlStatus::Changed)
    }
}

fn omml_status(x: &str) -> OmmlStatus {
    if x.trim().is_empty() {
        return OmmlStatus::Empty;
    }
    if equashion_math::from_omml(x).is_empty() {
        return OmmlStatus::NoEquation;
    }
    if corpus::omml_round_trip(x).1 { OmmlStatus::Stable } else { OmmlStatus::Changed }
}

/// Map the gallery groups to row indices; rows no group names go under "Other".
fn group_rows(rows: &[Row]) -> Vec<(&'static str, Vec<usize>)> {
    let mut out: Vec<(&'static str, Vec<usize>)> =
        GALLERY_GROUPS.iter().map(|(t, ids)| (*t, ids.iter().filter_map(|id| rows.iter().position(|r| r.id == *id)).collect())).collect();
    let other: Vec<usize> = (0..rows.len()).filter(|i| !out.iter().any(|(_, ix)| ix.contains(i))).collect();
    if !other.is_empty() {
        out.push(("Other", other));
    }
    out.retain(|(_, ix)| !ix.is_empty());
    out
}

/// Two columns side by side (left gets `split` of the width) from [`TWO_COLUMNS_FROM`] up,
/// one column below. `add(ui, i)` draws column `i`.
fn columns(ui: &mut Ui, split: f32, mut add: impl FnMut(&mut Ui, usize)) {
    let w = ui.available_width();
    if w >= TWO_COLUMNS_FROM {
        let gap = S24;
        let lw = ((w - gap) * split).floor();
        let rw = w - gap - lw;
        // The full height, so scroll areas inside a column get their own max height.
        let h = ui.available_height();
        let spacing = ui.spacing().item_spacing;
        ui.horizontal_top(|ui| {
            // No item gap between the columns themselves; each column gets the normal one back.
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.allocate_ui_with_layout(vec2(lw, h), Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing = spacing;
                ui.set_width(lw);
                add(ui, 0);
            });
            ui.add_space(gap);
            ui.allocate_ui_with_layout(vec2(rw, h), Layout::top_down(Align::Min), |ui| {
                ui.spacing_mut().item_spacing = spacing;
                ui.set_width(rw);
                add(ui, 1);
            });
        });
    } else {
        add(ui, 0);
        ui.add_space(S16);
        add(ui, 1);
    }
}

/// A surface panel: 10 px corners, 1 px border, 16 px padding.
fn panel(ui: &mut Ui, p: &Palette, add: impl FnOnce(&mut Ui)) {
    Frame::new().fill(p.surface).stroke(Stroke::new(1.0, p.border)).corner_radius(R_PANEL).inner_margin(Margin::same(S16 as i8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui);
    });
}

fn h2(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(RichText::new(text).font(theme::semibold(T_H2)).color(p.strong));
}

fn caption(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(RichText::new(text).size(T_CAPTION + 1.0).color(p.muted));
}

/// An accent-filled button for the main action of a panel: darker on hover and press, and
/// a ring in the strongest text colour when it has the keyboard.
fn primary_button(ui: &mut Ui, p: &Palette, text: &str, enabled: bool) -> egui::Response {
    ui.scope(|ui| {
        let w = &mut ui.visuals_mut().widgets;
        for (v, fill) in [(&mut w.inactive, p.accent), (&mut w.hovered, p.accent_hover), (&mut w.active, p.accent_pressed)] {
            v.bg_fill = fill;
            v.weak_bg_fill = fill;
            v.bg_stroke = Stroke::new(1.0, fill);
            v.fg_stroke = Stroke::new(1.0, p.on_accent);
        }
        w.active.bg_stroke = Stroke::new(2.0, p.strong);
        ui.add_enabled(enabled, egui::Button::new(text).wrap_mode(egui::TextWrapMode::Extend))
    })
    .inner
}

/// A right-aligned table cell of fixed width, for numbers.
fn num_cell(ui: &mut Ui, text: RichText) {
    ui.allocate_ui_with_layout(vec2(64.0, 20.0), Layout::right_to_left(Align::Center), |ui| {
        ui.add_space(S8);
        ui.label(text)
    });
}

/// A status badge: the result in words on a tinted pill.
fn badge(ui: &mut Ui, p: &Palette, text: &str, error: bool) -> egui::Response {
    let (fill, fg) = if error { (p.error_soft, p.error) } else { (p.accent_soft, p.accent_text) };
    Frame::new()
        .fill(fill)
        .corner_radius(R_CONTROL)
        .inner_margin(Margin::symmetric(S8 as i8, S4 as i8))
        .show(ui, |ui| ui.label(RichText::new(text).font(theme::semibold(T_CAPTION + 1.0)).color(fg)))
        .response
}

/// A tab or theme segment: text on a tinted background when selected, a hover fill, and the
/// accent focus ring when it has the keyboard.
fn segment(ui: &mut Ui, p: &Palette, selected: bool, text: &str, hint: Option<&str>) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(T_BODY), p.text);
    let size = vec2(galley.size().x + 2.0 * S12, 32.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, text));
    if ui.is_rect_visible(rect) {
        let pressed = resp.is_pointer_button_down_on();
        let (fill, fg) = if selected {
            (p.accent_soft, p.accent_text)
        } else if pressed {
            (p.control_active, p.strong)
        } else if resp.hovered() {
            (p.control_hover, p.strong)
        } else {
            (Color32::TRANSPARENT, p.text)
        };
        let painter = ui.painter();
        painter.rect_filled(rect, R_CONTROL, fill);
        if resp.has_focus() {
            painter.rect_stroke(rect, R_CONTROL, Stroke::new(2.0, p.accent), StrokeKind::Inside);
        }
        let pos = rect.center() - galley.size() / 2.0;
        painter.galley_with_override_text_color(pos2(pos.x.round(), pos.y.round()), galley, fg);
    }
    match hint {
        Some(h) => resp.on_hover_text(h),
        None => resp,
    }
}

/// How much a thumbnail of `size` shrinks in a card `w` wide.
fn fit(size: Vec2, w: f32) -> f32 {
    ((w - 2.0 * S12) / size.x.max(1.0)).min(PIC_H / size.y.max(1.0)).min(1.0)
}

/// A gallery card at `rect`: the equation (shrunk to fit), its id in mono and its source,
/// elided at the card edge.
fn card(ui: &mut Ui, p: &Palette, th: &Thumb, row: &Row, rect: Rect) -> egui::Response {
    let resp = ui.interact(rect, ui.id().with(("equashion-card", row.id)), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, row.id));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let hovered = resp.hovered() || resp.has_focus();
        let fill = if resp.is_pointer_button_down_on() { p.control_hover } else { p.surface };
        let stroke = if hovered { Stroke::new(1.0, p.accent) } else { Stroke::new(1.0, p.border) };
        painter.rect(rect, CornerRadius::same(R_PANEL), fill, stroke, StrokeKind::Inside);
        if resp.has_focus() {
            painter.rect_stroke(rect, R_PANEL, Stroke::new(2.0, p.accent), StrokeKind::Inside);
        }
        let pic = Rect::from_min_size(rect.min + vec2(S12, S12), vec2(rect.width() - 2.0 * S12, PIC_H));
        let r = Rect::from_center_size(pic.center(), th.size * fit(th.size, rect.width()));
        painter.image(th.tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), p.strong);
        let rule_y = pic.max.y + S8;
        painter.hline(rect.min.x + 1.0..=rect.max.x - 1.0, rule_y, Stroke::new(1.0, p.border));
        let text_w = rect.width() - 2.0 * S12;
        let line = |text: &str, color: Color32| {
            let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), FontId::monospace(T_CAPTION), color);
            job.wrap = egui::text::TextWrapping::truncate_at_width(text_w);
            ui.fonts_mut(|f| f.layout_job(job))
        };
        let x = rect.min.x + S12;
        painter.galley(pos2(x, rule_y + S8), line(row.id, p.text), p.text);
        painter.galley(pos2(x, rule_y + S8 + 16.0 + S4), line(row.linear, p.muted), p.muted);
    }
    resp.on_hover_text(row.linear)
}

fn footer(ui: &mut Ui, p: &Palette) {
    credit(ui, p, T_CAPTION, p.muted);
}

/// [`CREDIT`] with the MathLive address as a link.
fn credit(ui: &mut Ui, p: &Palette, size: f32, color: Color32) {
    let (before, rest) = CREDIT.split_once(MATHLIVE_SHOWN).unwrap_or((CREDIT, ""));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 2.0);
        ui.label(RichText::new(before).size(size).color(color));
        ui.hyperlink_to(RichText::new(MATHLIVE_SHOWN).size(size).color(p.accent), MATHLIVE_URL);
        ui.label(RichText::new(rest).size(size).color(color));
    });
}

fn about_tab(ui: &mut Ui, p: &Palette) {
    egui::ScrollArea::vertical().id_salt("equashion-about-scroll").auto_shrink([false, false]).show(ui, |ui| {
        ui.set_max_width(ui.available_width().min(760.0));
        let body = |ui: &mut Ui, s: &str| {
            ui.label(RichText::new(s).color(p.text));
        };
        let section = |ui: &mut Ui, title: &str| {
            ui.add_space(S24);
            h2(ui, p, title);
            ui.add_space(S4);
        };
        ui.label(RichText::new("equaSHion").font(theme::semibold(theme::T_DISPLAY)).color(p.strong));
        ui.label(RichText::new(TAGLINE).color(p.muted));

        section(ui, "What it is");
        body(
            ui,
            "An equation editor written in Rust: an egui widget, a layout engine, and OMML import and export for Word and PowerPoint equations. It runs natively and in the browser as WebAssembly; this page is the WebAssembly build, drawn with WebGL2.",
        );

        section(ui, "What is inside");
        let crates = [
            ("equashion-math", "math tree, OMML import and export, linear text format, edit model"),
            ("equashion-font", "bundled fonts, glyph fallback and text shaping"),
            ("equashion-layout", "TeX-style layout into a backend-neutral scene, with caret geometry"),
            ("equashion-render", "the scene as RGBA pixels (vello_cpu) or as SVG"),
            ("equashion-egui, equashion-web", "the editor as an egui widget, and this page"),
        ];
        egui::Grid::new("equashion-crates").num_columns(2).spacing(vec2(S16, S4)).show(ui, |ui| {
            for (name, what) in crates {
                ui.label(RichText::new(name).font(FontId::monospace(T_MONO)).color(p.strong));
                ui.label(RichText::new(what).color(p.text));
                ui.end_row();
            }
        });

        section(ui, "Credits and licence");
        credit(ui, p, T_BODY, p.text);
        ui.add_space(S8);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let t = |s: &str| RichText::new(s).color(p.text);
            ui.label(t("Engine code: the equation editor of "));
            ui.hyperlink_to(RichText::new("WordCraft").color(p.accent), "https://github.com/storytold/wordcraft");
            ui.label(t(" (copyright the ArtCraft Team and the WordCraft contributors), with three edit-model tests and their rules from "));
            ui.hyperlink_to(RichText::new("DeckCraft").color(p.accent), "https://github.com/storytold/deckcraft");
            ui.label(t(". Both were taken from the shuff57 forks, at WordCraft dbfe1b1 and DeckCraft 7972cf5. PROVENANCE.md lists every file taken and changed."));
        });
        ui.add_space(S8);
        body(
            ui,
            "Fonts, all under the SIL Open Font License 1.1: STIX Two Math, Source Serif 4 and Inter for equations; Geist and Geist Mono for this interface. ATTRIBUTION.md names each file and its source.",
        );
        ui.add_space(S8);
        body(ui, "Code licence: MIT OR Apache-2.0, at your option (LICENSE-MIT, LICENSE-APACHE, NOTICE).");

        section(ui, "Known limits");
        for item in [
            "Typed \\left and \\right are ignored. Typed ( ), [ ], { } and | | pairs still stretch, but \\langle, \\lfloor and \\lceil give plain characters, so stretchy angle, floor and ceiling brackets only come from OMML.",
            "linear() returns the normalised linear form, not the text you typed: \\frac{a}{b} reads back as a/b.",
            "Unknown backslash commands are kept as plain text and listed under Unknown commands.",
            "In the browser only the bundled fonts are used. A character none of them has is drawn as a missing glyph.",
            "There is no PNG or JPEG export of rendered equations yet.",
            "The page is a single WebGL2 canvas: screen readers and browser find do not see its text.",
        ] {
            ui.horizontal_top(|ui| {
                ui.label(RichText::new("-").color(p.muted));
                ui.add(egui::Label::new(RichText::new(item).color(p.text)).wrap());
            });
        }
        ui.add_space(S16);
    });
}
