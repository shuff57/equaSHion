//! Property test over the shared corpus: every caret position of every corpus equation (linear
//! rows and OMML samples) has a finite caret rectangle inside the canvas, and clicking it comes
//! back to a caret at the same place. The same property over every template is the unit test
//! `every_caret_position_of_every_template_has_a_rectangle` in `src/math.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::OnceLock;

use equashion_layout::{DEFAULT_FAMILY, EqLayout, FontDb, Rgb, layout_equation};
use equashion_math::{Caret, Math, Node, children, from_linear, from_omml, seq_at, units};

fn fonts() -> &'static FontDb {
    static DB: OnceLock<FontDb> = OnceLock::new();
    DB.get_or_init(FontDb::bundled_only)
}

fn corpus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus")
}

fn corpus() -> Vec<(String, Math, bool)> {
    let text = std::fs::read_to_string(corpus_dir().join("equations.tsv")).unwrap();
    let mut v: Vec<(String, Math, bool)> = text
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            (f[0].to_string(), from_linear(f[1]), f[2].trim() == "1")
        })
        .collect();
    let mut files: Vec<_> = std::fs::read_dir(corpus_dir().join("omml")).unwrap().flatten().map(|e| e.path()).collect();
    files.sort();
    for p in files {
        let m = from_omml(&std::fs::read_to_string(&p).unwrap());
        let display = m.para;
        v.push((p.file_name().unwrap().to_string_lossy().to_string(), m, display));
    }
    v
}

/// Every row path of a tree (the root included).
fn all_rows(seq: &[Node], path: &mut Vec<(usize, usize)>, out: &mut Vec<Vec<(usize, usize)>>) {
    out.push(path.clone());
    for (i, n) in seq.iter().enumerate() {
        for (c, k) in children(n).into_iter().enumerate() {
            path.push((i, c));
            all_rows(k, path, out);
            path.pop();
        }
    }
}

fn check(id: &str, m: &Math, l: &EqLayout) -> usize {
    let mut rows = vec![];
    all_rows(&m.body, &mut vec![], &mut rows);
    let mut n = 0;
    for path in rows {
        let Some(seq) = seq_at(&m.body, &path) else { continue };
        for pos in 0..=units(seq) {
            let c = Caret { path: path.clone(), pos };
            let r = l.caret_rect(&c).unwrap_or_else(|| panic!("{id}: no rect for {c:?}"));
            assert!(r.x0.is_finite() && r.y0.is_finite() && r.y1.is_finite() && r.y1 > r.y0, "{id}: {c:?} {r:?}");
            assert!(
                r.x0 >= -0.5 && r.x0 <= l.width() + 0.5 && r.y0 >= -0.5 && r.y1 <= l.height() + 0.5,
                "{id}: {c:?} {r:?} outside {}x{}",
                l.width(),
                l.height()
            );
            let got = l.hit(r.x0, (r.y0 + r.y1) / 2.0);
            let gr = l.caret_rect(&got).unwrap_or_else(|| panic!("{id}: hit gave {got:?} with no rect"));
            assert!((gr.x0 - r.x0).abs() < 2.0, "{id}: {c:?} -> {got:?}");
            n += 1;
        }
    }
    n
}

#[test]
fn every_caret_position_of_every_corpus_equation_has_a_finite_rect() {
    let mut total = 0;
    for (id, m, display) in corpus() {
        for size in [11.0, 24.0] {
            let l = layout_equation(fonts(), &m, DEFAULT_FAMILY, size, display, Rgb::BLACK, 4.0);
            total += check(&id, &m, &l);
        }
    }
    assert!(total > 500, "{total} caret positions checked");
}

/// The bundled fonts (Source Serif 4, Inter) lack some math symbols, and `bundled_only` has no
/// system fallback, so these render as .notdef boxes. Pinned here so the gap stays visible: the
/// list may only shrink (by bundling a math symbol font, see README.md).
#[test]
fn characters_the_bundled_fonts_lack_are_known() {
    let db = fonts();
    let regular = db.face(DEFAULT_FAMILY, "Regular");
    let mut missing: Vec<char> = corpus()
        .iter()
        .flat_map(|(_, m, _)| equashion_math::to_linear(m).chars().collect::<Vec<_>>())
        .filter(|c| !c.is_ascii() && !c.is_whitespace() && !regular.covers(*c) && db.fallback_for(*c, regular.id()).is_none())
        .collect();
    missing.sort_unstable();
    missing.dedup();
    let known = ['\u{2207}', '\u{222E}', '\u{2308}', '\u{2309}', '\u{230A}', '\u{230B}', '\u{27E8}', '\u{27E9}'];
    assert!(missing.iter().all(|c| known.contains(c)), "newly missing: {missing:?}");
    assert_eq!(missing, known, "update the list (and README.md) when a gap closes");
}
