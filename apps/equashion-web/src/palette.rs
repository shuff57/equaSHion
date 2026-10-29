//! The page's symbol palette: a tab bar above the editor (Structures, Greek letters, ... in the
//! order of Google Docs' equation toolbar),
//! each tab a few labelled groups of large buttons. Every face is laid out and rasterised by
//! equashion-render from the bundled fonts (never egui's fonts), tinted with the theme's text
//! colour. A button inserts at the editor's caret through `equashion_math::Editor`.

use std::collections::HashMap;

use egui::{Sense, TextureHandle, TextureOptions, Ui, Vec2, pos2, vec2};
use equashion_layout::{DEFAULT_FAMILY, FontDb, Rgb, layout_equation};
use equashion_math::{Editor, Math, Node, matrix_template, template, to_linear};

/// What a palette button inserts.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// A structure with empty places (a template id of `equashion_math::templates`, or a
    /// bracket pair built here).
    Structure { id: &'static str, label: &'static str, math: Math },
    /// One character.
    Symbol(char),
}

impl Item {
    /// The accessible label (and the test id).
    pub fn label(&self) -> String {
        match self {
            Item::Structure { label, .. } => (*label).to_string(),
            Item::Symbol(c) => c.to_string(),
        }
    }

    fn key(&self) -> String {
        match self {
            Item::Structure { id, .. } => format!("tpl-{id}"),
            Item::Symbol(c) => format!("sym-{:x}", *c as u32),
        }
    }

    fn hover(&self) -> String {
        match self {
            Item::Structure { label, math, .. } => format!("{label}: {}", to_linear(math)),
            Item::Symbol(c) => format!("{c}  U+{:04X}", *c as u32),
        }
    }
}

/// A labelled row of buttons.
pub struct Group {
    pub title: &'static str,
    pub items: Vec<Item>,
}

/// One tab of the palette: a context and the groups of buttons in it.
pub struct Tab {
    pub title: &'static str,
    pub groups: Vec<Group>,
}

fn tpl(id: &'static str, label: &'static str) -> Option<Item> {
    template(id).map(|t| Item::Structure { id, label, math: t.math })
}

/// A bracket pair around one empty place.
fn pair(id: &'static str, label: &'static str, open: &str, close: &str) -> Item {
    let math = Math::new(vec![Node::Delim { open: open.into(), close: close.into(), sep: "|".into(), items: vec![vec![Node::Text(String::new())]] }]);
    Item::Structure { id, label, math }
}

fn symbols(s: &str) -> Vec<Item> {
    s.chars().map(Item::Symbol).collect()
}

/// A group of templates (an id with no template is left out).
fn templates(title: &'static str, items: &[(&'static str, &'static str)]) -> Group {
    Group { title, items: items.iter().filter_map(|(id, label)| tpl(id, label)).collect() }
}

/// The group titled "Matrices and cases": the m x n builder is drawn right under it.
pub const MATRIX_GROUP: &str = "Matrices and cases";

