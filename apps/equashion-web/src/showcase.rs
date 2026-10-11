//! The page: one centred column with the equation editor and its symbol palette, a Copy row,
//! a paste-in field, examples and a short typing guide. Plain egui, so the native tests drive
//! it with egui_kittest; the browser shell (`web.rs`) carries out the clipboard requests.

use std::sync::Arc;

use egui::{Align, Color32, FontId, Frame, Layout, Margin, RichText, Sense, Stroke, StrokeKind, Theme, Ui, pos2, vec2};
use equashion_egui::EquationEditor;
use equashion_layout::FontDb;
use equashion_math::to_latex;

use crate::corpus::{self, Row};
use crate::export::{self, CopyKind, ImportError, Payload};
use crate::palette::{self, Item, Palette as SymbolPalette};
use crate::report::{self, DESC_MAX, Issue, Kind, Shot, Status, Submission, TITLE_MAX};
use crate::selftest::{self, Report};
use crate::theme::{self, Palette, R_CONTROL, R_PANEL, S4, S8, S12, S16, S24, S32, T_BODY, T_CAPTION, T_H2, T_MONO, ThemePref};

/// The one-line description next to the wordmark.
pub const TAGLINE: &str = "A pure-Rust equation editor";

/// The footer credit. The word MathLive links to [`MATHLIVE_URL`].
pub const CREDIT: &str = "Typing behaviour is modelled on MathLive. Independent Rust implementation, not affiliated.";
pub const MATHLIVE_URL: &str = "https://cortexjs.io/mathlive";

/// The examples under "Try it": (corpus id, plain name).
pub const EXAMPLES: &[(&str, &str)] = &[
    ("quadratic", "Quadratic formula"),
    ("euler", "Euler's identity"),
    ("sum_display", "Sum"),
    ("int_display", "Integral"),
    ("lim_display", "Limit"),
    ("pmatrix", "Matrix"),
    ("cases", "Piecewise function"),
    ("binom", "Binomial"),
    ("maxwell_faraday", "Maxwell's equation"),
    ("func", "Trig identity"),
];

/// The "How to type" guide: (what to type, what it does). Every line is checked against the
/// editor in `tests/showcase.rs`.
pub const HOW_TO_TYPE: &[(&str, &str)] = &[
    ("x^2", "Superscript. Typing + or the Right arrow leaves it."),
    ("x_1", "Subscript."),
    ("a/b", "Fraction. The Right arrow leaves the denominator."),
    ("sqrt", "Square root."),
    ("sum_(i=1)^n", "Sum with limits. int and prod work the same way."),
    ("alpha  \\alpha  pi", "Greek letters by name, with or without a backslash."),
    ("->", "Arrow."),
    ("Arrow keys, Tab", "Move between places."),
    ("Backspace", "Delete to the left."),
    ("Ctrl+Z, Ctrl+Y", "Undo and redo (Cmd on a Mac)."),
    ("Esc twice", "Go back to the equation last loaded."),
    ("Ctrl+V", "Paste LaTeX, MathML, OMML or plain text at the caret."),
    ("Palette", "Matrices, cases, brackets and accents."),
];

/// The line under the Copy row: where each format pastes. Only what was checked is claimed (see
/// the README); Word is named with the paste route to try, not as verified.
pub const COPY_GUIDE: &str = "LaTeX goes into Overleaf or Markdown math, MathML pastes as an equation in ONLYOFFICE (in Word, try Paste Special > Unformatted Text), and the image pastes anywhere.";

/// The disclosure that opens the report form.
pub const REPORT_LABEL: &str = "Report an issue";

/// The disclosure that opens the paste-in field.
pub const PASTE_LABEL: &str = "Paste LaTeX, MathML or OMML";

/// The page column's widest width, points.
const COLUMN_MAX: f32 = 900.0;
/// Height of the editing canvas, points.
const CANVAS_H: f32 = 170.0;
/// Height of the main buttons, points.
const BUTTON_H: f32 = 36.0;
/// How long "Copied" stays, seconds.
const CONFIRM_SECS: f64 = 2.0;

/// A clipboard write or download for the browser shell to carry out.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyRequest {
    pub id: u64,
    pub payload: Payload,
}

/// How a request ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Copied,
    Downloaded,
    /// The browser refused, or has no clipboard API.
    Blocked,
}

/// The state of the last Copy action.
#[derive(Clone, Debug)]
struct CopyStatus {
    id: u64,
    kind: CopyKind,
    outcome: Option<Outcome>,
    /// When the outcome was first shown (page time).
    shown_at: Option<f64>,
    /// The text to copy by hand when the clipboard was blocked.
    fallback: Option<String>,
    /// The fallback field still has to be focused and selected.
    select_fallback: bool,
}

/// Where the report form is.
#[derive(Clone, Debug, PartialEq, Eq)]
enum ReportStatus {
    Idle,
    Sending,
    Sent,
    Failed(String),
}

/// The "Report an issue" form and what is waiting for the browser shell.
struct ReportForm {
    open: bool,
    kind: Kind,
    title: String,
    description: String,
    shot: Option<Shot>,
    /// The file picker was asked for and has not answered.
    picking: bool,
    /// The shell has to open the file picker.
    want_pick: bool,
    /// A message about the form or the screenshot, in words.
    note: Option<String>,
    status: ReportStatus,
    outbox: Vec<Submission>,
    /// The browser's user agent and the page's address, set by the shell.
    browser: String,
    page_url: String,
    /// The public list of reported issues.
    issues: Vec<Issue>,
    /// The list was asked for at least once.
    listed: bool,
    loading: bool,
    /// The shell has to fetch the list.
    want_list: bool,
    list_error: Option<String>,
    /// Votes for the shell to post, and the ones in flight.
    vote_out: Vec<String>,
    voting: Vec<String>,
    vote_error: Option<String>,
}

impl ReportForm {
    fn new() -> Self {
        ReportForm {
            open: false,
            kind: Kind::Bug,
            title: String::new(),
            description: String::new(),
            shot: None,
            picking: false,
            want_pick: false,
            note: None,
            status: ReportStatus::Idle,
            outbox: Vec::new(),
            browser: String::new(),
            page_url: String::new(),
            issues: Vec::new(),
            listed: false,
            loading: false,
            want_list: false,
            list_error: None,
            vote_out: Vec::new(),
            voting: Vec::new(),
            vote_error: None,
        }
    }

    /// Open or close the form; the first time it opens, the list is asked for.
    fn set_open(&mut self, on: bool) {
        self.open = on;
        if on && !self.listed {
            self.refresh();
        }
    }

