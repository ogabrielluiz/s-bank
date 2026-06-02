//! Phase 1 acceptance: the audio path's cutoff tracks the vactrol resistance.
//!
//! Drives an impulse through the audio path at a fixed Rf, takes the magnitude
//! spectrum of the impulse response, and finds the -3 dB cutoff. Closing the gate
//! (large Rf) must lower the cutoff -- the "duller as quieter" behaviour. This is
//! a qualitative match to the paper's figures, not a bit-exact reference.

use realfft::RealFftPlanner;
use vactrol_core::audio_path::AudioPath;
use vactrol_core::params::{Components, Mode, Params};

const SR: f32 = 48_000.0;
const N: usize = 8192;

/// -3 dB cutoff (Hz) of the audio path's impulse response at a fixed Rf.
fn ir_cutoff(rf: f32) -> f32 {
    let comp = Components::default();
    let mut ap = AudioPath::new(SR);

    // Impulse response (linear: drive = 0, resonance = 0, pure Lowpass mode), so
    // the nonlinear/oversampling stage is bypassed and we see the plain SVF.
    let params = Params {
        mode: Mode::Lowpass,
        resonance: 0.0,
        cv_offset: 0.0,
        drive: 0.0,
        oversample: 1,
    };
    let mut ir = vec![0.0f32; N];
    for (i, s) in ir.iter_mut().enumerate() {
        let x = if i == 0 { 1.0 } else { 0.0 };
        *s = ap.process(x, rf, &params, &comp);
    }

    let mut planner = RealFftPlanner::<f32>::new();
    let r2c = planner.plan_fft_forward(N);
    let mut indata = r2c.make_input_vec();
    indata.copy_from_slice(&ir);
    let mut spectrum = r2c.make_output_vec();
    r2c.process(&mut indata, &mut spectrum).unwrap();

    let dc = spectrum[0].norm();
    // -3 dB relative to the DC/passband level.
    let threshold = dc * 0.70794576; // 10^(-3/20)
    for (k, bin) in spectrum.iter().enumerate().skip(1) {
        if bin.norm() <= threshold {
            return k as f32 * SR / N as f32;
        }
    }
    SR / 2.0
}

#[test]
fn cutoff_decreases_as_gate_closes() {
    let open = ir_cutoff(1_000.0); // bright / open (low Rf)
    let closed = ir_cutoff(10_000_000.0); // dull / nearly dark (off-resistance)

    // The physics: closing the gate drops the cutoff by well over a decade.
    assert!(
        open > closed * 10.0,
        "cutoff should drop sharply as Rf grows: open={open:.1} Hz, closed={closed:.1} Hz"
    );
    assert!(
        closed < 500.0,
        "near-dark cutoff should be low (Hz): {closed:.1}"
    );
    assert!(
        open > 5_000.0,
        "open-gate cutoff should be bright (Hz): {open:.1}"
    );
}

#[test]
fn cutoff_is_monotonic_in_rf() {
    let cuts: Vec<f32> = [1_000.0, 10_000.0, 100_000.0, 1_000_000.0]
        .iter()
        .map(|&rf| ir_cutoff(rf))
        .collect();
    for w in cuts.windows(2) {
        assert!(
            w[0] >= w[1],
            "cutoff must not increase as Rf increases: {cuts:?}"
        );
    }
}
