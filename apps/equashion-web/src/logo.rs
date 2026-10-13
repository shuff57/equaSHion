//! The animated equaSHion wordmark in the header, drawn with egui.
//!
//! The art is `assets/logo/equashion-motion.svg`: a caret types S / H in the linear form, the slash
//! sets itself into a fraction bar with S over H, and the rest of the word grows out either side.
//! The glyph outlines are read from that file at start-up, rasterised once into alpha masks (one
//! per piece, at the size and pixel density it is shown at) and then moved, faded and tinted
//! frame by frame. The timeline is the one the SVG's CSS keyframes describe (4.4 s, played once),
//! with the same easing curves. Pointing at the word, or tapping it, plays it again. Under
//! `prefers-reduced-motion` the mark is the settled word with no caret.

use egui::epaint::{Mesh, PathShape, Vertex};
use egui::{Color32, ColorImage, Context, FontId, Rect, Response, Sense, Stroke, TextureHandle, TextureOptions, Ui, pos2, vec2};

use crate::theme::Palette;

const SVG: &str = include_str!("../../../assets/logo/equashion-motion.svg");

/// The art's size, art units.
pub const ART_W: f32 = 720.0;
pub const ART_H: f32 = 220.0;
/// Points per art unit: the settled word (454 x 110 units) then fills a 118 x 29 point slot.
pub const SCALE: f32 = 0.26;
/// The settled word's box inside the art: left edge and vertical centre, art units.
const WORD_LEFT: f32 = 137.4;
const WORD_MID_Y: f32 = 110.0;
/// The header slot the word is laid out in, points.
pub const SLOT_W: f32 = 120.0;
pub const SLOT_H: f32 = 40.0;
/// The whole timeline, seconds.
const DURATION: f64 = 4.4;
/// The caret's blink period, seconds, and the share of it that it is on.
const BLINK: f64 = 1.06;
const BLINK_ON: f64 = 0.61;
/// Sub-rows per pixel row in the rasteriser.
const SUBROWS: usize = 8;

/// Fill colours, as in the SVG's own style block: (ink, accent) for light and dark.
const INK_LIGHT: Color32 = Color32::from_rgb(0x1d, 0x1d, 0x22);
const ACC_LIGHT: Color32 = Color32::from_rgb(0x2b, 0x62, 0xa3);
const INK_DARK: Color32 = Color32::from_rgb(0xec, 0xec, 0xf0);
const ACC_DARK: Color32 = Color32::from_rgb(0x7a, 0xae, 0xe6);

/// The pieces that are outlines, in paint order (the SVG's ids).
const PIECES: [&str; 8] = ["equa", "ion", "fS", "fH", "tS", "tSl", "tH", "bar"];
const OUTLINES: [&str; 7] = ["equa", "ion", "fS", "fH", "tS", "tSl", "tH"];

const BAR_X: f32 = 358.64;
const BAR_Y: f32 = 106.0;
const BAR_W: f32 = 70.49;
const BAR_H: f32 = 4.0;
const BAR_R: f32 = 2.0;
const CARET_Y: f32 = 72.0;
const CARET_W: f32 = 3.5;
const CARET_H: f32 = 66.0;

/// A cubic-bezier timing function, CSS style.
#[derive(Clone, Copy)]
struct Ease(f64, f64, f64, f64);

/// `cubic-bezier(.22,1,.36,1)`: the word grows out and settles.
const OUT: Ease = Ease(0.22, 1.0, 0.36, 1.0);
/// `cubic-bezier(.34,1.45,.5,1)`: the S, H and bar spring into place.
const SPRING: Ease = Ease(0.34, 1.45, 0.5, 1.0);
/// CSS `ease`, the default.
const EASE: Ease = Ease(0.25, 0.1, 0.25, 1.0);

