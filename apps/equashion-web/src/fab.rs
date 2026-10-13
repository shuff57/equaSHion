//! The two floating buttons and the panels they open, drawn with egui: Report a Bug (and the
//! public list of reports, with votes) bottom right, and the Buy Me a Coffee mug top right.
//!
//! This is plain egui and knows nothing of the browser. Whatever needs the network, the
//! clipboard-like DOM or storage is a [`Request`] the browser shell (`web.rs`) carries out and
//! answers through the `*_finished` / `screenshot_picked` methods, the same way the Copy buttons
//! work. The API is `docs/issue-reports.md`: `POST /api/issue-reports` files a report,
//! `GET /api/issue-reports` lists them, `POST /api/issue-reports/vote` votes.
//!
//! Buy Me a Coffee's hosted widget is an iframe and cannot be drawn here: the mug is an egui
//! button that asks the shell to click the widget's own chip (`#bmc-wbtn`), which opens BMC's
//! payment panel. The widget script is the only DOM left on the page besides the canvas.

use std::collections::HashSet;

use egui::{
    Align, Align2, Area, Color32, Context, FontId, Frame, Id, Layout, Margin, Order, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};
use serde_json::{Value, json};

use crate::theme::{self, Palette, R_CONTROL, S4, S8, S12, T_CAPTION};

/// Screenshots larger than this many characters of data URL are refused (about 1 MB of image).
pub const MAX_SHOT_CHARS: usize = 1_400_000;
const MAX_SHOT_TEXT: &str = "1 MB";
pub const TITLE_MAX: usize = 100;
pub const DESC_MAX: usize = 2000;
/// The set of voted issue ids, in `localStorage`.
pub const VOTED_KEY: &str = "equashion.voted";
/// Set in `sessionStorage` once the invitation has been shown.
pub const INVITE_KEY: &str = "equashion.coffee-invited";

/// How long after the page loads the invitation appears, and how long it stays, seconds.
const INVITE_DELAY: f64 = 4.0;
const INVITE_SHOWN: f64 = 10.0;
/// How long "Thank you" stays before the panel closes, seconds.
const THANKS_SECS: f64 = 2.2;

/// Window margins and sizes, points (the report buttons of raSHio).
const MARGIN: f32 = 24.0;
const MARGIN_MID: f32 = 16.0;
const MAIN_BTN: f32 = 48.0;
const SUB_BTN: f32 = 40.0;
const SUB_GAP: f32 = 12.0;
const PANEL_W: f32 = 340.0;
const PANEL_BOTTOM: f32 = 136.0;
const MUG_TOP: f32 = 80.0;

/// A kind of report, with raSHio's labels, hints and colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KindInfo {
    pub value: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub color: Color32,
    pub ink: Color32,
    pub title_placeholder: &'static str,
    pub body_placeholder: &'static str,
}

pub const KINDS: [KindInfo; 3] = [
    KindInfo {
        value: "bug",
        label: "Bug",
        hint: "doesn't work",
        color: Color32::from_rgb(0xf8, 0x71, 0x71),
        ink: Color32::from_rgb(0x17, 0x17, 0x1a),
        title_placeholder: "Matrix cells lose the caret after Tab",
        body_placeholder: "What did you try, and what happened instead?",
    },
    KindInfo {
        value: "quirk",
        label: "Quirk",
        hint: "works, but picky",
        color: Color32::from_rgb(0xfb, 0xbf, 0x24),
        ink: Color32::from_rgb(0x17, 0x17, 0x1a),
        title_placeholder: "Typing sqrt needs a space before the argument",
        body_placeholder: "What feels off or picky about how this works?",
    },
    KindInfo {
        value: "enhancement",
        label: "Improve",
        hint: "wish it had",
        color: Color32::from_rgb(0x22, 0xc5, 0x5e),
        ink: Color32::from_rgb(0x17, 0x17, 0x1a),
        title_placeholder: "Let me export the equation as a Word file",
        body_placeholder: "What would you like to be able to do?",
    },
];

/// Which panel view is showing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Form,
    List,
}

impl View {
    pub fn name(self) -> &'static str {
        match self {
            View::Form => "form",
            View::List => "list",
        }
    }

    pub fn parse(s: &str) -> Option<View> {
        match s {
            "form" => Some(View::Form),
            "list" => Some(View::List),
            _ => None,
        }
    }
}

/// One entry of the public list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub votes: u32,
    pub screenshot_url: Option<String>,
}

/// The list's state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListState {
    /// Not asked for yet (or stale, after a report or a vote).
    Idle,
    Loading,
    Loaded(Vec<Issue>),
    Failed(String),
}

/// Work for the browser shell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Request {
    /// `POST /api/issue-reports` with this JSON body (the shell adds `browser` and `pageUrl`).
    Submit(String),
    /// `GET /api/issue-reports`.
    LoadList,
    /// `POST /api/issue-reports/vote {"id": ...}`.
    Vote(String),
    /// Remember a vote in `localStorage` ([`VOTED_KEY`]).
    RememberVote(String),
    /// Open the browser's image file picker.
    PickScreenshot,
    /// Click Buy Me a Coffee's own chip.
    OpenCoffee,
}

/// An attached screenshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shot {
    pub name: String,
    pub data_url: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Invite {
    /// Not offered this session (or the widget is missing).
    Off,
    /// Armed; the clock starts on the next frame.
    Armed,
    Waiting {
        show_at: f64,
    },
    Shown {
        until: f64,
    },
    Done,
}