    /// Ask the shell for the list again.
    fn refresh(&mut self) {
        self.listed = true;
        self.loading = true;
        self.want_list = true;
        self.list_error = None;
    }

    /// Back to a clean slate for the next report (the open state stays).
    fn clear(&mut self) {
        self.kind = Kind::Bug;
        self.title.clear();
        self.description.clear();
        self.shot = None;
        self.note = None;
    }
}

/// The whole page.
pub struct Showcase {
    pub editor: EquationEditor,
    fonts: Arc<FontDb>,
    rows: Vec<Row>,
    symbols: SymbolPalette,
    report: Option<Report>,
    theme: ThemePref,
    reduced_motion: bool,
    /// Fonts and styles must be (re)installed on the context.
    style_dirty: bool,
    /// The Geist fonts are registered (they take effect from the frame after `set_fonts`).
    fonts_ready: bool,
    next_id: u64,
    requests: Vec<CopyRequest>,
    status: Option<CopyStatus>,
    import_open: bool,
    import_text: String,
    /// The last paste or import that was refused, in words.
    import_error: Option<&'static str>,
    howto_open: bool,
    about_open: bool,
    issue: ReportForm,
    /// The second Copy row (OMML, text, SVG) is shown.
    more_copy: bool,
    /// The format the big Copy button writes.
    copy_kind: CopyKind,
    /// Where each Copy button was drawn last frame (for the browser test's real clicks).
    copy_rects: Vec<(CopyKind, egui::Rect)>,
}

impl Default for Showcase {
    fn default() -> Self {
        Self::new()
    }
}

impl Showcase {
    pub fn new() -> Self {
        let editor = EquationEditor::with_linear(r"x=\frac{-b\pm\sqrt{b^2-4ac}}{2a}")
            .id_salt("showcase-editor")
            .show_palette(false)
            .show_linear_field(false)
            .canvas_height(CANVAS_H)
            .font_size(30.0);
        Showcase {
            editor,
            fonts: equashion_egui::bundled_fonts(),
            rows: corpus::rows(),
            symbols: SymbolPalette::default(),
            report: None,
            theme: ThemePref::System,
            reduced_motion: false,
            style_dirty: true,
            fonts_ready: false,
            next_id: 1,
            requests: Vec::new(),
            status: None,
            import_open: false,
            import_text: String::new(),
            import_error: None,
            howto_open: false,
            about_open: false,
            issue: ReportForm::new(),
            more_copy: false,
            copy_kind: CopyKind::Latex,
            copy_rects: Vec::new(),
        }
    }