impl Ease {
    fn at(self, t: f64) -> f64 {
        let Ease(x1, y1, x2, y2) = self;
        let (cx, bx, ax) = (3.0 * x1, 3.0 * (x2 - x1) - 3.0 * x1, 0.0);
        let ax = ax + 1.0 - cx - bx;
        let x = |s: f64| ((ax * s + bx) * s + cx) * s;
        let (cy, by) = (3.0 * y1, 3.0 * (y2 - y1) - 3.0 * y1);
        let ay = 1.0 - cy - by;
        let y = |s: f64| ((ay * s + by) * s + cy) * s;
        let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
        for _ in 0..28 {
            let mid = (lo + hi) / 2.0;
            if x(mid) < t {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        y((lo + hi) / 2.0)
    }
}

/// One keyframe's transform and opacity.
#[derive(Clone, Copy)]
struct Xf {
    opacity: f32,
    tx: f32,
    ty: f32,
    /// Degrees, about the piece's centre (the bar only).
    rot: f32,
    sx: f32,
}

const fn xf(opacity: f32, tx: f32, ty: f32) -> Xf {
    Xf { opacity, tx, ty, rot: 0.0, sx: 1.0 }
}

const SETTLED: Xf = xf(1.0, 0.0, 0.0);

fn lerp(a: Xf, b: Xf, u: f32) -> Xf {
    let l = |x: f32, y: f32| x + (y - x) * u;
    Xf { opacity: l(a.opacity, b.opacity).clamp(0.0, 1.0), tx: l(a.tx, b.tx), ty: l(a.ty, b.ty), rot: l(a.rot, b.rot), sx: l(a.sx, b.sx) }
}

/// The value of a keyframed property at `pct` percent of the timeline. Each interval is eased
/// on its own, as CSS does; before the first key and after the last the end values hold.
fn sample(keys: &[(f64, Xf)], pct: f64, ease: Ease) -> Xf {
    let Some(&(_, first)) = keys.first() else { return SETTLED };
    let mut cur = first;
    for w in keys.windows(2) {
        let ((p0, a), (p1, b)) = (w[0], w[1]);
        if pct >= p1 {
            cur = b;
            continue;
        }
        if pct < p0 {
            return cur;
        }
        let u = ((pct - p0) / (p1 - p0)).clamp(0.0, 1.0);
        return lerp(a, b, ease.at(u) as f32);
    }
    cur
}

// The keyframes of the SVG, percent of the 4.4 s timeline.
const ALL: [(f64, Xf); 3] = [(0.0, xf(1.0, -33.89, 0.0)), (53.41, xf(1.0, -33.89, 0.0)), (79.55, SETTLED)];
const EQUA: [(f64, Xf); 3] = [(0.0, xf(0.0, -56.0, 0.0)), (53.41, xf(0.0, -56.0, 0.0)), (79.55, SETTLED)];
const ION: [(f64, Xf); 3] = [(0.0, xf(0.0, 56.0, 0.0)), (53.41, xf(0.0, 56.0, 0.0)), (79.55, SETTLED)];
const FS: [(f64, Xf); 4] = [(0.0, xf(0.0, -37.2, 31.3)), (46.59, xf(0.0, -37.2, 31.3)), (49.77, xf(1.0, -37.2, 31.3)), (62.5, SETTLED)];
const FH: [(f64, Xf); 4] = [(0.0, xf(0.0, 37.2, -30.2)), (46.59, xf(0.0, 37.2, -30.2)), (49.77, xf(1.0, 37.2, -30.2)), (62.5, SETTLED)];
const BAR_FROM: Xf = Xf { opacity: 0.0, tx: 0.0, ty: 0.0, rot: -62.0, sx: 0.12 };
const BAR: [(f64, Xf); 4] = [(0.0, BAR_FROM), (46.59, BAR_FROM), (48.41, Xf { opacity: 1.0, ..BAR_FROM }), (61.36, SETTLED)];
const TS: [(f64, Xf); 5] = [(0.0, xf(0.0, 0.0, 0.0)), (14.09, xf(0.0, 0.0, 0.0)), (15.0, SETTLED), (46.59, SETTLED), (50.23, xf(0.0, 0.0, 0.0))];
const TSL: [(f64, Xf); 5] = [(0.0, xf(0.0, 0.0, 0.0)), (22.73, xf(0.0, 0.0, 0.0)), (23.64, SETTLED), (46.59, SETTLED), (48.41, xf(0.0, 0.0, 0.0))];
const TH: [(f64, Xf); 5] = [(0.0, xf(0.0, 0.0, 0.0)), (31.36, xf(0.0, 0.0, 0.0)), (32.27, SETTLED), (46.59, SETTLED), (50.23, xf(0.0, 0.0, 0.0))];
/// The caret's x, art units: it steps along the typed text, then rides to the end of the word.
const CARET_X: [(f64, f32); 11] = [
    (0.0, 333.34),
    (14.09, 333.34),
    (15.0, 373.54),
    (22.73, 373.54),
    (23.64, 410.74),
    (31.36, 410.74),
    (32.27, 450.94),
    (46.59, 450.94),
    (53.41, 434.38),
    (53.41, 434.38),
    (79.55, 589.87),
];

fn caret_x(pct: f64) -> f32 {
    let keys: Vec<(f64, Xf)> = CARET_X.iter().map(|&(p, x)| (p, xf(1.0, x, 0.0))).collect();
    sample(&keys, pct, OUT).tx
}

/// What every piece looks like at `pct` (None = the settled mark), as (id, transform), plus the
/// translation of the whole group.
fn pose(id: &str, pct: f64) -> Xf {
    match id {
        "equa" => sample(&EQUA, pct, OUT),
        "ion" => sample(&ION, pct, OUT),
        "fS" => sample(&FS, pct, SPRING),
        "fH" => sample(&FH, pct, SPRING),
        "bar" => sample(&BAR, pct, SPRING),
        "tS" => sample(&TS, pct, EASE),
        "tSl" => sample(&TSL, pct, EASE),
        "tH" => sample(&TH, pct, EASE),
        _ => SETTLED,
    }
}

/// An outline flattened to closed polylines, art units.
type Poly = Vec<Vec<(f64, f64)>>;

/// The `d` attribute of the element with `id="..."` in the SVG source.
fn path_data(id: &str) -> Option<&'static str> {
    let at = SVG.find(&format!("id=\"{id}\""))?;
    let rest = &SVG[at..];
    // The first d="..." after the id belongs to that element (the group wraps one path).
    let d = rest.find(" d=\"")? + 4;
    let end = rest[d..].find('"')?;
    Some(&rest[d..d + end])
}

/// Parse absolute `M L H V Q C Z` path data, flattening curves.
fn parse_path(d: &str) -> Option<Poly> {
    let mut polys: Poly = Vec::new();
    let mut cur: Vec<(f64, f64)> = Vec::new();
    let mut pos = (0.0_f64, 0.0_f64);
    let mut start = pos;
    let mut cmd = b'M';
    let b = d.as_bytes();
    let mut i = 0;
    let mut nums: Vec<f64> = Vec::new();
    let arity = |c: u8| match c {
        b'M' | b'L' => 2,
        b'H' | b'V' => 1,
        b'Q' => 4,
        b'C' => 6,
        _ => 0,
    };
    fn flush(polys: &mut Poly, cur: &mut Vec<(f64, f64)>) {
        if cur.len() >= 3 {
            polys.push(std::mem::take(cur));
        } else {
            cur.clear();
        }
    }
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_alphabetic() {
            if c == b'Z' || c == b'z' {
                flush(&mut polys, &mut cur);
                pos = start;
            } else if arity(c) == 0 {
                return None;
            }
            cmd = c;
            nums.clear();
            i += 1;
            continue;
        }
        if c == b' ' || c == b',' || c == b'\n' || c == b'\t' || c == b'\r' {
            i += 1;
            continue;
        }
        // A number: sign, digits, one dot, optional exponent.
        let s = i;
        if b[i] == b'-' || b[i] == b'+' {
            i += 1;
        }
        let mut dot = false;
        while i < b.len() && (b[i].is_ascii_digit() || (b[i] == b'.' && !dot)) {
            if b[i] == b'.' {
                dot = true;
            }
            i += 1;
        }
        if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
            i += 1;
            if i < b.len() && (b[i] == b'-' || b[i] == b'+') {
                i += 1;
            }
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
        }
        nums.push(d[s..i].parse().ok()?);
        let n = arity(cmd);
        if n == 0 || nums.len() < n {
            continue;
        }
        match cmd {
            b'M' => {
                flush(&mut polys, &mut cur);
                pos = (nums[0], nums[1]);
                start = pos;
                cur.push(pos);
                cmd = b'L';
            }
            b'L' => {
                pos = (nums[0], nums[1]);
                cur.push(pos);
            }
            b'H' => {
                pos.0 = nums[0];
                cur.push(pos);
            }
            b'V' => {
                pos.1 = nums[0];
                cur.push(pos);
            }
            b'Q' => {
                let (p0, p1, p2) = (pos, (nums[0], nums[1]), (nums[2], nums[3]));
                for k in 1..=10 {
                    let t = f64::from(k) / 10.0;
                    let u = 1.0 - t;
                    cur.push((u * u * p0.0 + 2.0 * u * t * p1.0 + t * t * p2.0, u * u * p0.1 + 2.0 * u * t * p1.1 + t * t * p2.1));
                }
                pos = p2;
            }
            b'C' => {
                let (p0, p1, p2, p3) = (pos, (nums[0], nums[1]), (nums[2], nums[3]), (nums[4], nums[5]));
                for k in 1..=12 {
                    let t = f64::from(k) / 12.0;
                    let u = 1.0 - t;
                    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
                    cur.push((w[0] * p0.0 + w[1] * p1.0 + w[2] * p2.0 + w[3] * p3.0, w[0] * p0.1 + w[1] * p1.1 + w[2] * p2.1 + w[3] * p3.1));
                }
                pos = p3;
            }
            _ => return None,
        }
        nums.clear();
    }
    flush(&mut polys, &mut cur);
    (!polys.is_empty()).then_some(polys)
}