/// Where each piece was drawn last frame, for click-outside tests and the browser's real clicks.
#[derive(Clone, Copy, Debug, Default)]
pub struct Rects {
    pub main: Option<Rect>,
    pub list: Option<Rect>,
    pub mug: Option<Rect>,
    pub panel: Option<Rect>,
    pub invite: Option<Rect>,
}

pub struct Fab {
    open: bool,
    view: View,
    kind: usize,
    title: String,
    description: String,
    shot: Option<Shot>,
    busy: bool,
    error: Option<String>,
    thanks_until: Option<f64>,
    list: ListState,
    voted: HashSet<String>,
    voting: HashSet<String>,
    vote_error: Option<(String, String)>,
    focus_title: bool,
    coffee_available: bool,
    coffee_clicks: u32,
    invite: Invite,
    requests: Vec<Request>,
    rects: Rects,
    browser: String,
    page_url: String,
    app_version: String,
}

impl Default for Fab {
    fn default() -> Self {
        Self::new()
    }
}

impl Fab {
    pub fn new() -> Self {
        Fab {
            open: false,
            view: View::Form,
            kind: 0,
            title: String::new(),
            description: String::new(),
            shot: None,
            busy: false,
            error: None,
            thanks_until: None,
            list: ListState::Idle,
            voted: HashSet::new(),
            voting: HashSet::new(),
            vote_error: None,
            focus_title: false,
            coffee_available: false,
            coffee_clicks: 0,
            invite: Invite::Off,
            requests: Vec::new(),
            rects: Rects::default(),
            browser: String::new(),
            page_url: String::new(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    // ---- state the shell and the hooks use ---------------------------------------------------

    /// What a report carries besides what was typed.
    pub fn set_environment(&mut self, browser: &str, page_url: &str) {
        self.browser = browser.to_string();
        self.page_url = page_url.to_string();
    }

    /// The issue ids this browser voted for (from `localStorage`).
    pub fn set_voted(&mut self, ids: impl IntoIterator<Item = String>) {
        self.voted = ids.into_iter().collect();
    }

    /// Buy Me a Coffee's loader is on the page: show the mug.
    pub fn set_coffee_available(&mut self, on: bool) {
        self.coffee_available = on;
    }

    /// Offer the invitation once: it appears a few seconds after the first frame.
    pub fn arm_invite(&mut self) {
        if self.coffee_available && self.invite == Invite::Off {
            self.invite = Invite::Armed;
        }
    }

    /// The shell found and clicked BMC's chip.
    pub fn coffee_clicked(&mut self) {
        self.coffee_clicks += 1;
    }

    pub fn coffee_clicks(&self) -> u32 {
        self.coffee_clicks
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn view(&self) -> View {
        self.view
    }

    pub fn kind(&self) -> &'static str {
        KINDS[self.kind].value
    }

    pub fn busy(&self) -> bool {
        self.busy
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub fn thanks(&self) -> bool {
        self.thanks_until.is_some()
    }

    pub fn list(&self) -> &ListState {
        &self.list
    }

    pub fn has_voted(&self, id: &str) -> bool {
        self.voted.contains(id)
    }

    pub fn invite_visible(&self) -> bool {
        matches!(self.invite, Invite::Shown { .. })
    }

    pub fn screenshot(&self) -> Option<&Shot> {
        self.shot.as_ref()
    }

    pub fn rects(&self) -> Rects {
        self.rects
    }

    pub fn take_requests(&mut self) -> Vec<Request> {
        std::mem::take(&mut self.requests)
    }

    /// Open the panel on `view`, or close it when it is already showing that view (what the
    /// buttons do).
    pub fn toggle(&mut self, view: View) {
        self.dismiss_invite();
        if self.open && self.view == view {
            self.close();
        } else {
            self.show_view(view);
            self.open = true;
        }
    }

    pub fn close(&mut self) {
        self.open = false;
        self.error = None;
    }

    fn show_view(&mut self, view: View) {
        self.view = view;
        self.error = None;
        if view == View::Form {
            self.focus_title = true;
        }
        if view == View::List && matches!(self.list, ListState::Idle | ListState::Failed(_)) {
            self.list = ListState::Loading;
            self.requests.push(Request::LoadList);
        }
    }

    pub fn set_kind(&mut self, value: &str) -> bool {
        match KINDS.iter().position(|k| k.value == value) {
            Some(i) => {
                self.kind = i;
                true
            }
            None => false,
        }
    }

    /// Fill the form (the `reportFill` hook). False for an unknown kind.
    pub fn fill(&mut self, kind: &str, title: &str, description: &str) -> bool {
        if !self.set_kind(kind) {
            return false;
        }
        self.title = title.chars().take(TITLE_MAX).collect();
        self.description = description.chars().take(DESC_MAX).collect();
        true
    }

    /// Press Send report. Refuses an empty title or description in words; otherwise queues the
    /// request. `latex` is the equation in the editor.
    pub fn submit(&mut self, latex: &str) {
        if self.busy {
            return;
        }
        let title = self.title.trim().to_string();
        let description = self.description.trim().to_string();
        if title.is_empty() {
            self.error = Some("Add a short title.".into());
            self.focus_title = true;
            return;
        }
        if description.is_empty() {
            self.error = Some("Add a description.".into());
            return;
        }
        self.error = None;
        self.busy = true;
        let mut body = json!({
            "title": title,
            "description": description,
            "kind": KINDS[self.kind].value,
            "latex": latex,
            "browser": self.browser,
            "pageUrl": self.page_url,
            "appVersion": self.app_version,
        });
        if let (Some(s), Some(o)) = (&self.shot, body.as_object_mut()) {
            o.insert("screenshot".into(), Value::String(s.data_url.clone()));
        }
        self.requests.push(Request::Submit(body.to_string()));
    }

    /// The shell's answer to [`Request::Submit`].
    pub fn submit_finished(&mut self, result: Result<(), String>, now: f64) {
        self.busy = false;
        match result {
            Ok(()) => {
                self.title.clear();
                self.description.clear();
                self.kind = 0;
                self.shot = None;
                self.error = None;
                self.list = ListState::Idle;
                self.thanks_until = Some(now + THANKS_SECS);
            }
            // The text and the screenshot stay, so the reporter can try again.
            Err(e) => self.error = Some(format!("{} Please try again.", if e.is_empty() { "The report could not be sent." } else { &e })),
        }
    }

    /// The shell's answer to [`Request::LoadList`]: the response body, or why there is none.
    pub fn list_finished(&mut self, result: Result<String, String>) {
        self.list = match result.and_then(|body| parse_list(&body)) {
            Ok(items) => ListState::Loaded(items),
            Err(e) => ListState::Failed(e),
        };
    }

    /// Vote for `id` (the Vote button). Ignored when already voted or in flight.
    pub fn vote(&mut self, id: &str) {
        if self.voted.contains(id) || self.voting.contains(id) {
            return;
        }
        self.voting.insert(id.to_string());
        self.vote_error = None;
        self.requests.push(Request::Vote(id.to_string()));
    }

    /// The shell's answer to [`Request::Vote`]: the new count.
    pub fn vote_finished(&mut self, id: &str, result: Result<u32, String>) {
        self.voting.remove(id);
        match result {
            Ok(votes) => {
                self.voted.insert(id.to_string());
                self.requests.push(Request::RememberVote(id.to_string()));
                if let ListState::Loaded(items) = &mut self.list {
                    if let Some(i) = items.iter_mut().find(|i| i.id == id) {
                        i.votes = votes;
                    }
                    // Most voted first, as the server sends them.
                    items.sort_by_key(|i| std::cmp::Reverse(i.votes));
                }
            }
            Err(e) => {
                let why = if e.is_empty() { "try again".to_string() } else { e };
                self.vote_error = Some((id.to_string(), format!("Your vote was not counted: {why}")));
            }
        }
    }

    /// The shell read the picked or pasted image (a data URL), or could not.
    pub fn screenshot_picked(&mut self, name: &str, result: Result<String, String>) {
        self.error = None;
        match result {
            Ok(url) if url.len() > MAX_SHOT_CHARS => {
                self.error = Some(format!("That image is over {MAX_SHOT_TEXT}. Try a smaller screenshot."));
            }
            Ok(url) => {
                let name = if name.is_empty() { "pasted image" } else { name };
                self.shot = Some(Shot { name: name.to_string(), data_url: url });
            }
            Err(e) => self.error = Some(e),
        }
    }

    pub fn open_coffee(&mut self) {
        self.close();
        self.dismiss_invite();
        self.requests.push(Request::OpenCoffee);
    }

    fn dismiss_invite(&mut self) {
        if !matches!(self.invite, Invite::Off | Invite::Done) {
            self.invite = Invite::Done;
        }
    }

    // ---- drawing -------------------------------------------------------------------------------

    /// Draw the buttons, the panel and the invitation over the page. Call before the page's own
    /// panels so Escape and a click outside reach the panel first. `latex` is read only when a
    /// report is sent.
    pub fn ui(&mut self, ctx: &Context, p: &Palette, now: f64, latex: &dyn Fn() -> String) {
        self.tick(ctx, now);
        let screen = ctx.content_rect();
        let margin = if (640.0..=1024.0).contains(&screen.width()) { MARGIN_MID } else { MARGIN };

        // Escape closes the panel (and the invitation) before the editor sees it; a press
        // anywhere else on the page closes the panel too.
        if self.open || self.invite_visible() {
            let esc = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
            if esc {
                self.close();
                self.dismiss_invite();
            }
        }
        if self.open && ctx.input(|i| i.pointer.any_pressed()) {
            let at = ctx.input(|i| i.pointer.interact_pos());
            let inside = |r: Option<Rect>| r.is_some_and(|r| at.is_some_and(|a| r.contains(a)));
            if at.is_some() && !inside(self.rects.panel) && !inside(self.rects.main) && !inside(self.rects.list) && !inside(self.rects.mug) {
                self.close();
            }
        }

        self.buttons(ctx, p, screen, margin);
        if self.open {
            self.panel(ctx, p, screen, margin, latex);
        } else {
            self.rects.panel = None;
        }
        self.invitation(ctx, p, screen, margin);
    }

    /// Time-driven state: the thank-you closing, the invitation's schedule.
    fn tick(&mut self, ctx: &Context, now: f64) {
        if let Some(until) = self.thanks_until {
            if now >= until {
                self.thanks_until = None;
                self.close();
            } else {
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(until - now));
            }
        }
        match self.invite {
            Invite::Armed => {
                self.invite = Invite::Waiting { show_at: now + INVITE_DELAY };
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(INVITE_DELAY));
            }
            Invite::Waiting { show_at } if now >= show_at => {
                self.invite = Invite::Shown { until: now + INVITE_SHOWN };
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(INVITE_SHOWN));
            }
            Invite::Waiting { show_at } => ctx.request_repaint_after(std::time::Duration::from_secs_f64(show_at - now)),
            Invite::Shown { until } if now >= until => self.invite = Invite::Done,
            _ => {}
        }
    }

    fn buttons(&mut self, ctx: &Context, p: &Palette, screen: Rect, margin: f32) {
        let main_pos = pos2(screen.right() - margin, screen.bottom() - margin);
        // The small button sits over the big one, centred on it.
        let list_pos = pos2(main_pos.x - (MAIN_BTN - SUB_BTN) / 2.0, main_pos.y - MAIN_BTN - SUB_GAP);

        let (main_resp, list_resp) = (
            fab_area(ctx, "equashion-fab-main", main_pos, Align2::RIGHT_BOTTOM, vec2(MAIN_BTN, MAIN_BTN), |ui, rect| {
                let r = ui.interact(rect, ui.id().with("btn"), Sense::click());
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Report a Bug"));
                let fill = hover_fill(&r, p.accent, p.accent_hover, p.accent_pressed);
                ui.painter().rect_filled(rect, 14.0, fill);
                shadow_hint(ui, rect, 14.0);
                bug_icon(ui, rect.center(), p.on_accent);
                focus_ring(ui, &r, rect, 14, p);
                r.on_hover_text("Report a Bug").on_hover_cursor(egui::CursorIcon::PointingHand)
            }),
            fab_area(ctx, "equashion-fab-list", list_pos, Align2::RIGHT_BOTTOM, vec2(SUB_BTN, SUB_BTN), |ui, rect| {
                let r = ui.interact(rect, ui.id().with("btn"), Sense::click());
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Reported issues"));
                let fill = hover_fill(&r, p.surface, p.control_hover, p.control_active);
                ui.painter().rect_filled(rect, 12.0, fill);
                shadow_hint(ui, rect, 12.0);
                list_icon(ui, rect.center(), p.text);
                focus_ring(ui, &r, rect, 12, p);
                r.on_hover_text("Reported issues").on_hover_cursor(egui::CursorIcon::PointingHand)
            }),
        );
        self.rects.main = Some(main_resp.rect);
        self.rects.list = Some(list_resp.rect);
        if main_resp.clicked() {
            self.toggle(View::Form);
        }
        if list_resp.clicked() {
            self.toggle(View::List);
        }

        if self.coffee_available {
            let mug_pos = pos2(screen.right() - margin, screen.top() + MUG_TOP);
            let r = fab_area(ctx, "equashion-fab-mug", mug_pos, Align2::RIGHT_TOP, vec2(SUB_BTN, SUB_BTN), |ui, rect| {
                let r = ui.interact(rect, ui.id().with("btn"), Sense::click());
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Buy me a coffee"));
                let fill = hover_fill(&r, p.surface, p.control_hover, p.control_active);
                ui.painter().rect_filled(rect, 12.0, fill);
                shadow_hint(ui, rect, 12.0);
                mug_icon(ui, rect.center(), p.text);
                focus_ring(ui, &r, rect, 12, p);
                r.on_hover_text("Buy me a coffee").on_hover_cursor(egui::CursorIcon::PointingHand)
            });
            self.rects.mug = Some(r.rect);
            if r.clicked() {
                self.open_coffee();
            }
        } else {
            self.rects.mug = None;
        }
    }

    fn invitation(&mut self, ctx: &Context, p: &Palette, screen: Rect, margin: f32) {
        if !self.invite_visible() || screen.width() < 640.0 {
            self.rects.invite = None;
            return;
        }
        let w = 260.0_f32.min(screen.width() - 108.0);
        let right = margin + SUB_BTN + S12;
        let pos = pos2(screen.right() - right, screen.top() + MUG_TOP);
        let out = Area::new(Id::new("equashion-invite")).order(Order::Foreground).pivot(Align2::RIGHT_TOP).fixed_pos(pos).show(ctx, |ui| {
            Frame::popup(ui.style())
                .fill(p.surface)
                .stroke(Stroke::new(1.0, p.border))
                .corner_radius(12)
                .inner_margin(Margin::symmetric(14, 12))
                .show(ui, |ui| {
                    ui.set_width(w - 28.0);
                    ui.add(
                        egui::Label::new(
                            RichText::new("If equaSHion saved you time, please consider buying me a coffee. Thank you.").size(13.0).color(p.text),
                        )
                        .wrap()
                        .sense(Sense::click()),
                    )
                })
        });
        self.rects.invite = Some(out.response.rect);
        if out.inner.inner.clicked() || out.response.clicked() {
            self.open_coffee();
        }
    }

    fn panel(&mut self, ctx: &Context, p: &Palette, screen: Rect, margin: f32, latex: &dyn Fn() -> String) {
        let bottom = if (640.0..=1024.0).contains(&screen.width()) { 128.0 } else { PANEL_BOTTOM };
        let w = PANEL_W.min(screen.width() - 32.0).max(200.0);
        let max_h = (screen.height() - 160.0).max(160.0);
        let pos = pos2(screen.right() - margin, screen.bottom() - bottom);
        let mut act = None::<Act>;
        let out = Area::new(Id::new("equashion-report-panel")).order(Order::Foreground).pivot(Align2::RIGHT_BOTTOM).fixed_pos(pos).show(ctx, |ui| {
            // Room for the whole panel from the first frame: a bottom-anchored area is otherwise
            // offered only the strip under its anchor and grows a step per frame.
            ui.set_max_height(max_h);
            Frame::popup(ui.style())
                .fill(p.surface)
                .stroke(Stroke::new(1.0, p.border))
                .corner_radius(12)
                .inner_margin(Margin::symmetric(20, 18))
                .show(ui, |ui| {
                    ui.set_width(w - 40.0);
                    egui::ScrollArea::vertical().max_height(max_h - 36.0).auto_shrink([true, true]).show(ui, |ui| {
                        ui.set_width(w - 40.0);
                        act = match self.view {
                            View::Form => self.form(ui, p),
                            View::List => self.list_view(ui, p),
                        };
                    });
                });
        });
        if self.rects.panel.is_none_or(|old| (old.height() - out.response.rect.height()).abs() > 0.5) {
            ctx.request_repaint();
        }
        self.rects.panel = Some(out.response.rect);
        match act {
            Some(Act::Close) => self.close(),
            Some(Act::Go(v)) => self.show_view(v),
            Some(Act::Submit) => self.submit(&latex()),
            Some(Act::Attach) => self.requests.push(Request::PickScreenshot),
            Some(Act::Retry) => {
                self.list = ListState::Idle;
                self.show_view(View::List);
            }
            Some(Act::Vote(id)) => self.vote(&id),
            None => {}
        }
    }

    fn heading(&self, ui: &mut Ui, p: &Palette, text: &str) -> bool {
        let mut close = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new(text).font(theme::semibold(16.0)).color(p.text));
            ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                let r = ui.add(egui::Button::new(RichText::new("\u{00d7}").size(18.0).color(p.muted)).frame(false));
                r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Close"));
                close = r.clicked();
            });
        });
        ui.add_space(S8);
        close
    }

    fn form(&mut self, ui: &mut Ui, p: &Palette) -> Option<Act> {
        let mut act = None;
        if self.heading(ui, p, "Report an issue") {
            act = Some(Act::Close);
        }
        if self.thanks_until.is_some() {
            ui.add_space(S4);
            ui.label(RichText::new("Thank you! Your report has been submitted.").size(13.0).color(p.text));
            return act;
        }

        // The kind.
        let gap = 6.0;
        let kw = (ui.available_width() - 2.0 * gap) / 3.0;
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for (i, k) in KINDS.iter().enumerate() {
                let (rect, r) = ui.allocate_exact_size(vec2(kw, 46.0), Sense::click());
                let on = self.kind == i;
                r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, on, k.label));
                let fg = if on { k.color } else { p.text };
                let painter = ui.painter();
                if r.hovered() && !on {
                    painter.rect_filled(rect, R_CONTROL, p.control_hover);
                }
                painter.rect_stroke(
                    rect,
                    R_CONTROL,
                    Stroke::new(if on { 2.0 } else { 1.0 }, if on { k.color } else { p.border }),
                    StrokeKind::Inside,
                );
                let a = painter.layout_no_wrap(k.label.to_string(), theme::semibold(14.0), fg);
                let b = painter.layout_no_wrap(k.hint.to_string(), FontId::proportional(11.0), fg.gamma_multiply(0.75));
                let y0 = rect.center().y - (a.size().y + b.size().y) / 2.0;
                painter.galley(pos2((rect.center().x - a.size().x / 2.0).round(), y0.round()), a.clone(), fg);
                painter.galley(pos2((rect.center().x - b.size().x / 2.0).round(), (y0 + a.size().y).round()), b, fg);
                if r.clicked() {
                    self.kind = i;
                }
            }
        });
        let kind = KINDS[self.kind];
        ui.add_space(S12);

        field_label(ui, p, "Title");
        let title_id = Id::new("equashion-report-title");
        let t = ui.add(
            egui::TextEdit::singleline(&mut self.title)
                .id(title_id)
                .char_limit(TITLE_MAX)
                .hint_text(kind.title_placeholder)
                .desired_width(f32::INFINITY)
                .margin(Margin::symmetric(10, 7)),
        );
        if self.focus_title {
            t.request_focus();
            self.focus_title = false;
        }
        ui.add_space(S8);
        field_label(ui, p, "Description");
        ui.add(
            egui::TextEdit::multiline(&mut self.description)
                .id(Id::new("equashion-report-description"))
                .char_limit(DESC_MAX)
                .hint_text(kind.body_placeholder)
                .desired_width(f32::INFINITY)
                .desired_rows(4)
                .margin(Margin::symmetric(10, 7)),
        );
        ui.add_space(S8);

        // The screenshot: a file, or an image pasted while this panel is open.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = S8;
            match &self.shot {
                Some(s) => {
                    ui.label(RichText::new(format!("Screenshot attached: {}", s.name)).size(11.0).color(p.muted));
                    if ui.add(egui::Button::new(RichText::new("Remove").size(12.0)).min_size(vec2(0.0, 28.0))).clicked() {
                        self.shot = None;
                    }
                }
                None => {
                    if ui.add(egui::Button::new(RichText::new("Attach screenshot").size(12.0)).min_size(vec2(0.0, 28.0))).clicked() {
                        act = Some(Act::Attach);
                    }
                    ui.label(RichText::new("or paste one \u{00b7} optional").size(11.0).color(p.muted));
                }
            }
        });
        ui.add_space(S8);

        if let Some(e) = &self.error {
            Frame::new().fill(p.error_soft).stroke(Stroke::new(1.0, p.error)).corner_radius(4).inner_margin(Margin::symmetric(9, 7)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.add(egui::Label::new(RichText::new(e).size(12.0).color(p.error)).wrap());
            });
            ui.add_space(S8);
        }
        ui.add(
            egui::Label::new(
                RichText::new("The equation in the editor, your browser and the app version are attached automatically. No account needed.")
                    .size(11.0)
                    .color(p.muted),
            )
            .wrap(),
        );
        ui.add_space(S12);

        // Send report, in the colour of the kind.
        let label = if self.busy { "Submitting..." } else { "Send report" };
        let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
        r.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, !self.busy, label));
        let fill = if self.busy {
            kind.color.gamma_multiply(0.6)
        } else if r.hovered() {
            kind.color.gamma_multiply(0.9)
        } else {
            kind.color
        };
        ui.painter().rect_filled(rect, R_CONTROL, fill);
        let g = ui.painter().layout_no_wrap(label.to_string(), theme::semibold(13.0), kind.ink);
        ui.painter().galley((rect.center() - g.size() / 2.0).round(), g, kind.ink);
        if r.has_focus() {
            ui.painter().rect_stroke(rect, R_CONTROL, Stroke::new(2.0, p.strong), StrokeKind::Outside);
        }
        if r.clicked() && !self.busy {
            act = Some(Act::Submit);
        }
        ui.add_space(S8);
        if ui.link(RichText::new("See what others reported").size(12.0).color(p.accent)).clicked() {
            act = Some(Act::Go(View::List));
        }
        act
    }

    fn list_view(&mut self, ui: &mut Ui, p: &Palette) -> Option<Act> {
        let mut act = None;
        if self.heading(ui, p, "Reported issues") {
            act = Some(Act::Close);
        }
        match &self.list {
            ListState::Idle | ListState::Loading => {
                ui.label(RichText::new("Loading...").size(12.0).color(p.muted));
            }
            ListState::Failed(e) => {
                let why = if e.is_empty() { "the network is unavailable." } else { e.as_str() };
                Frame::new().fill(p.error_soft).stroke(Stroke::new(1.0, p.error)).corner_radius(4).inner_margin(Margin::symmetric(9, 7)).show(
                    ui,
                    |ui| {
                        ui.set_width(ui.available_width());
                        ui.add(egui::Label::new(RichText::new(format!("The list could not be loaded: {why}")).size(12.0).color(p.error)).wrap());
                    },
                );
                if ui.link(RichText::new("Try again").size(12.0).color(p.accent)).clicked() {
                    act = Some(Act::Retry);
                }
            }
            ListState::Loaded(items) if items.is_empty() => {
                ui.label(RichText::new("Nothing has been reported yet.").size(12.0).color(p.muted));
            }
            ListState::Loaded(items) => {
                for (n, it) in items.iter().enumerate() {
                    if n > 0 {
                        let y = ui.cursor().min.y;
                        ui.painter().hline(ui.cursor().x_range(), y, Stroke::new(1.0, p.border));
                        ui.add_space(S8);
                    }
                    let has = self.voted.contains(&it.id);
                    let pending = self.voting.contains(&it.id);
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 10.0;
                        let (rect, r) = ui.allocate_exact_size(vec2(52.0, 44.0), Sense::click());
                        r.widget_info(|| {
                            egui::WidgetInfo::labeled(
                                egui::WidgetType::Button,
                                !has && !pending,
                                format!("{} {} votes", if has { "Voted," } else { "Vote," }, it.votes),
                            )
                        });
                        let (fill, stroke, fg) = if has {
                            (p.accent, p.accent, p.on_accent)
                        } else if r.hovered() && !pending {
                            (Color32::TRANSPARENT, p.accent, p.accent)
                        } else {
                            (Color32::TRANSPARENT, p.border, p.text)
                        };
                        let painter = ui.painter();
                        painter.rect_filled(rect, R_CONTROL, fill);
                        painter.rect_stroke(rect, R_CONTROL, Stroke::new(1.0, stroke), StrokeKind::Inside);
                        let a = painter.layout_no_wrap(it.votes.to_string(), theme::semibold(14.0), fg);
                        let b = painter.layout_no_wrap(if has { "Voted" } else { "Vote" }.to_string(), theme::semibold(12.0), fg);
                        let y0 = rect.center().y - (a.size().y + b.size().y) / 2.0;
                        painter.galley(pos2((rect.center().x - a.size().x / 2.0).round(), y0.round()), a.clone(), fg);
                        painter.galley(pos2((rect.center().x - b.size().x / 2.0).round(), (y0 + a.size().y).round()), b, fg);
                        let r = r.on_hover_text(if has { "You voted for this" } else { "This matters to me too" });
                        if r.clicked() && !has && !pending {
                            act = Some(Act::Vote(it.id.clone()));
                        }
                        ui.vertical(|ui| {
                            ui.add(egui::Label::new(RichText::new(&it.title).font(theme::semibold(13.0)).color(p.text)).wrap());
                            let kind = kind_label(&it.kind);
                            let status = status_label(&it.status);
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 0.0;
                                ui.label(RichText::new(format!("{kind} \u{00b7} {status}")).size(11.0).color(p.muted));
                                if let Some(u) = &it.screenshot_url {
                                    ui.label(RichText::new(" \u{00b7} ").size(11.0).color(p.muted));
                                    ui.hyperlink_to(RichText::new("Screenshot").size(11.0).color(p.accent), u);
                                }
                            });
                            let d: String = it.description.chars().take(240).collect();
                            let d = if it.description.chars().count() > 240 { format!("{d}...") } else { d };
                            ui.add(egui::Label::new(RichText::new(d).size(12.0).color(p.text)).wrap());
                            if let Some((id, why)) = &self.vote_error
                                && *id == it.id
                            {
                                ui.add(egui::Label::new(RichText::new(why).size(11.0).color(p.error)).wrap());
                            }
                        });
                    });
                    ui.add_space(S8);
                }
            }
        }
        ui.add_space(S8);
        if ui.link(RichText::new("Report an issue").size(12.0).color(p.accent)).clicked() {
            act = Some(Act::Go(View::Form));
        }
        act
    }
}