/// Every palette button, in tabs by context.
pub fn tabs() -> Vec<Tab> {
    let sym = |title, s| Group { title, items: symbols(s) };
    vec![
        Tab {
            title: "Structures",
            groups: vec![
                templates(
                    "Fractions and roots",
                    &[("frac", "Fraction"), ("binom", "Binomial coefficient"), ("sqrt", "Square root"), ("root", "Nth root")],
                ),
                templates("Scripts", &[("sup", "Superscript"), ("sub", "Subscript"), ("subsup", "Sub and superscript")]),
                templates(
                    "Large operators",
                    &[("sum", "Sum"), ("prod", "Product"), ("int", "Integral"), ("intlim", "Definite integral"), ("lim", "Limit")],
                ),
                Group {
                    title: "Brackets",
                    items: [tpl("paren", "Parentheses"), tpl("bracket", "Square brackets"), tpl("brace", "Braces"), tpl("abs", "Absolute value")]
                        .into_iter()
                        .flatten()
                        .chain([
                            pair("norm", "Norm", "\u{2016}", "\u{2016}"),
                            pair("floor", "Floor", "\u{230A}", "\u{230B}"),
                            pair("ceil", "Ceiling", "\u{2308}", "\u{2309}"),
                            pair("angle", "Angle brackets", "\u{27E8}", "\u{27E9}"),
                        ])
                        .collect(),
                },
                templates(
                    MATRIX_GROUP,
                    &[
                        ("matrix2", "2 by 2 matrix"),
                        ("bmatrix2", "2 by 2 matrix, square brackets"),
                        ("matrix3", "3 by 3 matrix"),
                        ("bmatrix3", "3 by 3 matrix, square brackets"),
                        ("cases", "Cases"),
                    ],
                ),
                templates(
                    "Accents",
                    &[
                        ("hat", "Hat"),
                        ("bar", "Bar"),
                        ("vec", "Vector arrow"),
                        ("overline", "Overline"),
                        ("underline", "Underline"),
                        ("overbrace", "Overbrace"),
                    ],
                ),
            ],
        },
        Tab {
            title: "Greek letters", groups: vec![sym("Lowercase", "αβγδεζηθικλμνξοπρστυφχψω"), sym("Uppercase", "ΓΔΘΛΞΠΣΦΨΩ")]
        },
        Tab { title: "Special symbols", groups: vec![sym("Calculus", "∞∂∇°′″"), sym("Logic", "∀∃∅∴∵"), sym("Dots", "…⋯")] },
        Tab {
            title: "Relations", groups: vec![sym("Equality", "≠≈≡≅∼∝"), sym("Order", "≤≥≪≫≺≻"), sym("Membership and subsets", "∈∉⊂⊆⊃⊇")]
        },
        Tab { title: "Math operations", groups: vec![sym("Arithmetic", "±∓×÷·∘∗"), sym("Sets and logic", "∪∩∧∨¬")] },
        Tab { title: "Arrows", groups: vec![sym("Single", "→←↑↓↔↦"), sym("Double", "⇒⇐⇔")] },
        Tab {
            title: "Functions",
            groups: vec![
                templates("Trigonometric", &[("sin", "Sine"), ("cos", "Cosine"), ("tan", "Tangent")]),
                templates(
                    "Hyperbolic",
                    &[
                        ("sinh", "Hyperbolic sine"),
                        ("cosh", "Hyperbolic cosine"),
                        ("tanh", "Hyperbolic tangent"),
                        ("csch", "Hyperbolic cosecant"),
                        ("sech", "Hyperbolic secant"),
                        ("coth", "Hyperbolic cotangent"),
                    ],
                ),
                templates("Logarithms", &[("log", "Logarithm"), ("ln", "Natural log")]),
                templates(
                    "Counting and rounding",
                    &[("nCr", "Combinations"), ("nPr", "Permutations"), ("lcm", "Least common multiple"), ("round", "Round"), ("sign", "Sign")],
                ),
                templates("Statistics", &[("mean", "Mean"), ("median", "Median"), ("stdev", "Standard deviation"), ("var", "Variance")]),
            ],
        },
    ]
}

/// A rendered button face.
struct Face {
    tex: TextureHandle,
    /// Size in points.
    size: Vec2,
}

/// Smallest button side, points.
pub const BUTTON: f32 = 36.0;
/// Tallest face inside a button, points.
const FACE_MAX_H: f32 = 26.0;
/// Symbol and structure face sizes, points.
const SYMBOL_PT: f64 = 20.0;
const STRUCTURE_PT: f64 = 18.0;

/// The palette state: the faces rendered so far and the selected tab.
pub struct Palette {
    tabs: Vec<Tab>,
    faces: HashMap<String, Face>,
    /// Index of the selected tab.
    pub tab: usize,
    /// The size and brackets the "Insert matrix" button builds.
    pub mat_rows: usize,
    pub mat_cols: usize,
    pub mat_square: bool,
}

impl Default for Palette {
    fn default() -> Self {
        Palette { tabs: tabs(), faces: HashMap::new(), tab: 0, mat_rows: 2, mat_cols: 3, mat_square: false }
    }
}

