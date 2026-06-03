//! Dump deterministic Strike reference buffers for C++↔Rust parity checking.
//!
//! `cargo run -q -p strike-core --example parity_dump -- <ping|gated|held>` prints
//! 12000 f32 samples (one per line). The C++ parity test reproduces the same scenarios
//! with the C++ port and compares. Regenerate the goldens with
//! `modules/rack/test/run_parity.sh --bless`.

use strike_core::{Strike, StrikeParams};

const SR: f32 = 48_000.0;
const N: usize = 12_000;

fn sine(i: usize, hz: f32) -> f32 {
    (std::f32::consts::TAU * hz * i as f32 / SR).sin()
}

fn main() {
    let case = std::env::args().nth(1).unwrap_or_default();
    let mut s = Strike::new(SR);
    // (audio_in, ctrl01, decay_mod, hit) per sample i, plus the params for the case.
    let (params, f): (StrikeParams, fn(usize) -> (f32, f32, f32, f32)) = match case.as_str() {
        "ping" => (
            StrikeParams { open: 0.0, decay: 0.5, material: 0.0 },
            |i| (1.0, 0.0, 0.0, if i < 10 { 5.0 } else { 0.0 }),
        ),
        "gated" => (
            StrikeParams { open: 0.0, decay: 0.6, material: 0.0 },
            |i| (sine(i, 220.0), 0.0, 0.0, if i < 10 { 5.0 } else { 0.0 }),
        ),
        "held" => (
            StrikeParams { open: 0.0, decay: 0.5, material: 1.0 },
            |i| (sine(i, 330.0), 1.0, 0.0, 0.0),
        ),
        other => {
            eprintln!("unknown case: {other}");
            std::process::exit(2);
        }
    };
    s.set_params(params);
    for i in 0..N {
        let (a, c, d, h) = f(i);
        println!("{:.9e}", s.process_sample(a, c, d, h));
    }
}
