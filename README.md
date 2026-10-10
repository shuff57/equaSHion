# equaSHion

An equation editor engine in Rust: a math tree with OMML (Word/PowerPoint) import and export
and a typed linear/LaTeX-like input format, an editing model (caret, selection, templates,
undo), TeX-style layout into a backend-neutral scene with caret geometry, and renderers for
that scene (RGBA pixels and SVG).

It is the equation editor extracted from WordCraft at `dbfe1b1`, with three edit-model tests
from DeckCraft at `7972cf5`. See [PROVENANCE.md](PROVENANCE.md) for what was taken and changed,
and [DIVERGENCE.md](DIVERGENCE.md) for behaviour that now differs from WordCraft.

## Crates

| Crate | Path | Depends on | What it does |
|---|---|---|---|
| `equashion-math` | `crates/math` | quick-xml, serde | Math tree, OMML import/export, linear format, templates, edit model (`Editor`, `Caret`, `Key`) |
| `equashion-font` | `crates/font` | skrifa, harfrust, kurbo | Font database (`FontDb`, `FontDb::bundled_only()`), outlines, fallback, shaping; bundles Source Serif 4 (default family) and Inter |
| `equashion-layout` | `crates/layout` | math, font, kurbo | `layout_equation` -> `EqLayout` (a `Scene` plus caret rectangles and hit testing), `layout_math_wrapped`, `LayoutCache`, the `MathFonts` trait |
| `equashion-render` | `crates/render` | layout, vello_cpu | `render_scene` / `render_eq` to premultiplied RGBA, `scene_to_svg` |
| `equashion-egui` | `crates/egui` | math, layout, render, egui | `EquationEditor`: the equation editor as an egui widget (palette, canvas with caret, linear field), no host types |
| `xtask` | `xtask` | std | `cargo xtask check`, `cargo xtask bless` |

```rust
use equashion_layout::{DEFAULT_FAMILY, FontDb, Rgb, layout_equation};

let fonts = FontDb::bundled_only();
let m = equashion_math::from_linear(r"x=\frac{-b\pm\sqrt{b^2-4ac}}{2a}");
let eq = layout_equation(&fonts, &m, DEFAULT_FAMILY, 24.0, true, Rgb::BLACK, 4.0);
let svg = equashion_render::scene_to_svg(&eq.scene, &fonts, DEFAULT_FAMILY);
let pixels = equashion_render::render_eq(&eq, &fonts, 2.0);
```

`equashion-layout`, `equashion-render` and `equashion-egui` build for `wasm32-unknown-unknown`.

## The egui widget

```rust
use equashion_egui::EquationEditor;

let mut editor = EquationEditor::with_linear(r"\frac{a}{b}"); // or ::new(), ::with_omml(xml)
// every frame:
let r = editor.show(ui);
if r.changed   { /* editor.linear(), editor.omml(), editor.math() */ }
if r.committed { /* Enter: keep editor.omml() */ }
if r.cancelled { editor.revert(); /* Esc (confirmed when there were changes) */ }
```

| Item | What it is |
|---|---|
| `EquationEditor::new()`, `with_linear(&str)`, `with_omml(&str)` | Construct (OMML import never fails, so `with_omml` returns `Self`) |
| `.id_salt(h)`, `.fonts(Arc<FontDb>)`, `.family(&str)`, `.font_size(pt)`, `.canvas_height(pt)`, `.show_palette(bool)`, `.show_linear_field(bool)`, `.auto_focus(bool)` | Builder options |
| `show(&mut self, &mut Ui) -> EditorResponse` | Draw palette, canvas, linear field and message line |
| `EditorResponse { changed, committed, cancelled, response, rect }` | `response` is the canvas, `rect` the whole widget |
| `linear()`, `omml()`, `math()`, `unknown_commands()` | Read the equation |
| `set_linear(&str)`, `set_omml(&str)`, `set_math(Math)` | Load (resets the Esc baseline) |
| `revert()`, `is_dirty()`, `mark_clean()`, `message()` | Esc baseline and the status line |
| `editor()`, `editor_mut()` + `sync_linear()` | The underlying `equashion_math::Editor` |
| `canvas_id()`, `linear_id()`, `id()` | egui ids (all derived from the widget id) |

Keys: typing builds the tree (`x^2`, `a/b`, `sqrt`, `alpha`, `\frac{..}{..}`), arrows / Home /
End / Backspace / Delete edit it, Tab and Shift+Tab move between places, Ctrl+Z, Ctrl+Shift+Z
and Ctrl+Y undo and redo, Ctrl+A selects all, Ctrl+C / Ctrl+X copy and cut the selection as
linear text, Enter commits, Esc cancels (the first Esc with changes only asks). A click places
the caret, a drag or Shift+click selects, also across rows.

Shared-context rules the widget keeps (pinned by `crates/egui/tests/shared_context.rs`): it never
calls `set_fonts`, `set_visuals` or `set_style`; it uses only `FontId::proportional` /
`FontId::monospace`; equation and palette glyphs are rasterised by `equashion-render` and drawn
as textures tinted with `ui.visuals()` colours (works in light and dark); texture names start
with `equashion/`; it reads keys only while its canvas has focus and never takes focus from
another widget.

```sh
cargo run -p equashion-egui --example widget            # native window, dark
cargo run -p equashion-egui --example widget -- --light # light theme
```

The example shows the widget with live linear and OMML readouts.

## Checks

```sh
cargo xtask check   # fmt --check, clippy -D warnings, all tests, wasm32 check of layout + render
cargo xtask bless   # rewrite tests/golden/ after an intended layout change (review the diff)
```

The golden tests lay out every item of `corpus/equations.tsv` and `corpus/omml/*.xml` in Source
Serif 4 at 24 pt with the bundled fonts only, and compare the SVG and a metrics snapshot
(`tests/golden/<id>.svg`, `tests/golden/<id>.json`). Property tests over the same corpus check
the OMML and linear round trips and that every caret position has a finite rectangle inside
the canvas.

## Not built yet

- `equashion-wasm` and a web page.
- A bundled math symbol font. Source Serif 4 and Inter lack ∇, ∮, ⌈⌉⌊⌋ and ⟨⟩; with
  `FontDb::bundled_only()` these draw as .notdef boxes (see DIVERGENCE.md). `FontDb::global()`
  (native only) falls back to system fonts, and `FontDb::add_font` takes any font.
- PNG/JPEG encoding of rendered pixels (WordCraft's used the `image` crate; not carried over).

## Licence

MIT OR Apache-2.0, as WordCraft. See [LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)
and [NOTICE](NOTICE). The bundled fonts are under the SIL Open Font License 1.1
([ATTRIBUTION.md](ATTRIBUTION.md)).
