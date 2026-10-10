//! What the Copy buttons put on the clipboard and what a paste reads back. Pure functions of
//! the equation tree, so the native tests check every payload without a browser.
//!
//! | Kind | MIME types written | Content |
//! |---|---|---|
//! | `latex` | `text/plain` | [`to_latex`] |
//! | `word` | `text/plain`, `text/html` | OMML (`m:oMath` with its namespace); an HTML fragment that carries the same OMML the way Word's own clipboard HTML does, with a PNG `<img>` for programs that do not read it |
//! | `text` | `text/plain` | [`to_linear`] |
//! | `image` | `image/png` | black ink on white, 2 pixels per point, 144 dpi |
//! | `svg` | `image/svg+xml` (a download, not the clipboard) | `equashion-render`'s SVG backend, black ink |

use equashion_layout::{DEFAULT_FAMILY, FontDb, Rgb, layout_equation};
use equashion_math::{Math, from_linear, from_omml, to_latex, to_linear, to_omml, unknown_commands};

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
    Word,
    Text,
    Image,
    Svg,
}

impl CopyKind {
    pub const ALL: [CopyKind; 5] = [CopyKind::Latex, CopyKind::Word, CopyKind::Text, CopyKind::Image, CopyKind::Svg];

    /// The name the `copy(kind)` hook takes.
    pub fn name(self) -> &'static str {
        match self {
            CopyKind::Latex => "latex",
            CopyKind::Word => "word",
            CopyKind::Text => "text",
            CopyKind::Image => "image",
            CopyKind::Svg => "svg",
        }
    }

    /// A kind by name (any case).
    pub fn parse(name: &str) -> Option<CopyKind> {
        let n = name.trim().to_ascii_lowercase();
        CopyKind::ALL.into_iter().find(|k| k.name() == n)
    }

    /// The button label.
    pub fn title(self) -> &'static str {
        match self {
            CopyKind::Latex => "Copy LaTeX",
            CopyKind::Word => "Copy for Word",
            CopyKind::Text => "Copy text",
            CopyKind::Image => "Copy image",
            CopyKind::Svg => "Download SVG",
        }
    }

    /// The button tooltip: what it puts where.
    pub fn hint(self) -> &'static str {
        match self {
            CopyKind::Latex => "LaTeX source, for Overleaf, Markdown, Anki or any LaTeX editor",
            CopyKind::Word => "An Office Math equation (OMML) for Word, PowerPoint and Outlook on the desktop",
            CopyKind::Text => "Plain linear text such as a/b and x^2",
            CopyKind::Image => "A PNG picture of the equation, black on white",
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
        CopyKind::Word => {
            let omml = to_omml(m);
            let png = png_bytes(m, fonts);
            let html = word_html(&omml, &to_linear(m), &png);
            (vec![("text/plain", Data::Text(omml)), ("text/html", Data::Text(html))], None)
        }
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

/// Standard base64 (for the `<img>` data URL).
pub fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let b = [c.first().copied().unwrap_or(0), c.get(1).copied().unwrap_or(0), c.get(2).copied().unwrap_or(0)];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (i, shift) in [18u32, 12, 6, 0].into_iter().enumerate() {
            if i <= c.len() {
                out.push(char::from(T[((n >> shift) & 63) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// The HTML half of "Copy for Word": the OMML inside the `[if gte msEquation 12]` conditional
/// comment Word writes around equations in its own clipboard HTML, and a PNG `<img>` in the
/// branch every other program reads. Not verified against a real Word install.
pub fn word_html(omml: &str, alt: &str, png: &[u8]) -> String {
    let img =
        if png.is_empty() { html_escape(alt) } else { format!("<img alt=\"{}\" src=\"data:image/png;base64,{}\">", html_escape(alt), base64(png)) };
    format!(
        "<html xmlns:o=\"urn:schemas-microsoft-com:office:office\" xmlns:w=\"urn:schemas-microsoft-com:office:word\" xmlns:m=\"{ns}\" xmlns=\"http://www.w3.org/TR/REC-html40\"><body><!--StartFragment--><!--[if gte msEquation 12]>{omml}<![endif]--><![if !msEquation]>{img}<![endif]><!--EndFragment--></body></html>",
        ns = equashion_math::OMML_NS,
    )
}

/// What a pasted or imported text was read as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Omml,
    Latex,
    Linear,
}

/// Tell OMML (`<m:oMath` / `<oMath` XML), LaTeX (has a backslash) and linear text apart.
pub fn detect(text: &str) -> Format {
    let t = text.trim_start();
    if t.starts_with('<') && (t.contains("<m:oMath") || t.contains("<oMath")) {
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
}

impl ImportError {
    pub fn message(self) -> &'static str {
        match self {
            ImportError::Empty => "Nothing to load. Paste LaTeX, OMML or plain text first.",
            ImportError::TooLong => "That text is too long to load (over 20000 characters).",
            ImportError::NoEquation => "No equation could be read from that OMML. Check that the m:oMath element is complete.",
        }
    }
}

/// Read `text` as OMML, LaTeX or linear text. Never panics; hostile input gives an error or
/// what could be read.
pub fn import(text: &str) -> Result<Imported, ImportError> {
    if text.trim().is_empty() {
        return Err(ImportError::Empty);
    }
    if text.chars().count() > MAX_IMPORT {
        return Err(ImportError::TooLong);
    }
    let format = detect(text);
    let math = match format {
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
        let word = payload(CopyKind::Word, &m, &f);
        let omml = word.fallback_text().unwrap_or_default();
        assert!(omml.starts_with("<m:oMath xmlns:m=") && omml.contains("<m:f>"), "{omml}");
        let html = word.get("text/html").and_then(Data::as_text).unwrap_or_default();
        assert!(html.contains("msEquation") && html.contains("<m:f>") && html.contains("data:image/png;base64,iVBORw0KGgo"), "{html}");
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
    fn base64_matches_known_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn detect_and_import() {
        assert_eq!(detect("<m:oMath><m:r><m:t>x</m:t></m:r></m:oMath>"), Format::Omml);
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
        ];
        let f = fonts();
        for c in &cases {
            if let Ok(i) = import(c) {
                // Small enough to export: the payloads must not panic either.
                for k in [CopyKind::Latex, CopyKind::Word, CopyKind::Text] {
                    let _ = payload(k, &i.math, &f);
                }
            }
        }
    }
}