impl Palette {
    /// The titles of the tabs, in order.
    pub fn tab_titles(&self) -> Vec<&'static str> {
        self.tabs.iter().map(|t| t.title).collect()
    }

    /// Select the tab with this title; false when there is none.
    pub fn select_tab(&mut self, title: &str) -> bool {
        match self.tabs.iter().position(|t| t.title == title) {
            Some(i) => {
                self.tab = i;
                true
            }
            None => false,
        }
    }

    fn face(&mut self, ui: &Ui, fonts: &FontDb, item: &Item) -> Option<&Face> {
        let ppp = ui.ctx().pixels_per_point();
        let key = format!("{}@{ppp}", item.key());
        if !self.faces.contains_key(&key) {
            let (math, pt) = match item {
                // The structure with its empty places drawn as boxes.
                Item::Structure { math, .. } => (Editor::new(math.clone()).display_math(), STRUCTURE_PT),
                Item::Symbol(c) => (Math::new(vec![Node::Text(c.to_string())]), SYMBOL_PT),
            };
            let l = layout_equation(fonts, &math, DEFAULT_FAMILY, pt, false, Rgb::WHITE, 1.0);
            let img = equashion_render::render_scene(&l.scene, fonts, f64::from(ppp.max(1.0)) * 2.0, None);
            let ci = egui::ColorImage::from_rgba_premultiplied([img.width as usize, img.height as usize], &img.pixels);
            let tex = ui.ctx().load_texture(format!("equashion/page-palette/{}", item.key()), ci, TextureOptions::LINEAR);
            self.faces.insert(key.clone(), Face { tex, size: vec2(l.width() as f32, l.height() as f32) });
        }
        self.faces.get(&key)
    }

    /// Draw the tab bar and the selected tab's groups; returns the item clicked this frame.
    pub fn ui(&mut self, ui: &mut Ui, fonts: &FontDb, p: &crate::theme::Palette) -> Option<Item> {
        let mut clicked = None;
        let titles = self.tab_titles();
        if let Some(i) = tab_bar(ui, p, &titles, self.tab) {
            self.tab = i;
        }
        ui.add_space(crate::theme::S8);
        let tab = self.tab.min(self.tabs.len().saturating_sub(1));
        let groups = std::mem::take(&mut self.tabs[tab].groups);
        for g in &groups {
            if let Some(i) = self.group(ui, fonts, p, g) {
                clicked = Some(i);
            }
            if g.title == MATRIX_GROUP
                && let Some(i) = self.matrix_builder(ui, p)
            {
                clicked = Some(i);
            }
        }
        self.tabs[tab].groups = groups;
        clicked
    }

    /// The m x n matrix builder: rows, columns, bracket style and an Insert button.
    fn matrix_builder(&mut self, ui: &mut Ui, p: &crate::theme::Palette) -> Option<Item> {
        const LABEL_W: f32 = 112.0;
        let mut clicked = None;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = vec2(crate::theme::S8, crate::theme::S4);
            ui.allocate_ui_with_layout(vec2(LABEL_W, BUTTON), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_width(LABEL_W);
                ui.label(egui::RichText::new("Matrix").size(crate::theme::T_BODY).color(p.muted));
            });
            ui.add(egui::DragValue::new(&mut self.mat_rows).range(1..=10).speed(0.1)).on_hover_text("Rows");
            ui.label(egui::RichText::new("\u{00D7}").size(crate::theme::T_BODY).color(p.muted));
            ui.add(egui::DragValue::new(&mut self.mat_cols).range(1..=10).speed(0.1)).on_hover_text("Columns");
            for (square, text) in [(false, "( )"), (true, "[ ]")] {
                if ui.selectable_label(self.mat_square == square, text).on_hover_text("Bracket style").clicked() {
                    self.mat_square = square;
                }
            }
            let hover = format!("Insert an empty {} by {} matrix", self.mat_rows, self.mat_cols);
            if ui.button("Insert matrix").on_hover_text(hover).clicked() {
                let (open, close) = if self.mat_square { ("[", "]") } else { ("(", ")") };
                let math = matrix_template(self.mat_rows, self.mat_cols, open, close);
                clicked = Some(Item::Structure { id: "matrix-mn", label: "Matrix", math });
            }
        });
        ui.add_space(crate::theme::S8);
        clicked
    }

    /// One group: its label on the left (above the buttons when the column is narrow) and the
    /// buttons, wrapping.
    fn group(&mut self, ui: &mut Ui, fonts: &FontDb, p: &crate::theme::Palette, g: &Group) -> Option<Item> {
        const LABEL_W: f32 = 112.0;
        let narrow = ui.available_width() < 640.0;
        let mut clicked = None;
        let title = egui::RichText::new(g.title).size(crate::theme::T_BODY).color(p.muted);
        ui.horizontal_top(|ui| {
            if narrow {
                ui.vertical(|ui| {
                    ui.label(title);
                    ui.add_space(crate::theme::S4);
                    clicked = self.buttons(ui, fonts, g);
                });
            } else {
                ui.allocate_ui_with_layout(vec2(LABEL_W, BUTTON), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_width(LABEL_W);
                    ui.label(title);
                });
                clicked = self.buttons(ui, fonts, g);
            }
        });
        ui.add_space(crate::theme::S8);
        clicked
    }

    fn buttons(&mut self, ui: &mut Ui, fonts: &FontDb, g: &Group) -> Option<Item> {
        let mut clicked = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(crate::theme::S4, crate::theme::S4);
            for item in &g.items {
                let Some(face) = self.face(ui, fonts, item) else { continue };
                if button(ui, face, &item.label(), &item.hover()).clicked() {
                    clicked = Some(item.clone());
                }
            }
        });
        clicked
    }
}

