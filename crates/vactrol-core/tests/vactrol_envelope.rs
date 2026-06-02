//! Phase 1 acceptance: the vactrol has a fast attack and a slow decay, and a CV
//! impulse produces a "pluck" envelope.

use vactrol_core::control_path::ControlPath;
use vactrol_core::params::{Components, Mode};
use vactrol_core::vactrol::Vactrol;
use vactrol_core::{Lpg, Params};

const SR: f32 = 48_000.0;

/// Settle a fresh vactrol at a constant CV and return its steady-state resistance.
fn settle(cv: f32, secs: f32) -> f32 {
    let comp = Components::default();
    let mut ctrl = ControlPath::new(SR);
    let mut v = Vactrol::new(SR);
    let mut rf = v.resistance();
    for _ in 0..(SR * secs) as usize {
        let current = ctrl.process(cv, 0.0, &comp);
        rf = v.process(current, &comp);
    }
    rf
}

/// Samples to reach 63.2% of the linear transition from `from` to `to` -- one
/// time constant in the same sense the datasheet uses ("turn-on to 63% of final
/// R_ON"). This isolates the configured time constant from the (very different)
/// start/end resistances of attack vs decay.
fn samples_to_tau(open_first: bool, from: f32, to: f32) -> usize {
    let comp = Components::default();
    let mut ctrl = ControlPath::new(SR);
    let mut v = Vactrol::new(SR);
    // For the decay measurement we must start from the open state.
    if open_first {
        for _ in 0..(SR * 1.0) as usize {
            let current = ctrl.process(8.0, 0.0, &comp);
            v.process(current, &comp);
        }
    }
    let cv = if open_first { 0.0 } else { 8.0 };
    for i in 0..(SR * 3.0) as usize {
        let current = ctrl.process(cv, 0.0, &comp);
        let rf = v.process(current, &comp);
        let progress = (rf - from) / (to - from);
        if progress >= 0.632 {
            return i;
        }
    }
    usize::MAX
}

#[test]
fn attack_is_faster_than_decay() {
    let rf_open = settle(8.0, 1.0);
    let rf_dark = settle(0.0, 2.0);
    assert!(
        rf_open < rf_dark,
        "open resistance should be far below dark"
    );

    // Attack: dark -> open. Decay: open -> dark.
    let attack_n = samples_to_tau(false, rf_dark, rf_open);
    let decay_n = samples_to_tau(true, rf_open, rf_dark);

    assert!(
        attack_n != usize::MAX,
        "attack never reached one time constant"
    );
    assert!(
        decay_n != usize::MAX,
        "decay never reached one time constant"
    );
    assert!(
        attack_n * 3 < decay_n,
        "attack tau ({attack_n} samples) should be much faster than decay tau ({decay_n} samples)"
    );
}

#[test]
fn cv_impulse_produces_a_pluck() {
    let mut lpg = Lpg::new(SR);
    lpg.set_params(Params {
        mode: Mode::Both,
        resonance: 0.0,
        cv_offset: 0.0,
        drive: 0.0, // linear, so the envelope is read cleanly
    });

    let n = (SR * 0.5) as usize;
    let gate = (SR * 0.005) as usize; // 5 ms gate
                                      // DC carrier so the output amplitude tracks the gate envelope directly.
    let env: Vec<f32> = (0..n)
        .map(|i| {
            let cv = if i < gate { 8.0 } else { 0.0 };
            lpg.process_sample(1.0, cv).abs()
        })
        .collect();

    let (peak_idx, &peak) = env
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .unwrap();

    // Fast attack: the peak lands early.
    assert!(
        peak_idx < (SR * 0.1) as usize,
        "pluck peak should be early: idx={peak_idx}"
    );
    // Slow decay: the tail is well below the peak by the end.
    assert!(
        env[n - 1] < peak * 0.7,
        "pluck should decay: end={:.4}, peak={peak:.4}",
        env[n - 1]
    );
    // ...but the decay is gradual, not an immediate cutoff.
    let mid = env[(SR * 0.05) as usize];
    assert!(
        mid > peak * 0.3,
        "decay should be gradual: mid={mid:.4}, peak={peak:.4}"
    );
}
