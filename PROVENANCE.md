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

Filled in phase by phase; see the sections below.

## Licence

WordCraft at `dbfe1b1` is `MIT OR Apache-2.0` (`license` field of its workspace `Cargo.toml`,
`LICENSE-MIT`, `LICENSE-APACHE`, `NOTICE`). `LICENSE-MIT` and `LICENSE-APACHE` are copied
unchanged; they keep the upstream copyright line ("ArtCraft Team and the WordCraft
contributors"), which both licences require to be preserved. `NOTICE` is rewritten for this
repository: it keeps that copyright line, adds "Modified by shuff57, 2026", and lists only the
third-party material bundled here.