fn bounds(polys: &Poly) -> (f64, f64, f64, f64) {
    let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for &(x, y) in polys.iter().flatten() {
        b = (b.0.min(x), b.1.min(y), b.2.max(x), b.3.max(y));
    }
    b
}

/// Non-zero fill coverage of `polys` over a `w` x `h` pixel grid whose top left is art point
/// `origin` and whose pixels are `scale` pixels per art unit. Exact horizontally, `SUBROWS`
/// samples vertically.
fn rasterise(polys: &Poly, origin: (f64, f64), scale: f64, w: usize, h: usize) -> Vec<u8> {
    let mut edges: Vec<((f64, f64), (f64, f64))> = Vec::new();
    for p in polys {
        for k in 0..p.len() {
            let a = p[k];
            let b = p[(k + 1) % p.len()];
            let a = ((a.0 - origin.0) * scale, (a.1 - origin.1) * scale);
            let b = ((b.0 - origin.0) * scale, (b.1 - origin.1) * scale);
            if (a.1 - b.1).abs() > f64::EPSILON {
                edges.push((a, b));
            }
        }
    }
    let mut cover = vec![0.0_f32; w * h];
    let mut xs: Vec<(f64, i32)> = Vec::new();
    for y in 0..h {
        for s in 0..SUBROWS {
            let sy = y as f64 + (s as f64 + 0.5) / SUBROWS as f64;
            xs.clear();
            for &(a, b) in &edges {
                let (lo, hi) = if a.1 < b.1 { (a.1, b.1) } else { (b.1, a.1) };
                if sy >= lo && sy < hi {
                    let x = a.0 + (sy - a.1) / (b.1 - a.1) * (b.0 - a.0);
                    xs.push((x, if b.1 > a.1 { 1 } else { -1 }));
                }
            }
            xs.sort_by(|p, q| p.0.total_cmp(&q.0));
            let mut wind = 0;
            for k in 0..xs.len().saturating_sub(1) {
                wind += xs[k].1;
                if wind == 0 {
                    continue;
                }
                let (x0, x1) = (xs[k].0.max(0.0), xs[k + 1].0.min(w as f64));
                if x1 <= x0 {
                    continue;
                }
                let (c0, c1) = (x0.floor() as usize, (x1.ceil() as usize).min(w));
                for px in c0..c1 {
                    let l = (px as f64).max(x0);
                    let r = ((px + 1) as f64).min(x1);
                    if r > l {
                        cover[y * w + px] += ((r - l) / SUBROWS as f64) as f32;
                    }
                }
            }
        }
    }
    cover.into_iter().map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8).collect()
}

