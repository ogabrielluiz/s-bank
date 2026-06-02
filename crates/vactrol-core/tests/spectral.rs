//! Phase 2/4 acceptance: aliasing from the nonlinear stage is reduced by
//! oversampling + ADAA, and below a locked threshold for the recommended config.
//!
//! Method: drive a full-scale sine hard into the buffer nonlinearity (VCA mode,
//! gate open, high drive). The `tanh` generates odd harmonics; those above Nyquist
//! fold back to inharmonic bins. We FFT a coherently-sampled steady-state segment
//! (rectangular window, F0 on a bin), treat the ladder of true harmonics
//! `k·f0 < fs/2` as wanted, and sum everything else (excluding DC) as aliasing.
//! Reported relative to the fundamental.

use realfft::RealFftPlanner;
use std::f32::consts::TAU;
use vactrol_core::{Lpg, Mode, Params};

const SR: f32 = 48_000.0;
const N: usize = 16_384;
const F0: f32 = 9_000.0; // high fundamental: folding is worst here

/// Aliasing energy (dB relative to the fundamental) for a given config.
fn aliasing_db(oversample: u8, adaa: bool) -> f32 {
    let mut lpg = Lpg::new(SR);
    lpg.set_params(Params {
        mode: Mode::Vca, // bright corner: the buffer sees ~full-scale sine
        resonance: 0.0,
        cv_offset: 0.0,
        drive: 5.0,
        oversample,
        adaa,
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
fn oversampling_and_adaa_reduce_aliasing() {
    let base = aliasing_db(1, false);
    let adaa_only = aliasing_db(1, true);
    let os2_adaa = aliasing_db(2, true);
    let os4_adaa = aliasing_db(4, true);
    println!(
        "aliasing dB rel fundamental: 1x={base:.1}, 1x+ADAA={adaa_only:.1}, \
         2x+ADAA={os2_adaa:.1}, 4x+ADAA={os4_adaa:.1}"
    );

    // Each measure should improve on the naive baseline.
    assert!(
        adaa_only < base - 3.0,
        "ADAA should reduce aliasing vs baseline"
    );
    assert!(
        os2_adaa < base - 6.0,
        "2x+ADAA should clearly beat baseline"
    );
    assert!(os4_adaa <= os2_adaa + 1.0, "4x should be no worse than 2x");

    // Locked regression bar for the recommended config (empirical). 2x + ADAA on
    // a 9 kHz full-scale sine through a tanh at drive 5 measures ~ -42.5 dB with
    // the 61-tap halfband; -40 dB leaves a little headroom for platform variation.
    assert!(
        os2_adaa < -40.0,
        "2x+ADAA aliasing should sit below -40 dB, got {os2_adaa:.1}"
    );
}
