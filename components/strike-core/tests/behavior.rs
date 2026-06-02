//! Behavioral tests for the Strike gate — the signature behaviors from the design
//! spec. Tolerance/threshold based, deterministic (seeds pinned).

use std::f32::consts::TAU;
use strike_core::{ImperfectionConfig, Strike, StrikeParams};

const SR: f32 = 48_000.0;

fn params(open: f32, decay: f32, material: f32) -> StrikeParams {
    StrikeParams {
        open,
        decay,
        material,
    }
}

/// Samples the control stays above 5% of its peak after a single HIT, with `audio_in`
/// held at DC (so the control == the ping/EG-out envelope). A proxy for ring length.
fn decay_samples(decay: f32) -> usize {
    let mut s = Strike::new(SR);
    s.set_params(params(0.0, decay, 0.0));
    let n = (SR as usize) * 8;
    let mut peak = 0.0f32;
    let mut ctrl = Vec::with_capacity(n);
    for i in 0..n {
        let hit = if i < 10 { 5.0 } else { 0.0 };
        s.process_sample(1.0, 0.0, 0.0, hit);
        let c = s.last_control();
        peak = peak.max(c);
        ctrl.push(c);
    }
    let thr = peak * 0.05;
    ctrl.iter().rposition(|&c| c > thr).unwrap_or(0)
}

#[test]
fn zero_bleed_full_close() {
    // open=0, no HIT, no CTRL: the gate is fully closed → exact silence even with
    // audio at the input. (The vactrol core, by contrast, bleeds.)
    let mut s = Strike::new(SR);
    s.set_params(params(0.0, 0.5, 0.0));
    let mut maxabs = 0.0f32;
    for i in 0..4800 {
        let a = (i as f32 * 0.05).sin();
        maxabs = maxabs.max(s.process_sample(a, 0.0, 0.0, 0.0).abs());
    }
    assert!(maxabs < 1.0e-6, "expected silence when closed, got {maxabs:.2e}");
}

#[test]
fn ping_emits_envelope() {
    let mut s = Strike::new(SR);
    s.set_params(params(0.0, 0.5, 0.0));
    // DC in, no HIT → closed (no ping without a strike).
    let mut closed = 0.0f32;
    for _ in 0..480 {
        closed = closed.max(s.process_sample(1.0, 0.0, 0.0, 0.0).abs());
    }
    assert!(closed < 1.0e-6, "DC in with no HIT must stay closed: {closed:.2e}");
    // HIT with DC in → the raw envelope appears at the output, then decays.
    let mut out = Vec::new();
    for i in 0..(SR as usize * 2) {
        let hit = if i < 10 { 5.0 } else { 0.0 };
        out.push(s.process_sample(1.0, 0.0, 0.0, hit));
    }
    let peak = out.iter().cloned().fold(0.0f32, f32::max);
    let tail = out.last().unwrap().abs();
    assert!(peak > 0.1, "ping should produce an envelope, peak={peak:.3}");
    assert!(tail < peak * 0.1, "ping envelope should decay, tail={tail:.3} peak={peak:.3}");
}

#[test]
fn decay_is_dual_mode_short_vs_long() {
    let short = decay_samples(0.1);
    let long = decay_samples(0.8);
    assert!(
        long > short * 3,
        "long decay should ring far longer: long={long} short={short}"
    );
}

#[test]
fn memory_effect_accumulates() {
    // Peak control for a single HIT vs several rapid HITs (no envelope reset).
    fn peak_control(hits: usize, spacing: usize) -> f32 {
        let mut s = Strike::new(SR);
        s.set_params(params(0.0, 0.5, 0.0));
        let total = spacing * hits + 4000;
        let mut peak = 0.0f32;
        let mut fired = 0usize;
        let mut next = 0usize;
        for i in 0..total {
            let mut hit = 0.0;
            if i == next && fired < hits {
                hit = 5.0; // single-sample pulse → clean rising edge
                fired += 1;
                next = i + spacing;
            }
            s.process_sample(1.0, 0.0, 0.0, hit);
            peak = peak.max(s.last_control());
        }
        peak
    }
    let single = peak_control(1, 1);
    let multi = peak_control(6, 300);
    assert!(
        multi > single + 0.1,
        "rapid strikes should open wider: single={single:.3} multi={multi:.3}"
    );
}

