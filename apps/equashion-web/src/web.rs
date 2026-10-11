//! The browser shell: eframe's web runner (WebGL2 through glow), the clipboard and download
//! side of the Copy buttons, and the `window.equashion` test hooks. The canvas is opaque to the
//! DOM, so tests read and drive the app through:
//!
//! | Hook | Returns |
//! |---|---|
//! | `linear()` | the editor's equation as linear text |
//! | `omml()` | the editor's equation as OMML |
//! | `latex()` | the editor's equation as LaTeX |
//! | `setLinear(s)` | loads `s` into the editor; returns `linear()` |
//! | `setLatex(s)` | loads LaTeX `s` (a `$..$` wrapper is dropped); returns `latex()` |
//! | `paste(s)` | what Ctrl+V in the editor does with `s`; returns `latex()`, or `null` when refused |
//! | `copy(kind)` | presses a Copy button (`latex`, `mathml`, `image`, `omml`, `text`, `svg`; `word` is an alias of `mathml`); a Promise of `{kind, status, items}` where `items` maps each MIME type to a string or a `Uint8Array`, and `status` is `copied`, `downloaded` or `blocked` |
//! | `copyStatus()` | `{kind, state}` of the last Copy action, or `null` |
//! | `copyButtonRect(kind)` | `{x, y, w, h}` of that Copy button in CSS pixels (for real clicks), or `null` when it is not on screen (the big button shows the picked format; every format is in the drop-down) |
//! | `showMoreCopy(on)` | opens or closes the format drop-down; returns `on` |
//! | `examples()` | `[{id, name}]` of the Try it examples |
//! | `loadExample(idOrName)` | loads one; returns `latex()`, or `null` when unknown |
//! | `selftest()` | `{ok, total, failures: [{id, reason}], missingGlyphs}` (runs it now) |
//! | `galleryCount()` | number of corpus rows |
//! | `activeTab()` | always `editor` (the page has one view) |
//! | `setTab(name)` | accepted and ignored; returns true |
//! | `showAbout(on)` | opens or closes the About panel |
//! | `setDisclosures(paste, howto, report)` | opens or closes the paste-in, "How to type" and "Report an issue" disclosures |
//! | `fillReport(kind, title, description)` | fills the report form (`kind` is `bug`, `quirk` or `enhancement`); false for an unknown kind |
//! | `submitReport()` | presses Send report (an empty title or description stays on the page with a note) |
//! | `reportStatus()` | `idle`, `sending`, `sent` or `failed` |
//! | `paletteTabs()` | the palette's tab titles, in order |
//! | `setPaletteTab(title)` | selects a palette tab; false when there is no such title |
//! | `unknownCommands()` | array of the equation's unknown backslash commands |
//! | `theme()` | the theme in effect: `light` or `dark` |
//! | `themePreference()` | `system`, `light` or `dark` |
//! | `setTheme(name)` | `system`, `light` or `dark`; true when `name` is one of them |
//! | `ready` | `true` once the hooks are installed |
//!
//! The app state is shared with the hooks through `Rc<RefCell<..>>`: JavaScript calls run
//! between frames on the one wasm thread, so a hook never meets a frame in progress.
//!
//! Clipboard writes go through `window.equashionClipboard` (in `index.html`): text kinds through
//! a `copy` event (`execCommand('copy')`, which keeps `text/html` as written), then the async
//! Clipboard API; the image through `navigator.clipboard.write` with an `image/png` ClipboardItem.

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Array, Function, Object, Promise, Reflect, Uint8Array};
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::export::{CopyKind, Data};
use crate::report::{Kind, Shot, Submission};
use crate::theme::{ThemePref, theme_name};
use crate::{CopyRequest, Outcome, Showcase};

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
                    let mut app = Showcase::new();
                    app.set_reduced_motion(prefers_reduced_motion());
                    app.set_report_context(&browser_name(), &page_url());
                    app.install(&cc.egui_ctx);
                    let shared: Shared = Rc::new(RefCell::new(app));
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
        let (requests, reports, pick) = match self.shared.try_borrow_mut() {
            Ok(mut s) => {
                s.ui(ui);
                if s.report_accepts_images()
                    && let Some(shot) = take_pasted()
                {
                    s.shot_loaded(shot);
                }
                (s.take_requests(), s.take_reports(), s.take_shot_request())
            }
            Err(_) => (Vec::new(), Vec::new(), false),
        };
        for r in requests {
            let (a, c) = (self.shared.clone(), ui.ctx().clone());
            wasm_bindgen_futures::spawn_local(async move {
                let _ = execute(a, c, r).await;
            });
        }
        for r in reports {
            let (a, c) = (self.shared.clone(), ui.ctx().clone());
            wasm_bindgen_futures::spawn_local(async move {
                send_report(a, c, r).await;
            });
        }
        if pick {
            let (a, c) = (self.shared.clone(), ui.ctx().clone());
            wasm_bindgen_futures::spawn_local(async move {
                pick_screenshot(a, c).await;
            });
        }
    }
}

