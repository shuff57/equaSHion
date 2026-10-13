//! What the Copy buttons put on the clipboard and what a paste reads back. Pure functions of
//! the equation tree, so the native tests check every payload without a browser.
//!
//! | Kind | MIME types written | Content |
//! |---|---|---|
//! | `latex` | `text/plain` | [`to_latex`] |
//! | `mathml` (alias `word`) | `text/plain`, `text/html` | MathML: the `<math xmlns=".." display="block">` element as text, and the same element in an HTML fragment (`<!--StartFragment-->`) with nothing else in it |
//! | `omml` | `text/plain` | OMML: `m:oMathPara` / `m:oMath` with its namespace ([`to_omml`]) |
//! | `text` | `text/plain` | [`to_linear`] |
//! | `image` | `image/png` | black ink on white, 2 pixels per point, 144 dpi |
//! | `svg` | `image/svg+xml` (a download, not the clipboard) | `equashion-render`'s SVG backend, black ink |
//!
//! Where the MathML was checked (see the README): pasted into ONLYOFFICE Desktop Editors 9.4
//! (it reads the `text/html` copy and makes an equation; the `text/plain` copy alone stays text),
//! and opened as a `.mml` file in LibreOffice 26.2 Math.

use equashion_layout::{DEFAULT_FAMILY, FontDb, Rgb, layout_equation};
use equashion_math::{Math, from_linear, from_mathml, from_omml, to_latex, to_linear, to_mathml_display, to_omml, unknown_commands};

/// Equation size of the image and SVG exports, points.
pub const EXPORT_PT: f64 = 20.0;
/// Pixels per point of the PNG export.
pub const PNG_SCALE: f64 = 2.0;
/// Space around the equation in the image and SVG exports, points.
const EXPORT_PAD: f64 = 6.0;
/// Pasted or imported text longer than this is refused (characters).
pub const MAX_IMPORT: usize = 20_000;

/// One of the Copy row's actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CopyKind {
    Latex,
    MathMl,
    Image,
    Omml,
    Text,
    Svg,
}

impl CopyKind {
    /// Every kind, in button order.
    pub const ALL: [CopyKind; 6] = [CopyKind::Latex, CopyKind::MathMl, CopyKind::Image, CopyKind::Omml, CopyKind::Text, CopyKind::Svg];

    /// The name the `copy(kind)` hook takes.
    pub fn name(self) -> &'static str {
        match self {
            CopyKind::Latex => "latex",
            CopyKind::MathMl => "mathml",
            CopyKind::Image => "image",
            CopyKind::Omml => "omml",
            CopyKind::Text => "text",
            CopyKind::Svg => "svg",
        }
    }

    /// A kind by name (any case). `word` is the old name of `mathml`.
    pub fn parse(name: &str) -> Option<CopyKind> {
        let n = name.trim().to_ascii_lowercase();
        if n == "word" {
            return Some(CopyKind::MathMl);
        }
        CopyKind::ALL.into_iter().find(|k| k.name() == n)
    }

    /// The button label.
    pub fn title(self) -> &'static str {
        match self {
            CopyKind::Latex => "Copy LaTeX (Overleaf, Markdown, Anki)",
            CopyKind::MathMl => "Copy MathML (ONLYOFFICE, LibreOffice Math, Word)",
            CopyKind::Image => "Copy image (PNG, pastes anywhere)",
            CopyKind::Omml => "Copy OMML (Word equation XML)",
            CopyKind::Text => "Copy text (plain, like a/b)",
            CopyKind::Svg => "Download SVG (black ink)",
        }
    }

    /// The short name shown on the button.
    pub fn short(self) -> &'static str {
        match self {
            CopyKind::Latex => "LaTeX",
            CopyKind::MathMl => "MathML",
            CopyKind::Image => "Image",
            CopyKind::Omml => "OMML",
            CopyKind::Text => "Text",
            CopyKind::Svg => "SVG",
        }
    }

    /// The button tooltip: what it puts where.
    pub fn hint(self) -> &'static str {
        match self {
            CopyKind::Latex => "LaTeX source, for Overleaf, Markdown with math, Anki or any LaTeX editor",
            CopyKind::MathMl => {
                "MathML. ONLYOFFICE: paste it as an equation. LibreOffice: Math, Tools > Import MathML from Clipboard (Collabora Office is built on LibreOffice, so the same route should apply; untested). Word: Paste Special > Unformatted Text. Calibre and other e-book tools read MathML in HTML"
            }
            CopyKind::Image => "A PNG picture of the equation, black on white",
            CopyKind::Omml => "Office Math XML (an m:oMath element), for programs and code that read OMML",
            CopyKind::Text => "Plain linear text such as a/b and x^2",
            CopyKind::Svg => "Save the equation as an SVG file, black ink",
        }
    }

    /// True when the action saves a file instead of using the clipboard.
    pub fn is_download(self) -> bool {
        self == CopyKind::Svg
    }
}

