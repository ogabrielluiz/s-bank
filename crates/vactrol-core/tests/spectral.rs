//! Aliasing from the in-loop resonance nonlinearity is reduced by oversampling the
//! solve, and sits below a locked threshold for the recommended 2x config.
//!
//! Method: drive a full-scale sine hard into a high-resonance Lowpass cell (the
//! tanh lives in the resonance feedback loop). Its harmonics fold above Nyquist.
//! We FFT a coherently-sampled steady-state segment (rectangular window, F0 on a
//! bin), treat the ladder of true harmonics `k·f0 < fs/2` as wanted, and sum
//! everything else (excluding DC) as aliasing, relative to the fundamental.

use realfft::RealFftPlanner;
use std::f32::consts::TAU;
use vactrol_core::{Lpg, Mode, Params};

const SR: f32 = 48_000.0;
const N: usize = 16_384;
const F0: f32 = 9_000.0; // high fundamental: folding is worst here

/// Aliasing energy (dB relative to the fundamental) for a given oversampling
/// factor. The nonlinearity lives in the resonance feedback loop, so we drive a
/// high-resonance Lowpass cell hard with a full-scale sine through a wide-open
/// gate; the in-loop tanh generates harmonics that fold without oversampling.
fn aliasing_db(oversample: u8) -> f32 {
    let mut lpg = Lpg::new(SR);
    lpg.set_params(Params {
        mode: Mode::Lowpass,
        resonance: 0.85,
        cv_offset: 0.0,
        drive: 5.0,
        oversample,
        adaa: true,
    });

    // Hold the gate open and let the envelope/filter settle before capturing.
    let warmup = SR as usize / 4;
    let mut out = vec![0.0f32; N];
    for i in 0..warmup + N {
        let x = (TAU * F0 * i as f32 / SR).sin();
        let y = lpg.process_sample(x, 8.0);
        if i >= warmup {
            out[i - warmup] = y;
        }
    }

    // F0 lands exactly on a bin (N samples == whole number of cycles), so the
    // sampling is coherent and a rectangular window measures aliasing without
    // leakage (a window's sidelobes would floor the measurement well above the
    // true alias level).
    let mut planner = RealFftPlanner::<f32>::new();
    let r2c = planner.plan_fft_forward(N);
    let mut indata = r2c.make_input_vec();
    indata.copy_from_slice(&out);
    let mut spectrum = r2c.make_output_vec();
    r2c.process(&mut indata, &mut spectrum).unwrap();

    let bin_hz = SR / N as f32;
    let power: Vec<f32> = spectrum.iter().map(|c| c.norm_sqr()).collect();

    // Wanted bins: the true harmonic ladder of f0 below Nyquist, +/- a few bins.
    let guard = 3i64;
    let is_wanted = |bin: usize| -> bool {
        if bin == 0 {
            return true; // ignore DC
        }
        let mut k = 1;
        loop {
            let hf = k as f32 * F0;
            if hf >= SR / 2.0 {
                return false;
            }
            let hbin = (hf / bin_hz).round() as i64;
            if (bin as i64 - hbin).abs() <= guard {
                return true;
            }
            k += 1;
        }
    };

    let fund_bin = (F0 / bin_hz).round() as usize;
    let fund: f32 = ((fund_bin as i64 - guard).max(0) as usize..=fund_bin + guard as usize)
        .map(|b| power[b])
        .sum();

    let alias: f32 = power
        .iter()
        .enumerate()
        .filter(|&(b, _)| !is_wanted(b))
        .map(|(_, &p)| p)
        .sum();

    10.0 * (alias / fund).log10()
}

#[test]
fn oversampling_reduces_aliasing() {
    let base = aliasing_db(1);
    let os2 = aliasing_db(2);
    let os4 = aliasing_db(4);
    println!("aliasing dB rel fundamental: 1x={base:.1}, 2x={os2:.1}, 4x={os4:.1}");

    // Oversampling the in-loop nonlinear solve must reduce folded-back energy;
    // 4x clears it substantially (this operating point is a torture case for 2x).
    assert!(os2 < base - 2.0, "2x should beat 1x: {os2:.1} vs {base:.1}");
    assert!(
        os4 < base - 15.0,
        "4x should clearly beat 1x: {os4:.1} vs {base:.1}"
    );
    assert!(os4 < os2, "4x should beat 2x: {os4:.1} vs {os2:.1}");

    // Locked regression bar for the recommended 2x config (empirical: ~-20 dB on
    // this torture case -- full-scale 9 kHz into a resonance-0.85 cell at drive 5;
    // typical use aliases far less). -16 dB leaves headroom for platform/SIMD
    // variation while still catching real regressions.
    assert!(
        os2 < -16.0,
        "2x aliasing should sit below -16 dB, got {os2:.1}"
    );
}
