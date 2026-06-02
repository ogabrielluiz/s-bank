//! The clean, zero-bleed low-pass-gate cell.
//!
//! A 2-pole TPT/ZDF state-variable low-pass (Cytomic topology) whose cutoff is swept
//! by the control signal, multiplied by a VCA that also tracks the control. The VCA
//! guarantees a **full close** (zero bleed) at control 0 — unlike a vactrol, which
//! leaks. There is **no resonance** (fixed low Q): the timbre comes from the swept
//! cutoff and the MATERIAL ceiling, not from Q. "More filter than VCA": as the gate
//! opens, the cutoff rises so brightness/loudness are dominated by the filter, with
//! the VCA finishing the close.

use std::f32::consts::PI;

/// Lowest cutoff (Hz) at a fully-closed gate.
const F_MIN: f32 = 20.0;
/// Fixed quality factor — Butterworth, no resonance.
const Q: f32 = 0.707;

#[derive(Debug, Clone)]
pub struct Gate {
    sr: f32,
    ic1eq: f32,
    ic2eq: f32,
}

impl Gate {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sr: sample_rate,
            ic1eq: 0.0,
            ic2eq: 0.0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sr = sample_rate;
    }

    pub fn reset(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
    }

    /// Process one sample.
    ///
    /// * `audio` – input sample.
    /// * `control` – gate opening in `0..=1` (0 = fully closed/silent).
    /// * `cutoff_ceiling` – cutoff (Hz) at a fully-open gate (from MATERIAL).
    #[inline]
    pub fn process(&mut self, audio: f32, control: f32, cutoff_ceiling: f32) -> f32 {
        let c = control.clamp(0.0, 1.0);
        // Exponential cutoff sweep F_MIN → ceiling with the control.
        let fc = (F_MIN * (cutoff_ceiling / F_MIN).powf(c)).clamp(F_MIN, 0.45 * self.sr);

        // Cytomic SVF (Zavalishin TPT), low-pass output.
        let g = (PI * fc / self.sr).tan();
        let k = 1.0 / Q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;

        let v3 = audio - self.ic2eq;
        let v1 = a1 * self.ic1eq + a2 * v3;
        let v2 = self.ic2eq + a2 * self.ic1eq + a3 * v3;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;
        let lp = v2;

        // VCA tracks control → guarantees a clean full close (zero bleed).
        c * lp
    }
}
