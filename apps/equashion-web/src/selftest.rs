//! Parse -> layout -> render over the whole corpus, in the page.

use equashion_layout::{DEFAULT_FAMILY, FontDb, Prim, Rgb, layout_equation};
use equashion_math::{Math, from_linear, from_omml, to_omml};

use crate::corpus;

/// The result for one corpus item.
#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    /// Row id, or `omml/<file stem>` for an OMML sample.
    pub id: String,
    pub ok: bool,
    /// Why it failed.
    pub reason: Option<String>,
    /// Glyphs drawn as .notdef (the fonts lack the character). Reported, not a failure.
    pub missing_glyphs: usize,
    /// Canvas size, points.
    pub size: (f64, f64),
}

/// The whole run.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub checks: Vec<Check>,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.checks.iter().all(|c| c.ok)
    }
    pub fn failures(&self) -> impl Iterator<Item = &Check> {
        self.checks.iter().filter(|c| !c.ok)
    }
}

fn check(fonts: &FontDb, id: String, m: &Math, display: bool) -> Check {
    let mut c = Check { id, ok: false, reason: None, missing_glyphs: 0, size: (0.0, 0.0) };
    if m.is_empty() {
        c.reason = Some("parsed to an empty equation".into());
        return c;
    }
    let l = layout_equation(fonts, m, DEFAULT_FAMILY, 24.0, display, Rgb::BLACK, 4.0);
    c.size = (l.width(), l.height());
    if !(l.width().is_finite() && l.height().is_finite() && l.width() > 0.0 && l.height() > 0.0) {
        c.reason = Some(format!("layout size {:?}", c.size));
        return c;
    }
    c.missing_glyphs =
        l.scene.items.iter().map(|p| if let Prim::Glyphs { glyphs, .. } = p { glyphs.iter().filter(|g| g.0 == 0).count() } else { 0 }).sum();
    let r = equashion_render::render_scene(&l.scene, fonts, 1.0, None);
    let ink = r.pixels.as_chunks::<4>().0.iter().filter(|p| p[3] > 0).count();
    if ink == 0 {
        c.reason = Some("rendered nothing".into());
        return c;
    }
    let o = to_omml(m);
    if to_omml(&from_omml(&o)) != o {
        c.reason = Some("OMML round trip is not stable".into());
        return c;
    }
    c.ok = true;
    c
}

/// Run the selftest over `corpus/equations.tsv` and `corpus/omml/*.xml`.
pub fn run(fonts: &FontDb) -> Report {
    let mut checks: Vec<Check> = corpus::rows().iter().map(|r| check(fonts, r.id.to_string(), &from_linear(r.linear), r.display)).collect();
    for (name, x) in corpus::OMML_SAMPLES {
        let m = from_omml(x);
        checks.push(check(fonts, format!("omml/{name}"), &m, m.para));
    }
    Report { checks }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_corpus_passes() {
        let r = run(&equashion_egui::bundled_fonts());
        assert_eq!(r.checks.len(), corpus::rows().len() + corpus::OMML_SAMPLES.len());
        let bad: Vec<_> = r.failures().collect();
        assert!(bad.is_empty(), "{bad:?}");
    }
}
