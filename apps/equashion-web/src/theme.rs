//! The showcase's look: colour tokens for a light and a dark theme, one corner-radius scale,
//! one spacing scale, a type scale, and the Geist / Geist Mono fonts.
//!
//! Only this app sets fonts and styles. The `equashion-egui` widget never does (its
//! shared-context rule); it reads `ui.visuals()`, so it follows whatever is installed here.
//!
//! | Token | Light | Dark | Used for |
//! |---|---|---|---|
//! | accent | `#2b62a3` | `#7aaee6` | buttons, caret, selection, links, active tab, focus |
//! | accent hover / pressed | `#24558f` / `#1f4a7d` | `#93bdeb` / `#a9cbf0` | primary button states (text `#fcfcfc` / `#141416`) |
//! | accent tint | `#d5e0ec` | `#2f3b4a` | selected tab, badges, text selection (accent text `#1f4f87` / `#a9cbf0`) |
//! | bg | `#f6f6f7` | `#141416` | page |
//! | surface | `#fcfcfc` | `#1b1b1e` | cards, panels, editor canvas |
//! | sunken | `#efeff1` | `#0f0f11` | text fields, code |
//! | border | `#dcdce0` | `#2c2c31` | panel and card outlines, separators |
//! | control | `#ececef` | `#26262a` | button fill (hover `#e2e2e6` / `#2f2f34`, pressed `#d9d9de` / `#38383e`) |
//! | control stroke | `#8a8a93` | `#6e6e78` | button and field outlines (3:1 against the page) |
//! | muted | `#5c5c66` | `#a3a3ad` | captions, placeholders, secondary text (AA) |
//! | text | `#2a2a30` | `#dcdce1` | body |
//! | strong | `#17171a` | `#f2f2f4` | headings, equation ink |
//! | error | `#b42318` | `#f2877d` | error states only |
//!
//! Radii: 6 px for controls (buttons, fields, the editor canvas, badges), 10 px for panels and
//! cards. Spacing: 4, 8, 12, 16, 24, 32. Type: display 22, h2 16, body 14, caption 12, mono 13.

use std::sync::Arc;

use egui::{Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Stroke, Style, TextStyle, Theme, Visuals, vec2};

/// Corner radius of controls: buttons, text fields, the editor canvas, badges.
pub const R_CONTROL: u8 = 6;
/// Corner radius of panels and cards.
pub const R_PANEL: u8 = 10;

/// The spacing scale, points.
pub const S4: f32 = 4.0;
pub const S8: f32 = 8.0;
pub const S12: f32 = 12.0;
pub const S16: f32 = 16.0;
pub const S24: f32 = 24.0;
pub const S32: f32 = 32.0;

/// The type scale, points.
pub const T_DISPLAY: f32 = 22.0;
pub const T_H2: f32 = 16.0;
pub const T_BODY: f32 = 14.0;
pub const T_CAPTION: f32 = 12.0;
pub const T_MONO: f32 = 13.0;

/// The semibold UI face (headings, the wordmark, button-like emphasis).
pub const SEMIBOLD: &str = "geist-semibold";

/// One theme's colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub accent: Color32,
    /// Text on an accent fill.
    pub on_accent: Color32,
    /// Accent fill of a hovered and of a pressed primary button.
    pub accent_hover: Color32,
    pub accent_pressed: Color32,
    /// Accent-tinted background: the selected tab, badges and text selection.
    pub accent_soft: Color32,
    /// Accent text on `accent_soft`.
    pub accent_text: Color32,
    pub bg: Color32,
    pub surface: Color32,
    pub sunken: Color32,
    pub border: Color32,
    pub control: Color32,
    pub control_hover: Color32,
    pub control_active: Color32,
    pub control_stroke: Color32,
    pub muted: Color32,
    pub text: Color32,
    pub strong: Color32,
    pub error: Color32,
    pub error_soft: Color32,
}

const fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub const LIGHT: Palette = Palette {
    accent: hex(0x2b62a3),
    on_accent: hex(0xfcfcfc),
    accent_hover: hex(0x24558f),
    accent_pressed: hex(0x1f4a7d),
    accent_soft: hex(0xd5e0ec),
    accent_text: hex(0x1f4f87),
    bg: hex(0xf6f6f7),
    surface: hex(0xfcfcfc),
    sunken: hex(0xefeff1),
    border: hex(0xdcdce0),
    control: hex(0xececef),
    control_hover: hex(0xe2e2e6),
    control_active: hex(0xd9d9de),
    control_stroke: hex(0x8a8a93),
    muted: hex(0x5c5c66),
    text: hex(0x2a2a30),
    strong: hex(0x17171a),
    error: hex(0xb42318),
    error_soft: hex(0xf5e6e5),
};

pub const DARK: Palette = Palette {
    accent: hex(0x7aaee6),
    on_accent: hex(0x141416),
    accent_hover: hex(0x93bdeb),
    accent_pressed: hex(0xa9cbf0),
    accent_soft: hex(0x2f3b4a),
    accent_text: hex(0xa9cbf0),
    bg: hex(0x141416),
    surface: hex(0x1b1b1e),
    sunken: hex(0x0f0f11),
    border: hex(0x2c2c31),
    control: hex(0x26262a),
    control_hover: hex(0x2f2f34),
    control_active: hex(0x38383e),
    control_stroke: hex(0x6e6e78),
    muted: hex(0xa3a3ad),
    text: hex(0xdcdce1),
    strong: hex(0xf2f2f4),
    error: hex(0xf2877d),
    error_soft: hex(0x392a2b),
};

/// The palette of the theme `ui` is drawn in.
pub fn palette(ui: &egui::Ui) -> &'static Palette {
    if ui.visuals().dark_mode { &DARK } else { &LIGHT }
}

/// The theme preference the page offers: follow the system, or force one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemePref {
    #[default]
    System,
    Light,
    Dark,
}

impl ThemePref {
    pub const ALL: [ThemePref; 3] = [ThemePref::System, ThemePref::Light, ThemePref::Dark];

    /// The name the test hooks use (`system`, `light`, `dark`).
    pub fn name(self) -> &'static str {
        match self {
            ThemePref::System => "system",
            ThemePref::Light => "light",
            ThemePref::Dark => "dark",
        }
    }

    /// The label of the toggle segment.
    pub fn title(self) -> &'static str {
        match self {
            ThemePref::System => "System",
            ThemePref::Light => "Light",
            ThemePref::Dark => "Dark",
        }
    }

    /// A preference by name (any case).
    pub fn parse(name: &str) -> Option<ThemePref> {
        let n = name.trim().to_ascii_lowercase();
        ThemePref::ALL.into_iter().find(|t| t.name() == n)
    }

    pub fn to_egui(self) -> egui::ThemePreference {
        match self {
            ThemePref::System => egui::ThemePreference::System,
            ThemePref::Light => egui::ThemePreference::Light,
            ThemePref::Dark => egui::ThemePreference::Dark,
        }
    }
}

/// The name of an egui theme (`light` / `dark`).
pub fn theme_name(t: Theme) -> &'static str {
    match t {
        Theme::Light => "light",
        Theme::Dark => "dark",
    }
}

/// Geist (Regular, SemiBold) and Geist Mono (Regular) in front of egui's default fonts, which
/// stay as the fallback for characters Geist lacks.
pub fn fonts() -> FontDefinitions {
    let mut defs = FontDefinitions::default();
    let data = |bytes: &'static [u8]| Arc::new(FontData::from_static(bytes));
    defs.font_data.insert("geist".into(), data(include_bytes!("../../../assets/fonts/Geist-Regular.ttf")));
    defs.font_data.insert(SEMIBOLD.into(), data(include_bytes!("../../../assets/fonts/Geist-SemiBold.ttf")));
    defs.font_data.insert("geist-mono".into(), data(include_bytes!("../../../assets/fonts/GeistMono-Regular.ttf")));
    let proportional = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mut semibold = vec![SEMIBOLD.to_string()];
    semibold.extend(proportional.iter().cloned());
    defs.families.entry(FontFamily::Proportional).or_default().insert(0, "geist".into());
    defs.families.entry(FontFamily::Monospace).or_default().insert(0, "geist-mono".into());
    defs.families.insert(FontFamily::Name(SEMIBOLD.into()), semibold);
    defs
}

