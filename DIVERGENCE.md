# Divergence from WordCraft dbfe1b1

Behaviour that differs from the imported WordCraft code, and why. Structural changes (the
`Scene` seam, the `MathFonts` trait, the trimmed font crate) are in PROVENANCE.md; this file is
about what a user would notice.

## Edit model: exponent rules from DeckCraft 7972cf5

Three DeckCraft tests were ported into `crates/math/src/edit/tests.rs`. All three failed against
WordCraft's `edit.rs`. In each case DeckCraft's behaviour was chosen, because it matches how
people type the formulas (and MathLive): the exponent stops where a reader would expect it to.
The rules were ported into `edit.rs` from DeckCraft's `edit.rs`. All 134 WordCraft math tests
still pass unchanged.

| Test | WordCraft dbfe1b1 | Now (DeckCraft 7972cf5) |
|---|---|---|
| `a_single_digit_exponent_ends_at_the_next_term` | `mv^2/2` became `mv^(2/2)`; `x^2sin x` became `x^(2 sin x)` | A digits-only exponent ends at the next character that is not a digit, `.`, `^`, `_` or whitespace: `(mv^2)/2`, `x^2 · sin x`. `2^10` still keeps going. |
| `letters_keep_going_in_an_exponent` | `x^n+1` left the exponent at `+` (`x^n + 1`); `e^ipi` stayed `e^(ipi)` | An operator leaves a digits-only exponent (`x^2+1`) but stays in a letter exponent (`x^(n+1)`, `2^(n−1)`). A three-letter run ending in `pi` converts the `pi` (`e^(iπ)`). |
| `a_parenthesised_exponent_is_the_group_itself` | `x^(n+1)` dropped the parentheses but left the caret in the exponent | The parentheses are still dropped, and closing the group also takes the caret out of the exponent. |

What changed in `edit.rs`:

- `type_char_raw`: before handling the character, a caret at the end of a digits-only script
  slot steps out of it unless the character is a digit, `.`, `^`, `_`, whitespace or a control
  character. It is skipped inside a typed LaTeX `{...}` argument (`brace_depth > 0`), so
  `x^{2a}` still works. DeckCraft has no LaTeX brace handling, so this guard is new.
- `leave_func_arg`: an operator leaves a script slot only when the slot holds digits.
- `unwrap_frac_paren`: after unwrapping the group of a script slot, the caret steps out of the
  slot. Fractions, radicals and big-operator limits behave as before.
- `autoconvert`: a three-letter run ending in `pi` converts the `pi`.
- New helpers `slot_is_digits` and `exit_script_slot`, taken from DeckCraft.

Side effect: `x^2 y` used to give `x^(2y)`; it now gives `x^2 y`, because the `y` ends the
digit exponent. The same applies to subscripts (`a_1b` is `a_1 b`, `x_i+1` is `x_(i+1)`).

## Default equation font

WordCraft's equation editor asked for "Cambria Math", which is not bundled. Its font database
then fell back to Source Sans 3 without saying so. equaSHion names its default family,
`equashion_font::DEFAULT_FAMILY` = "Source Serif 4", bundles all four styles of it, and pins
that family in the golden tests. An unknown family still falls back, now to Source Serif 4.
"Cambria Math" is still present in some OMML test data in `crates/math/src/tests.rs`, because
Word writes it there, but it is never a default.