/// What a click in the panel asked for.
enum Act {
    Close,
    Go(View),
    Submit,
    Attach,
    Retry,
    Vote(String),
}

fn kind_label(kind: &str) -> &'static str {
    match kind {
        "quirk" => "Quirk",
        "enhancement" => "Improve",
        _ => "Bug",
    }
}

fn status_label(status: &str) -> &'static str {
    match status {
        "in_progress" => "In progress",
        "fixed" => "Fixed",
        "deferred" => "Deferred",
        _ => "Open",
    }
}

/// The list in a `GET /api/issue-reports` body, most voted first.
pub fn parse_list(body: &str) -> Result<Vec<Issue>, String> {
    let v: Value = serde_json::from_str(body).map_err(|_| "the answer was not understood.".to_string())?;
    if let Some(e) = v.get("error").and_then(Value::as_str) {
        return Err(e.to_string());
    }
    let text = |r: &Value, k: &str| r.get(k).and_then(Value::as_str).unwrap_or_default().to_string();
    let mut out: Vec<Issue> = v
        .get("reports")
        .and_then(Value::as_array)
        .map(|rs| {
            rs.iter()
                .map(|r| Issue {
                    id: match r.get("id") {
                        Some(Value::String(s)) => s.clone(),
                        Some(other) => other.to_string(),
                        None => String::new(),
                    },
                    kind: text(r, "kind"),
                    title: text(r, "title"),
                    description: text(r, "description"),
                    status: text(r, "status"),
                    votes: r.get("votes").and_then(Value::as_u64).unwrap_or(0).min(u64::from(u32::MAX)) as u32,
                    screenshot_url: r.get("screenshotUrl").and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string),
                })
                .filter(|i| !i.id.is_empty())
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|i| std::cmp::Reverse(i.votes));
    Ok(out)
}