    /// The page has one view; the `activeTab` hook reports it as `editor`.
    pub fn active_tab(&self) -> &'static str {
        "editor"
    }

    /// The theme preference (System follows `prefers-color-scheme`).
    pub fn theme_pref(&self) -> ThemePref {
        self.theme
    }

    pub fn set_theme_pref(&mut self, t: ThemePref) {
        if self.theme != t {
            self.theme = t;
            self.style_dirty = true;
        }
    }

    /// The theme in effect on `ctx` once the preference is applied.
    pub fn effective_theme(&self, ctx: &egui::Context) -> Theme {
        match self.theme {
            ThemePref::Light => Theme::Light,
            ThemePref::Dark => Theme::Dark,
            ThemePref::System => ctx.system_theme().unwrap_or(Theme::Light),
        }
    }

    /// Honour `prefers-reduced-motion`: no egui animations, no caret blink.
    pub fn set_reduced_motion(&mut self, on: bool) {
        self.reduced_motion = on;
        self.editor.set_caret_blink(!on);
        self.style_dirty = true;
    }

    pub fn reduced_motion(&self) -> bool {
        self.reduced_motion
    }

    /// Rows of `corpus/equations.tsv` (the `galleryCount` hook).
    pub fn gallery_count(&self) -> usize {
        self.rows.len()
    }

    /// The equation as LaTeX.
    pub fn latex(&self) -> String {
        to_latex(self.editor.math())
    }

    /// Load LaTeX (or linear text), dropping a `$..$` or `\[..\]` wrapper.
    pub fn set_latex(&mut self, s: &str) {
        self.editor.set_linear(export::strip_math_delimiters(s));
        self.import_error = None;
    }

    /// The examples: (corpus id, name, linear text), in page order.
    pub fn examples(&self) -> Vec<(&'static str, &'static str, &'static str)> {
        EXAMPLES.iter().filter_map(|(id, name)| self.rows.iter().find(|r| r.id == *id).map(|r| (*id, *name, r.linear))).collect()
    }

    /// Load an example by corpus id or name (any case). False when there is no such example.
    pub fn load_example(&mut self, key: &str) -> bool {
        let k = key.trim().to_ascii_lowercase();
        let Some((_, _, linear)) = self.examples().into_iter().find(|(id, name, _)| *id == k || name.to_ascii_lowercase() == k) else {
            return false;
        };
        let display = self.rows.iter().find(|r| r.linear == linear).is_some_and(|r| r.display);
        let mut m = equashion_math::from_linear(linear);
        m.para = display;
        self.editor.set_math(m);
        self.import_error = None;
        true
    }

    /// Paste text at the caret (what Ctrl+V in the editor does): MathML, OMML, LaTeX or linear text,
    /// replacing the selection. A refused paste leaves the equation alone.
    pub fn paste(&mut self, text: &str) -> Result<(), ImportError> {
        let imported = match export::import(text) {
            Ok(i) => i,
            Err(e) => {
                self.import_error = Some(e.message());
                return Err(e);
            }
        };
        let blank = self.editor.editor().is_blank();
        let ed = self.editor.editor_mut();
        ed.insert_template(&imported.math);
        if blank {
            ed.math_mut().para = imported.math.para;
        }
        self.editor.sync_linear();
        self.import_error = None;
        Ok(())
    }

    /// Replace the whole equation with `text` (the paste-in field's Load).
    pub fn import(&mut self, text: &str) -> Result<(), ImportError> {
        match export::import(text) {
            Ok(i) => {
                self.editor.set_math(i.math);
                self.import_error = None;
                Ok(())
            }
            Err(e) => {
                self.import_error = Some(e.message());
                Err(e)
            }
        }
    }

    /// The sentence shown about unknown commands, or the last refused paste.
    pub fn notice(&self) -> Option<String> {
        self.import_error.map(str::to_string).or_else(|| export::unknown_sentence(&self.editor.unknown_commands()))
    }

    /// Build what a Copy button writes, without queueing it.
    pub fn payload(&self, kind: CopyKind) -> Payload {
        export::payload(kind, self.editor.math(), &self.fonts)
    }

    /// Press a Copy button: queue the request for the browser shell; returns its id.
    pub fn request_copy(&mut self, kind: CopyKind) -> u64 {
        let p = self.payload(kind);
        self.queue(p)
    }

    fn queue(&mut self, payload: Payload) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.status = Some(CopyStatus { id, kind: payload.kind, outcome: None, shown_at: None, fallback: None, select_fallback: false });
        self.requests.push(CopyRequest { id, payload });
        id
    }

    /// The requests queued since the last call (the browser shell carries them out).
    pub fn take_requests(&mut self) -> Vec<CopyRequest> {
        std::mem::take(&mut self.requests)
    }

    /// The browser shell reports how request `id` ended. A blocked copy shows its text in a
    /// field to copy by hand. `fallback` is the request's plain text.
    pub fn copy_finished(&mut self, id: u64, outcome: Outcome, fallback: Option<String>) {
        if let Some(s) = self.status.as_mut()
            && s.id == id
        {
            s.outcome = Some(outcome);
            s.shown_at = None;
            if outcome == Outcome::Blocked {
                s.select_fallback = fallback.is_some();
                s.fallback = fallback;
            }
        }
    }

    /// The last Copy action: (kind, state) with state `pending`, `copied`, `downloaded` or `blocked`.
    pub fn copy_status(&self) -> Option<(&'static str, &'static str)> {
        self.status.as_ref().map(|s| {
            let state = match s.outcome {
                None => "pending",
                Some(Outcome::Copied) => "copied",
                Some(Outcome::Downloaded) => "downloaded",
                Some(Outcome::Blocked) => "blocked",
            };
            (s.kind.name(), state)
        })
    }

    /// Where the button of `kind` was drawn last frame, in points.
    pub fn copy_button_rect(&self, kind: CopyKind) -> Option<egui::Rect> {
        self.copy_rects.iter().find(|(k, _)| *k == kind).map(|(_, r)| *r)
    }

    /// Run (or re-run) the corpus selftest (the `selftest` hook; not on the page).
    pub fn run_selftest(&mut self) -> &Report {
        self.report.insert(selftest::run(&self.fonts))
    }

    pub fn report(&self) -> Option<&Report> {
        self.report.as_ref()
    }

    pub fn about_open(&self) -> bool {
        self.about_open
    }

    pub fn set_about_open(&mut self, on: bool) {
        self.about_open = on;
    }

    /// Select a palette tab by title; false when there is none.
    pub fn set_palette_tab(&mut self, title: &str) -> bool {
        self.symbols.select_tab(title)
    }

    /// The titles of the palette tabs, in order.
    pub fn palette_tabs(&self) -> Vec<&'static str> {
        self.symbols.tab_titles()
    }

    /// Open or close the paste-in, "How to type" and "Report an issue" disclosures.
    pub fn set_disclosures(&mut self, import: bool, howto: bool, report: bool) {
        self.import_open = import;
        self.howto_open = howto;
        self.issue.set_open(report);
    }

    /// Fill in the report form (the browser tests and the native tests do this).
    pub fn fill_report(&mut self, kind: Kind, title: &str, description: &str) {
        self.issue.kind = kind;
        self.issue.title = title.to_string();
        self.issue.description = description.to_string();
    }

    /// The page's address and the browser's user agent, sent with each report.
    pub fn set_report_context(&mut self, browser: &str, page_url: &str) {
        self.issue.browser = browser.to_string();
        self.issue.page_url = page_url.to_string();
    }

    /// Reports waiting to be sent (the shell posts them and answers with [`Self::report_finished`]).
    pub fn take_reports(&mut self) -> Vec<Submission> {
        std::mem::take(&mut self.issue.outbox)
    }

    /// The report form asked for the file picker (true once per click).
    pub fn take_shot_request(&mut self) -> bool {
        std::mem::take(&mut self.issue.want_pick)
    }

    /// The file picker answered, or an image was pasted.
    pub fn shot_loaded(&mut self, result: Result<Shot, String>) {
        self.issue.picking = false;
        match result {
            Ok(s) => match report::shot_problem(&s.data_url) {
                None => {
                    self.issue.shot = Some(s);
                    self.issue.note = None;
                }
                Some(why) => self.issue.note = Some(why.to_string()),
            },
            // An empty reason is a cancelled picker: nothing to say.
            Err(why) => self.issue.note = (!why.is_empty()).then_some(why),
        }
    }

    /// A report was sent (`Ok`) or could not be (`Err` with the reason).
    pub fn report_finished(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.issue.status = ReportStatus::Sent;
                self.issue.clear();
                self.issue.refresh();
            }
            Err(why) => self.issue.status = ReportStatus::Failed(why),
        }
    }

    /// `idle`, `sending`, `sent` or `failed`.
    pub fn report_status(&self) -> &'static str {
        match self.issue.status {
            ReportStatus::Idle => "idle",
            ReportStatus::Sending => "sending",
            ReportStatus::Sent => "sent",
            ReportStatus::Failed(_) => "failed",
        }
    }

    /// Send the form: checks it, then queues it for the shell. A form with no title or no
    /// description stays on the page with a note.
    pub fn submit_report(&mut self) {
        if self.issue.status == ReportStatus::Sending {
            return;
        }
        if let Some(why) = report::problem(&self.issue.title, &self.issue.description) {
            self.issue.note = Some(why.to_string());
            return;
        }
        let latex = self.latex();
        let r = &mut self.issue;
        r.note = None;
        r.status = ReportStatus::Sending;
        r.outbox.push(Submission {
            kind: r.kind,
            title: r.title.clone(),
            description: r.description.clone(),
            screenshot: r.shot.as_ref().map(|s| s.data_url.clone()),
            latex,
            browser: r.browser.clone(),
            page_url: r.page_url.clone(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        });
    }

    /// The report form asked for the list (true once per request).
    pub fn take_list_request(&mut self) -> bool {
        std::mem::take(&mut self.issue.want_list)
    }

    /// The list arrived, or could not be fetched.
    pub fn issues_loaded(&mut self, result: Result<Vec<Issue>, String>) {
        self.issue.loading = false;
        match result {
            Ok(mut items) => {
                items.sort_by_key(|i| std::cmp::Reverse(i.votes));
                self.issue.issues = items;
                self.issue.list_error = None;
            }
            Err(why) => self.issue.list_error = Some(why),
        }
    }

    /// The reported issues, most voted first.
    pub fn issues(&self) -> &[Issue] {
        &self.issue.issues
    }

    /// Vote for an issue (once: a voted or in-flight issue is ignored).
    pub fn vote(&mut self, id: &str) {
        let issue = &mut self.issue;
        if issue.voting.iter().any(|v| v == id) || issue.issues.iter().any(|i| i.id == id && i.voted) {
            return;
        }
        issue.vote_error = None;
        issue.voting.push(id.to_string());
        issue.vote_out.push(id.to_string());
    }

    /// Votes waiting to be posted.
    pub fn take_votes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.issue.vote_out)
    }

    /// A vote was counted (`Ok` with the new total) or refused.
    pub fn vote_finished(&mut self, id: &str, result: Result<u32, String>) {
        let issue = &mut self.issue;
        issue.voting.retain(|v| v != id);
        match result {
            Ok(n) => {
                if let Some(i) = issue.issues.iter_mut().find(|i| i.id == id) {
                    i.votes = n;
                    i.voted = true;
                }
                issue.issues.sort_by_key(|i| std::cmp::Reverse(i.votes));
            }
            Err(why) => issue.vote_error = Some(why),
        }
    }

    /// The report form is open.
    pub fn report_open(&self) -> bool {
        self.issue.open
    }

    /// The report form is waiting for an image to be pasted or picked.
    pub fn report_accepts_images(&self) -> bool {
        self.issue.open && self.issue.shot.is_none()
    }

    /// The format drop-down next to the Copy button is open.
    pub fn more_copy(&self) -> bool {
        self.more_copy
    }

    /// Open or close the format drop-down.
    pub fn set_more_copy(&mut self, on: bool) {
        self.more_copy = on;
    }

    /// The format the big Copy button writes.
    pub fn copy_kind(&self) -> CopyKind {
        self.copy_kind
    }

    /// Pick the format the big Copy button writes.
    pub fn set_copy_kind(&mut self, kind: CopyKind) {
        self.copy_kind = kind;
    }

    /// Install the fonts and the theme on `ctx` now. [`Showcase::ui`] does it on its first
    /// frame otherwise (and then skips drawing that frame, because new fonts only apply from
    /// the next one).
    pub fn install(&mut self, ctx: &egui::Context) {
        theme::install(ctx, self.theme, self.reduced_motion);
        self.style_dirty = false;
        self.fonts_ready = true;
    }

    /// Draw the page.
    pub fn ui(&mut self, ui: &mut Ui) {
        if !self.fonts_ready {
            self.install(ui.ctx());
            ui.ctx().request_discard("equashion: fonts installed");
            return;
        }
        if self.style_dirty {
            theme::install(ui.ctx(), self.theme, self.reduced_motion);
            self.style_dirty = false;
            ui.ctx().request_repaint();
        }
        let p = *theme::palette(ui);
        egui::Panel::top("equashion-header").exact_size(64.0).frame(Frame::new().fill(p.surface).stroke(Stroke::NONE)).show(ui, |ui| {
            let r = ui.max_rect();
            ui.painter().hline(r.x_range(), r.max.y - 0.5, Stroke::new(1.0, p.border));
            centred(ui, |ui| self.header(ui, &p));
        });
        egui::CentralPanel::default().frame(Frame::new().fill(p.bg)).show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt("equashion-page").auto_shrink([false, false]).show(ui, |ui| {
                centred(ui, |ui| self.page(ui, &p));
            });
        });
    }

    fn header(&mut self, ui: &mut Ui, p: &Palette) {
        let w = ui.available_width();
        ui.allocate_ui_with_layout(vec2(w, 64.0), Layout::left_to_right(Align::Center), |ui| {
            ui.label(RichText::new("equaSHion").font(theme::semibold(T_DISPLAY_WORDMARK)).color(p.strong));
            if w >= 560.0 {
                ui.add_space(S12);
                ui.label(RichText::new(TAGLINE).size(T_BODY).color(p.muted));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = S4;
                let mut chosen = None;
                // Right to left: the last segment first.
                for t in ThemePref::ALL.into_iter().rev() {
                    let hint = match t {
                        ThemePref::System => "Follow the system light or dark setting",
                        ThemePref::Light => "Always use the light theme",
                        ThemePref::Dark => "Always use the dark theme",
                    };
                    if segment(ui, p, self.theme == t, t.title(), hint).clicked() {
                        chosen = Some(t);
                    }
                }
                if let Some(t) = chosen {
                    self.set_theme_pref(t);
                }
            });
        });
    }

    fn page(&mut self, ui: &mut Ui, p: &Palette) {
        ui.add_space(S32);
        panel(ui, p, |ui| self.editor_card(ui, p));
        ui.add_space(S32);

        // Paste in.
        if palette::disclosure(ui, p, PASTE_LABEL, self.import_open).clicked() {
            self.import_open = !self.import_open;
        }
        if self.import_open {
            ui.add_space(S8);
            self.import_panel(ui, p);
        }
        ui.add_space(S32);

        // Try it.
        ui.label(RichText::new("Try it").font(theme::semibold(T_H2)).color(p.strong));
        ui.add_space(S12);
        let mut load = None;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(S8, S8);
            for (id, name, linear) in self.examples() {
                let b = egui::Button::new(RichText::new(name).size(T_BODY)).min_size(vec2(0.0, 32.0)).wrap_mode(egui::TextWrapMode::Extend);
                if ui.add(b).on_hover_text(linear).clicked() {
                    load = Some(id);
                }
            }
        });
        if let Some(id) = load {
            self.load_example(id);
            self.focus_editor(ui);
        }
        ui.add_space(S16);
        if palette::disclosure(ui, p, "How to type", self.howto_open).clicked() {
            self.howto_open = !self.howto_open;
        }
        if self.howto_open {
            ui.add_space(S8);
            how_to_type(ui, p);
        }
        ui.add_space(S16);
        if palette::disclosure(ui, p, REPORT_LABEL, self.issue.open).clicked() {
            self.issue.set_open(!self.issue.open);
        }
        if self.issue.open {
            ui.add_space(S8);
            panel(ui, p, |ui| self.report_panel(ui, p));
        }
        ui.add_space(S32 + S16);
        self.footer(ui, p);
        ui.add_space(S32);
    }

    fn focus_editor(&self, ui: &Ui) {
        let id = self.editor.canvas_id();
        ui.memory_mut(|m| m.request_focus(id));
    }

    fn editor_card(&mut self, ui: &mut Ui, p: &Palette) {
        if let Some(item) = self.symbols.ui(ui, &self.fonts, p) {
            let ed = self.editor.editor_mut();
            match item {
                Item::Structure { math, .. } => ed.insert_template(&math),
                Item::Symbol(c) => ed.insert_symbol(c),
            }
            self.editor.sync_linear();
            self.import_error = None;
            self.focus_editor(ui);
        }
        ui.add_space(S12);

        // Ctrl+V in the canvas: read the text as MathML, OMML, LaTeX or linear text, not as keystrokes.
        let canvas = self.editor.canvas_id();
        if ui.memory(|m| m.has_focus(canvas)) {
            let pasted: Vec<String> = ui.input_mut(|i| {
                let mut out = Vec::new();
                i.events.retain(|e| match e {
                    egui::Event::Paste(t) => {
                        out.push(t.clone());
                        false
                    }
                    _ => true,
                });
                out
            });
            for t in pasted {
                let _ = self.paste(&t);
            }
        }
        let r = self.editor.show(ui);
        if r.cancelled {
            self.editor.revert();
        }
        if r.changed {
            self.import_error = None;
        }
        if let Some(n) = self.notice() {
            ui.add_space(S4);
            ui.add(egui::Label::new(RichText::new(n).size(T_BODY).color(p.error)).wrap());
        }
        ui.add_space(S12);
        self.copy_row(ui, p);
        ui.add_space(S12);
        // What Copy LaTeX copies, selectable.
        let w = ui.available_width();
        ui.allocate_ui_with_layout(vec2(w, BUTTON_H), Layout::left_to_right(Align::Center), |ui| {
            ui.label(RichText::new("LaTeX").size(T_BODY).color(p.muted));
            ui.add_space(S4);
            let latex = self.latex();
            let mut t = latex.as_str();
            ui.add(
                egui::TextEdit::singleline(&mut t)
                    .id_salt("equashion-latex")
                    .font(FontId::monospace(T_MONO + 1.0))
                    .desired_width(f32::INFINITY)
                    .margin(Margin::symmetric(S8 as i8, S8 as i8)),
            )
            .on_hover_text("This is what Copy LaTeX copies");
        });
        self.fallback(ui, p);
    }

    fn copy_row(&mut self, ui: &mut Ui, p: &Palette) {
        let now = ui.input(|i| i.time);
        let mut pressed = None;
        let mut chosen = None;
        let mut toggle_menu = false;
        self.copy_rects.clear();
        let kind = self.copy_kind;
        // One big button with its format drop-down attached, then the inline confirmation.
        // The inline confirmation, for two seconds, replaces the label on the button.
        let mut label = kind.title();
        if let Some(s) = self.status.as_mut()
            && let Some(o) = s.outcome
            && o != Outcome::Blocked
        {
            let at = *s.shown_at.get_or_insert(now);
            if now - at < CONFIRM_SECS {
                label = if o == Outcome::Downloaded { "Downloaded" } else { "Copied" };
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
            } else {
                self.status = None;
            }
        }
        // One full-width button with its format drop-down built in.
        let (main, chevron) = split_button(ui, p, label, self.more_copy);
        let main = main.on_hover_text(kind.hint());
        let chevron = chevron.on_hover_text("Choose the format to copy");
        let row_left = main.rect.left();
        self.copy_rects.push((kind, main.rect));
        if main.clicked() {
            pressed = Some(kind);
        }
        if chevron.clicked() {
            toggle_menu = true;
        }
        if self.more_copy {
            // Below the button; above it when the six rows would run off the bottom of the window.
            let below = ui.ctx().content_rect().max.y - chevron.rect.bottom() >= MENU_H;
            let (anchor, pivot) = if below {
                (pos2(row_left, chevron.rect.bottom() + S4), egui::Align2::LEFT_TOP)
            } else {
                (pos2(row_left, chevron.rect.top() - S4), egui::Align2::LEFT_BOTTOM)
            };
            let area = egui::Area::new(egui::Id::new("equashion-copy-menu")).order(egui::Order::Foreground).pivot(pivot).fixed_pos(anchor).show(
                ui.ctx(),
                |ui| {
                    Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_min_width((chevron.rect.right() - row_left - 2.0 * S8).max(230.0));
                        ui.spacing_mut().item_spacing = vec2(S4, S4);
                        for k in CopyKind::ALL {
                            let r = menu_row(ui, p, k.title(), k == kind).on_hover_text(k.hint());
                            self.copy_rects.push((k, r.rect));
                            if r.clicked() {
                                chosen = Some(k);
                            }
                        }
                    })
                },
            );
            let outside = ui.input(|i| i.pointer.any_click()) && !toggle_menu && !area.response.contains_pointer() && !chevron.contains_pointer();
            if outside || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.more_copy = false;
            }
        }
        ui.add_space(S8);
        ui.add(egui::Label::new(RichText::new(COPY_GUIDE).size(T_BODY).color(p.muted)).wrap());
        if toggle_menu {
            self.more_copy = !self.more_copy;
        }
        if let Some(k) = chosen {
            self.copy_kind = k;
            self.more_copy = false;
        }
        if let Some(kind) = pressed {
            self.request_copy(kind);
            self.focus_editor(ui);
        }
    }

    /// What to do when the clipboard was blocked: the text in a selected field, or for the
    /// image a PNG download.
    fn fallback(&mut self, ui: &mut Ui, p: &Palette) {
        let Some(s) = self.status.as_ref() else { return };
        if s.outcome != Some(Outcome::Blocked) {
            return;
        }
        let (kind, text, select) = (s.kind, s.fallback.clone(), s.select_fallback);
        ui.add_space(S12);
        let mut close = false;
        let mut download_png = false;
        Frame::new().fill(p.sunken).stroke(Stroke::new(1.0, p.border)).corner_radius(R_CONTROL).inner_margin(Margin::same(S12 as i8)).show(
            ui,
            |ui| {
                ui.set_width(ui.available_width());
                match &text {
                    Some(t) => {
                        ui.add(
                            egui::Label::new(
                                RichText::new("The browser did not allow copying. The text below is selected: press Ctrl+C (Cmd+C on a Mac).")
                                    .size(T_BODY)
                                    .color(p.text),
                            )
                            .wrap(),
                        );
                        ui.add_space(S8);
                        let mut v = t.as_str();
                        let out = egui::TextEdit::multiline(&mut v)
                            .id_salt("equashion-fallback")
                            .font(FontId::monospace(T_MONO))
                            .desired_width(f32::INFINITY)
                            .desired_rows(if matches!(kind, CopyKind::MathMl | CopyKind::Omml) { 6 } else { 2 })
                            .margin(Margin::same(S8 as i8))
                            .show(ui);
                        if select {
                            let mut st = out.state;
                            st.cursor.set_char_range(Some(egui::text::CCursorRange::two(
                                egui::text::CCursor::new(0),
                                egui::text::CCursor::new(t.chars().count()),
                            )));
                            st.store(ui.ctx(), out.response.id);
                            out.response.request_focus();
                        }
                    }
                    None => {
                        ui.add(
                            egui::Label::new(
                                RichText::new("The browser did not allow copying a picture. Download it as a PNG file instead.")
                                    .size(T_BODY)
                                    .color(p.text),
                            )
                            .wrap(),
                        );
                    }
                }
                ui.add_space(S8);
                ui.horizontal(|ui| {
                    if text.is_none() && main_button(ui, p, "Download PNG", true).clicked() {
                        download_png = true;
                    }
                    if main_button(ui, p, "Close", false).clicked() {
                        close = true;
                    }
                });
            },
        );
        if let Some(s) = self.status.as_mut() {
            s.select_fallback = false;
        }
        if download_png {
            let mut pl = self.payload(CopyKind::Image);
            pl.filename = Some("equation.png");
            self.queue(pl);
        } else if close {
            self.status = None;
            self.focus_editor(ui);
        }
    }

    /// The "Report an issue" form: kind, title, description, an optional screenshot and Send.
    fn report_panel(&mut self, ui: &mut Ui, p: &Palette) {
        let r = &mut self.issue;
        let sending = r.status == ReportStatus::Sending;
        ui.add(
            egui::Label::new(
                RichText::new(
                    "Something wrong or missing? Tell us. You do not need an account. The report includes the equation in the editor and your browser, so it can be reproduced.",
                )
                .size(T_BODY)
                .color(p.muted),
            )
            .wrap(),
        );
        ui.add_space(S12);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = S4;
            for k in Kind::ALL {
                if segment(ui, p, r.kind == k, k.label(), k.hint()).clicked() {
                    r.kind = k;
                }
            }
            ui.add_space(S8);
            ui.label(RichText::new(r.kind.hint()).size(T_BODY).color(p.muted));
        });
        ui.add_space(S8);
        ui.add(
            egui::TextEdit::singleline(&mut r.title)
                .id_salt("equashion-report-title")
                .font(FontId::proportional(T_BODY))
                .desired_width(f32::INFINITY)
                .char_limit(TITLE_MAX)
                .margin(Margin::same(S8 as i8))
                .hint_text(r.kind.title_hint()),
        );
        ui.add_space(S8);
        ui.add(
            egui::TextEdit::multiline(&mut r.description)
                .id_salt("equashion-report-description")
                .font(FontId::proportional(T_BODY))
                .desired_width(f32::INFINITY)
                .desired_rows(5)
                .char_limit(DESC_MAX)
                .margin(Margin::same(S8 as i8))
                .hint_text(r.kind.body_hint()),
        );
        ui.add_space(S8);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(S8, S8);
            match r.shot.clone() {
                Some(s) => {
                    ui.label(RichText::new(format!("Screenshot attached: {}", s.name)).size(T_BODY).color(p.text));
                    if main_button(ui, p, "Remove screenshot", false).clicked() {
                        r.shot = None;
                    }
                }
                None => {
                    let busy = r.picking;
                    let label = if busy { "Attaching..." } else { "Attach screenshot" };
                    if ui.add_enabled_ui(!busy, |ui| main_button(ui, p, label, false)).inner.clicked() {
                        r.picking = true;
                        r.want_pick = true;
                    }
                    ui.label(RichText::new("or paste one (Ctrl+V). Optional.").size(T_BODY).color(p.muted));
                }
            }
        });
        ui.add_space(S8);
        let mut send = false;
        ui.horizontal(|ui| {
            let label = if sending { "Sending..." } else { "Send report" };
            if ui.add_enabled_ui(!sending, |ui| main_button(ui, p, label, true)).inner.clicked() {
                send = true;
            }
            let (text, color) = match (&r.note, &r.status) {
                (Some(n), _) => (n.clone(), p.error),
                (None, ReportStatus::Sent) => ("Thank you! Your report has been sent.".to_string(), p.accent_text),
                (None, ReportStatus::Failed(why)) => (format!("Could not send the report: {why} Try again."), p.error),
                _ => (String::new(), p.muted),
            };
            if !text.is_empty() {
                ui.add(egui::Label::new(RichText::new(text).size(T_BODY).color(color)).wrap());
            }
        });
        if send {
            self.submit_report();
        }
        self.issue_list(ui, p);
    }

    /// "Reported so far": the public list with status and a Vote button on each.
    fn issue_list(&mut self, ui: &mut Ui, p: &Palette) {
        ui.add_space(S16);
        let y = ui.cursor().min.y;
        ui.painter().hline(ui.cursor().min.x..=ui.cursor().min.x + ui.available_width(), y, Stroke::new(1.0, p.border));
        ui.add_space(S12);
        ui.label(RichText::new("Reported so far").font(theme::semibold(T_BODY + 2.0)).color(p.strong));
        ui.add_space(S8);
        let r = &self.issue;
        let mut refresh = false;
        if r.loading {
            ui.label(RichText::new("Loading...").size(T_BODY).color(p.muted));
        } else if let Some(why) = &r.list_error {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("The list could not be loaded: {why}")).size(T_BODY).color(p.error));
                if ui.link(RichText::new("Try again").size(T_BODY).color(p.accent)).clicked() {
                    refresh = true;
                }
            });
        } else if r.issues.is_empty() {
            ui.label(RichText::new("Nothing has been reported yet.").size(T_BODY).color(p.muted));
        }
        if let Some(why) = &r.vote_error {
            ui.label(RichText::new(format!("Your vote was not counted: {why}")).size(T_BODY).color(p.error));
        }
        let mut voted = None;
        for i in r.issues.clone() {
            ui.add_space(S8);
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing = vec2(S12, S4);
                let pending = r.voting.contains(&i.id);
                let label = if i.voted { format!("Voted ({})", i.votes) } else { format!("Vote ({})", i.votes) };
                let b = ui.add_enabled_ui(!i.voted && !pending, |ui| main_button(ui, p, &label, !i.voted && !pending));
                if b.inner.on_hover_text("This matters to me too").clicked() {
                    voted = Some(i.id.clone());
                }
                ui.vertical(|ui| {
                    ui.add(egui::Label::new(RichText::new(&i.title).size(T_BODY).color(p.strong)).wrap());
                    let status = if i.status == Status::Fixed { p.accent_text } else { p.muted };
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = S8;
                        ui.label(RichText::new(i.kind.label()).size(T_CAPTION + 1.0).color(p.muted));
                        ui.label(RichText::new(i.status.label()).size(T_CAPTION + 1.0).color(status));
                        if let Some(url) = &i.screenshot_url {
                            ui.hyperlink_to(RichText::new("Screenshot").size(T_CAPTION + 1.0).color(p.accent), url);
                        }
                    });
                    let mut text: String = i.description.chars().take(240).collect();
                    if i.description.chars().count() > 240 {
                        text.push_str("...");
                    }
                    ui.add(egui::Label::new(RichText::new(text).size(T_BODY).color(p.text)).wrap());
                });
            });
        }
        if refresh {
            self.issue.refresh();
        }
        if let Some(id) = voted {
            self.vote(&id);
        }
    }

    fn import_panel(&mut self, ui: &mut Ui, p: &Palette) {
        ui.add(
            egui::Label::new(
                RichText::new(
                    "Paste LaTeX (with or without $ signs), a MathML math element, an m:oMath element from Word, or plain text like a/b. Load replaces the equation.",
                )
                .size(T_BODY)
                .color(p.muted),
            )
            .wrap(),
        );
        ui.add_space(S8);
        ui.add(
            egui::TextEdit::multiline(&mut self.import_text)
                .id_salt("equashion-import")
                .font(FontId::monospace(T_MONO))
                .desired_width(f32::INFINITY)
                .desired_rows(4)
                .margin(Margin::same(S8 as i8))
                .hint_text(r"\frac{a}{b}   or   <math>...</math>   or   <m:oMath>...</m:oMath>"),
        );
        ui.add_space(S8);
        if main_button(ui, p, "Load", true).clicked() {
            let t = self.import_text.clone();
            if self.import(&t).is_ok() {
                self.focus_editor(ui);
            }
        }
    }

    fn footer(&mut self, ui: &mut Ui, p: &Palette) {
        let w = ui.available_width();
        let y = ui.cursor().min.y;
        ui.painter().hline(ui.cursor().min.x..=ui.cursor().min.x + w, y, Stroke::new(1.0, p.border));
        ui.add_space(S16);
        let mut toggle = false;
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, S4);
            let small = |s: &str| RichText::new(s).size(T_CAPTION + 1.0).color(p.muted);
            let (before, after) = CREDIT.split_once("MathLive").unwrap_or((CREDIT, ""));
            ui.label(small(before));
            ui.hyperlink_to(RichText::new("MathLive").size(T_CAPTION + 1.0).color(p.accent), MATHLIVE_URL).on_hover_text(MATHLIVE_URL);
            ui.label(small(after));
        });
        ui.add_space(S4);
        if ui.link(RichText::new("About and licences").size(T_CAPTION + 1.0).color(p.accent)).clicked() {
            toggle = true;
        }
        if toggle {
            self.about_open = !self.about_open;
        }
        if self.about_open {
            ui.add_space(S16);
            let r = panel(ui, p, |ui| about(ui, p));
            if toggle {
                ui.scroll_to_rect(r.response.rect, Some(Align::Max));
            }
        }
    }
}

