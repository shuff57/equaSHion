//! The page driven natively with egui_kittest (the browser smoke test covers the shell).
#![cfg(not(target_arch = "wasm32"))]

use egui::{Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use equashion_math::{Editor, Key as MKey, Math, to_latex};
use equashion_web::export::Data;
use equashion_web::{COPY_GUIDE, CREDIT, CopyKind, EXAMPLES, HOW_TO_TYPE, Outcome, PASTE_LABEL, Showcase, ThemePref, corpus};

fn harness_at(w: f32) -> Harness<'static, Showcase> {
    // The header wordmark plays for 4.4 s and asks for a frame every 16 ms while it does, which
    // `Harness::run` would wait out: the tests see the settled mark, as a reduced-motion visitor does.
    let mut app = Showcase::new();
    app.set_reduced_motion(true);
    let mut h = Harness::builder().with_size(vec2(w, 1400.0)).build_ui_state(
        |ui, s: &mut Showcase| {
            s.ui(ui);
        },
        app,
    );
    h.run();
    h
}

fn harness() -> Harness<'static, Showcase> {
    harness_at(1280.0)
}

#[test]
fn one_view_no_tabs() {
    let h = harness();
    assert_eq!(h.state().active_tab(), "editor");
    for gone in ["Gallery", "OMML", "Selftest", "Editor"] {
        assert!(h.query_by_label(gone).is_none(), "{gone} is still on the page");
    }
    h.get_by_label("equaSHion");
    h.get_by_label("A pure-Rust equation editor");
    h.get_by_label("Try it");
    assert_eq!(h.state().gallery_count(), corpus::rows().len());
}

#[test]
fn the_selftest_still_runs_from_the_hook() {
    let mut h = harness();
    let rep = h.state_mut().run_selftest();
    assert_eq!(rep.checks.len(), corpus::rows().len() + corpus::OMML_SAMPLES.len());
    assert!(rep.ok(), "{:?}", rep.failures().collect::<Vec<_>>());
}

#[test]
fn copy_buttons_queue_their_payloads() {
    let mut h = harness();
    h.state_mut().set_latex(r"\frac{a}{b}");
    h.run();
    // One button per format, all showing, none behind a menu.
    for kind in CopyKind::ALL {
        h.get_by_label(kind.title());
    }
    for kind in CopyKind::ALL {
        h.state_mut().take_requests();
        h.get_by_label(kind.title()).click();
        h.run();
        let reqs = h.state_mut().take_requests();
        assert_eq!(reqs.len(), 1, "{kind:?}");
        let p = &reqs[0].payload;
        assert_eq!(p.kind, kind);
        match kind {
            CopyKind::Latex => assert_eq!(p.fallback_text(), Some(r"\frac{a}{b}")),
            CopyKind::MathMl => {
                assert!(
                    p.fallback_text().is_some_and(
                        |t| t.starts_with("<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"block\">") && t.contains("<mfrac>")
                    )
                );
                assert!(p.get("text/html").and_then(Data::as_text).is_some_and(|h| h.contains("<!--StartFragment--><math xmlns=")));
            }
            CopyKind::Omml => {
                assert!(p.fallback_text().is_some_and(|t| t.starts_with("<m:oMath xmlns:m=") && t.contains("<m:f>")));
                assert_eq!(p.items.len(), 1);
            }
            CopyKind::Text => assert_eq!(p.fallback_text(), Some("a/b")),
            CopyKind::Image => assert!(matches!(p.get("image/png"), Some(Data::Bytes(b)) if b.starts_with(b"\x89PNG"))),
            CopyKind::Svg => assert!(p.get("image/svg+xml").and_then(Data::as_text).is_some_and(|s| s.starts_with("<svg"))),
        }
        assert_eq!(h.state().copy_status(), Some((kind.name(), "pending")));
    }
}

#[test]
fn copied_confirms_and_blocked_shows_a_fallback() {
    let mut h = harness();
    let id = h.state_mut().request_copy(CopyKind::Latex);
    h.state_mut().copy_finished(id, Outcome::Copied, None);
    // "Copied" keeps asking for frames until it goes away.
    h.run_steps(3);
    h.get_by_label("Copied");
    assert_eq!(h.state().copy_status(), Some(("latex", "copied")));

    let id = h.state_mut().request_copy(CopyKind::Text);
    h.state_mut().copy_finished(id, Outcome::Blocked, Some("a/b".into()));
    h.run_steps(3);
    assert!(h.query_by_label("Copied").is_none());
    h.get_by_label_contains("did not allow copying");
    h.get_by_label("Close").click();
    h.run();
    assert_eq!(h.state().copy_status(), None);

    // A blocked image offers a PNG download instead.
    let id = h.state_mut().request_copy(CopyKind::Image);
    let _ = h.state_mut().take_requests();
    h.state_mut().copy_finished(id, Outcome::Blocked, None);
    h.run();
    h.get_by_label("Download PNG").click();
    h.run();
    let reqs = h.state_mut().take_requests();
    assert_eq!(reqs.len(), 1);
    assert_eq!(reqs[0].payload.filename, Some("equation.png"));
}

