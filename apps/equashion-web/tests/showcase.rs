//! The showcase driven natively with egui_kittest (the browser smoke test covers the shell).
#![cfg(not(target_arch = "wasm32"))]

use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use equashion_web::{CREDIT, GALLERY_GROUPS, OmmlStatus, Showcase, Tab, ThemePref, corpus};

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
    for (title, tab) in [("Gallery", Tab::Gallery), ("OMML", Tab::Omml), ("Selftest", Tab::Selftest), ("About", Tab::About), ("Editor", Tab::Editor)]
    {
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
    h.get_by_label("Round trip stable");
    assert_eq!(h.state().omml_status(), OmmlStatus::Stable);
    let (_, out, same) = h.state().omml_page();
    assert!(same && out.contains("oMath"));
    h.state_mut().set_omml_input("not omml at all");
    h.run();
    assert_eq!(h.state().omml_status(), OmmlStatus::NoEquation);
    h.get_by_label("No equation found");
    h.state_mut().set_omml_input("  ");
    h.run();
    assert_eq!(h.state().omml_status(), OmmlStatus::Empty);
    h.get_by_label("No input");
    h.state_mut().set_omml_input("<m:oMath><m:r><m:t>x</m:t></m:r></m:oMath>");
    h.run();
    assert!(h.state().omml_page().2);
    assert_eq!(h.state().omml_status(), OmmlStatus::Stable);
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

#[test]
fn every_corpus_row_is_in_exactly_one_gallery_group() {
    let h = harness();
    let groups = h.state().gallery_groups();
    let mut ids: Vec<&str> = groups.iter().flat_map(|(_, ids)| ids.iter().copied()).collect();
    assert_eq!(ids.len(), corpus::rows().len(), "{groups:?}");
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), corpus::rows().len());
    // Every row is placed by name: nothing falls through to "Other".
    assert!(groups.iter().all(|(t, _)| *t != "Other"), "{groups:?}");
    assert_eq!(groups.len(), GALLERY_GROUPS.len());
}

#[test]
fn the_theme_toggle_switches_light_and_dark() {
    let mut h = harness();
    assert_eq!(h.state().theme_pref(), ThemePref::System);
    h.get_by_label("Dark").click();
    h.run();
    assert_eq!(h.state().theme_pref(), ThemePref::Dark);
    assert_eq!(h.ctx.theme(), egui::Theme::Dark);
    assert!(h.ctx.style_of(h.ctx.theme()).visuals.dark_mode);
    h.get_by_label("Light").click();
    h.run();
    assert_eq!(h.ctx.theme(), egui::Theme::Light);
    assert!(!h.ctx.style_of(h.ctx.theme()).visuals.dark_mode);
    // The editor widget draws its ink in the theme's strongest text colour.
    assert_eq!(h.ctx.style_of(h.ctx.theme()).visuals.strong_text_color(), equashion_web::theme::LIGHT.strong);
    h.get_by_label("System").click();
    h.run();
    assert_eq!(h.state().theme_pref(), ThemePref::System);
}

#[test]
fn reduced_motion_turns_animation_off() {
    let mut h = harness();
    assert!(h.ctx.style_of(h.ctx.theme()).animation_time > 0.0);
    h.state_mut().set_reduced_motion(true);
    h.run();
    assert_eq!(h.ctx.style_of(h.ctx.theme()).animation_time, 0.0);
}

#[test]
fn the_about_tab_and_footer_carry_the_credit() {
    let mut h = harness();
    // Footer, on every tab.
    h.get_by_label("cortexjs.io/mathlive");
    h.state_mut().set_tab(Tab::About);
    h.run();
    h.get_by_label("What it is");
    h.get_by_label("What is inside");
    h.get_by_label("Credits and licence");
    h.get_by_label("Known limits");
    // The credit again in the About text, with the same link.
    assert_eq!(h.get_all_by_label("cortexjs.io/mathlive").count(), 2);
    assert_eq!(h.get_all_by_label("Typing behaviour is modelled on MathLive (").count(), 2);
    assert!(CREDIT.contains("not a port") && CREDIT.contains("no MathLive code is used"));
}

#[test]
fn visible_strings_have_no_long_dashes() {
    for src in [
        include_str!("../src/showcase.rs"),
        include_str!("../src/theme.rs"),
        include_str!("../../../crates/egui/src/lib.rs"),
        include_str!("../../../crates/egui/src/palette.rs"),
    ] {
        for (n, line) in src.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            assert!(!line.contains('\u{2014}') && !line.contains('\u{2013}'), "line {}: {line}", n + 1);
        }
    }
}