/// A palette button: the face tinted with the text colour, the theme's button frame, an
/// accessible label. Clickable but not focusable, so a click leaves the keyboard with the editor.
fn button(ui: &mut Ui, face: &Face, label: &str, hover: &str) -> egui::Response {
    let k = (FACE_MAX_H / face.size.y.max(1.0)).min(1.0);
    let f = face.size * k;
    let size = vec2((f.x + 16.0).max(BUTTON), BUTTON);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::CLICK);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label));
    if ui.is_rect_visible(rect) {
        let vis = ui.style().interact(&resp);
        ui.painter().rect(rect, vis.corner_radius, vis.weak_bg_fill, vis.bg_stroke, egui::StrokeKind::Inside);
        let r = egui::Rect::from_center_size(rect.center(), f);
        ui.painter().image(face.tex.id(), r, egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), vis.text_color());
    }
    resp.on_hover_text(hover)
}

/// A tab bar: one label per tab, the selected one in the accent colour over a 2 px accent
/// underline, a hairline under the row. Returns the tab clicked this frame.
fn tab_bar(ui: &mut Ui, p: &crate::theme::Palette, titles: &[&'static str], selected: usize) -> Option<usize> {
    let mut clicked = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(crate::theme::S4, crate::theme::S4);
        for (i, title) in titles.iter().enumerate() {
            let on = i == selected;
            let font = egui::FontId::proportional(crate::theme::T_BODY);
            let galley = ui.painter().layout_no_wrap((*title).to_string(), font, p.muted);
            let (rect, resp) = ui.allocate_exact_size(vec2(galley.size().x + 20.0, 32.0), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Tab, true, on, *title));
            if ui.is_rect_visible(rect) {
                let painter = ui.painter();
                if resp.hovered() && !on {
                    painter.rect_filled(rect, crate::theme::R_CONTROL, p.control_hover);
                }
                let color = if on {
                    p.accent
                } else if resp.hovered() {
                    ui.visuals().text_color()
                } else {
                    p.muted
                };
                let pos = rect.center() - galley.size() / 2.0;
                painter.galley_with_override_text_color(pos2(pos.x.round(), pos.y.round()), galley, color);
                if on {
                    painter.hline(rect.min.x + 6.0..=rect.max.x - 6.0, rect.max.y - 1.0, egui::Stroke::new(2.0, p.accent));
                }
                if resp.has_focus() {
                    painter.rect_stroke(rect, crate::theme::R_CONTROL, egui::Stroke::new(2.0, p.accent), egui::StrokeKind::Inside);
                }
            }
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                clicked = Some(i);
            }
        }
    });
    clicked
}