/// Clipboard or file data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Data {
    Text(String),
    Bytes(Vec<u8>),
}

impl Data {
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Data::Text(t) => Some(t),
            Data::Bytes(_) => None,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Data::Text(t) => t.len(),
            Data::Bytes(b) => b.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// What one Copy action writes: (MIME type, data) pairs, and a file name for a download.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Payload {
    pub kind: CopyKind,
    pub items: Vec<(&'static str, Data)>,
    pub filename: Option<&'static str>,
}

impl Payload {
    /// The data of one MIME type.
    pub fn get(&self, mime: &str) -> Option<&Data> {
        self.items.iter().find(|(m, _)| *m == mime).map(|(_, d)| d)
    }

    /// The plain text a person can copy by hand when the clipboard is blocked.
    pub fn fallback_text(&self) -> Option<&str> {
        self.get("text/plain").and_then(Data::as_text)
    }
}

/// Build the payload of `kind` for `m`.
pub fn payload(kind: CopyKind, m: &Math, fonts: &FontDb) -> Payload {
    let text = |t: String| vec![("text/plain", Data::Text(t))];
    let (items, filename) = match kind {
        CopyKind::Latex => (text(to_latex(m)), None),
        CopyKind::Text => (text(to_linear(m)), None),
        CopyKind::MathMl => {
            let mathml = to_mathml_display(m, true);
            let html = mathml_html(&mathml);
            (vec![("text/plain", Data::Text(mathml)), ("text/html", Data::Text(html))], None)
        }
        CopyKind::Omml => (text(to_omml(m)), None),
        CopyKind::Image => (vec![("image/png", Data::Bytes(png_bytes(m, fonts)))], None),
        CopyKind::Svg => (vec![("image/svg+xml", Data::Text(svg(m, fonts)))], Some("equation.svg")),
    };
    Payload { kind, items, filename }
}

/// The equation as an SVG document, black ink.
pub fn svg(m: &Math, fonts: &FontDb) -> String {
    let l = layout_equation(fonts, m, DEFAULT_FAMILY, EXPORT_PT, m.para, Rgb::BLACK, EXPORT_PAD);
    equashion_render::scene_to_svg(&l.scene, fonts, DEFAULT_FAMILY)
}

/// The equation as a PNG: black ink on white at [`PNG_SCALE`] pixels per point, tagged with
/// the matching resolution so programs that honour it show it at its point size.
pub fn png_bytes(m: &Math, fonts: &FontDb) -> Vec<u8> {
    let l = layout_equation(fonts, m, DEFAULT_FAMILY, EXPORT_PT, m.para, Rgb::BLACK, EXPORT_PAD);
    let img = equashion_render::render_scene(&l.scene, fonts, PNG_SCALE, Some(Rgb::WHITE));
    encode_png(img.width, img.height, &img.to_straight())
}

/// RGBA8 (straight alpha) to PNG bytes; empty on an encoder error.
pub fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, width.max(1), height.max(1));
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        // Pixels per metre: PNG_SCALE pixels per point, 72 points per inch.
        let ppm = (PNG_SCALE * 72.0 / 0.0254).round() as u32;
        enc.set_pixel_dims(Some(png::PixelDimensions { xppu: ppm, yppu: ppm, unit: png::Unit::Meter }));
        let Ok(mut w) = enc.write_header() else { return Vec::new() };
        if w.write_image_data(rgba).is_err() {
            return Vec::new();
        }
    }
    out
}

