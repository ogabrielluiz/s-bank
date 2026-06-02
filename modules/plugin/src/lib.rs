//! S-Bank VCV Rack plugin core.
//!
//! This crate is the single staticlib the C++ Rack adapter links. It depends on the
//! S-Bank **components** (`vactrol-core`, `strike-core`, …) — which are pure DSP rlibs
//! with no C ABI of their own — and re-exports each one over a flat C boundary. Keeping
//! all the `extern "C"` here means the components stay clean and reusable, and the
//! plugin bundles them behind one Rust runtime (two Rust staticlibs can't co-link into
//! a single plugin).
//!
//! C ABI surface:
//! - `vactrol_lpg_*` — the Buchla-292 vactrol low-pass gate (`ffi_vactrol`).
//! - `strike_*` — the clean EG-driven gate (`ffi_strike`).

pub mod ffi_strike;
pub mod ffi_vactrol;
