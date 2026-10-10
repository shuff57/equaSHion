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
| `xtask` | `xtask` | std | `cargo xtask check`, `cargo xtask bless` |

```rust
use equashion_layout::{DEFAULT_FAMILY, FontDb, Rgb, layout_equation};

let fonts = FontDb::bundled_only();
let m = equashion_math::from_linear(r"x=\frac{-b\pm\sqrt{b^2-4ac}}{2a}");
let eq = layout_equation(&fonts, &m, DEFAULT_FAMILY, 24.0, true, Rgb::BLACK, 4.0);
let svg = equashion_render::scene_to_svg(&eq.scene, &fonts, DEFAULT_FAMILY);
let pixels = equashion_render::render_eq(&eq, &fonts, 2.0);
```

`equashion-layout` and `equashion-render` build for `wasm32-unknown-unknown`.

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

- `equashion-egui`: an egui widget (WordCraft's `eqdialog.rs` has not been ported).
- `equashion-wasm` and a web page.
- A bundled math symbol font. Source Serif 4 and Inter lack ∇, ∮, ⌈⌉⌊⌋ and ⟨⟩; with
  `FontDb::bundled_only()` these draw as .notdef boxes (see DIVERGENCE.md). `FontDb::global()`
  (native only) falls back to system fonts, and `FontDb::add_font` takes any font.
- PNG/JPEG encoding of rendered pixels (WordCraft's used the `image` crate; not carried over).

## Licence

MIT OR Apache-2.0, as WordCraft. See [LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)
and [NOTICE](NOTICE). The bundled fonts are under the SIL Open Font License 1.1
([ATTRIBUTION.md](ATTRIBUTION.md)).