#[test]
fn ctrl_v_in_the_editor_reads_mathml_omml_latex_and_text() {
    let mut h = harness();
    let omml = "<m:oMath xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\"><m:f><m:num><m:r><m:t>1</m:t></m:r></m:num><m:den><m:r><m:t>2</m:t></m:r></m:den></m:f></m:oMath>";
    let mathml = "<math xmlns=\"http://www.w3.org/1998/Math/MathML\" display=\"block\"><msup><mi>x</mi><mn>2</mn></msup></math>";
    for (pasted, latex) in [(omml, r"\frac{1}{2}"), (mathml, "x^2"), (r"$\sqrt{x}$", r"\sqrt{x}"), ("a^2+b^2", "a^2+b^2")] {
        h.state_mut().set_latex("");
        h.run();
        h.event(egui::Event::Paste(pasted.to_string()));
        h.run();
        assert_eq!(h.state().latex(), latex, "{pasted}");
    }
    // At the caret, after what is there.
    h.state_mut().set_latex("x+");
    h.run();
    h.event(egui::Event::Paste(r"\alpha".into()));
    h.run();
    assert_eq!(h.state().latex(), r"x+\alpha");
    // Unknown commands get one plain sentence.
    h.state_mut().set_latex("");
    h.run();
    h.event(egui::Event::Paste(r"\foo+\bar".into()));
    h.run();
    h.get_by_label_contains("are not known commands");
    // Broken OMML is refused with a sentence and leaves the equation alone.
    h.state_mut().set_latex("y");
    h.run();
    h.event(egui::Event::Paste("<m:oMath><m:f>".into()));
    h.run();
    assert_eq!(h.state().latex(), "y");
    h.get_by_label_contains("No equation could be read");
}

#[test]
fn the_import_field_replaces_the_equation() {
    let mut h = harness();
    h.get_by_label(PASTE_LABEL).click();
    h.run();
    h.get_by_label("Load");
    assert!(h.state_mut().import(r"\frac{p}{q}").is_ok());
    assert_eq!(h.state().latex(), r"\frac{p}{q}");
    assert!(h.state_mut().import("   ").is_err());
}

#[test]
fn examples_load_by_click() {
    let mut h = harness();
    assert_eq!(h.state().examples().len(), EXAMPLES.len(), "an example id is not in the corpus");
    assert!((8..=10).contains(&EXAMPLES.len()));
    h.get_by_label("Euler's identity").click();
    h.run();
    assert_eq!(h.state().latex(), r"e^{i\pi}+1=0");
    assert!(h.state().editor.math().para);
    assert!(h.state_mut().load_example("binomial"));
    assert!(!h.state_mut().load_example("nope"));
}

#[test]
fn the_palette_tabs_group_the_buttons_by_context() {
    let mut h = harness();
    h.state_mut().set_latex("");
    h.run();
    // Every tab is on the bar; only the first tab's groups are drawn.
    for t in ["Structures", "Greek letters", "Special symbols", "Relations", "Math operations", "Arrows", "Functions"] {
        h.get_by_label(t);
    }
    for g in ["Fractions and roots", "Scripts", "Large operators", "Brackets", "Matrices and cases", "Accents"] {
        h.get_by_label(g);
    }
    assert!(h.query_by_label("Lowercase").is_none());
    h.get_by_label("Fraction").click();
    h.run();
    assert_eq!(h.state().latex(), r"\frac{}{}");
    h.state_mut().set_latex("");
    h.run();
    h.get_by_label("Greek letters").click();
    h.run();
    assert!(h.query_by_label("Fraction").is_none());
    for g in ["Lowercase", "Uppercase"] {
        h.get_by_label(g);
    }
    h.get_by_label("π").click();
    h.run();
    assert_eq!(h.state().latex(), r"\pi");
    h.get_by_label("Functions").click();
    h.run();
    for g in ["Trigonometric", "Hyperbolic", "Logarithms", "Counting and rounding", "Statistics"] {
        h.get_by_label(g);
    }
    assert!(h.state_mut().set_palette_tab("Arrows"));
    h.run();
    h.get_by_label("Double");
}