/// A rasterised piece: its mask and where it sits in the art.
struct Mask {
    id: &'static str,
    tex: TextureHandle,
    /// The art-unit rectangle the texture covers.
    art: Rect,
}

struct Raster {
    /// Pixels per point it was rasterised for.
    ppp: f32,
    masks: Vec<Mask>,
}

/// The wordmark: outlines parsed once, masks rasterised for the pixel density in use.
pub struct Logo {
    outlines: Vec<(&'static str, Poly)>,
    raster: Option<Raster>,
    /// When the animation (re)started, page time. None before the first frame it is on screen.
    start: Option<f64>,
    hovered: bool,
}

impl Default for Logo {
    fn default() -> Self {
        Self::new()
    }
}

impl Logo {
    pub fn new() -> Self {
        let outlines = OUTLINES.iter().filter_map(|id| path_data(id).and_then(parse_path).map(|p| (*id, p))).collect::<Vec<_>>();
        let outlines = if outlines.len() == OUTLINES.len() { outlines } else { Vec::new() };
        Logo { outlines, raster: None, start: None, hovered: false }
    }

    /// The outlines were found in the art (otherwise the header falls back to plain text).
    pub fn available(&self) -> bool {
        !self.outlines.is_empty()
    }

    /// Play the animation again from the start.
    pub fn replay(&mut self) {
        self.start = None;
    }

