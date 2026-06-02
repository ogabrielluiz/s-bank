//! SIMD path correctness: the four-lane `LpgX4` must agree with the scalar `Lpg`
//! within the tolerance of `wide`'s transcendental approximations, and the four
//! lanes must stay independent (no cross-lane contamination).

use vactrol_core::{Lpg, LpgX4, Mode, Params};
use wide::f32x4;

const SR: f32 = 48_000.0;

fn params() -> Params {
    Params {
        mode: Mode::Both,
        resonance: 0.3,
        cv_offset: 0.0,
        drive: 1.5,
        oversample: 2,
    }
}

/// Run a fixed pluck through a scalar voice.
fn scalar_run(audio: &[f32], cv: &[f32]) -> Vec<f32> {
    let mut lpg = Lpg::new(SR);
    lpg.set_params(params());
    audio
        .iter()
        .zip(cv)
        .map(|(&a, &c)| lpg.process_sample(a, c))
        .collect()
}

fn pluck_input(n: usize) -> (Vec<f32>, Vec<f32>) {
    let gate = (SR * 0.005) as usize;
    let audio: Vec<f32> = (0..n).map(|i| (i as f32 * 0.2).sin()).collect();
    let cv: Vec<f32> = (0..n).map(|i| if i < gate { 8.0 } else { 0.0 }).collect();
    (audio, cv)
}

fn max_abs_err(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f32, f32::max)
}

#[test]
fn simd_lane_matches_scalar() {
    let n = SR as usize / 4;
    let (audio, cv) = pluck_input(n);
    let scalar = scalar_run(&audio, &cv);

    // Feed the identical signal to all four lanes.
    let mut x4 = LpgX4::new(SR);
    x4.set_params(params());
    let simd: Vec<[f32; 4]> = (0..n)
        .map(|i| {
            x4.process(f32x4::splat(audio[i]), f32x4::splat(cv[i]))
                .to_array()
        })
        .collect();

    for lane in 0..4 {
        let lane_out: Vec<f32> = simd.iter().map(|s| s[lane]).collect();
        let err = max_abs_err(&scalar, &lane_out);
        let peak = scalar.iter().map(|x| x.abs()).fold(0.0, f32::max);
        assert!(
            err < 1.0e-3 * peak.max(1.0),
            "lane {lane} should match scalar within tolerance: err={err:.2e}, peak={peak:.3}"
        );
    }
}

#[test]
fn simd_lanes_are_independent() {
    // Four genuinely different voices: different CV gate levels and audio phases.
    let n = SR as usize / 8;
    let gate = (SR * 0.005) as usize;
    let cv_levels = [8.0f32, 4.0, 2.0, 1.0];
    let phases = [0.20f32, 0.13, 0.31, 0.07];

    // Reference: each voice run independently through its own scalar instance.
    let mut refs: Vec<Vec<f32>> = Vec::new();
    for v in 0..4 {
        let mut lpg = Lpg::new(SR);
        lpg.set_params(params());
        let out: Vec<f32> = (0..n)
            .map(|i| {
                let cv = if i < gate { cv_levels[v] } else { 0.0 };
                lpg.process_sample((i as f32 * phases[v]).sin(), cv)
            })
            .collect();
        refs.push(out);
    }

    // SIMD: all four together.
    let mut x4 = LpgX4::new(SR);
    x4.set_params(params());
    let simd: Vec<[f32; 4]> = (0..n)
        .map(|i| {
            let a = f32x4::from([
                (i as f32 * phases[0]).sin(),
                (i as f32 * phases[1]).sin(),
                (i as f32 * phases[2]).sin(),
                (i as f32 * phases[3]).sin(),
            ]);
            let c = f32x4::from([
                if i < gate { cv_levels[0] } else { 0.0 },
                if i < gate { cv_levels[1] } else { 0.0 },
                if i < gate { cv_levels[2] } else { 0.0 },
                if i < gate { cv_levels[3] } else { 0.0 },
            ]);
            x4.process(a, c).to_array()
        })
        .collect();

    for v in 0..4 {
        let lane_out: Vec<f32> = simd.iter().map(|s| s[v]).collect();
        let err = max_abs_err(&refs[v], &lane_out);
        let peak = refs[v].iter().map(|x| x.abs()).fold(0.0, f32::max);
        assert!(
            err < 1.0e-3 * peak.max(1.0),
            "lane {v} must equal its independent scalar voice: err={err:.2e}, peak={peak:.3}"
        );
    }
}

#[test]
fn simd_stays_finite_on_pathological_input() {
    let mut x4 = LpgX4::new(SR);
    x4.set_params(Params {
        mode: Mode::Lowpass,
        resonance: 1.0,
        cv_offset: 0.0,
        drive: 4.0,
        oversample: 4,
    });
    for i in 0..SR as usize {
        let t = i as f32 / SR;
        let a = f32x4::from([1.0, -1.0, (t * 9000.0).sin(), 1000.0]);
        let c = f32x4::from([10.0, 0.0, 5.0 * (t * 2000.0).sin(), 1.0e3]);
        let y = x4.process(a, c).to_array();
        assert!(y.iter().all(|v| v.is_finite()), "non-finite at {i}: {y:?}");
    }
}
