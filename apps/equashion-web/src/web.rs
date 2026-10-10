//! The browser shell: eframe's web runner (WebGL2 through glow) and the `window.equashion`
//! test hooks. The canvas is opaque to the DOM, so tests read and drive the app through:
//!
//! | Hook | Returns |
//! |---|---|
//! | `linear()` | the editor's equation as linear text |
//! | `omml()` | the editor's equation as OMML |
//! | `setLinear(s)` | loads `s` into the editor; returns `linear()` |
//! | `selftest()` | `{ok, total, failures: [{id, reason}], missingGlyphs}` (runs it now) |
//! | `galleryCount()` | number of gallery cards |
//! | `activeTab()` | `editor`, `gallery`, `omml` or `selftest` |
//! | `setTab(name)` | switches tab; true when `name` is a tab |
//! | `unknownCommands()` | array of the linear field's unknown backslash commands |
//! | `ready` | `true` once the hooks are installed |
//!
//! The app state is shared with the hooks through `Rc<RefCell<..>>`: JavaScript calls run
//! between frames on the one wasm thread, so a hook never meets a frame in progress.

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Array, Object, Reflect};
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::*;

use crate::{Showcase, Tab};

const CANVAS_ID: &str = "equashion_canvas";
const LOADING_ID: &str = "equashion_loading";

type Shared = Rc<RefCell<Showcase>>;

/// Start the app on `<canvas id="equashion_canvas">`.
pub fn start() {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            log::error!("no document");
            return;
        };
        let Some(canvas) = document.get_element_by_id(CANVAS_ID).and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok()) else {
            log::error!("missing <canvas id=\"{CANVAS_ID}\">");
            return;
        };
        let result = eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(|cc| {
                    let shared: Shared = Rc::new(RefCell::new(Showcase::new()));
                    if let Err(e) = install_hooks(shared.clone(), cc.egui_ctx.clone()) {
                        log::error!("equashion: could not install window.equashion: {e:?}");
                    }
                    Ok(Box::new(WebApp { shared }))
                }),
            )
            .await;
        if let Some(el) = document.get_element_by_id(LOADING_ID) {
            match result {
                Ok(()) => el.remove(),
                Err(e) => el.set_inner_html(&format!("<p>equaSHion failed to start: {e:?}</p><p>A browser with WebGL2 is required.</p>")),
            }
        }
    });
}

struct WebApp {
    shared: Shared,
}

impl eframe::App for WebApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ui, |ui| {
            if let Ok(mut s) = self.shared.try_borrow_mut() {
                s.ui(ui);
            }
        });
    }
}

fn set(obj: &Object, name: &str, v: &JsValue) -> Result<(), JsValue> {
    Reflect::set(obj, &JsValue::from_str(name), v).map(|_| ())
}

/// Read the shared app (or a default when a hook re-enters, which cannot happen on one thread).
fn with<R: Default>(s: &Shared, f: impl FnOnce(&mut Showcase) -> R) -> R {
    s.try_borrow_mut().map(|mut a| f(&mut a)).unwrap_or_default()
}

fn install_hooks(s: Shared, ctx: egui::Context) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let obj = Object::new();

    let a = s.clone();
    let f = Closure::<dyn Fn() -> String>::new(move || with(&a, |app| app.editor.linear()));
    set(&obj, "linear", f.as_ref())?;
    f.forget();

    let a = s.clone();
    let f = Closure::<dyn Fn() -> String>::new(move || with(&a, |app| app.editor.omml()));
    set(&obj, "omml", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn(String) -> String>::new(move |x: String| {
        let out = with(&a, |app| {
            app.editor.set_linear(&x);
            app.editor.linear()
        });
        c.request_repaint();
        out
    });
    set(&obj, "setLinear", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn() -> JsValue>::new(move || {
        let v = with(&a, |app| report_js(app).unwrap_or(JsValue::NULL));
        c.request_repaint();
        v
    });
    set(&obj, "selftest", f.as_ref())?;
    f.forget();

    let a = s.clone();
    let f = Closure::<dyn Fn() -> u32>::new(move || with(&a, |app| app.gallery_count() as u32));
    set(&obj, "galleryCount", f.as_ref())?;
    f.forget();

    let a = s.clone();
    let f = Closure::<dyn Fn() -> String>::new(move || with(&a, |app| app.tab().name().to_string()));
    set(&obj, "activeTab", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx);
    let f = Closure::<dyn Fn(String) -> bool>::new(move |name: String| {
        let ok = with(&a, |app| match Tab::parse(&name) {
            Some(t) => {
                app.set_tab(t);
                true
            }
            None => false,
        });
        c.request_repaint();
        ok
    });
    set(&obj, "setTab", f.as_ref())?;
    f.forget();

    let a = s;
    let f = Closure::<dyn Fn() -> Array>::new(move || {
        let names = with(&a, |app| app.editor.unknown_commands());
        names.iter().map(|n| JsValue::from_str(n)).collect()
    });
    set(&obj, "unknownCommands", f.as_ref())?;
    f.forget();

    set(&obj, "ready", &JsValue::TRUE)?;
    set(&window, "equashion", &obj)?;
    log::info!("equashion: window.equashion hooks installed");
    Ok(())
}

/// Run the selftest and describe it as `{ok, total, failures: [{id, reason}], missingGlyphs}`.
fn report_js(app: &mut Showcase) -> Result<JsValue, JsValue> {
    let rep = app.run_selftest();
    let out = Object::new();
    set(&out, "ok", &JsValue::from_bool(rep.ok()))?;
    set(&out, "total", &JsValue::from_f64(rep.checks.len() as f64))?;
    let missing: usize = rep.checks.iter().map(|c| c.missing_glyphs).sum();
    set(&out, "missingGlyphs", &JsValue::from_f64(missing as f64))?;
    let failures = Array::new();
    for c in rep.failures() {
        let f = Object::new();
        set(&f, "id", &JsValue::from_str(&c.id))?;
        set(&f, "reason", &JsValue::from_str(c.reason.as_deref().unwrap_or("")))?;
        failures.push(&f);
    }
    set(&out, "failures", &failures)?;
    Ok(out.into())
}