/// The wordmark size.
const T_DISPLAY_WORDMARK: f32 = 20.0;

/// Draw `add` in the page column: at most [`COLUMN_MAX`] wide, centred, with side gutters.
fn centred<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let full = ui.available_width();
    let gutter = if full >= 760.0 { S32 } else { S24 };
    let col = (full - 2.0 * gutter).clamp(200.0, COLUMN_MAX);
    let left = ((full - col) / 2.0).max(0.0).floor();
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_space(left);
        ui.allocate_ui_with_layout(vec2(col, ui.available_height()), Layout::top_down(Align::Min), |ui| {
            ui.spacing_mut().item_spacing = vec2(S8, S4);
            ui.set_width(col);
            add(ui)
        })
        .inner
    })
    .inner
}

/// A surface panel: panel radius, 1 px border, 24 px padding (16 px in a narrow column).
fn panel<R>(ui: &mut Ui, p: &Palette, add: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
    let pad = if ui.available_width() < 720.0 { S16 } else { S24 };
    Frame::new().fill(p.surface).stroke(Stroke::new(1.0, p.border)).corner_radius(R_PANEL).inner_margin(Margin::same(pad as i8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    })
}

/// A 36 px high button that never wraps its label; `primary` fills it with the accent.
fn main_button(ui: &mut Ui, p: &Palette, text: &str, primary: bool) -> egui::Response {
    ui.scope(|ui| {
        if primary {
            let w = &mut ui.visuals_mut().widgets;
            for (v, fill) in [(&mut w.inactive, p.accent), (&mut w.hovered, p.accent_hover), (&mut w.active, p.accent_pressed)] {
                v.bg_fill = fill;
                v.weak_bg_fill = fill;
                v.bg_stroke = Stroke::new(1.0, fill);
                v.fg_stroke = Stroke::new(1.0, p.on_accent);
            }
            w.active.bg_stroke = Stroke::new(2.0, p.strong);
        }
        ui.spacing_mut().button_padding = vec2(S16, S8);
        ui.add(egui::Button::new(RichText::new(text).size(T_BODY)).min_size(vec2(0.0, BUTTON_H)).wrap_mode(egui::TextWrapMode::Extend))
    })
    .inner
}