/// The error of a failed call: the server's own words when it sent some.
pub fn error_from(status: u16, body: &str) -> String {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_string))
        .unwrap_or_else(|| format!("The server answered {status}."))
}

/// The vote count in a `POST .../vote` answer.
pub fn parse_votes(body: &str) -> Option<u32> {
    serde_json::from_str::<Value>(body).ok()?.get("votes")?.as_u64().map(|v| v.min(u64::from(u32::MAX)) as u32)
}

fn field_label(ui: &mut Ui, p: &Palette, text: &str) {
    ui.label(RichText::new(text).font(theme::semibold(T_CAPTION)).color(p.muted));
    ui.add_space(S4);
}

/// Show an area of exactly `size` anchored at `at` and run `add` with its rectangle.
fn fab_area(
    ctx: &Context,
    name: &'static str,
    at: Pos2,
    pivot: Align2,
    size: egui::Vec2,
    add: impl FnOnce(&mut Ui, Rect) -> egui::Response,
) -> egui::Response {
    Area::new(Id::new(name))
        .order(Order::Foreground)
        .pivot(pivot)
        .fixed_pos(at)
        .show(ctx, |ui| {
            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
            add(ui, rect)
        })
        .inner
}

fn hover_fill(r: &egui::Response, rest: Color32, hover: Color32, down: Color32) -> Color32 {
    if r.is_pointer_button_down_on() {
        down
    } else if r.hovered() || r.has_focus() {
        hover
    } else {
        rest
    }
}

