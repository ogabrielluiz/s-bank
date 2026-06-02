//! Phase 3 acceptance for the analogue imperfection layer.

use vactrol_core::imperfection::PinkNoise;
use vactrol_core::{ImperfectionConfig, Lpg, Mode, Params};

const SR: f32 = 48_000.0;

fn pluck_params() -> Params {
    Params {
        mode: Mode::Both,
        resonance: 0.3,
        cv_offset: 0.0,
        drive: 1.0,
        oversample: 2,
    }
}

/// Run a fixed pluck and collect the output.
fn run(lpg: &mut Lpg, n: usize) -> Vec<f32> {
    let gate = (SR * 0.005) as usize;
    (0..n)
        .map(|i| {
            let cv = if i < gate { 8.0 } else { 0.0 };
            let audio = (i as f32 * 0.2).sin();
            lpg.process_sample(audio, cv)
        })
        .collect()
}

/// Settled RMS of a steady tone through an open VCA gate -- a stable level metric
/// that isolates the bounded effect of component tolerance from the cutoff-knee
/// sensitivity of a Both-mode pluck transient.
fn steady_rms(lpg: &mut Lpg) -> f32 {
    lpg.set_params(Params {
        mode: Mode::Vca,
        resonance: 0.0,
        cv_offset: 0.0,
        drive: 1.0,
        oversample: 2,
    });
    let warmup = SR as usize / 4;
    let n = SR as usize / 4;
    let mut sum_sq = 0.0f64;
    for i in 0..warmup + n {
        let audio = (i as f32 * 0.2).sin();
        let y = lpg.process_sample(audio, 8.0); // gate held open
        if i >= warmup {
            sum_sq += (y as f64) * (y as f64);
        }
    }
    (sum_sq / n as f64).sqrt() as f32
}

#[test]
fn different_seeds_differ_but_stay_bounded() {
    let cfg = ImperfectionConfig {
        enabled: true,
        ..Default::default()
    };
    let mut a = Lpg::with_seed(SR, 1);
    let mut b = Lpg::with_seed(SR, 2);
    a.set_imperfection(cfg);
    b.set_imperfection(cfg);

    let rms_a = steady_rms(&mut a);
    let rms_b = steady_rms(&mut b);

    assert!(rms_a.is_finite() && rms_b.is_finite() && rms_a > 0.0 && rms_b > 0.0);
    // Must differ (the per-instance fingerprint is real)...
    assert!(
        (rms_a - rms_b).abs() > 1.0e-5,
        "instances should differ: {rms_a} vs {rms_b}"
    );
    // ...but the difference stays within a bounded tolerance band.
    let ratio = rms_a / rms_b;
    assert!(
        (0.7..1.43).contains(&ratio),
        "instance levels should stay bounded: rms_a={rms_a:.5}, rms_b={rms_b:.5}, ratio={ratio:.3}"
    );
}

#[test]
fn serialize_roundtrip_is_reproducible() {
    let cfg = ImperfectionConfig {
        enabled: true,
        ..Default::default()
    };
    let mut src = Lpg::with_seed(SR, 0xABCD);
    src.set_params(pluck_params());
    src.set_imperfection(cfg);
    let state = src.serialized_state();

    // Round-trip the state through JSON.
    let json = serde_json::to_string(&state).unwrap();
    let restored: vactrol_core::SerializedState = serde_json::from_str(&json).unwrap();

    let mut a = Lpg::from_state(SR, &state);
    let mut b = Lpg::from_state(SR, &restored);
    let oa = run(&mut a, SR as usize / 4);
    let ob = run(&mut b, SR as usize / 4);

    assert_eq!(oa, ob, "JSON round-trip must reproduce identical output");
}

#[test]
fn bypass_matches_deterministic_core() {
    // Default Lpg has imperfection disabled: it must equal an explicitly bypassed
    // instance sample-for-sample.
    let mut plain = Lpg::new(SR);
    plain.set_params(pluck_params());

    let mut bypassed = Lpg::new(SR);
    bypassed.set_params(pluck_params());
    bypassed.set_imperfection(ImperfectionConfig {
        enabled: false,
        ..Default::default()
    });

    let op = run(&mut plain, SR as usize / 4);
    let ob = run(&mut bypassed, SR as usize / 4);
    assert_eq!(op, ob, "bypassed layer must match the deterministic core");

    // Enabling then disabling must return to the baseline (tolerance reverts).
    bypassed.set_imperfection(ImperfectionConfig {
        enabled: true,
        ..Default::default()
    });
    bypassed.reset();
    bypassed.set_imperfection(ImperfectionConfig {
        enabled: false,
        ..Default::default()
    });
    bypassed.reset();
    let ob2 = run(&mut bypassed, SR as usize / 4);
    assert_eq!(op, ob2, "disabling must restore the baseline output");
}

#[test]
fn pink_noise_has_equal_energy_per_octave() {
    use realfft::RealFftPlanner;

    let n = 1usize << 15; // 32768
    let mut pink = PinkNoise::default();
    // Simple deterministic white source.
    let mut state: u64 = 0x1234_5678;
    let mut white = || {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let u = (state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32;
        (u as f32 / u32::MAX as f32) * 2.0 - 1.0
    };
    let mut buf: Vec<f32> = (0..n).map(|_| pink.next(white())).collect();

    let mut planner = RealFftPlanner::<f32>::new();
    let r2c = planner.plan_fft_forward(n);
    let mut spectrum = r2c.make_output_vec();
    r2c.process(&mut buf, &mut spectrum).unwrap();

    let bin_hz = SR / n as f32;
    let band_power = |lo: f32, hi: f32| -> f32 {
        let a = (lo / bin_hz) as usize;
        let b = (hi / bin_hz) as usize;
        spectrum[a..b].iter().map(|c| c.norm_sqr()).sum::<f32>()
    };

    // Pink noise carries roughly equal energy in each octave (PSD ~ 1/f), whereas
    // white would have ~10x more energy in the decade-higher band.
    let low = band_power(100.0, 200.0); // one octave
    let high = band_power(1_600.0, 3_200.0); // one octave, 4 octaves up
    let ratio = high / low;
    assert!(
        (0.3..3.0).contains(&ratio),
        "pink energy should be ~equal per octave (ratio {ratio:.2}); white would be ~10x"
    );
}
