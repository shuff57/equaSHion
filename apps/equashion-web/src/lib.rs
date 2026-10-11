//! The equaSHion web page: one egui app with the equation editor, a symbol palette, Copy
//! buttons (LaTeX, MathML, image; OMML, text and SVG behind "More"), a paste-in field and examples.
//!
//! Everything but the browser shell ([`web`], wasm only) is plain egui, so the native tests
//! drive the same [`Showcase`] with egui_kittest.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod corpus;
pub mod export;
pub mod palette;
pub mod report;
pub mod selftest;
mod showcase;
pub mod theme;

#[cfg(target_arch = "wasm32")]
pub mod web;

pub use export::CopyKind;
pub use report::{Kind as ReportKind, Shot, Submission};
pub use showcase::{COPY_GUIDE, CREDIT, CopyRequest, EXAMPLES, HOW_TO_TYPE, MATHLIVE_URL, Outcome, PASTE_LABEL, REPORT_LABEL, Showcase, TAGLINE};
pub use theme::ThemePref;
