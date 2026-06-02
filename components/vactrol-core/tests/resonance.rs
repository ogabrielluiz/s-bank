//! Resonance behaviour of the Sallen-Key feedback in the ladder model: higher
//! resonance rings longer, and the top of the range self-oscillates but stays
//! bounded (limited by the in-loop tanh, the `amax` behaviour of the real gate).

use vactrol_core::{Lpg, Mode, Params};

const SR: f32 = 48_000.0;
/// Partially-open gate (Rf ~ 49 kΩ) puts the resonant frequency near 7 kHz, well
/// in band. (A fully-open gate sits above Nyquist, so nothing would ring.) With
/// the authors' control circuit this corresponds to a control voltage near 7 V.
const CV_RES: f32 = 7.0;

fn params(resonance: f32) -> Params {
    Params {
        mode: Mode::Lowpass,
        resonance,
        cv_offset: 0.0,
        drive: 1.0,
        oversample: 2,
    }
}

/// Total ring energy following a brief excitation (gate held open). A more
/// resonant cell rings longer, so it integrates more energy. Measured from just
/// after the excitation over a generous window, so it captures decay *length*
/// rather than the level at one late instant.
fn ring_energy(resonance: f32) -> f64 {
    let mut lpg = Lpg::new(SR);
    lpg.set_params(params(resonance));
    for _ in 0..(SR as usize / 4) {
        lpg.process_sample(0.0, CV_RES);
    }
    // Brief impulse-like excitation.
    for i in 0..16 {
        lpg.process_sample(if i == 0 { 1.0 } else { 0.0 }, CV_RES);
    }
    // Integrate the whole decaying tail.
    let mut energy = 0.0f64;
    for _ in 0..(SR as usize / 4) {
        let y = lpg.process_sample(0.0, CV_RES);
        assert!(y.is_finite());
        energy += (y as f64) * (y as f64);
    }
    energy
}

#[test]
fn higher_resonance_rings_longer() {
    let low = ring_energy(0.2);
    let high = ring_energy(0.9);
    assert!(
        high > low * 4.0,
        "higher resonance should sustain more ring energy: low={low:.3e}, high={high:.3e}"
    );
}

#[test]
fn full_resonance_self_oscillates_but_stays_bounded() {
    let mut lpg = Lpg::new(SR);
    lpg.set_params(params(1.0));

    // Open the gate, give a nudge, then feed silence.
    for _ in 0..(SR as usize / 4) {
        lpg.process_sample(0.0, CV_RES);
    }
    for i in 0..32 {
        lpg.process_sample(if i == 0 { 1.0 } else { 0.0 }, CV_RES);
    }
    // Long after any damped filter would be silent, sample the output.
    for _ in 0..(SR as usize / 2) {
        lpg.process_sample(0.0, CV_RES);
    }
    let mut peak = 0.0f32;
    for _ in 0..SR as usize / 2 {
        let y = lpg.process_sample(0.0, CV_RES).abs();
        assert!(y.is_finite(), "self-oscillation must stay finite");
        peak = peak.max(y);
    }
    assert!(
        peak > 1.0e-3,
        "full resonance should sustain self-oscillation: peak={peak:.3e}"
    );
    assert!(
        peak < 10.0,
        "self-oscillation must stay bounded: peak={peak:.3e}"
    );
}
