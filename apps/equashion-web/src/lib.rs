//! The equaSHion web showcase: one egui app with four tabs (Editor, Gallery, OMML, Selftest).
//!
//! Everything but the browser shell ([`web`], wasm only) is plain egui, so the native tests
//! drive the same [`Showcase`] with egui_kittest.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

pub mod corpus;
pub mod selftest;
mod showcase;

#[cfg(target_arch = "wasm32")]
pub mod web;

pub use showcase::{Showcase, Tab};