/// A semibold font id.
pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

/// The style of one theme. `reduced_motion` turns egui's animations off.
pub fn style(p: &Palette, dark: bool, reduced_motion: bool) -> Style {
    let mut s = Style::default();
    s.text_styles = [
        (TextStyle::Heading, semibold(T_DISPLAY)),
        (TextStyle::Name("h2".into()), semibold(T_H2)),
        (TextStyle::Body, FontId::proportional(T_BODY)),
        (TextStyle::Button, FontId::proportional(T_BODY)),
        (TextStyle::Small, FontId::proportional(T_CAPTION)),
        (TextStyle::Monospace, FontId::monospace(T_MONO)),
    ]
    .into();
    s.animation_time = if reduced_motion { 0.0 } else { 0.08 };
    s.scroll_animation = if reduced_motion { egui::style::ScrollAnimation::none() } else { s.scroll_animation };

    let sp = &mut s.spacing;
    sp.item_spacing = vec2(S8, S4);
    sp.button_padding = vec2(S12, S4);
    sp.interact_size = vec2(40.0, 28.0);
    sp.window_margin = Margin::same(S12 as i8);
    sp.menu_margin = Margin::same(S8 as i8);

    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    v.dark_mode = dark;
    v.override_text_color = None;
    v.weak_text_color = Some(p.muted);
    v.panel_fill = p.bg;
    v.window_fill = p.surface;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_corner_radius = CornerRadius::same(R_PANEL);
    v.menu_corner_radius = CornerRadius::same(R_CONTROL);
    v.faint_bg_color = if dark { hex(0x18181b) } else { hex(0xf1f1f3) };
    v.extreme_bg_color = p.surface;
    v.text_edit_bg_color = Some(p.sunken);
    v.code_bg_color = p.sunken;
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.error;
    v.error_fg_color = p.error;
    v.selection.bg_fill = p.accent_soft;
    v.selection.stroke = Stroke::new(1.0, p.accent);
    v.text_cursor.stroke = Stroke::new(2.0, p.accent);
    v.text_cursor.blink = !reduced_motion;
    v.window_shadow = egui::Shadow::NONE;
    v.popup_shadow = egui::Shadow { offset: [0, 4], blur: 12, spread: 0, color: Color32::from_black_alpha(if dark { 90 } else { 30 }) };
    v.striped = false;
    v.disabled_alpha = 0.7;

    let r = CornerRadius::same(R_CONTROL);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = p.surface;
    w.noninteractive.weak_bg_fill = p.surface;
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
    w.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
    w.noninteractive.corner_radius = r;
    w.inactive.bg_fill = p.control;
    w.inactive.weak_bg_fill = p.control;
    w.inactive.bg_stroke = Stroke::new(1.0, p.control_stroke);
    w.inactive.fg_stroke = Stroke::new(1.0, p.text);
    w.inactive.corner_radius = r;
    w.hovered.bg_fill = p.control_hover;
    w.hovered.weak_bg_fill = p.control_hover;
    w.hovered.bg_stroke = Stroke::new(1.0, p.accent);
    w.hovered.fg_stroke = Stroke::new(1.5, p.strong);
    w.hovered.corner_radius = r;
    w.hovered.expansion = 0.0;
    // Pressed and keyboard-focused widgets: the accent ring. `strong_text_color()` is the
    // active foreground, which the editor widget uses for equation ink.
    w.active.bg_fill = p.control_active;
    w.active.weak_bg_fill = p.control_active;
    w.active.bg_stroke = Stroke::new(2.0, p.accent);
    w.active.fg_stroke = Stroke::new(2.0, p.strong);
    w.active.corner_radius = r;
    w.active.expansion = 0.0;
    w.open = w.active;
    w.open.bg_stroke = Stroke::new(1.0, p.accent);
    s.visuals = v;
    s
}