/// Room the format menu needs below its button, points (six rows, gaps and frame).
const MENU_H: f32 = 6.0 * (BUTTON_H + S4) + 2.0 * S8;

/// The Copy button and its drop-down as one accent pill: the label on the left, a divider and
/// an arrow on the right. Returns the label's response and the arrow's.
fn split_button(ui: &mut Ui, p: &Palette, label: &str, open: bool) -> (egui::Response, egui::Response) {
    let h = BUTTON_H + 4.0;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover());
    let arrow_w = BUTTON_H + 8.0;
    let main_rect = egui::Rect::from_min_max(rect.min, pos2(rect.max.x - arrow_w, rect.max.y));
    let arrow_rect = egui::Rect::from_min_max(pos2(rect.max.x - arrow_w, rect.min.y), rect.max);
    let id = ui.id().with("equashion-copy-split");
    let main = ui.interact(main_rect, id.with("main"), Sense::click());
    let arrow = ui.interact(arrow_rect, id.with("arrow"), Sense::click());
    main.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    arrow.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Choose format"));
    if ui.is_rect_visible(rect) {
        let fill = |r: &egui::Response| {
            if r.is_pointer_button_down_on() {
                p.accent_pressed
            } else if r.hovered() || r.has_focus() {
                p.accent_hover
            } else {
                p.accent
            }
        };
        let r = R_CONTROL;
        let painter = ui.painter();
        painter.rect_filled(main_rect, egui::CornerRadius { nw: r, sw: r, ne: 0, se: 0 }, fill(&main));
        painter.rect_filled(arrow_rect, egui::CornerRadius { nw: 0, sw: 0, ne: r, se: r }, fill(&arrow));
        // A hairline between the two halves.
        let x = arrow_rect.min.x;
        painter.line_segment([pos2(x, rect.min.y + 8.0), pos2(x, rect.max.y - 8.0)], Stroke::new(1.0, p.on_accent.gamma_multiply(0.45)));
        let galley = painter.layout_no_wrap(label.to_string(), FontId::proportional(T_BODY + 1.0), p.on_accent);
        let pos = main_rect.center() - galley.size() / 2.0;
        painter.galley(pos2(pos.x.round(), pos.y.round()), galley, p.on_accent);
        let c = arrow_rect.center();
        let (w, hh) = (6.0, 3.5);
        let pts = if open {
            vec![pos2(c.x - w, c.y + hh), pos2(c.x + w, c.y + hh), pos2(c.x, c.y - hh)]
        } else {
            vec![pos2(c.x - w, c.y - hh), pos2(c.x + w, c.y - hh), pos2(c.x, c.y + hh)]
        };
        painter.add(egui::Shape::convex_polygon(pts, p.on_accent, Stroke::NONE));
        for resp in [&main, &arrow] {
            if resp.has_focus() {
                painter.rect_stroke(resp.rect, R_CONTROL, Stroke::new(2.0, p.strong), StrokeKind::Inside);
            }
        }
    }
    (main, arrow)
}