/// The request's data as `{mime: string | Uint8Array}`.
fn items_js(r: &CopyRequest) -> Result<Object, JsValue> {
    let items = Object::new();
    for (mime, data) in &r.payload.items {
        let v: JsValue = match data {
            Data::Text(t) => JsValue::from_str(t),
            Data::Bytes(b) => Uint8Array::from(b.as_slice()).into(),
        };
        set(&items, mime, &v)?;
    }
    Ok(items)
}

/// Carry out a Copy request (clipboard write or download), tell the app how it ended, and
/// return `{kind, status, items}`. The clipboard call starts synchronously, inside the user
/// activation of the click.
async fn execute(s: Shared, ctx: egui::Context, r: CopyRequest) -> Result<JsValue, JsValue> {
    let items = items_js(&r)?;
    let status = match start_write(&r, &items) {
        Ok(p) => JsFuture::from(p).await.ok().and_then(|v| v.as_string()).unwrap_or_else(|| "blocked".into()),
        Err(_) => "blocked".into(),
    };
    let outcome = match status.as_str() {
        "copied" => Outcome::Copied,
        "downloaded" => Outcome::Downloaded,
        _ => Outcome::Blocked,
    };
    let fallback = r.payload.fallback_text().map(str::to_string);
    if let Ok(mut app) = s.try_borrow_mut() {
        app.copy_finished(r.id, outcome, fallback);
    }
    ctx.request_repaint();
    let out = Object::new();
    set(&out, "kind", &JsValue::from_str(r.payload.kind.name()))?;
    set(&out, "status", &JsValue::from_str(&status))?;
    set(&out, "items", &items)?;
    Ok(out.into())
}

fn start_write(r: &CopyRequest, items: &Object) -> Result<Promise, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let helper = Reflect::get(&window, &JsValue::from_str("equashionClipboard"))?;
    let (name, args): (&str, Vec<JsValue>) = match r.payload.filename {
        Some(f) => ("download", vec![JsValue::from_str(f), items.into()]),
        None => ("write", vec![items.into()]),
    };
    let f: Function = Reflect::get(&helper, &JsValue::from_str(name))?.dyn_into()?;
    let v = f.apply(&helper, &args.into_iter().collect::<Array>())?;
    Ok(Promise::resolve(&v))
}

/// Call `window.equashionReport[name](args)`.
fn report_helper(name: &str, args: &[JsValue]) -> Result<Promise, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let helper = Reflect::get(&window, &JsValue::from_str("equashionReport"))?;
    let f: Function = Reflect::get(&helper, &JsValue::from_str(name))?.dyn_into()?;
    let v = f.apply(&helper, &args.iter().cloned().collect::<Array>())?;
    Ok(Promise::resolve(&v))
}

/// POST one report and tell the app how it went.
async fn send_report(s: Shared, ctx: egui::Context, r: Submission) {
    let json = JsValue::from_str(&r.to_json());
    let answer = match report_helper("send", &[json]) {
        Ok(p) => JsFuture::from(p).await.ok().and_then(|v| v.as_string()),
        Err(_) => None,
    };
    let result = match answer.as_deref() {
        Some("sent") => Ok(()),
        Some(why) if !why.is_empty() => Err(why.to_string()),
        _ => Err("The report could not be sent.".to_string()),
    };
    if let Ok(mut app) = s.try_borrow_mut() {
        app.report_finished(result);
    }
    ctx.request_repaint();
}