/// A faint drop shadow under a button.
fn shadow_hint(ui: &Ui, rect: Rect, radius: f32) {
    ui.painter().rect_stroke(rect.expand(0.5), radius, Stroke::new(1.0, Color32::from_black_alpha(28)), StrokeKind::Outside);
}

fn focus_ring(ui: &Ui, r: &egui::Response, rect: Rect, radius: u8, p: &Palette) {
    if r.has_focus() {
        ui.painter().rect_stroke(rect.expand(2.0), radius, Stroke::new(2.0, p.strong), StrokeKind::Outside);
    }
}

/// The bug icon (raSHio's, 16 units wide), centred on `c` at 1.5 points per unit.
fn bug_icon(ui: &Ui, c: Pos2, col: Color32) {
    let u = 1.5;
    let at = |x: f32, y: f32| pos2(c.x + (x - 8.0) * u, c.y + (y - 8.0) * u);
    let s = Stroke::new(1.3 * u, col);
    let painter = ui.painter();
    // The head: an arc over the body.
    let head: Vec<Pos2> = (0..=10)
        .map(|k| {
            let a = std::f32::consts::PI * (1.0 + k as f32 / 10.0);
            at(8.0 + 2.5 * a.cos(), 5.5 + 2.5 * a.sin())
        })
        .collect();
    painter.add(egui::Shape::line(head, s));
    painter.rect_stroke(Rect::from_min_max(at(4.5, 5.5), at(11.5, 13.0)), 3.5 * u, s, StrokeKind::Middle);
    for (a, b) in [
        ((8.0, 5.5), (8.0, 13.0)),
        ((4.5, 8.0), (1.5, 8.0)),
        ((11.5, 8.0), (14.5, 8.0)),
        ((4.8, 11.0), (2.5, 12.7)),
        ((11.2, 11.0), (13.5, 12.7)),
        ((5.2, 3.4), (3.6, 1.8)),
        ((10.8, 3.4), (12.4, 1.8)),
    ] {
        painter.line_segment([at(a.0, a.1), at(b.0, b.1)], s);
    }
}