/// One row of the format menu: no border, a tinted background under the pointer, and the
/// picked format in the accent colour.
fn menu_row(ui: &mut Ui, p: &Palette, text: &str, selected: bool) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), BUTTON_H), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, text));
    if ui.is_rect_visible(rect) {
        let (fill, fg) = if resp.is_pointer_button_down_on() {
            (p.control_active, p.strong)
        } else if resp.hovered() || resp.has_focus() {
            (p.control_hover, p.strong)
        } else if selected {
            (Color32::TRANSPARENT, p.accent_text)
        } else {
            (Color32::TRANSPARENT, p.text)
        };
        let painter = ui.painter();
        painter.rect_filled(rect, R_CONTROL, fill);
        let galley = painter.layout_no_wrap(text.to_string(), FontId::proportional(T_BODY), fg);
        let y = rect.center().y - galley.size().y / 2.0;
        painter.galley(pos2((rect.min.x + S12).round(), y.round()), galley, fg);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A theme segment: text on a tinted background when selected, a hover fill, and the accent
/// focus ring when it has the keyboard.
fn segment(ui: &mut Ui, p: &Palette, selected: bool, text: &str, hint: &str) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_string(), FontId::proportional(T_BODY), p.text);
    let size = vec2(galley.size().x + 2.0 * S12, 32.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, text));
    if ui.is_rect_visible(rect) {
        let pressed = resp.is_pointer_button_down_on();
        let (fill, fg) = if selected {
            (p.accent_soft, p.accent_text)
        } else if pressed {
            (p.control_active, p.strong)
        } else if resp.hovered() {
            (p.control_hover, p.strong)
        } else {
            (Color32::TRANSPARENT, p.text)
        };
        let painter = ui.painter();
        painter.rect_filled(rect, R_CONTROL, fill);
        if resp.has_focus() {
            painter.rect_stroke(rect, R_CONTROL, Stroke::new(2.0, p.accent), StrokeKind::Inside);
        }
        let pos = rect.center() - galley.size() / 2.0;
        painter.galley_with_override_text_color(pos2(pos.x.round(), pos.y.round()), galley, fg);
    }
    resp.on_hover_text(hint)
}