/// A picked file or pasted image as the app's [`Shot`].
fn shot_from(v: &JsValue) -> Option<Result<Shot, String>> {
    if v.is_null() || v.is_undefined() {
        return None;
    }
    if let Some(e) = Reflect::get(v, &JsValue::from_str("error")).ok().and_then(|e| e.as_string()) {
        return Some(Err(e));
    }
    let name = Reflect::get(v, &JsValue::from_str("name")).ok().and_then(|e| e.as_string())?;
    let data_url = Reflect::get(v, &JsValue::from_str("dataUrl")).ok().and_then(|e| e.as_string())?;
    Some(Ok(Shot { name, data_url }))
}

/// Open the file picker; the app takes the image (or the refusal), or just stops waiting.
async fn pick_screenshot(s: Shared, ctx: egui::Context) {
    let v = match report_helper("pickScreenshot", &[]) {
        Ok(p) => JsFuture::from(p).await.unwrap_or(JsValue::NULL),
        Err(_) => JsValue::NULL,
    };
    if let Ok(mut app) = s.try_borrow_mut() {
        match shot_from(&v) {
            Some(r) => app.shot_loaded(r),
            None => app.shot_loaded(Err(String::new())),
        }
    }
    ctx.request_repaint();
}

/// An image pasted since the last frame, if any (it is taken, so it is read once).
fn take_pasted() -> Option<Result<Shot, String>> {
    let window = web_sys::window()?;
    let helper = Reflect::get(&window, &JsValue::from_str("equashionReport")).ok()?;
    let v = Reflect::get(&helper, &JsValue::from_str("pasted")).ok()?;
    let shot = shot_from(&v)?;
    let _ = Reflect::set(&helper, &JsValue::from_str("pasted"), &JsValue::NULL);
    Some(shot)
}

fn browser_name() -> String {
    web_sys::window().and_then(|w| w.navigator().user_agent().ok()).unwrap_or_default()
}

fn page_url() -> String {
    web_sys::window().and_then(|w| w.location().href().ok()).unwrap_or_default()
}

