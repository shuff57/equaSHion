//! The equaSHion web showcase: one egui app with five tabs (Editor, Gallery, OMML, Selftest, About).
//!
//! Everything but the browser shell ([`web`], wasm only) is plain egui, so the native tests
//! drive the same [`Showcase`] with egui_kittest.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod corpus;
pub mod selftest;
mod showcase;
pub mod theme;

#[cfg(target_arch = "wasm32")]
pub mod web;

pub use showcase::{CREDIT, GALLERY_GROUPS, OmmlStatus, Showcase, TAGLINE, Tab};
pub use theme::ThemePref;