/// The list icon: three rules with bullets.
fn list_icon(ui: &Ui, c: Pos2, col: Color32) {
    let u = 1.25;
    let at = |x: f32, y: f32| pos2(c.x + (x - 8.0) * u, c.y + (y - 8.0) * u);
    let s = Stroke::new(1.4 * u, col);
    let painter = ui.painter();
    for y in [4.0, 8.0, 12.0] {
        painter.line_segment([at(5.5, y), at(13.5, y)], s);
        painter.circle_filled(at(2.5, y), 0.9 * u, col);
    }
}

/// A mug with a handle and two wisps of steam (24 units wide, 1 point per unit).
fn mug_icon(ui: &Ui, c: Pos2, col: Color32) {
    let at = |x: f32, y: f32| pos2(c.x + (x - 12.0) * 0.92, c.y + (y - 12.0) * 0.92);
    let s = Stroke::new(1.5, col);
    let painter = ui.painter();
    painter.rect_stroke(Rect::from_min_max(at(3.2, 8.6), at(15.6, 18.4)), 2.0, s, StrokeKind::Middle);
    let handle: Vec<Pos2> = (0..=10)
        .map(|k| {
            let a = -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * k as f32 / 10.0;
            at(15.4 + 3.0 * a.cos(), 13.9 + 3.3 * a.sin())
        })
        .collect();
    painter.add(egui::Shape::line(handle, s));
    for x in [8.4, 11.7] {
        painter.line_segment([at(x, 5.6), at(x, 1.4)], Stroke::new(1.5, col.gamma_multiply(0.8)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_is_read_most_voted_first() {
        let body = r#"{"reports":[
            {"id":"7","kind":"bug","title":"a","description":"d","status":"open","votes":1},
            {"id":8,"kind":"quirk","title":"b","description":"d","status":"fixed","votes":4,"screenshotUrl":"/s?id=1"}
        ]}"#;
        let items = parse_list(body).expect("list");
        assert_eq!(items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(), ["8", "7"]);
        assert_eq!(items[0].screenshot_url.as_deref(), Some("/s?id=1"));
        assert!(parse_list("not json").is_err());
        assert_eq!(parse_list(r#"{"error":"nope"}"#), Err("nope".to_string()));
    }

    #[test]
    fn errors_use_the_servers_words() {
        assert_eq!(error_from(429, r#"{"error":"Too many reports."}"#), "Too many reports.");
        assert_eq!(error_from(502, "<html>"), "The server answered 502.");
        assert_eq!(parse_votes(r#"{"votes":3}"#), Some(3));
    }

    #[test]
    fn an_empty_report_is_refused_and_a_full_one_is_queued() {
        let mut f = Fab::new();
        f.set_environment("UA", "https://example.test/");
        f.toggle(View::Form);
        f.submit("x");
        assert_eq!(f.error(), Some("Add a short title."));
        assert!(f.take_requests().is_empty());
        assert!(f.fill("enhancement", "  A title ", "Words"));
        f.submit(r"\frac{a}{b}");
        assert!(f.busy());
        let Some(Request::Submit(body)) = f.take_requests().pop() else { panic!("no submit") };
        let v: Value = serde_json::from_str(&body).expect("json");
        assert_eq!(v["title"], "A title");
        assert_eq!(v["kind"], "enhancement");
        assert_eq!(v["latex"], r"\frac{a}{b}");
        assert_eq!(v["browser"], "UA");
        assert_eq!(v["pageUrl"], "https://example.test/");
        assert!(v.get("screenshot").is_none());
        // Failing keeps the text; succeeding clears it and says thanks.
        f.submit_finished(Err("Too many reports.".into()), 1.0);
        assert_eq!(f.error(), Some("Too many reports. Please try again."));
        f.submit_finished(Ok(()), 2.0);
        assert!(f.thanks());
        assert_eq!(f.kind(), "bug");
        assert!(!f.fill("nope", "", ""));
    }

    #[test]
    fn one_vote_per_issue() {
        let mut f = Fab::new();
        f.toggle(View::List);
        assert_eq!(f.take_requests(), vec![Request::LoadList]);
        f.list_finished(Ok(r#"{"reports":[{"id":"1","kind":"bug","title":"t","description":"","status":"open","votes":2}]}"#.into()));
        f.vote("1");
        f.vote("1");
        assert_eq!(f.take_requests(), vec![Request::Vote("1".into())]);
        f.vote_finished("1", Ok(3));
        assert!(f.has_voted("1"));
        assert_eq!(f.take_requests(), vec![Request::RememberVote("1".into())]);
        f.vote("1");
        assert!(f.take_requests().is_empty());
        let ListState::Loaded(items) = f.list() else { panic!("not loaded") };
        assert_eq!(items[0].votes, 3);
    }

    #[test]
    fn screenshots_are_checked_for_size() {
        let mut f = Fab::new();
        f.screenshot_picked("big.png", Ok("x".repeat(MAX_SHOT_CHARS + 1)));
        assert!(f.screenshot().is_none());
        assert!(f.error().is_some_and(|e| e.contains("over 1 MB")));
        f.screenshot_picked("", Ok("data:image/png;base64,AAAA".into()));
        assert_eq!(f.screenshot().map(|s| s.name.as_str()), Some("pasted image"));
    }
}