/// Install the fonts and both themes' styles on `ctx` and select `pref`.
pub fn install(ctx: &egui::Context, pref: ThemePref, reduced_motion: bool) {
    ctx.set_fonts(fonts());
    ctx.set_style_of(Theme::Light, style(&LIGHT, false, reduced_motion));
    ctx.set_style_of(Theme::Dark, style(&DARK, true, reduced_motion));
    ctx.set_theme(pref.to_egui());
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG relative luminance.
    fn lum(c: Color32) -> f64 {
        let f = |v: u8| {
            let c = f64::from(v) / 255.0;
            if c <= 0.03928 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * f(c.r()) + 0.7152 * f(c.g()) + 0.0722 * f(c.b())
    }

    fn contrast(a: Color32, b: Color32) -> f64 {
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn every_text_colour_meets_wcag_aa_in_both_themes() {
        for (name, p) in [("light", LIGHT), ("dark", DARK)] {
            for (fg_name, fg) in [("muted", p.muted), ("text", p.text), ("strong", p.strong), ("accent", p.accent), ("error", p.error)] {
                for (bg_name, bg) in
                    [("bg", p.bg), ("surface", p.surface), ("sunken", p.sunken), ("control", p.control), ("control_active", p.control_active)]
                {
                    let c = contrast(fg, bg);
                    assert!(c >= 4.5 || (fg_name == "accent" && bg_name.starts_with("control")), "{name}: {fg_name} on {bg_name} is {c:.2}");
                }
            }
            for fill in [p.accent, p.accent_hover, p.accent_pressed] {
                assert!(contrast(p.on_accent, fill) >= 4.5, "{name}: on_accent on {fill:?}");
            }
            assert!(contrast(p.accent_text, p.accent_soft) >= 4.5, "{name}: accent_text");
            // egui draws a selected label's text in the accent on the selection tint.
            assert!(contrast(p.accent, p.accent_soft) >= 4.5, "{name}: accent on accent_soft");
            assert!(contrast(p.error, p.error_soft) >= 4.5, "{name}: error badge");
            assert!(contrast(p.control_stroke, p.bg) >= 3.0, "{name}: control outline");
            assert!(contrast(p.accent, p.bg) >= 3.0, "{name}: focus ring");
        }
    }

    #[test]
    fn no_pure_black_or_white_and_one_accent_hue() {
        for p in [LIGHT, DARK] {
            for c in [p.bg, p.surface, p.sunken, p.text, p.strong, p.on_accent] {
                assert_ne!(c, Color32::BLACK);
                assert_ne!(c, Color32::WHITE);
            }
        }
        // Both accents are the same blue (hue within 2 degrees), saturation under 80%.
        let hsl = |c: Color32| {
            let (r, g, b) = (f64::from(c.r()) / 255.0, f64::from(c.g()) / 255.0, f64::from(c.b()) / 255.0);
            let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
            let l = (mx + mn) / 2.0;
            let d = mx - mn;
            let s = if d == 0.0 { 0.0 } else { d / (1.0 - (2.0 * l - 1.0).abs()) };
            let h = if mx == b { 60.0 * ((r - g) / d) + 240.0 } else { 0.0 };
            (h, s)
        };
        let (h1, s1) = hsl(LIGHT.accent);
        let (h2, s2) = hsl(DARK.accent);
        assert!((h1 - h2).abs() < 2.0, "{h1} {h2}");
        assert!(s1 < 0.8 && s2 < 0.8, "{s1} {s2}");
    }

    #[test]
    fn theme_names_round_trip() {
        for t in ThemePref::ALL {
            assert_eq!(ThemePref::parse(&t.name().to_uppercase()), Some(t));
        }
        assert_eq!(ThemePref::parse("sepia"), None);
    }
}
