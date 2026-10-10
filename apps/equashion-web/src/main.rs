//! equaSHion in the browser: `cargo xtask web` builds it with trunk into `dist/`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(target_arch = "wasm32")]
fn main() {
    equashion_web::web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("equashion-web only runs in the browser: `cargo xtask web` builds it, `cargo xtask serve` serves it");
}