    /// Show the settled word from the first frame (the animation already played elsewhere).
    pub fn settle(&mut self) {
        self.start = Some(-DURATION);
    }

    fn ensure_raster(&mut self, ctx: &Context) {
        let ppp = ctx.pixels_per_point();
        if self.raster.as_ref().is_some_and(|r| (r.ppp - ppp).abs() < 1e-3) {
            return;
        }
        let scale = f64::from(SCALE) * f64::from(ppp);
        let masks = self
            .outlines
            .iter()
            .map(|(id, polys)| {
                let (x0, y0, x1, y1) = bounds(polys);
                // One pixel of margin so edge coverage is not clipped.
                let pad = 1.0 / scale;
                let (ox, oy) = (x0 - pad, y0 - pad);
                let w = (((x1 - x0) * scale).ceil() as usize + 3).max(1);
                let h = (((y1 - y0) * scale).ceil() as usize + 3).max(1);
                let alpha = rasterise(polys, (ox, oy), scale, w, h);
                let pixels = alpha.into_iter().map(Color32::from_white_alpha).collect();
                let tex = ctx.load_texture(format!("equashion-logo-{id}"), ColorImage::new([w, h], pixels), TextureOptions::LINEAR);
                let art = Rect::from_min_size(pos2(ox as f32, oy as f32), vec2((w as f64 / scale) as f32, (h as f64 / scale) as f32));
                Mask { id, tex, art }
            })
            .collect();
        self.raster = Some(Raster { ppp, masks });
    }

    /// Lay the word out in a `SLOT_W` x `SLOT_H` slot and paint it. `reduced_motion` shows the
    /// settled mark with no caret.
    pub fn ui(&mut self, ui: &mut Ui, p: &Palette, reduced_motion: bool) -> Response {
        let (slot, resp) = ui.allocate_exact_size(vec2(SLOT_W, SLOT_H), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, "equaSHion"));
        if !self.available() {
            ui.painter().text(slot.left_center(), egui::Align2::LEFT_CENTER, "equaSHion", FontId::proportional(20.0), p.strong);
            return resp;
        }
        let now = ui.input(|i| i.time);
        // A replay on entering the word or tapping it, not on every move inside it.
        let over = resp.hovered();
        let pressed = over && ui.input(|i| i.pointer.any_pressed());
        if (over && !self.hovered) || pressed {
            self.start = None;
        }
        self.hovered = over;
        let start = *self.start.get_or_insert(now);
        let t = if reduced_motion { DURATION } else { (now - start).max(0.0) };
        let pct = (t / DURATION * 100.0).min(100.0);
        self.ensure_raster(ui.ctx());

        let dark = ui.visuals().dark_mode;
        let (ink, acc) = if dark { (INK_DARK, ACC_DARK) } else { (INK_LIGHT, ACC_LIGHT) };
        // The art's top left, so the settled word starts at the slot's left edge, centred.
        let origin = pos2(slot.min.x - WORD_LEFT * SCALE, slot.center().y - WORD_MID_Y * SCALE);
        let to_screen = |x: f32, y: f32| pos2(origin.x + x * SCALE, origin.y + y * SCALE);
        let all = sample(&ALL, pct, OUT).tx;
        let painter = ui.painter();