#[test]
fn decay_is_frequency_dependent() {
    // Higher-pitched input rings shorter than lower-pitched input at the same DECAY.
    fn decay_for_pitch(freq: f32) -> usize {
        let mut s = Strike::new(SR);
        s.set_params(params(0.0, 0.6, 0.0));
        let dph = TAU * freq / SR;
        let mut ph = 0.0f32;
        // Warm up the pitch tracker (no HIT → control stays 0).
        for _ in 0..(SR as usize / 4) {
            s.process_sample(ph.sin(), 0.0, 0.0, 0.0);
            ph += dph;
        }
        let n = (SR as usize) * 6;
        let mut peak = 0.0f32;
        let mut ctrl = Vec::with_capacity(n);
        for i in 0..n {
            let hit = if i < 10 { 5.0 } else { 0.0 };
            s.process_sample(ph.sin(), 0.0, 0.0, hit);
            ph += dph;
            let c = s.last_control();
            peak = peak.max(c);
            ctrl.push(c);
        }
        let thr = peak * 0.05;
        ctrl.iter().rposition(|&c| c > thr).unwrap_or(0)
    }
    let high = decay_for_pitch(880.0);
    let low = decay_for_pitch(110.0);
    assert!(
        low > (high as f32 * 1.5) as usize,
        "low pitch should ring longer: low={low} high={high}"
    );
}

/// Brightness proxy (first-difference energy ratio) and RMS for a low+high test tone
/// held open at a given CTRL level and MATERIAL.
fn brightness_rms(ctrl: f32, material: f32) -> (f32, f32) {
    let mut s = Strike::new(SR);
    s.set_params(params(0.0, 0.5, material));
    let (dl, dh) = (TAU * 100.0 / SR, TAU * 5000.0 / SR);
    let (mut pl, mut ph) = (0.0f32, 0.0f32);
    let (mut prev, mut hf, mut tot) = (0.0f32, 0.0f32, 0.0f32);
    let n = SR as usize;
    for i in 0..n {
        let a = 0.5 * pl.sin() + 0.5 * ph.sin();
        pl += dl;
        ph += dh;
        let y = s.process_sample(a, ctrl, 0.0, 0.0);
        if i > n / 2 {
            let d = y - prev;
            hf += d * d;
            tot += y * y;
        }
        prev = y;
    }
    (hf / (tot + 1.0e-12), (tot / (n as f32 / 2.0)).sqrt())
}

#[test]
fn more_filter_than_vca_highs_open_first() {
    // Opening the gate raises brightness (cutoff sweep), not just level.
    let (bright_open, rms_open) = brightness_rms(1.0, 0.0);
    let (bright_half, rms_half) = brightness_rms(0.3, 0.0);
    assert!(
        bright_open > bright_half,
        "more open should be brighter: open={bright_open:.4} half={bright_half:.4}"
    );
    assert!(rms_open > rms_half, "more open should be louder");
}

#[test]
fn material_morph_softer_is_duller_and_quieter() {
    let (bright_hard, rms_hard) = brightness_rms(1.0, 0.0);
    let (bright_soft, rms_soft) = brightness_rms(1.0, 1.0);
    assert!(
        bright_hard > bright_soft,
        "hard should be brighter than soft: hard={bright_hard:.4} soft={bright_soft:.4}"
    );
    assert!(rms_hard > rms_soft, "hard should be louder than soft");
}

#[test]
fn imperfection_off_is_deterministic_on_differs() {
    fn run(imp: bool) -> Vec<f32> {
        let mut s = Strike::with_seed(SR, 0xABCD);
        s.set_params(params(0.0, 0.5, 0.0));
        if imp {
            s.set_imperfection(ImperfectionConfig {
                enabled: true,
                ..Default::default()
            });
        }
        let d = TAU * 220.0 / SR;
        let mut ph = 0.0f32;
        let mut out = Vec::new();
        for i in 0..24_000 {
            let hit = if i == 0 { 5.0 } else { 0.0 };
            out.push(s.process_sample(ph.sin(), 0.0, 0.0, hit));
            ph += d;
        }
        out
    }
    let off1 = run(false);
    let off2 = run(false);
    assert_eq!(off1, off2, "imperfection off must be deterministic");
    let on = run(true);
    let diff = off1
        .iter()
        .zip(&on)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(diff > 0.0, "imperfection on should change the output");
    assert!(on.iter().all(|v| v.is_finite()), "imperfection output must stay finite");
}