/// A disclosure line: a chevron and a label, accent coloured, toggled by a click.
pub fn disclosure(ui: &mut Ui, p: &crate::theme::Palette, label: &str, open: bool) -> egui::Response {
    let font = egui::FontId::proportional(crate::theme::T_BODY);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, p.accent);
    let size = vec2(galley.size().x + 22.0, 28.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::DisclosureTriangle, true, label));
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let c = pos2(rect.min.x + 7.0, rect.center().y);
        let pts = if open {
            vec![pos2(c.x - 4.5, c.y - 2.5), pos2(c.x + 4.5, c.y - 2.5), pos2(c.x, c.y + 3.0)]
        } else {
            vec![pos2(c.x - 2.5, c.y - 4.5), pos2(c.x + 3.0, c.y), pos2(c.x - 2.5, c.y + 4.5)]
        };
        let color = if resp.hovered() { p.accent_hover } else { p.accent };
        painter.add(egui::Shape::convex_polygon(pts, color, egui::Stroke::NONE));
        let pos = pos2(rect.min.x + 18.0, rect.center().y - galley.size().y / 2.0);
        painter.galley_with_override_text_color(pos2(pos.x.round(), pos.y.round()), galley, color);
        if resp.hovered() {
            let y = pos.y.round() + 18.0;
            painter.hline(rect.min.x + 18.0..=rect.max.x - 4.0, y, egui::Stroke::new(1.0, color));
        }
        if resp.has_focus() {
            painter.rect_stroke(rect, crate::theme::R_CONTROL, egui::Stroke::new(2.0, p.accent), egui::StrokeKind::Inside);
        }
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

#[cfg(test)]
mod tests {
    use super::*;
    use equashion_math::{from_linear, to_latex};

    fn all_items() -> Vec<(&'static str, Item)> {
        tabs().into_iter().flat_map(|t| t.groups.into_iter().flat_map(|g| g.items.into_iter().map(move |i| (g.title, i)))).collect()
    }

    #[test]
    fn tabs_group_every_button_by_context() {
        let t = tabs();
        assert_eq!(
            t.iter().map(|t| t.title).collect::<Vec<_>>(),
            ["Structures", "Greek letters", "Special symbols", "Relations", "Math operations", "Arrows", "Functions"]
        );
        let structures = &t[0].groups;
        assert_eq!(
            structures.iter().map(|g| (g.title, g.items.len())).collect::<Vec<_>>(),
            [("Fractions and roots", 4), ("Scripts", 3), ("Large operators", 5), ("Brackets", 8), (MATRIX_GROUP, 5), ("Accents", 6)],
            "a template id is missing"
        );
        // A group title never repeats a tab title, so each label names one thing.
        for g in t.iter().flat_map(|t| &t.groups) {
            assert!(!t.iter().any(|x| x.title == g.title), "{}", g.title);
            assert!(!g.items.is_empty(), "{}", g.title);
        }
    }

    #[test]
    fn every_structure_inserts_and_bracket_pairs_round_trip() {
        for (title, i) in all_items() {
            if let Item::Structure { math, label, .. } = i {
                let mut e = Editor::new(Math::default());
                e.type_str("x");
                e.insert_template(&math);
                assert!(e.is_valid(), "{title}: {label}");
            }
        }
        // The bracket pairs built here print as stretchy \left..\right LaTeX. (Read back, the
        // parser keeps them as plain characters: a known limit listed in About.)
        let items = all_items();
        for (want, id) in [("Vert", "norm"), ("lfloor", "floor"), ("lceil", "ceil"), ("langle", "angle")] {
            let found = items.iter().find_map(|(_, i)| match i {
                Item::Structure { id: x, math, .. } if *x == id => Some(math),
                _ => None,
            });
            assert!(found.is_some(), "{id}");
            let Some(math) = found else { continue };
            let mut e = Editor::new(Math::default());
            e.insert_template(math);
            e.type_str("x");
            let tex = to_latex(e.math());
            assert!(tex.starts_with(&format!("\\left\\{want}")) && tex.contains("\\right"), "{tex}");
            assert!(!to_latex(&from_linear(&tex)).is_empty());
        }
    }

    #[test]
    fn no_symbol_is_listed_twice() {
        let mut seen = std::collections::HashSet::new();
        for (_, i) in all_items() {
            assert!(seen.insert(i.key()), "{} twice", i.label());
        }
    }

    #[test]
    fn a_tab_can_be_selected_by_title() {
        let mut p = Palette::default();
        assert!(p.select_tab("Functions") && p.tab == 6);
        assert!(!p.select_tab("Nope") && p.tab == 6);
    }
}
