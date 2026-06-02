//! Phase 1 acceptance: the topology-preserving core stays finite and bounded
//! under fast modulation and pathological inputs -- the property the direct-form
//! bilinear transfer function lacks (it diverges under fast modulation). This is
//! the single result that de-risks the library thesis.

use std::f32::consts::TAU;
use vactrol_core::{Lpg, Mode, Params};

const SR: f32 = 48_000.0;

#[test]
fn finite_under_fast_modulation() {
    let mut lpg = Lpg::new(SR);
    for mode in [Mode::Both, Mode::Vca, Mode::Lowpass] {
        lpg.reset();
        lpg.set_params(Params {
            mode,
            resonance: 1.0, // near the self-oscillation boundary
            cv_offset: 0.0,
            drive: 4.0,
        });

        let n = SR as usize * 2;
        let mut max_abs = 0.0f32;
        for i in 0..n {
            let t = i as f32 / SR;
            // Audio-rate CV modulation to the rails plus a harsh audio input.
            let cv = 10.0 * (TAU * 1_000.0 * t).sin();
            let audio = (TAU * 3_000.0 * t).sin() * if i % 2 == 0 { 1.0 } else { -1.0 };
            let y = lpg.process_sample(audio, cv);
            assert!(
                y.is_finite(),
                "non-finite output at sample {i} in mode {mode:?}"
            );
            max_abs = max_abs.max(y.abs());
        }
        assert!(
            max_abs < 100.0,
            "output blew up (max |y| = {max_abs}) in mode {mode:?}"
        );
    }
}

/// Maps a sample index to an `(audio, cv)` input pair.
type InputGen = fn(usize) -> (f32, f32);

#[test]
fn pathological_inputs_stay_finite() {
    let inputs: &[(&str, InputGen)] = &[
        ("silence", |_| (0.0, 0.0)),
        ("dc_audio", |_| (1.0, 5.0)),
        ("rail_cv", |_| (1.0, 1.0e3)),
        ("audio_impulse", |i| (if i == 0 { 1.0 } else { 0.0 }, 5.0)),
        ("cv_impulse", |i| (1.0, if i == 0 { 1.0e3 } else { 0.0 })),
        ("max_audio", |_| (1.0e3, 5.0)),
        ("alternating", |i| {
            (if i % 2 == 0 { 1.0 } else { -1.0 }, 5.0)
        }),
    ];

    for (name, f) in inputs {
        let mut lpg = Lpg::new(SR);
        lpg.set_params(Params {
            mode: Mode::Both,
            resonance: 0.9,
            cv_offset: 0.0,
            drive: 2.0,
        });
        for i in 0..SR as usize {
            let (a, c) = f(i);
            let y = lpg.process_sample(a, c);
            assert!(
                y.is_finite(),
                "non-finite output for input '{name}' at sample {i}"
            );
        }
    }
}

#[test]
fn decays_to_silence_without_drift() {
    let mut lpg = Lpg::new(SR);
    lpg.set_params(Params::default());
    // Excite, then feed silence and assert the output settles near zero.
    for i in 0..SR as usize {
        let cv = if i < 240 { 8.0 } else { 0.0 };
        lpg.process_sample((i as f32 * 0.1).sin(), cv);
    }
    // Feed a full second of silence; the gate closes and the filter rings down.
    let n = SR as usize;
    let tail: Vec<f32> = (0..n).map(|_| lpg.process_sample(0.0, 0.0).abs()).collect();
    // Assert the *settled* portion (final 100 ms) is silent, and that it is
    // strictly below the early tail (decaying, not a limit cycle).
    let settle_start = n - (SR * 0.1) as usize;
    let settled_max = tail[settle_start..].iter().cloned().fold(0.0f32, f32::max);
    let early_max = tail[..(SR * 0.1) as usize]
        .iter()
        .cloned()
        .fold(0.0f32, f32::max);
    assert!(
        settled_max < 1.0e-6,
        "expected settling to silence, got {settled_max}"
    );
    assert!(settled_max < early_max, "tail should decay, not sustain");
}
