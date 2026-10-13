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
//! | `copyButtonRect(kind)` | `{x, y, w, h}` of that Copy button in CSS pixels (for real clicks), or `null` when it is not on screen |
//! | `examples()` | `[{id, name}]` of the Try it examples |
//! | `loadExample(idOrName)` | loads one; returns `latex()`, or `null` when unknown |
//! | `selftest()` | `{ok, total, failures: [{id, reason}], missingGlyphs}` (runs it now) |
//! | `galleryCount()` | number of corpus rows |
//! | `version()` | the app version, sent with issue reports |
//! | `activeTab()` | always `editor` (the page has one view) |
//! | `setTab(name)` | accepted and ignored; returns true |
//! | `showAbout(on)` | opens or closes the About panel |
//! | `setDisclosures(paste, howto)` | opens or closes the paste-in and "How to type" disclosures |
//! | `paletteTabs()` | the palette's tab titles, in order |
//! | `setPaletteTab(title)` | selects a palette tab; false when there is no such title |
//! | `unknownCommands()` | array of the equation's unknown backslash commands |
//! | `theme()` | the theme in effect: `light` or `dark` |
//! | `themePreference()` | `system`, `light` or `dark` |
//! | `setTheme(name)` | `system`, `light` or `dark`; true when `name` is one of them |
//! | `reportShow(view)` | opens the report panel on `form` or `list` (not a toggle); false for another name |
//! | `reportClose()` | closes the report panel |
//! | `reportFill(kind, title, description)` | fills the form (`bug`, `quirk` or `enhancement`); false for another kind |
//! | `reportSubmit()` | presses Send report (an empty title or description is refused in the panel) |
//! | `reportVote(id)` | presses Vote on that issue |
//! | `reportState()` | `{open, view, kind, busy, error, thanks, list, listError, items: [{id, kind, title, status, votes, voted}]}` |
//! | `fabRects()` | `{main, list, mug, panel, invite}`, each `{x, y, w, h}` in CSS pixels or `null` (for real clicks) |
//! | `coffeeClicks()` | how many times the mug has clicked Buy Me a Coffee's own button |
//! | `coffeeOpen()` | presses the mug |
//! | `replayLogo()` | plays the header wordmark again |
//! | `ready` | `true` once the hooks are installed |
//!
//! The app state is shared with the hooks through `Rc<RefCell<..>>`: JavaScript calls run
//! between frames on the one wasm thread, so a hook never meets a frame in progress.
//!
//! The report panel, the list of reports and the coffee mug are egui (`fab.rs`); this file does
//! what the browser has to: `fetch` for `/api/issue-reports`, `localStorage` and `sessionStorage`,
//! the image file picker and image paste, and clicking Buy Me a Coffee's own button.
//!
//! Clipboard writes are done here (`write_clipboard`): text kinds through a `copy` event
//! (`execCommand('copy')`, which keeps `text/html` as written), then the async Clipboard API;
//! the image through `navigator.clipboard.write` with an `image/png` ClipboardItem.

use std::cell::RefCell;
use std::rc::Rc;

use js_sys::{Array, Function, Object, Promise, Reflect, Uint8Array};
use wasm_bindgen::JsCast as _;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, future_to_promise};

use crate::export::{CopyKind, Data};
use crate::fab::{self, Request, View};
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
                    if splash_wanted() {
                        // The splash plays the wordmark while the page loads; the header's copy then
                        // starts where it ended instead of playing it a second time.
                        app.settle_logo();
                    }
                    app.install(&cc.egui_ctx);
                    let shared: Shared = Rc::new(RefCell::new(app));
                    if let Err(e) = install_hooks(shared.clone(), cc.egui_ctx.clone()) {
                        log::error!("equashion: could not install window.equashion: {e:?}");
                    }
                    setup_fab(&shared);
                    install_paste(shared.clone(), cc.egui_ctx.clone());
                    Ok(Box::new(WebApp { shared }))
                }),
            )
            .await;
        if result.is_ok() {
            hold_splash().await;
        }
        if let Some(el) = document.get_element_by_id(LOADING_ID) {
            match result {
                Ok(()) => fade_out(&el).await,
                Err(e) => el.set_inner_html(&format!("<p>equaSHion failed to start: {e:?}</p><p>A browser with WebGL2 is required.</p>")),
            }
        }
    });
}

