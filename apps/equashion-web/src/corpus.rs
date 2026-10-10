//! The repository corpus, compiled in: `corpus/equations.tsv` and `corpus/omml/*.xml`.

/// `corpus/equations.tsv` verbatim.
pub const EQUATIONS_TSV: &str = include_str!("../../../corpus/equations.tsv");

/// `corpus/omml/*.xml`: (file stem, contents).
pub const OMML_SAMPLES: &[(&str, &str)] = &[
    ("deckcraft_pptx_roundtrip_frac", include_str!("../../../corpus/omml/deckcraft_pptx_roundtrip_frac.xml")),
    ("deckcraft_pptx_roundtrip_frac_rpr", include_str!("../../../corpus/omml/deckcraft_pptx_roundtrip_frac_rpr.xml")),
    ("wordcraft_docx_fixture_ssup", include_str!("../../../corpus/omml/wordcraft_docx_fixture_ssup.xml")),
    ("wordcraft_docx_roundtrip_display_rad", include_str!("../../../corpus/omml/wordcraft_docx_roundtrip_display_rad.xml")),
    ("wordcraft_docx_roundtrip_foreign_attrs", include_str!("../../../corpus/omml/wordcraft_docx_roundtrip_foreign_attrs.xml")),
    ("wordcraft_docx_roundtrip_frac", include_str!("../../../corpus/omml/wordcraft_docx_roundtrip_frac.xml")),
];

/// One row of `equations.tsv`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: &'static str,
    pub linear: &'static str,
    /// Display style (the third column is `1`).
    pub display: bool,
}

/// The rows of `equations.tsv` (comments and blank lines skipped).
pub fn rows() -> Vec<Row> {
    EQUATIONS_TSV
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .filter_map(|l| {
            let mut cols = l.split('\t');
            let id = cols.next()?.trim();
            let linear = cols.next()?;
            let display = cols.next().is_some_and(|d| d.trim() == "1");
            Some(Row { id, linear, display })
        })
        .collect()
}

/// The idempotence check of the OMML page: `to_omml(from_omml(x))` is a fixed point.
pub fn omml_round_trip(x: &str) -> (String, bool) {
    use equashion_math::{from_omml, to_omml};
    let once = to_omml(&from_omml(x));
    let twice = to_omml(&from_omml(&once));
    let same = once == twice;
    (once, same)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_corpus_has_44_rows_with_unique_ids() {
        let r = rows();
        assert_eq!(r.len(), 44);
        let mut ids: Vec<_> = r.iter().map(|r| r.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 44);
        assert!(r.iter().any(|r| r.display) && r.iter().any(|r| !r.display));
    }

    #[test]
    fn every_omml_sample_round_trips_idempotently() {
        for (name, x) in OMML_SAMPLES {
            let (out, same) = omml_round_trip(x);
            assert!(same, "{name}: {out}");
            assert!(out.contains("oMath"), "{name}");
        }
    }
}
