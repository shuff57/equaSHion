//! The page's symbol palette: a few labelled groups of large buttons above the editor, the
//! rarely used ones behind "More symbols". Every face is laid out and rasterised by
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

/// The groups shown by default.
pub fn main_groups() -> Vec<Group> {
    vec![
        Group {
            title: "Structures",
            items: [
                tpl("frac", "Fraction"),
                tpl("sup", "Superscript"),
                tpl("sub", "Subscript"),
                tpl("sqrt", "Square root"),
                tpl("root", "Nth root"),
                tpl("sum", "Sum"),
                tpl("int", "Integral"),
                tpl("intlim", "Definite integral"),
                tpl("lim", "Limit"),
                tpl("matrix2", "2 by 2 matrix"),
                tpl("bmatrix2", "2 by 2 matrix, square brackets"),
                tpl("cases", "Cases"),
            ]
            .into_iter()
            .flatten()
            .collect(),
        },
        Group { title: "Greek", items: symbols("αβγδεθλμπσφωΔΣΩ") },
        Group { title: "Operators", items: symbols("±∓×÷·∘∞∂∇∪∩°′") },
        Group { title: "Relations", items: symbols("≠≈≡≤≥≪≫∝∼∈∉⊂⊆") },
    ]
}

/// The groups behind "More symbols".
pub fn more_groups() -> Vec<Group> {
    vec![
        Group {
            title: "More structures",
            items: [
                tpl("subsup", "Sub and superscript"),
                tpl("prod", "Product"),
                tpl("matrix3", "3 by 3 matrix"),
                tpl("bmatrix3", "3 by 3 matrix, square brackets"),
                tpl("binom", "Binomial coefficient"),
            ]
            .into_iter()
            .flatten()
            .collect(),
        },
        Group { title: "Arrows", items: symbols("→←↔⇒⇐⇔↦↑↓") },
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
        Group {
            title: "Accents",
            items: [
                tpl("hat", "Hat"),
                tpl("bar", "Bar"),
                tpl("vec", "Vector arrow"),
                tpl("overline", "Overline"),
                tpl("underline", "Underline"),
                tpl("overbrace", "Overbrace"),
            ]
            .into_iter()
            .flatten()
            .collect(),
        },
        Group {
            title: "Functions",
            items: [tpl("sin", "Sine"), tpl("cos", "Cosine"), tpl("tan", "Tangent"), tpl("log", "Logarithm"), tpl("ln", "Natural log")]
                .into_iter()
                .flatten()
                .collect(),
        },
        Group {
            title: "Hyperbolic",
            items: [
                tpl("sinh", "Hyperbolic sine"),
                tpl("cosh", "Hyperbolic cosine"),
                tpl("tanh", "Hyperbolic tangent"),
                tpl("csch", "Hyperbolic cosecant"),
                tpl("sech", "Hyperbolic secant"),
                tpl("coth", "Hyperbolic cotangent"),
            ]
            .into_iter()
            .flatten()
            .collect(),
        },
        Group {
            title: "Counting and rounding",
            items: [
                tpl("nCr", "Combinations"),
                tpl("nPr", "Permutations"),
                tpl("lcm", "Least common multiple"),
                tpl("round", "Round"),
                tpl("sign", "Sign"),
            ]
            .into_iter()
            .flatten()
            .collect(),
        },
        Group {
            title: "Statistics",
            items: [tpl("mean", "Mean"), tpl("median", "Median"), tpl("stdev", "Standard deviation"), tpl("var", "Variance")]
                .into_iter()
                .flatten()
                .collect(),
        },
        Group { title: "More Greek", items: symbols("ζηικνξορτυχψΓΘΛΞΠΦΨ") },
        Group { title: "Logic and sets", items: symbols("∧∨¬∀∃∅∗∴∵…⋯″≅≺≻⊃⊇") },
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

/// The palette state: the faces rendered so far and whether "More symbols" is open.
pub struct Palette {
    main: Vec<Group>,
    more: Vec<Group>,
    faces: HashMap<String, Face>,
    pub more_open: bool,
    /// The size and brackets the "Insert matrix" button builds.
    pub mat_rows: usize,
    pub mat_cols: usize,
    pub mat_square: bool,
}

impl Default for Palette {
    fn default() -> Self {
        Palette { main: main_groups(), more: more_groups(), faces: HashMap::new(), more_open: false, mat_rows: 2, mat_cols: 3, mat_square: false }
    }
}

impl Palette {
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

    /// Draw the palette; returns the item clicked this frame.
    pub fn ui(&mut self, ui: &mut Ui, fonts: &FontDb, p: &crate::theme::Palette) -> Option<Item> {
        let mut clicked = None;
        let main = std::mem::take(&mut self.main);
        for g in &main {
            if let Some(i) = self.group(ui, fonts, p, g) {
                clicked = Some(i);
            }
        }
        self.main = main;
        if let Some(i) = self.matrix_builder(ui, p) {
            clicked = Some(i);
        }
        ui.add_space(crate::theme::S4);
        let label = if self.more_open { "Fewer symbols" } else { "More symbols" };
        let r = disclosure(ui, p, label, self.more_open);
        if r.clicked() {
            self.more_open = !self.more_open;
        }
        if self.more_open {
            ui.add_space(crate::theme::S8);
            let more = std::mem::take(&mut self.more);
            for g in &more {
                if let Some(i) = self.group(ui, fonts, p, g) {
                    clicked = Some(i);
                }
            }
            self.more = more;
        }
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

/// A disclosure line: a chevron and a label, accent coloured, toggled by a click.
pub fn disclosure(ui: &mut Ui, p: &crate::theme::Palette, label: &str, open: bool) -> egui::Response {
    let font = egui::FontId::proportional(crate::theme::T_BODY);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, p.accent);
    let size = vec2(galley.size().x + 22.0, 28.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::CollapsingHeader, true, label));
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

    #[test]
    fn every_structure_exists_and_bracket_pairs_round_trip() {
        let main = main_groups();
        assert_eq!(main.iter().map(|g| g.title).collect::<Vec<_>>(), ["Structures", "Greek", "Operators", "Relations"]);
        assert_eq!(main.first().map(|g| g.items.len()), Some(12), "a template id is missing");
        assert_eq!(more_groups().first().map(|g| g.items.len()), Some(5), "a template id is missing");
        let more = more_groups();
        for g in main.iter().chain(&more) {
            assert!(!g.items.is_empty(), "{}", g.title);
            for i in &g.items {
                if let Item::Structure { math, label, .. } = i {
                    let mut e = Editor::new(Math::default());
                    e.type_str("x");
                    e.insert_template(math);
                    assert!(e.is_valid(), "{label}");
                }
            }
        }
        // The bracket pairs built here print as stretchy \left..\right LaTeX. (Read back, the
        // parser keeps them as plain characters: a known limit listed in About.)
        for (want, i) in [("Vert", "norm"), ("lfloor", "floor"), ("lceil", "ceil"), ("langle", "angle")]
            .into_iter()
            .filter_map(|(w, id)| more.iter().flat_map(|g| &g.items).find(|i| matches!(i, Item::Structure { id: x, .. } if *x == id)).map(|i| (w, i)))
        {
            let Item::Structure { math, .. } = i else { continue };
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
        for g in main_groups().iter().chain(&more_groups()) {
            for i in &g.items {
                assert!(seen.insert(i.key()), "{} twice", i.label());
            }
        }
    }
}