/// The HTML half of "Copy MathML": the `<math>` element alone in a clipboard fragment. No image
/// or other markup, so a program that understands MathML in HTML (ONLYOFFICE) has nothing else
/// to prefer.
pub fn mathml_html(mathml: &str) -> String {
    format!("<html><body><!--StartFragment-->{mathml}<!--EndFragment--></body></html>")
}

/// What a pasted or imported text was read as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    MathMl,
    Omml,
    Latex,
    Linear,
}

/// True when `t` holds a MathML `<math>` start tag, with or without a prefix (`<mml:math`).
fn has_math_tag(t: &str) -> bool {
    t.match_indices('<').any(|(i, _)| {
        let rest = &t[i + 1..];
        let name_len = rest.find(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, ':' | '_' | '-' | '.'))).unwrap_or(rest.len());
        let name = &rest[..name_len];
        let after = rest[name_len..].chars().next();
        name.rsplit(':').next() == Some("math") && after.is_none_or(|c| c.is_whitespace() || c == '>' || c == '/')
    })
}

/// Tell MathML (a `<math>` element), OMML (`<m:oMath` / `<oMath` XML), LaTeX (has a backslash)
/// and linear text apart.
pub fn detect(text: &str) -> Format {
    let t = text.trim_start();
    if t.starts_with('<') && has_math_tag(t) {
        Format::MathMl
    } else if t.starts_with('<') && (t.contains("<m:oMath") || t.contains("<oMath")) {
        Format::Omml
    } else if t.contains('\\') || t.starts_with('$') {
        Format::Latex
    } else {
        Format::Linear
    }
}

/// Strip the math-mode wrapper people copy along with LaTeX: `$..$`, `$$..$$`, `\(..\)`, `\[..\]`.
pub fn strip_math_delimiters(s: &str) -> &str {
    let t = s.trim();
    for (open, close) in [("$$", "$$"), ("\\[", "\\]"), ("\\(", "\\)"), ("$", "$")] {
        if t.len() >= open.len() + close.len()
            && let Some(inner) = t.strip_prefix(open).and_then(|r| r.strip_suffix(close))
        {
            return inner.trim();
        }
    }
    t
}

/// A pasted or imported text read as an equation.
#[derive(Clone, Debug, PartialEq)]
pub struct Imported {
    pub math: Math,
    pub format: Format,
    /// Backslash commands the parser did not know (kept as plain text).
    pub unknown: Vec<String>,
}

/// Why an import was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportError {
    Empty,
    TooLong,
    /// OMML that gave no equation.
    NoEquation,
    /// MathML that gave no equation.
    NoMathMl,
}

impl ImportError {
    pub fn message(self) -> &'static str {
        match self {
            ImportError::Empty => "Nothing to load. Paste LaTeX, MathML, OMML or plain text first.",
            ImportError::TooLong => "That text is too long to load (over 20000 characters).",
            ImportError::NoEquation => "No equation could be read from that OMML. Check that the m:oMath element is complete.",
            ImportError::NoMathMl => "No equation could be read from that MathML. Check that the math element is complete.",
        }
    }
}

/// Read `text` as MathML, OMML, LaTeX or linear text. Never panics; hostile input gives an
/// error or what could be read.
pub fn import(text: &str) -> Result<Imported, ImportError> {
    if text.trim().is_empty() {
        return Err(ImportError::Empty);
    }
    if text.chars().count() > MAX_IMPORT {
        return Err(ImportError::TooLong);
    }
    let format = detect(text);
    let math = match format {
        Format::MathMl => match from_mathml(text) {
            Ok(m) if has_text(&m.body) => m,
            _ => return Err(ImportError::NoMathMl),
        },
        Format::Omml => {
            let m = from_omml(text);
            if !has_text(&m.body) {
                return Err(ImportError::NoEquation);
            }
            m
        }
        Format::Latex | Format::Linear => from_linear(strip_math_delimiters(text)),
    };
    let unknown = unknown_commands(&math);
    Ok(Imported { math, format, unknown })
}