/// Type `s` into a fresh edit model.
fn typed(s: &str) -> Editor {
    let mut e = Editor::new(Math::default());
    e.type_str(s);
    e
}

#[test]
fn every_how_to_type_line_does_what_it_says() {
    let keys: Vec<&str> = HOW_TO_TYPE.iter().map(|(k, _)| *k).collect();
    assert_eq!(
        keys,
        [
            "x^2",
            "x_1",
            "a/b",
            "sqrt",
            "sum_(i=1)^n",
            "alpha  \\alpha  pi",
            "->",
            "Arrow keys, Tab",
            "Backspace",
            "Ctrl+Z, Ctrl+Y",
            "Esc twice",
            "Ctrl+V",
            "Palette"
        ],
        "a line was added or changed: check it below"
    );
    // x^2: superscript; + or Right leaves it.
    assert_eq!(typed("x^2+1").latex(), "x^2+1");
    let mut e = typed("x^2");
    e.key(MKey::Right, false);
    e.type_str("-1");
    assert_eq!(e.latex(), "x^2-1");
    // x_1
    assert_eq!(typed("x_1").latex(), "x_1");
    // a/b; Right leaves the denominator.
    let mut e = typed("a/b");
    assert_eq!(e.latex(), r"\frac{a}{b}");
    e.key(MKey::Right, false);
    e.type_str("+1");
    assert_eq!(e.latex(), r"\frac{a}{b}+1");
    // sqrt
    assert_eq!(typed("sqrt x").latex(), r"\sqrt{x}");
    // sum with limits; int and prod the same way.
    assert_eq!(typed("sum_(i=1)^n").latex(), r"\sum_{i=1}^n");
    assert_eq!(typed("int_0^1").latex(), r"\int_0^1");
    assert_eq!(typed("prod_(k=1)^n").latex(), r"\prod_{k=1}^n");
    // Greek by name, with or without a backslash.
    assert_eq!(typed("alpha").latex(), r"\alpha");
    assert_eq!(typed(r"\alpha").latex(), r"\alpha");
    assert_eq!(typed("pi").latex(), r"\pi");
    // ->
    assert_eq!(typed("x->y").latex(), r"x\to y");
    // Arrow keys and Tab move between places.
    let mut e = typed("a/b");
    e.key(MKey::Up, false);
    e.type_str("c");
    assert_eq!(e.latex(), r"\frac{ac}{b}");
    let mut e = typed("a/b");
    e.key(MKey::Tab, false);
    e.type_str("+1");
    assert_eq!(e.latex(), r"\frac{a}{b}+1");
    // Backspace
    let mut e = typed("x+y");
    e.key(MKey::Backspace, false);
    assert_eq!(e.latex(), "x+");
    // Undo and redo
    let mut e = typed("x+y");
    assert!(e.undo());
    assert_eq!(e.latex(), "");
    assert!(e.redo());
    assert_eq!(e.latex(), "x+y");
    // Esc twice, Ctrl+Z / Ctrl+Y and Ctrl+V on the real page.
    let mut h = harness();
    h.state_mut().load_example("euler");
    h.run();
    h.event(egui::Event::Text("+x".into()));
    h.run();
    assert_eq!(h.state().latex(), r"e^{i\pi}+1=0+x");
    h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    h.run();
    assert_eq!(h.state().latex(), r"e^{i\pi}+1=0");
    h.key_press_modifiers(Modifiers::COMMAND, Key::Y);
    h.run();
    assert_eq!(h.state().latex(), r"e^{i\pi}+1=0+x");
    h.key_press(Key::Escape);
    h.run();
    h.key_press(Key::Escape);
    h.run();
    assert_eq!(h.state().latex(), r"e^{i\pi}+1=0");
    // Palette: matrices and cases.
    h.state_mut().set_latex("");
    h.run();
    h.get_by_label("2 by 2 matrix").click();
    h.run();
    assert!(to_latex(h.state().editor.math()).contains("pmatrix"));
    h.get_by_label("Cases").click();
    h.run();
    assert!(h.state().latex().contains("cases"));
    // Square-bracket matrix, and the m x n builder.
    h.state_mut().set_latex("");
    h.run();
    h.get_by_label("2 by 2 matrix, square brackets").click();
    h.run();
    assert!(h.state().latex().contains("bmatrix"), "{}", h.state().latex());
    h.state_mut().set_latex("");
    h.run();
    h.get_by_label("Insert matrix").click();
    h.run();
    let tex = h.state().latex();
    assert!(tex.contains("pmatrix"), "{tex}");
    assert_eq!(tex.matches("\\\\").count(), 1, "2 rows: {tex}");
}