fn how_to_type(ui: &mut Ui, p: &Palette) {
    Frame::new().fill(p.surface).stroke(Stroke::new(1.0, p.border)).corner_radius(R_PANEL).inner_margin(Margin::same(S16 as i8)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        egui::Grid::new("equashion-howto").num_columns(2).spacing(vec2(S24, S8)).show(ui, |ui| {
            for (keys, what) in HOW_TO_TYPE {
                ui.label(RichText::new(*keys).font(FontId::monospace(T_MONO + 1.0)).color(p.strong));
                ui.label(RichText::new(*what).size(T_BODY).color(p.text));
                ui.end_row();
            }
        });
    });
}

fn about(ui: &mut Ui, p: &Palette) {
    let body = |ui: &mut Ui, s: &str| {
        ui.add(egui::Label::new(RichText::new(s).size(T_BODY).color(p.text)).wrap());
    };
    let section = |ui: &mut Ui, title: &str, first: bool| {
        if !first {
            ui.add_space(S16);
        }
        ui.label(RichText::new(title).font(theme::semibold(T_H2)).color(p.strong));
        ui.add_space(S4);
    };
    section(ui, "What it is", true);
    body(
        ui,
        "An equation editor written in Rust: you type the formatted equation directly, then copy it as LaTeX, MathML, an Office equation (OMML), text or a picture. This page is the WebAssembly build, drawn with WebGL2.",
    );
    section(ui, "Credits", false);
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let t = |s: &str| RichText::new(s).size(T_BODY).color(p.text);
        ui.label(t("Typing behaviour is modelled on "));
        ui.hyperlink_to(RichText::new("MathLive").size(T_BODY).color(p.accent), MATHLIVE_URL);
        ui.label(t("; equaSHion is an independent Rust implementation, not affiliated with MathLive, and uses none of its code. The editing engine comes from the equation editors of "));
        ui.hyperlink_to(RichText::new("WordCraft").size(T_BODY).color(p.accent), "https://github.com/storytold/wordcraft");
        ui.label(t(" and "));
        ui.hyperlink_to(RichText::new("DeckCraft").size(T_BODY).color(p.accent), "https://github.com/storytold/deckcraft");
        ui.label(t(" (PROVENANCE.md lists what was taken). Fonts: STIX Two Math, Source Serif 4, Inter, Geist and Geist Mono, all under the SIL Open Font License 1.1."));
    });
    section(ui, "Licence", false);
    body(ui, "MIT OR Apache-2.0, at your option.");
    section(ui, "Known limits", false);
    for item in [
        "Copy MathML and Copy image depend on what your browser lets a page put on the clipboard; see the README.",
        "LibreOffice Writer pastes MathML as plain text; in LibreOffice Math use Tools > Import MathML from Clipboard. LibreOffice drops accents such as a hat.",
        "ONLYOFFICE shows the characters <, > and & in a pasted equation as &lt;, &gt; and &amp;.",
        "Typed or pasted \\langle, \\lfloor, \\lceil and \\Vert give plain characters; the Brackets group makes stretchy ones.",
        "Unknown backslash commands are kept as plain text.",
        "Only the bundled fonts are used. A character none of them has shows as a missing glyph.",
        "The page is one WebGL2 canvas, so screen readers and browser find do not see its text.",
    ] {
        ui.horizontal_top(|ui| {
            ui.label(RichText::new("-").size(T_BODY).color(p.muted));
            body(ui, item);
        });
    }
}