/// True when some place of `seq` holds a character (an OMML fragment cut off after its
/// structure tags reads as empty places only).
fn has_text(seq: &[equashion_math::Node]) -> bool {
    let mut stack: Vec<&[equashion_math::Node]> = vec![seq];
    let mut budget = 1_000_000usize;
    while let Some(s) = stack.pop() {
        for n in s {
            budget = budget.saturating_sub(1);
            if budget == 0 {
                return true;
            }
            match n.bare() {
                equashion_math::Node::Text(t) if t.trim().is_empty() => {}
                equashion_math::Node::Text(_) => return true,
                other => {
                    let kids = equashion_math::children(other);
                    // A structure with nothing inside (a bare operator such as a sum sign) counts.
                    if kids.is_empty() {
                        return true;
                    }
                    stack.extend(kids.into_iter().map(Vec::as_slice));
                }
            }
        }
    }
    false
}

/// One plain sentence naming unknown commands, or `None` when there are none.
pub fn unknown_sentence(unknown: &[String]) -> Option<String> {
    match unknown {
        [] => None,
        [one] => Some(format!("{one} is not a known command, so it is shown as plain text.")),
        many => Some(format!("{} are not known commands, so they are shown as plain text.", many.join(", "))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fonts() -> std::sync::Arc<FontDb> {
        equashion_egui::bundled_fonts()
    }

    #[test]
    fn every_kind_names_round_trip() {
        for k in CopyKind::ALL {
            assert_eq!(CopyKind::parse(&k.name().to_uppercase()), Some(k));
        }
        assert_eq!(CopyKind::parse("pdf"), None);
    }

    #[test]
    fn payloads_carry_the_right_types() {
        let f = fonts();
        let m = from_linear(r"\frac{a}{b}");
        let latex = payload(CopyKind::Latex, &m, &f);
        assert_eq!(latex.fallback_text(), Some(r"\frac{a}{b}"));
        let mathml = payload(CopyKind::MathMl, &m, &f);
        let x = mathml.fallback_text().unwrap_or_default();
        assert_eq!(x, "<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"block\"><mfrac><mi>a</mi><mi>b</mi></mfrac></math>");
        let html = mathml.get("text/html").and_then(Data::as_text).unwrap_or_default();
        assert_eq!(html, format!("<html><body><!--StartFragment-->{x}<!--EndFragment--></body></html>"));
        assert!(!html.contains("<img") && !html.contains("msEquation"));
        assert_eq!(payload(CopyKind::parse("word").unwrap_or(CopyKind::Latex), &m, &f), mathml);
        let omml = payload(CopyKind::Omml, &m, &f);
        assert_eq!(omml.items.len(), 1);
        let o = omml.fallback_text().unwrap_or_default();
        assert!(o.starts_with("<m:oMath xmlns:m=") && o.contains("<m:f>"), "{o}");
        assert_eq!(payload(CopyKind::Text, &m, &f).fallback_text(), Some("a/b"));
        let png = payload(CopyKind::Image, &m, &f);
        let Some(Data::Bytes(b)) = png.get("image/png") else { panic!("no png") };
        assert_eq!(&b[..8], b"\x89PNG\r\n\x1a\n");
        assert!(b.len() > 200);
        let svg = payload(CopyKind::Svg, &m, &f);
        assert!(svg.get("image/svg+xml").and_then(Data::as_text).unwrap_or_default().starts_with("<svg"));
        assert_eq!(svg.filename, Some("equation.svg"));
    }

    #[test]
    fn detect_and_import() {
        assert_eq!(detect("<m:oMath><m:r><m:t>x</m:t></m:r></m:oMath>"), Format::Omml);
        assert_eq!(detect("<math xmlns=\"http://www.w3.org/1998/Math/MathML\"><mi>x</mi></math>"), Format::MathMl);
        assert_eq!(detect("<mml:math><mml:mi>x</mml:mi></mml:math>"), Format::MathMl);
        assert_eq!(detect("<html><body><!--StartFragment--><math><mi>x</mi></math>"), Format::MathMl);
        assert_eq!(detect("<mathx>"), Format::Linear);
        assert_eq!(detect("  <oMath><r><t>x</t></r></oMath>"), Format::Omml);
        assert_eq!(detect(r"\frac{a}{b}"), Format::Latex);
        assert_eq!(detect("$x^2$"), Format::Latex);
        assert_eq!(detect("a/b + x^2"), Format::Linear);
        assert_eq!(strip_math_delimiters(" $$x^2$$ "), "x^2");
        assert_eq!(strip_math_delimiters(r"\[ a \]"), "a");
        assert_eq!(strip_math_delimiters("$"), "$");
        let i = import(r"$\frac{1}{2}$").unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(to_latex(&i.math), r"\frac{1}{2}");
        let i = import("<m:oMath><m:f><m:num><m:r><m:t>1</m:t></m:r></m:num><m:den><m:r><m:t>2</m:t></m:r></m:den></m:f></m:oMath>")
            .unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!(i.format, Format::Omml);
        assert_eq!(to_linear(&i.math), "1/2");
        let i = import(r"\foo+\baz x").unwrap_or_else(|e| panic!("{e:?}"));
        let s = unknown_sentence(&i.unknown).unwrap_or_default();
        assert!(s.contains("foo") && s.contains("baz"), "{s}");
        assert_eq!(import("   "), Err(ImportError::Empty));
        assert_eq!(import("<m:oMath"), Err(ImportError::NoEquation));
        assert_eq!(import("<m:oMath><m:f>"), Err(ImportError::NoEquation));
        assert!(import("<m:oMath><m:f><m:num><m:r><m:t>1").is_ok());
        let i = import("<math display=\"block\"><msqrt><mi>x</mi></msqrt></math>").unwrap_or_else(|e| panic!("{e:?}"));
        assert_eq!((i.format, to_latex(&i.math).as_str(), i.math.para), (Format::MathMl, r"\sqrt{x}", true));
        assert_eq!(import("<math></math>"), Err(ImportError::NoMathMl));
        assert_eq!(import("<math><mrow>"), Err(ImportError::NoMathMl));
    }

    #[test]
    fn hostile_input_never_panics() {
        let deep_frac = r"\frac{".repeat(5000);
        let deep_omml = "<m:oMath>".to_string() + &"<m:f><m:num>".repeat(3000);
        let cases = [
            "\0\0\0".to_string(),
            "{".repeat(10_000),
            "(".repeat(10_000),
            deep_frac,
            deep_omml,
            "<m:oMath><m:r><m:t>&#xFFFFFFFF;&bogus;</m:t></m:r></m:oMath>".to_string(),
            "\\".repeat(5000),
            "x".repeat(MAX_IMPORT + 1),
            "^_^_^_/ / / sqrt sqrt \\begin{matrix}&&&\\\\\\end{cases}".to_string(),
            "\u{202E}\u{FEFF}\u{1F600}".to_string(),
            format!("<math>{}", "<mfrac><mrow>".repeat(4000)),
            "<math><mi>&bogus;</mi><mfenced separators=''/><mmultiscripts><mprescripts/></mmultiscripts></math>".to_string(),
        ];
        let f = fonts();
        for c in &cases {
            if let Ok(i) = import(c) {
                // Small enough to export: the payloads must not panic either.
                for k in [CopyKind::Latex, CopyKind::MathMl, CopyKind::Omml, CopyKind::Text] {
                    let _ = payload(k, &i.math, &f);
                }
            }
        }
    }
}