#[test]
fn the_six_copy_buttons_are_all_showing_in_even_rows_at_1280_and_700_px() {
    for w in [1280.0, 700.0] {
        let h = harness_at(w);
        let rects: Vec<egui::Rect> = CopyKind::ALL.iter().map(|k| h.get_by_label(k.title()).rect()).collect();
        assert!(rects.iter().all(|r| r.height() >= 36.0 && r.width() >= 100.0), "{w}: {rects:?}");
        // Buttons in one row share a top edge; rows are 3 or 6 wide.
        let first_row = rects.iter().filter(|r| (r.min.y - rects[0].min.y).abs() < 0.5).count();
        assert!(first_row == 6 || first_row == 3, "{w}: {first_row} in the first row: {rects:?}");
        assert!(rects.windows(2).all(|p| p[0].max.x <= p[1].min.x + 0.5 || p[1].min.y > p[0].max.y - 0.5), "{w}: {rects:?}");
    }
}

#[test]
fn the_copy_guide_is_one_line_of_true_claims() {
    let h = harness();
    h.get_by_label(COPY_GUIDE);
    // One sentence per target, and Word is only ever a suggestion (no Office install here).
    assert!(COPY_GUIDE.contains("ONLYOFFICE") && COPY_GUIDE.contains("Paste Special > Unformatted Text"));
    assert!(COPY_GUIDE.contains("in Word, try"));
    // One sentence.
    assert!(COPY_GUIDE.ends_with('.') && COPY_GUIDE.matches(". ").count() == 0, "{COPY_GUIDE}");
    assert!(!COPY_GUIDE.contains("LibreOffice"));
    assert_eq!(
        CopyKind::ALL.map(CopyKind::title),
        [
            "Copy LaTeX (Overleaf, Markdown, Anki)",
            "Copy MathML (ONLYOFFICE, LibreOffice Math, Word)",
            "Copy image (PNG, pastes anywhere)",
            "Copy OMML (Word equation XML)",
            "Copy text (plain, like a/b)",
            "Download SVG (black ink)",
        ]
    );
    assert_eq!(CopyKind::parse("word"), Some(CopyKind::MathMl));
    assert_eq!(CopyKind::parse("MATHML"), Some(CopyKind::MathMl));
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
    // The editor widget draws its ink in the theme's strongest text colour.
    assert_eq!(h.ctx.style_of(h.ctx.theme()).visuals.strong_text_color(), equashion_web::theme::LIGHT.strong);
    h.get_by_label("System").click();
    h.run();
    assert_eq!(h.state().theme_pref(), ThemePref::System);
}

#[test]
fn reduced_motion_turns_animation_off() {
    // Not through `harness()`: that one is already reduced.
    let mut h = Harness::builder().with_size(vec2(1280.0, 1400.0)).build_ui_state(|ui, s: &mut Showcase| s.ui(ui), Showcase::new());
    h.step();
    h.step();
    assert!(h.ctx.style_of(h.ctx.theme()).animation_time > 0.0);
    h.state_mut().set_reduced_motion(true);
    h.run();
    assert_eq!(h.ctx.style_of(h.ctx.theme()).animation_time, 0.0);
}

#[test]
fn the_footer_credits_mathlive_and_opens_about() {
    // Tall enough that the footer is on screen.
    let mut app = Showcase::new();
    app.set_reduced_motion(true);
    let mut h = Harness::builder().with_size(vec2(1280.0, 2000.0)).build_ui_state(|ui, s: &mut Showcase| s.ui(ui), app);
    h.run();
    h.get_by_label("MathLive");
    assert_eq!(CREDIT, "Typing behaviour is modelled on MathLive. Independent Rust implementation, not affiliated.");
    for word in ["port", "rewrite", "fork"] {
        assert!(!CREDIT.to_lowercase().contains(word), "{word}");
    }
    assert!(h.query_by_label("Known limits").is_none());
    h.get_by_label("About and licences").click();
    h.run();
    assert!(h.state().about_open());
    for s in ["What it is", "Credits", "Licence", "Known limits", "MIT OR Apache-2.0, at your option."] {
        h.get_by_label(s);
    }
}