/// `prefers-reduced-motion: reduce` is set in the browser.
fn prefers_reduced_motion() -> bool {
    web_sys::window().and_then(|w| w.match_media("(prefers-reduced-motion: reduce)").ok().flatten()).is_some_and(|m| m.matches())
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
    let f = Closure::<dyn Fn() -> String>::new(move || with(&a, |app| app.active_tab().to_string()));
    set(&obj, "activeTab", f.as_ref())?;
    f.forget();

    // The page has one view: any name is accepted and ignored.
    let f = Closure::<dyn Fn(JsValue) -> bool>::new(move |_name: JsValue| true);
    set(&obj, "setTab", f.as_ref())?;
    f.forget();

    let a = s.clone();
    let f = Closure::<dyn Fn() -> String>::new(move || with(&a, |app| app.latex()));
    set(&obj, "latex", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn(String) -> String>::new(move |x: String| {
        let out = with(&a, |app| {
            app.set_latex(&x);
            app.latex()
        });
        c.request_repaint();
        out
    });
    set(&obj, "setLatex", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn(String) -> JsValue>::new(move |x: String| {
        let out = with(&a, |app| app.paste(&x).ok().map(|()| app.latex()));
        c.request_repaint();
        out.map_or(JsValue::NULL, |l| JsValue::from_str(&l))
    });
    set(&obj, "paste", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn(String) -> Promise>::new(move |kind: String| {
        let Some(k) = CopyKind::parse(&kind) else {
            return Promise::reject(&JsValue::from_str(&format!("unknown kind {kind}: latex, mathml, image, omml, text or svg")));
        };
        let requests = with(&a, |app| {
            app.request_copy(k);
            app.take_requests()
        });
        let (a, c) = (a.clone(), c.clone());
        wasm_bindgen_futures::future_to_promise(async move {
            let mut last = JsValue::NULL;
            for r in requests {
                last = execute(a.clone(), c.clone(), r).await?;
            }
            Ok(last)
        })
    });
    set(&obj, "copy", f.as_ref())?;
    f.forget();

    let a = s.clone();
    let f = Closure::<dyn Fn() -> JsValue>::new(move || {
        let st = with(&a, |app| app.copy_status());
        match st {
            Some((kind, state)) => {
                let o = Object::new();
                let _ = set(&o, "kind", &JsValue::from_str(kind));
                let _ = set(&o, "state", &JsValue::from_str(state));
                o.into()
            }
            None => JsValue::NULL,
        }
    });
    set(&obj, "copyStatus", f.as_ref())?;
    f.forget();

    let a = s.clone();
    let f = Closure::<dyn Fn(String) -> JsValue>::new(move |kind: String| {
        let r = with(&a, |app| CopyKind::parse(&kind).and_then(|k| app.copy_button_rect(k)));
        match r {
            Some(r) => {
                let o = Object::new();
                for (k, v) in [("x", r.min.x), ("y", r.min.y), ("w", r.width()), ("h", r.height())] {
                    let _ = set(&o, k, &JsValue::from_f64(f64::from(v)));
                }
                o.into()
            }
            None => JsValue::NULL,
        }
    });
    set(&obj, "copyButtonRect", f.as_ref())?;
    f.forget();

    let a = s.clone();
    let f = Closure::<dyn Fn() -> Array>::new(move || {
        let list = with(&a, |app| app.examples().into_iter().map(|(id, name, _)| (id, name)).collect::<Vec<_>>());
        list.into_iter()
            .map(|(id, name)| {
                let o = Object::new();
                let _ = set(&o, "id", &JsValue::from_str(id));
                let _ = set(&o, "name", &JsValue::from_str(name));
                JsValue::from(o)
            })
            .collect()
    });
    set(&obj, "examples", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn(String) -> JsValue>::new(move |key: String| {
        let out = with(&a, |app| app.load_example(&key).then(|| app.latex()));
        c.request_repaint();
        out.map_or(JsValue::NULL, |l| JsValue::from_str(&l))
    });
    set(&obj, "loadExample", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn(bool) -> bool>::new(move |on: bool| {
        with(&a, |app| app.set_about_open(on));
        c.request_repaint();
        true
    });
    set(&obj, "showAbout", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn(bool) -> bool>::new(move |on: bool| {
        with(&a, |app| app.set_more_copy(on));
        c.request_repaint();
        on
    });
    set(&obj, "showMoreCopy", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn(bool, bool, bool) -> bool>::new(move |paste: bool, howto: bool, report: bool| {
        with(&a, |app| app.set_disclosures(paste, howto, report));
        c.request_repaint();
        true
    });
    set(&obj, "setDisclosures", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn(String, String, String) -> bool>::new(move |kind: String, title: String, description: String| {
        let kind = Kind::ALL.into_iter().find(|k| k.value() == kind);
        let ok = kind.is_some();
        with(&a, |app| {
            if let Some(k) = kind {
                app.fill_report(k, &title, &description);
            }
        });
        c.request_repaint();
        ok
    });
    set(&obj, "fillReport", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn() -> bool>::new(move || {
        with(&a, |app| app.submit_report());
        c.request_repaint();
        true
    });
    set(&obj, "submitReport", f.as_ref())?;
    f.forget();

    let a = s.clone();
    let f = Closure::<dyn Fn() -> String>::new(move || with(&a, |app| app.report_status().to_string()));
    set(&obj, "reportStatus", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn(String) -> bool>::new(move |title: String| {
        let ok = with(&a, |app| app.set_palette_tab(&title));
        c.request_repaint();
        ok
    });
    set(&obj, "setPaletteTab", f.as_ref())?;
    f.forget();

    let a = s.clone();
    let f = Closure::<dyn Fn() -> Array>::new(move || with(&a, |app| app.palette_tabs()).into_iter().map(JsValue::from_str).collect());
    set(&obj, "paletteTabs", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn() -> String>::new(move || with(&a, |app| theme_name(app.effective_theme(&c)).to_string()));
    set(&obj, "theme", f.as_ref())?;
    f.forget();

    let a = s.clone();
    let f = Closure::<dyn Fn() -> String>::new(move || with(&a, |app| app.theme_pref().name().to_string()));
    set(&obj, "themePreference", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn(String) -> bool>::new(move |name: String| {
        let ok = with(&a, |app| match ThemePref::parse(&name) {
            Some(t) => {
                app.set_theme_pref(t);
                true
            }
            None => false,
        });
        c.request_repaint();
        ok
    });
    set(&obj, "setTheme", f.as_ref())?;
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
