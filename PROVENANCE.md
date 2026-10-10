# Provenance

equaSHion is the equation editor extracted from WordCraft, with three edit-model tests taken
from DeckCraft. It has its own git history: the files were imported with `git archive` from the
commits below, not with `git filter-repo`, so the upstream history is not carried over.

## Sources

| Repository | Commit | Branch | Role |
|---|---|---|---|
| `shuff57/wordcraft` (fork of `storytold/wordcraft`) | `dbfe1b1` | `equation-editor` | Base implementation: math tree, OMML, linear format, templates, edit model, equation layout, font database, rasteriser |
| `shuff57/deckcraft` | `7972cf5` | — | Three edit-model tests only (see below) |

Every file was read from those commits with
`git -C <repo> archive <commit> <paths> | tar -x -C <scratch>`, never from a working tree.

## What was taken

### equashion-math (`crates/math`) <- WordCraft `crates/math`

Taken whole: `lib.rs`, `edit.rs`, `edit/tests.rs`, `linear.rs`, `omml.rs`, `templates.rs`,
`tests.rs`, `xml.rs`, `Cargo.toml` (dependencies `serde`, `quick-xml`).

Changed:
- Package renamed `wordcraft-math` -> `equashion-math`; the crate doc comment no longer names
  WordCraft.
- `edit.rs`: DeckCraft's exponent rules (see DIVERGENCE.md).
- `edit/tests.rs`: three tests appended from DeckCraft `7972cf5`
  `crates/math/src/edit/tests.rs`: `a_single_digit_exponent_ends_at_the_next_term`,
  `letters_keep_going_in_an_exponent`, `a_parenthesised_exponent_is_the_group_itself`. Nothing
  else was taken from DeckCraft.

### equashion-font (`crates/font`) <- WordCraft `crates/fonts` (trimmed)

Taken: `src/fontdb.rs` (`FontFace`, `FontDb`, outlines, `fallback_for`, the system font catalog
behind `cfg(not(target_arch = "wasm32"))`), and from `src/lib.rs` `shape`, `feature`, `is_rtl`,
`first_glyph`, `ShapedGlyph` and `DEFAULT_FAMILY`, with the tests that only need bundled fonts.

Trimmed:
- `word.rs` (Word font-name resolution), `build.rs` and the craft-fonts Japanese/Chinese fonts
  (`CraftFont`, `japanese_*_fonts`, `chinese_fonts`, `ui_cjk_fonts`), `tests_sysfonts.rs`,
  `tests/system_fonts.rs`, the `variable_font_named_instances_are_styles` test (it read fonts
  from the system font folders) and the two craft-fonts tests.
- `FaceRef` (a `&'static` handle made by leaking an `Arc` per face). Scenes name faces with the
  new `FaceId(u32)` (the face's process-unique id) and backends resolve it with the new
  `FontDb::face_by_id`.
- Bundled fonts: Source Serif 4 Regular/Italic/Bold/BoldItalic and Inter Regular, byte-identical
  copies of WordCraft's `assets/fonts/` files, with their OFL texts. Source Sans 3, Source Serif
  4 Semibold, Inter Medium/SemiBold and JetBrains Mono are not bundled.
- `FALLBACK_FAMILY` (an unknown family's replacement and the first fallback face) was "Source
  Sans 3"; it is now `DEFAULT_FAMILY`, "Source Serif 4". The last-resort face is Source Serif 4
  Regular.

Added: `FontDb::bundled_only()` (no font folders, system fallback off), `FaceId`,
`FontFace::face_id`, `FontDb::face_by_id`, and tests that pin the bundled family list and the
default family.

### equashion-layout (`crates/layout`) <- WordCraft `crates/layout/src/math.rs`

Taken: all of `math.rs` (`MathBox`, `SeqGeo`, `Lay`, `EqLayout`, `hit_geo`, `layout_equation`,
`layout_math_wrapped`, `MathBox::last_line_width`, the drawn delimiters, radicals, big
operators and braces, line wrapping and shrink-to-fit) and its 32 unit tests. Nothing else from
WordCraft's layout crate: `para.rs`, `display.rs`, `hit.rs`, `table.rs`, `fields.rs` and the
paragraph engine in `lib.rs` stay out.

Host coupling cut, at the smallest seam:
- `wordcraft_doc::props::Rgb` -> a local `Rgb(pub u8, pub u8, pub u8)` (`scene.rs`).
- `wordcraft_geom::Rect` (used only to emit fraction bars as `Draw::Fill`) -> `kurbo::Rect`.
- `crate::display::Draw` -> the new backend-neutral `Scene { items: Vec<Prim>, width, height }`
  with `Prim::{Glyphs, Rule, Path}`, mirroring `math.rs`'s own `Item`. `emit()` lost its `alpha`
  argument and the `Draw::Glyphs` fields `text` and `link`, which equations never set.
  `EqLayout { draws, width, height }` became `EqLayout { scene }` with `width()` and
  `height()`.
- `FontDb::global()` (5 call sites: `Faces::new`, `ink`, `Lay::pick`, `layout_equation`, and
  `layout_math`'s caller) and `wordcraft_fonts::shape` -> an explicit `&dyn MathFonts`
  argument, a trait implemented for `FontDb`. The trait has `face`, `fallback_for`, `outline`
  as specified plus `face_by_id`, which a backend needs to draw a `Prim::Glyphs`.
- `layout_math` and `layout_math_wrapped` took the base `&FontFace` only to read its family;
  they now take the family name.
- The process-wide `CACHE` static behind `layout_cached` -> an optional, host-owned
  `LayoutCache` (`LayoutCache::layout`); `layout_src` is the uncached form. `Src` and `Cached`
  are public.

The only reach into paragraph layout was `para.rs` calling `layout_cached` and storing the
`MathBox`; nothing in `math.rs` called into `para.rs` or `display.rs` apart from `Draw`.

Tests: the 32 `math.rs` unit tests, with the same assertions, now laid out against
`FontDb::bundled_only()` in Inter as upstream. `layout_cache_returns_the_same_box` and
`emit_places_glyphs_relative_to_origin` were rewritten for `LayoutCache` and `Prim`. From
`layout/src/tests.rs`, the equation tests were rewritten to call `layout_equation` /
`layout_math_wrapped` directly (`src/tests.rs`): taller-than-text with glyphs, bar and radical
(plus the caret-width check that replaced the paragraph cluster check), hostile and broken
OMML input, wrapping at operators, display style. Not ported, because they test the paragraph
rather than the equation: `display_equation_is_centred_on_its_own_line`,
`line_before_a_display_equation_is_not_justified`, and the line-padding half of
`display_equation_line_has_room_above_and_below`.

## Licence

WordCraft at `dbfe1b1` is `MIT OR Apache-2.0` (`license` field of its workspace `Cargo.toml`,
`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`). `LICENSE-MIT` and `LICENSE-APACHE` are copied
unchanged; they keep the upstream copyright line ("ArtCraft Team and the WordCraft
contributors"), which both licences require to be preserved. `NOTICE` is rewritten for this
repository: it keeps that copyright line, adds "Modified by shuff57, 2026", and lists only the
third-party material bundled here.