#[test]
fn the_licence_line_matches_the_licence_files() {
    let cargo = include_str!("../../../Cargo.toml");
    assert!(cargo.contains("license = \"MIT OR Apache-2.0\""));
    assert!(
        include_str!("../../../LICENSE-MIT").contains("MIT License") || include_str!("../../../LICENSE-MIT").contains("Permission is hereby granted")
    );
    assert!(include_str!("../../../LICENSE-APACHE").contains("Apache License"));
}

#[test]
fn visible_strings_have_no_long_dashes() {
    for src in [
        include_str!("../src/showcase.rs"),
        include_str!("../src/palette.rs"),
        include_str!("../src/export.rs"),
        include_str!("../src/fab.rs"),
        include_str!("../src/logo.rs"),
        include_str!("../src/theme.rs"),
        include_str!("../index.html"),
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

#[test]
fn the_header_links_to_the_source_at_every_width() {
    for w in [1280.0, 700.0, 390.0] {
        let h = harness_at(w);
        h.get_by_label("GitHub");
        h.get_by_label("equaSHion");
        for t in ["System", "Light", "Dark"] {
            h.get_by_label(t);
        }
    }
    assert!(equashion_web::GITHUB_URL.starts_with("https://github.com/shuff57/equaSHion"));
}

fn fab_harness() -> Harness<'static, Showcase> {
    harness_at(1280.0)
}

#[test]
fn the_report_buttons_are_bottom_right_and_open_the_panel() {
    let mut h = fab_harness();
    h.get_by_label("Report a Bug");
    h.get_by_label("Reported issues");
    assert!(!h.state().fab().is_open());
    let (main, list) = (h.state().fab().rects().main.expect("main"), h.state().fab().rects().list.expect("list"));
    assert!((main.width() - 48.0).abs() < 0.5 && (list.width() - 40.0).abs() < 0.5);
    assert!((1280.0 - main.right() - 24.0).abs() < 1.5, "{main:?}");
    assert!(list.bottom() < main.top());
    h.get_by_label("Report a Bug").click();
    h.run();
    assert!(h.state().fab().is_open());
    h.get_by_label("Report an issue");
    for k in ["Bug", "Quirk", "Improve"] {
        h.get_by_label(k);
    }
    h.get_by_label("Send report");
    // An empty report is refused in words.
    h.get_by_label("Send report").click();
    h.run();
    assert_eq!(h.state().fab().error(), Some("Add a short title."));
    assert!(h.state_mut().fab_mut().take_requests().is_empty());
    h.get_by_label("Add a short title.");
}

#[test]
fn escape_closes_the_panel_and_a_click_outside_does_too() {
    let mut h = fab_harness();
    h.state_mut().fab_mut().toggle(equashion_web::fab::View::Form);
    h.run();
    assert!(h.state().fab().is_open());
    h.key_press(Key::Escape);
    h.run();
    assert!(!h.state().fab().is_open());
    h.state_mut().fab_mut().toggle(equashion_web::fab::View::List);
    h.run();
    assert!(h.state().fab().is_open());
    h.get_by_label("Try it").click();
    h.run();
    assert!(!h.state().fab().is_open());
}

#[test]
fn a_report_is_queued_with_the_equation_in_it() {
    use equashion_web::fab::{Request, View};
    let mut h = fab_harness();
    h.state_mut().set_latex(r"\frac{a}{b}");
    h.state_mut().fab_mut().toggle(View::Form);
    assert!(h.state_mut().fab_mut().fill("quirk", "Picky sqrt", "It wants a space."));
    h.run();
    h.get_by_label("Send report").click();
    h.run();
    let reqs = h.state_mut().fab_mut().take_requests();
    let [Request::Submit(body)] = reqs.as_slice() else { panic!("{reqs:?}") };
    assert!(body.contains("\"latex\":\"\\\\frac{a}{b}\""), "{body}");
    assert!(body.contains("\"kind\":\"quirk\""));
    h.get_by_label("Submitting...");
}

#[test]
fn the_mug_needs_the_widget_and_asks_the_shell_to_click_it() {
    use equashion_web::fab::Request;
    let mut h = fab_harness();
    assert!(h.query_by_label("Buy me a coffee").is_none(), "no widget, no mug");
    h.state_mut().fab_mut().set_coffee_available(true);
    h.run();
    h.get_by_label("Buy me a coffee").click();
    h.run();
    assert_eq!(h.state_mut().fab_mut().take_requests(), vec![Request::OpenCoffee]);
}

#[test]
fn the_wordmark_is_there_at_every_width() {
    for w in [1280.0, 390.0] {
        let h = harness_at(w);
        h.get_by_label("equaSHion");
    }
}
