//! The showcase driven natively with egui_kittest (the browser smoke test covers the shell).
#![cfg(not(target_arch = "wasm32"))]

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use equashion_web::{Showcase, Tab, corpus};

fn harness() -> Harness<'static, Showcase> {
    let mut h = Harness::builder().with_size(vec2(1100.0, 900.0)).build_ui_state(
        |ui, s: &mut Showcase| {
            s.ui(ui);
        },
        Showcase::new(),
    );
    h.run();
    h
}

#[test]
fn tabs_switch_by_click_and_by_name() {
    let mut h = harness();
    assert_eq!(h.state().tab(), Tab::Editor);
    for (title, tab) in [("Gallery", Tab::Gallery), ("OMML", Tab::Omml), ("Selftest", Tab::Selftest), ("Editor", Tab::Editor)] {
        h.get_by_label(title).click();
        h.run();
        assert_eq!(h.state().tab(), tab);
    }
    for t in Tab::ALL {
        assert_eq!(Tab::parse(&t.name().to_uppercase()), Some(t));
    }
    assert_eq!(Tab::parse("nope"), None);
}

#[test]
fn the_gallery_shows_every_corpus_row_and_a_click_loads_it() {
    let mut h = harness();
    assert_eq!(h.state().gallery_count(), corpus::rows().len());
    h.state_mut().set_tab(Tab::Gallery);
    h.run();
    h.get_by_label("frac_simple").click();
    h.run();
    assert_eq!(h.state().tab(), Tab::Editor);
    assert!(h.state().editor.omml().contains("<m:f>"), "{}", h.state().editor.omml());
    let names: Vec<String> = h.ctx.tex_manager().read().allocated().map(|(_, m)| m.name.clone()).collect();
    assert!(names.iter().filter(|n| n.starts_with("equashion/gallery/")).count() >= 10, "{names:?}");
}

#[test]
fn the_selftest_tab_runs_the_corpus() {
    let mut h = harness();
    h.state_mut().set_tab(Tab::Selftest);
    h.run();
    h.get_by_label("Run selftest").click();
    h.run();
    let rep = h.state().report().expect("a report");
    assert_eq!(rep.checks.len(), corpus::rows().len() + corpus::OMML_SAMPLES.len());
    assert!(rep.ok(), "{:?}", rep.failures().collect::<Vec<_>>());
}

#[test]
fn the_omml_tab_reports_idempotence() {
    let mut h = harness();
    h.state_mut().set_tab(Tab::Omml);
    h.run();
    h.get_by_label("round-trip idempotent: yes");
    let (_, out, same) = h.state().omml_page();
    assert!(same && out.contains("oMath"));
    h.state_mut().set_omml_input("<m:oMath><m:r><m:t>x</m:t></m:r></m:oMath>");
    h.run();
    assert!(h.state().omml_page().2);
    h.get_by_label("Load into editor").click();
    h.run();
    assert_eq!(h.state().editor.linear(), "x");
}

#[test]
fn the_editor_tab_shows_readouts_and_unknown_commands() {
    let mut h = harness();
    h.state_mut().editor.set_linear(r"\foo+x");
    h.run();
    h.get_by_label("Unknown commands");
    assert!(h.state().editor.unknown_commands().iter().any(|c| c.contains("foo")));
    assert_eq!(h.get_all_by_label("Copy").count(), 2);
}