        if let Some(r) = &self.raster {
            for id in PIECES {
                let m = pose(id, pct);
                if m.opacity <= 0.0 {
                    continue;
                }
                let colour = if matches!(id, "fS" | "fH" | "bar") { acc } else { ink }.gamma_multiply(m.opacity);
                if id == "bar" {
                    let c = (BAR_X + BAR_W / 2.0, BAR_Y + BAR_H / 2.0);
                    let (sin, cos) = m.rot.to_radians().sin_cos();
                    let mut pts = Vec::new();
                    // A rounded rectangle, scaled across then turned about its centre, as the SVG does.
                    for (cx, cy, a0) in [
                        (BAR_X + BAR_W - BAR_R, BAR_Y + BAR_R, -90.0_f32),
                        (BAR_X + BAR_W - BAR_R, BAR_Y + BAR_H - BAR_R, 0.0),
                        (BAR_X + BAR_R, BAR_Y + BAR_H - BAR_R, 90.0),
                        (BAR_X + BAR_R, BAR_Y + BAR_R, 180.0),
                    ] {
                        for k in 0..=6 {
                            let a = (a0 + 90.0 * k as f32 / 6.0).to_radians();
                            let (px, py) = (cx + BAR_R * a.cos(), cy + BAR_R * a.sin());
                            let (dx, dy) = ((px - c.0) * m.sx, py - c.1);
                            let (rx, ry) = (dx * cos - dy * sin, dx * sin + dy * cos);
                            pts.push(to_screen(c.0 + rx + m.tx + all, c.1 + ry + m.ty));
                        }
                    }
                    painter.add(PathShape::convex_polygon(pts, colour, Stroke::NONE));
                    continue;
                }
                let Some(mask) = r.masks.iter().find(|k| k.id == id) else { continue };
                let min = to_screen(mask.art.min.x + m.tx + all, mask.art.min.y + m.ty);
                let rect = Rect::from_min_size(min, mask.art.size() * SCALE);
                let mut mesh = Mesh::with_texture(mask.tex.id());
                let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                let i = mesh.vertices.len() as u32;
                for (pt, uvp) in [
                    (rect.left_top(), uv.left_top()),
                    (rect.right_top(), uv.right_top()),
                    (rect.right_bottom(), uv.right_bottom()),
                    (rect.left_bottom(), uv.left_bottom()),
                ] {
                    mesh.vertices.push(Vertex { pos: pt, uv: uvp, color: colour });
                }
                mesh.indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
                painter.add(mesh);
            }
        }

        // The caret: on for the first 61% of each blink period, hidden under reduced motion.
        if !reduced_motion {
            let on = (t % BLINK) / BLINK < BLINK_ON;
            if on {
                let x = caret_x(pct) + all;
                let r = Rect::from_min_max(to_screen(x, CARET_Y), to_screen(x + CARET_W, CARET_Y + CARET_H));
                painter.rect_filled(r, 0.0, acc);
            }
            // Next frame: every frame while the timeline runs, then at the next blink edge.
            let ctx = ui.ctx();
            if t < DURATION {
                ctx.request_repaint_after(std::time::Duration::from_millis(16));
            } else {
                let phase = (t % BLINK) / BLINK;
                let edge = if phase < BLINK_ON { BLINK_ON } else { 1.0 };
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(((edge - phase) * BLINK).max(0.02)));
            }
        }
        resp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_art_has_every_outline() {
        for id in OUTLINES {
            let polys = path_data(id).and_then(parse_path).unwrap_or_default();
            assert!(!polys.is_empty(), "{id} has no outline");
        }
        assert!(path_data("bar").is_some());
    }

    #[test]
    fn a_square_is_filled_and_a_hole_is_not() {
        // Outer 10 x 10, inner 4 x 4 wound the other way.
        let outer = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let inner = vec![(3.0, 3.0), (3.0, 7.0), (7.0, 7.0), (7.0, 3.0)];
        let px = rasterise(&vec![outer, inner], (0.0, 0.0), 1.0, 10, 10);
        assert_eq!(px[0], 255);
        assert_eq!(px[5 * 10 + 5], 0);
        assert_eq!(px[9 * 10 + 9], 255);
    }

    #[test]
    fn the_timeline_ends_settled() {
        for id in PIECES {
            let m = pose(id, 100.0);
            let want = if matches!(id, "tS" | "tSl" | "tH") { 0.0 } else { 1.0 };
            assert!((m.opacity - want).abs() < 1e-3, "{id}");
            assert!(m.tx.abs() < 1e-3 && m.ty.abs() < 1e-3, "{id}");
        }
        assert!((caret_x(100.0) - 589.87).abs() < 1e-3);
    }

    #[test]
    fn easing_runs_from_zero_to_one() {
        for e in [OUT, SPRING, EASE] {
            assert!(e.at(0.0).abs() < 1e-6);
            assert!((e.at(1.0) - 1.0).abs() < 1e-6);
        }
    }
}
