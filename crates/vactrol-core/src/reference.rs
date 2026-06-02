//! Deterministic reference renders for golden-file regression testing.
//!
//! Shared by the harness (`bless` writes these to `testdata/golden/`) and the
//! correctness test (which compares fresh renders against the committed goldens
//! with tolerance, never bit-exact). Keeping the case definitions here means the
//! generator and the checker can never drift apart.

use crate::{ImperfectionConfig, Lpg, Mode, Params};

/// Sample rate for all reference renders.
pub const SAMPLE_RATE: f32 = 48_000.0;
/// Length of each reference buffer (100 ms).
pub const CASE_LEN: usize = 4_800;
/// Fixed seed for the imperfection reference case.
pub const REF_SEED: u64 = 0x00C0_FFEE;

/// All golden case names.
pub fn case_names() -> &'static [&'static str] {
    &["pluck_both", "vca_tone", "lowpass_sweep", "imperfection_on"]
}

/// Render a named case to a fresh buffer. Panics on an unknown name.
pub fn render(name: &str) -> Vec<f32> {
    match name {
        "pluck_both" => render_pluck(),
        "vca_tone" => render_vca_tone(),
        "lowpass_sweep" => render_lowpass_sweep(),
        "imperfection_on" => render_imperfection(),
        other => panic!("unknown reference case: {other}"),
    }
}

fn sine(i: usize, hz: f32) -> f32 {
    (std::f32::consts::TAU * hz * i as f32 / SAMPLE_RATE).sin()
}

/// Both-mode pluck: a short CV gate into a 220 Hz tone.
fn render_pluck() -> Vec<f32> {
    let mut lpg = Lpg::new(SAMPLE_RATE);
    lpg.set_params(Params {
        mode: Mode::Both,
        resonance: 0.3,
        cv_offset: 0.0,
        drive: 1.0,
        oversample: 2,
        adaa: true,
    });
    let gate = (SAMPLE_RATE * 0.005) as usize;
    (0..CASE_LEN)
        .map(|i| {
            let cv = if i < gate { 8.0 } else { 0.0 };
            lpg.process_sample(sine(i, 220.0), cv)
        })
        .collect()
}

/// VCA steady tone with the gate held open.
fn render_vca_tone() -> Vec<f32> {
    let mut lpg = Lpg::new(SAMPLE_RATE);
    lpg.set_params(Params {
        mode: Mode::Vca,
        resonance: 0.0,
        cv_offset: 0.0,
        drive: 2.0,
        oversample: 2,
        adaa: true,
    });
    (0..CASE_LEN)
        .map(|i| lpg.process_sample(sine(i, 1_000.0), 8.0))
        .collect()
}

/// Lowpass mode with a rising CV ramp through a 500 Hz tone.
fn render_lowpass_sweep() -> Vec<f32> {
    let mut lpg = Lpg::new(SAMPLE_RATE);
    lpg.set_params(Params {
        mode: Mode::Lowpass,
        resonance: 0.5,
        cv_offset: 0.0,
        drive: 1.0,
        oversample: 2,
        adaa: true,
    });
    (0..CASE_LEN)
        .map(|i| {
            let cv = 8.0 * i as f32 / CASE_LEN as f32;
            lpg.process_sample(sine(i, 500.0), cv)
        })
        .collect()
}

/// Both-mode pluck with the imperfection layer enabled (fixed seed).
fn render_imperfection() -> Vec<f32> {
    let mut lpg = Lpg::with_seed(SAMPLE_RATE, REF_SEED);
    lpg.set_params(Params {
        mode: Mode::Both,
        resonance: 0.3,
        cv_offset: 0.0,
        drive: 1.0,
        oversample: 2,
        adaa: true,
    });
    lpg.set_imperfection(ImperfectionConfig {
        enabled: true,
        ..Default::default()
    });
    let gate = (SAMPLE_RATE * 0.005) as usize;
    (0..CASE_LEN)
        .map(|i| {
            let cv = if i < gate { 8.0 } else { 0.0 };
            lpg.process_sample(sine(i, 220.0), cv)
        })
        .collect()
}