/// How long the loading splash stays up at least, from the start of the page: about 4 s, after
/// the word has set (3.5 s of the wordmark's motion). It stays longer only while the app is still
/// loading. `?splash=0` skips it (the browser tests).
const SPLASH_MS: f64 = 4000.0;

fn splash_wanted() -> bool {
    let search = web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default();
    !search.trim_start_matches('?').split('&').any(|kv| kv == "splash=0")
}

/// Wait out what is left of the splash's animation.
async fn hold_splash() {
    if !splash_wanted() {
        return;
    }
    let now = web_sys::window().and_then(|w| w.performance()).map_or(SPLASH_MS, |p| p.now());
    if now < SPLASH_MS {
        sleep_ms((SPLASH_MS - now).ceil() as i32).await;
    }
}

/// Fade the splash away (the CSS has the transition), then remove it.
async fn fade_out(el: &web_sys::Element) {
    if let Some(h) = el.dyn_ref::<web_sys::HtmlElement>() {
        let _ = h.style().set_property("opacity", "0");
        sleep_ms(380).await;
    }
    el.remove();
}

struct WebApp {
    shared: Shared,
}

impl eframe::App for WebApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let (requests, fab_requests) = match self.shared.try_borrow_mut() {
            Ok(mut s) => {
                s.ui(ui);
                (s.take_requests(), s.fab_mut().take_requests())
            }
            Err(_) => (Vec::new(), Vec::new()),
        };
        for r in requests {
            let (a, c) = (self.shared.clone(), ui.ctx().clone());
            wasm_bindgen_futures::spawn_local(async move {
                let _ = execute(a, c, r).await;
            });
        }
        for r in fab_requests {
            let (a, c) = (self.shared.clone(), ui.ctx().clone());
            wasm_bindgen_futures::spawn_local(async move {
                run_fab_request(a, c, r).await;
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

/// Start the clipboard write or download; the promise resolves to `copied`, `downloaded` or
/// `blocked`. Everything that needs the click's user activation happens before this returns.
fn start_write(r: &CopyRequest, items: &Object) -> Result<Promise, JsValue> {
    let status = match r.payload.filename {
        Some(name) => download(name, items),
        None => return write_clipboard(items),
    };
    Ok(Promise::resolve(&JsValue::from_str(status)))
}

/// Text kinds go through a `copy` event so `text/html` is written as given; otherwise the
/// async Clipboard API (`writeText`, or a `ClipboardItem` for several types or bytes).
fn write_clipboard(items: &Object) -> Result<Promise, JsValue> {
    let blocked = || Promise::resolve(&JsValue::from_str("blocked"));
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let mimes: Vec<String> = Object::keys(items).iter().filter_map(|m| m.as_string()).collect();
    let binary = mimes.iter().any(|m| !Reflect::get(items, &JsValue::from_str(m)).is_ok_and(|v| v.is_string()));

    if !binary && copy_event(&window, items, &mimes) {
        return Ok(Promise::resolve(&JsValue::from_str("copied")));
    }

    let Ok(clip) = Reflect::get(window.navigator().as_ref(), &JsValue::from_str("clipboard")) else { return Ok(blocked()) };
    if clip.is_undefined() || clip.is_null() {
        return Ok(blocked());
    }
    let call = |name: &str, args: &Array| -> Result<Promise, JsValue> {
        let f: Function = Reflect::get(&clip, &JsValue::from_str(name))?.dyn_into()?;
        f.apply(&clip, args)?.dyn_into()
    };
    let started = if !binary && mimes.len() == 1 {
        let text = Reflect::get(items, &JsValue::from_str(&mimes[0]))?;
        call("writeText", &Array::of1(&text))
    } else {
        let ctor = Reflect::get(&js_sys::global(), &JsValue::from_str("ClipboardItem"))?;
        if ctor.is_undefined() {
            return Ok(blocked());
        }
        let blobs = Object::new();
        for m in &mimes {
            let blob = blob_of(&Reflect::get(items, &JsValue::from_str(m))?, m)?;
            set(&blobs, m, &blob)?;
        }
        let ctor: Function = ctor.dyn_into()?;
        let item = Reflect::construct(&ctor, &Array::of1(&blobs))?;
        call("write", &Array::of1(&Array::of1(&item)))
    };
    match started {
        Ok(p) => Ok(future_to_promise(async move {
            Ok(JsFuture::from(p).await.map_or_else(|_| JsValue::from_str("blocked"), |_| JsValue::from_str("copied")))
        })),
        Err(_) => Ok(blocked()),
    }
}

/// Run `execCommand('copy')` with a capturing `copy` listener on the window (ahead of eframe's
/// own handler) that sets every MIME type on the event. True when it wrote them.
fn copy_event(window: &web_sys::Window, items: &Object, mimes: &[String]) -> bool {
    let Some(doc) = window.document().and_then(|d| d.dyn_into::<web_sys::HtmlDocument>().ok()) else { return false };
    if !doc.query_command_supported("copy") {
        return false;
    }
    let done = Rc::new(std::cell::Cell::new(false));
    let (flag, data, names) = (done.clone(), items.clone(), mimes.to_vec());
    let on_copy = Closure::<dyn FnMut(web_sys::ClipboardEvent)>::new(move |e: web_sys::ClipboardEvent| {
        e.stop_immediate_propagation();
        e.prevent_default();
        if let Some(dt) = e.clipboard_data() {
            for m in &names {
                if let Some(t) = Reflect::get(&data, &JsValue::from_str(m)).ok().and_then(|v| v.as_string()) {
                    let _ = dt.set_data(m, &t);
                }
            }
            flag.set(true);
        }
    });
    let f: &Function = on_copy.as_ref().unchecked_ref();
    let _ = window.add_event_listener_with_callback_and_bool("copy", f, true);
    let ok = doc.exec_command("copy").unwrap_or(false);
    let _ = window.remove_event_listener_with_callback_and_bool("copy", f, true);
    ok && done.get()
}

fn blob_of(data: &JsValue, mime: &str) -> Result<JsValue, JsValue> {
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(mime);
    Ok(web_sys::Blob::new_with_str_sequence_and_options(&Array::of1(data), &opts)?.into())
}

/// Save the first item as a file through a throwaway `<a download>`.
fn download(name: &str, items: &Object) -> &'static str {
    let go = || -> Result<(), JsValue> {
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        let mime = Object::keys(items).get(0).as_string().ok_or("no data")?;
        let blob = blob_of(&Reflect::get(items, &JsValue::from_str(&mime))?, &mime)?;
        let url = web_sys::Url::create_object_url_with_blob(&blob.unchecked_into())?;
        let a: web_sys::HtmlAnchorElement = document.create_element("a")?.dyn_into()?;
        a.set_href(&url);
        a.set_download(name);
        a.style().set_property("display", "none")?;
        let body = document.body().ok_or("no body")?;
        body.append_child(&a)?;
        a.click();
        a.remove();
        let revoke = Closure::once_into_js(move || {
            let _ = web_sys::Url::revoke_object_url(&url);
        });
        window.set_timeout_with_callback_and_timeout_and_arguments_0(revoke.unchecked_ref(), 10_000)?;
        Ok(())
    };
    if go().is_ok() { "downloaded" } else { "blocked" }
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

    let f = Closure::<dyn Fn() -> String>::new(|| env!("CARGO_PKG_VERSION").to_string());
    set(&obj, "version", f.as_ref())?;
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
    let f = Closure::<dyn Fn(bool, bool) -> bool>::new(move |paste: bool, howto: bool| {
        with(&a, |app| app.set_disclosures(paste, howto));
        c.request_repaint();
        true
    });
    set(&obj, "setDisclosures", f.as_ref())?;
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

    let a = s.clone();
    let f = Closure::<dyn Fn() -> Array>::new(move || {
        let names = with(&a, |app| app.editor.unknown_commands());
        names.iter().map(|n| JsValue::from_str(n)).collect()
    });
    set(&obj, "unknownCommands", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn(String) -> bool>::new(move |view: String| {
        let ok = with(&a, |app| match View::parse(&view) {
            Some(v) => {
                let f = app.fab_mut();
                if !(f.is_open() && f.view() == v) {
                    f.toggle(v);
                }
                true
            }
            None => false,
        });
        c.request_repaint();
        ok
    });
    set(&obj, "reportShow", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn() -> bool>::new(move || {
        with(&a, |app| app.fab_mut().close());
        c.request_repaint();
        true
    });
    set(&obj, "reportClose", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn(String, String, String) -> bool>::new(move |kind: String, title: String, description: String| {
        let ok = with(&a, |app| app.fab_mut().fill(&kind, &title, &description));
        c.request_repaint();
        ok
    });
    set(&obj, "reportFill", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn() -> bool>::new(move || {
        with(&a, |app| {
            let latex = app.latex();
            app.fab_mut().submit(&latex);
        });
        c.request_repaint();
        true
    });
    set(&obj, "reportSubmit", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn(String) -> bool>::new(move |id: String| {
        with(&a, |app| app.fab_mut().vote(&id));
        c.request_repaint();
        true
    });
    set(&obj, "reportVote", f.as_ref())?;
    f.forget();

    let a = s.clone();
    let f = Closure::<dyn Fn() -> JsValue>::new(move || with(&a, |app| fab_state_js(app.fab()).unwrap_or(JsValue::NULL)));
    set(&obj, "reportState", f.as_ref())?;
    f.forget();

    let a = s.clone();
    let f = Closure::<dyn Fn() -> JsValue>::new(move || with(&a, |app| fab_rects_js(app.fab()).unwrap_or(JsValue::NULL)));
    set(&obj, "fabRects", f.as_ref())?;
    f.forget();

    let a = s.clone();
    let f = Closure::<dyn Fn() -> u32>::new(move || with(&a, |app| app.fab().coffee_clicks()));
    set(&obj, "coffeeClicks", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn() -> bool>::new(move || {
        with(&a, |app| app.fab_mut().open_coffee());
        c.request_repaint();
        true
    });
    set(&obj, "coffeeOpen", f.as_ref())?;
    f.forget();

    let (a, c) = (s.clone(), ctx.clone());
    let f = Closure::<dyn Fn() -> bool>::new(move || {
        with(&a, |app| app.replay_logo());
        c.request_repaint();
        true
    });
    set(&obj, "replayLogo", f.as_ref())?;
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

// ---- the report panel and the mug: what the browser has to do ---------------------------------

fn rect_js(r: egui::Rect) -> Result<JsValue, JsValue> {
    let o = Object::new();
    for (k, v) in [("x", r.min.x), ("y", r.min.y), ("w", r.width()), ("h", r.height())] {
        set(&o, k, &JsValue::from_f64(f64::from(v)))?;
    }
    Ok(o.into())
}

fn fab_rects_js(f: &fab::Fab) -> Result<JsValue, JsValue> {
    let r = f.rects();
    let o = Object::new();
    for (k, v) in [("main", r.main), ("list", r.list), ("mug", r.mug), ("panel", r.panel), ("invite", r.invite)] {
        let v = match v {
            Some(v) => rect_js(v)?,
            None => JsValue::NULL,
        };
        set(&o, k, &v)?;
    }
    Ok(o.into())
}

fn fab_state_js(f: &fab::Fab) -> Result<JsValue, JsValue> {
    let o = Object::new();
    set(&o, "open", &JsValue::from_bool(f.is_open()))?;
    set(&o, "view", &JsValue::from_str(f.view().name()))?;
    set(&o, "kind", &JsValue::from_str(f.kind()))?;
    set(&o, "busy", &JsValue::from_bool(f.busy()))?;
    set(&o, "thanks", &JsValue::from_bool(f.thanks()))?;
    set(&o, "error", &f.error().map_or(JsValue::NULL, JsValue::from_str))?;
    let (state, why, items) = match f.list() {
        fab::ListState::Idle => ("idle", None, None),
        fab::ListState::Loading => ("loading", None, None),
        fab::ListState::Failed(e) => ("failed", Some(e.as_str()), None),
        fab::ListState::Loaded(items) => ("loaded", None, Some(items)),
    };
    set(&o, "list", &JsValue::from_str(state))?;
    set(&o, "listError", &why.map_or(JsValue::NULL, JsValue::from_str))?;
    let arr = Array::new();
    for it in items.into_iter().flatten() {
        let i = Object::new();
        set(&i, "id", &JsValue::from_str(&it.id))?;
        set(&i, "kind", &JsValue::from_str(&it.kind))?;
        set(&i, "title", &JsValue::from_str(&it.title))?;
        set(&i, "status", &JsValue::from_str(&it.status))?;
        set(&i, "votes", &JsValue::from_f64(f64::from(it.votes)))?;
        set(&i, "voted", &JsValue::from_bool(f.has_voted(&it.id)))?;
        arr.push(&i);
    }
    set(&o, "items", &arr)?;
    Ok(o.into())
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

fn session_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.session_storage().ok().flatten()
}

fn read_voted() -> Vec<String> {
    local_storage()
        .and_then(|s| s.get_item(fab::VOTED_KEY).ok().flatten())
        .and_then(|raw| serde_json::from_str::<Vec<String>>(&raw).ok())
        .unwrap_or_default()
}

/// Tell the report panel what it needs from the page: the browser, the address, the votes this
/// browser cast, whether Buy Me a Coffee's widget is on the page and whether to invite once.
fn setup_fab(shared: &Shared) {
    let Some(window) = web_sys::window() else { return };
    let Ok(mut app) = shared.try_borrow_mut() else { return };
    let ua = window.navigator().user_agent().unwrap_or_default();
    let href = window.location().href().unwrap_or_default();
    let f = app.fab_mut();
    f.set_environment(&ua, &href);
    f.set_voted(read_voted());
    let widget = window.document().is_some_and(|d| d.query_selector("script[data-name=\"BMC-Widget\"]").ok().flatten().is_some());
    f.set_coffee_available(widget);
    // Once per browser session: a failing sessionStorage means no invitation.
    if let Some(s) = session_storage()
        && s.get_item(fab::INVITE_KEY).ok().flatten().is_none()
        && s.set_item(fab::INVITE_KEY, "1").is_ok()
    {
        f.arm_invite();
    }
}

fn now(ctx: &egui::Context) -> f64 {
    ctx.input(|i| i.time)
}

/// Carry out one request of the report panel and report how it ended.
async fn run_fab_request(s: Shared, ctx: egui::Context, r: Request) {
    match r {
        Request::Submit(body) => {
            let result = match fetch("POST", "/api/issue-reports", Some(&body)).await {
                Ok((status, text)) if (200..300).contains(&status) => {
                    let _ = text;
                    Ok(())
                }
                Ok((status, text)) => Err(fab::error_from(status, &text)),
                Err(e) => Err(e),
            };
            if let Ok(mut app) = s.try_borrow_mut() {
                app.fab_mut().submit_finished(result, now(&ctx));
            }
        }
        Request::LoadList => {
            let result = match fetch("GET", "/api/issue-reports", None).await {
                Ok((status, text)) if (200..300).contains(&status) => Ok(text),
                Ok((status, text)) => Err(fab::error_from(status, &text)),
                Err(e) => Err(e),
            };
            if let Ok(mut app) = s.try_borrow_mut() {
                app.fab_mut().list_finished(result);
            }
        }
        Request::Vote(id) => {
            let body = serde_json::json!({ "id": id }).to_string();
            let result = match fetch("POST", "/api/issue-reports/vote", Some(&body)).await {
                Ok((status, text)) if (200..300).contains(&status) => {
                    fab::parse_votes(&text).ok_or_else(|| "The answer was not understood.".to_string())
                }
                Ok((status, text)) => Err(fab::error_from(status, &text)),
                Err(e) => Err(e),
            };
            if let Ok(mut app) = s.try_borrow_mut() {
                app.fab_mut().vote_finished(&id, result);
            }
        }
        Request::RememberVote(id) => {
            let mut voted = read_voted();
            if !voted.contains(&id) {
                voted.push(id);
            }
            if let (Some(store), Ok(raw)) = (local_storage(), serde_json::to_string(&voted)) {
                let _ = store.set_item(fab::VOTED_KEY, &raw);
            }
        }
        Request::PickScreenshot => pick_screenshot(s.clone(), ctx.clone()),
        Request::OpenCoffee => open_coffee(s.clone(), ctx.clone()).await,
    }
    ctx.request_repaint();
}

/// A `fetch` that answers (status, body). The error is the browser's own message.
async fn fetch(method: &str, url: &str, body: Option<&str>) -> Result<(u16, String), String> {
    let window = web_sys::window().ok_or_else(|| "no window".to_string())?;
    let init = web_sys::RequestInit::new();
    init.set_method(method);
    let headers = web_sys::Headers::new().map_err(js_msg)?;
    if let Some(b) = body {
        headers.set("content-type", "application/json").map_err(js_msg)?;
        init.set_body(&JsValue::from_str(b));
    } else {
        headers.set("accept", "application/json").map_err(js_msg)?;
    }
    init.set_headers(&headers);
    let req = web_sys::Request::new_with_str_and_init(url, &init).map_err(js_msg)?;
    let resp: web_sys::Response =
        JsFuture::from(window.fetch_with_request(&req)).await.map_err(js_msg)?.dyn_into().map_err(|_| "no response".to_string())?;
    let status = resp.status();
    let text = match resp.text() {
        Ok(p) => JsFuture::from(p).await.ok().and_then(|v| v.as_string()).unwrap_or_default(),
        Err(_) => String::new(),
    };
    Ok((status, text))
}

fn js_msg(e: JsValue) -> String {
    js_sys::Reflect::get(&e, &JsValue::from_str("message")).ok().and_then(|m| m.as_string()).or_else(|| e.as_string()).unwrap_or_default()
}

/// Read an image file as a data URL and hand it to the panel.
fn read_image(shared: Shared, ctx: egui::Context, file: web_sys::File) {
    let name = file.name();
    let ty = file.type_();
    if !matches!(ty.as_str(), "image/png" | "image/jpeg" | "image/gif" | "image/webp") {
        if let Ok(mut app) = shared.try_borrow_mut() {
            app.fab_mut().screenshot_picked(&name, Err("Screenshots must be images (PNG, JPEG, GIF or WebP).".into()));
        }
        ctx.request_repaint();
        return;
    }
    let Ok(reader) = web_sys::FileReader::new() else { return };
    let (r2, s2, c2, n2) = (reader.clone(), shared.clone(), ctx.clone(), name.clone());
    let onload = Closure::once_into_js(move || {
        let url = r2.result().ok().and_then(|v| v.as_string()).ok_or_else(|| "That image could not be read.".to_string());
        if let Ok(mut app) = s2.try_borrow_mut() {
            app.fab_mut().screenshot_picked(&n2, url);
        }
        c2.request_repaint();
    });
    reader.set_onload(Some(onload.unchecked_ref()));
    let (s3, c3) = (shared, ctx);
    let onerror = Closure::once_into_js(move || {
        if let Ok(mut app) = s3.try_borrow_mut() {
            app.fab_mut().screenshot_picked("", Err("That image could not be read.".into()));
        }
        c3.request_repaint();
    });
    reader.set_onerror(Some(onerror.unchecked_ref()));
    let _ = reader.read_as_data_url(&file);
}

/// Open the browser's file picker for an image (a throwaway `<input type=file>`).
fn pick_screenshot(shared: Shared, ctx: egui::Context) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    let Ok(el) = document.create_element("input") else { return };
    let Ok(input) = el.dyn_into::<web_sys::HtmlInputElement>() else { return };
    input.set_type("file");
    input.set_accept("image/png,image/jpeg,image/gif,image/webp");
    let i2 = input.clone();
    let onchange = Closure::once_into_js(move || {
        if let Some(file) = i2.files().and_then(|l| l.get(0)) {
            read_image(shared, ctx, file);
        }
    });
    input.set_onchange(Some(onchange.unchecked_ref()));
    input.click();
}

/// An image pasted while the report panel is open is attached to the report.
fn install_paste(shared: Shared, ctx: egui::Context) {
    let Some(window) = web_sys::window() else { return };
    let cb = Closure::<dyn FnMut(web_sys::ClipboardEvent)>::new(move |e: web_sys::ClipboardEvent| {
        let open = shared.try_borrow().map(|a| a.fab().is_open() && a.fab().view() == View::Form).unwrap_or(false);
        if !open {
            return;
        }
        let Some(items) = e.clipboard_data().map(|d| d.items()) else { return };
        for k in 0..items.length() {
            let Some(item) = items.get(k) else { continue };
            if item.kind() == "file"
                && item.type_().starts_with("image/")
                && let Ok(Some(file)) = item.get_as_file()
            {
                e.prevent_default();
                read_image(shared.clone(), ctx.clone(), file);
                return;
            }
        }
    });
    let _ = window.add_event_listener_with_callback("paste", cb.as_ref().unchecked_ref());
    cb.forget();
}

async fn sleep_ms(ms: i32) {
    let p = Promise::new(&mut |resolve, _| {
        if let Some(w) = web_sys::window() {
            let _ = w.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
        }
    });
    let _ = JsFuture::from(p).await;
}

/// Click Buy Me a Coffee's own button. Its loader is async, so a fast click waits for the chip
/// (up to 5 s; with no network nothing happens). BMC sets the panel's height from the window on
/// every open, so the cap is re-applied for a few frames.
async fn open_coffee(shared: Shared, ctx: egui::Context) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return };
    for _ in 0..50 {
        if let Some(chip) = document.get_element_by_id("bmc-wbtn").and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()) {
            chip.click();
            if let Ok(mut app) = shared.try_borrow_mut() {
                app.fab_mut().coffee_clicked();
            }
            ctx.request_repaint();
            for _ in 0..10 {
                cap_coffee_panel(&document);
                sleep_ms(16).await;
            }
            return;
        }
        sleep_ms(100).await;
    }
}

/// The room left below the mug, not a share of the window: the panel hangs from 132 px.
fn cap_coffee_panel(document: &web_sys::Document) {
    let Some(frame) = document.get_element_by_id("bmc-iframe").and_then(|e| e.dyn_into::<web_sys::HtmlElement>().ok()) else { return };
    let inner = web_sys::window().and_then(|w| w.inner_height().ok()).and_then(|v| v.as_f64()).unwrap_or(800.0);
    let cap = (inner - 156.0).clamp(240.0, 560.0);
    let _ = frame.style().set_property("height", &format!("{cap}px"));
}
