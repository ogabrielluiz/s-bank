//! Audio path: topology-preserving (TPT/ZDF) 2-pole core for the 292 cell.
//!
//! The continuous network maps onto two trapezoidal integrators (C1, C2) solved
//! as a single delay-free linear system each sample (Zavalishin-style TPT state
//! variable filter). Unlike the direct-form bilinear transfer function, this
//! preserves the capacitor states and stays stable under any rate of modulation.
//!
//! The vactrol resistance `Rf` sets the cutoff (Both/Lowpass) and, via the
//! potential divider `Rα / (Rα + 2·Rf)` (Eq. 12), the DC gain. Resonance maps to
//! the SVF damping `k`; `k -> 0` approaches self-oscillation (the `amax` boundary)
//! but the linear solve stays finite.
//!
//! The scalar `f32` path here is written so a lane-parametric SIMD variant (Rack's
//! `float_4`) is a drop-in: `process` is branch-light and operates one frame at a
//! time over plain arithmetic.

use crate::nonlinear;
use crate::params::{Components, Mode};

#[derive(Debug, Clone)]
pub struct AudioPath {
    sample_rate: f32,
    /// Trapezoidal integrator states (the C1, C2 "equivalent" capacitor states).
    ic1eq: f32,
    ic2eq: f32,
}

impl AudioPath {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            ic1eq: 0.0,
            ic2eq: 0.0,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    pub fn reset(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
    }

    /// Cutoff (Hz) implied by `Rf` for the given mode, clamped to a stable range.
    #[inline]
    fn cutoff_hz(&self, rf: f32, mode: Mode, comp: &Components) -> f32 {
        let nyq_limit = 0.45 * self.sample_rate;
        match mode {
            // VCA is dominated by the divider; hold the corner bright so it acts
            // as an amplitude gate rather than a filter.
            Mode::Vca => nyq_limit,
            // Both / Lowpass: corner tracks Rf, so closing the gate dulls the tone.
            _ => {
                let fc = 1.0 / (2.0 * std::f32::consts::PI * rf * comp.c1);
                fc.clamp(20.0, nyq_limit)
            }
        }
    }

    /// DC gain from the potential divider, Eq. 12.
    #[inline]
    fn dc_gain(rf: f32, mode: Mode, comp: &Components) -> f32 {
        let r_alpha = match mode {
            Mode::Vca => comp.r_alpha_vca,
            _ => comp.r_alpha_both,
        };
        r_alpha / (r_alpha + 2.0 * rf)
    }

    /// Process one sample. `rf` is the live vactrol resistance (ohms).
    #[inline]
    pub fn process(
        &mut self,
        x: f32,
        rf: f32,
        mode: Mode,
        resonance: f32,
        drive: f32,
        comp: &Components,
    ) -> f32 {
        let fc = self.cutoff_hz(rf, mode, comp);
        let g = (std::f32::consts::PI * fc / self.sample_rate).tan();

        // Damping: resonance 0 -> heavily damped (k = 2), 1 -> near self-osc.
        // Lowpass mode engages the Sallen-Key (C3) path: a touch more resonant.
        let mut k = (2.0 * (1.0 - resonance)).max(0.02);
        if matches!(mode, Mode::Lowpass) {
            k = (k * 0.8).max(0.02);
        }

        // TPT state variable filter: one linear solve, unconditionally stable.
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - self.ic2eq;
        let v1 = a1 * self.ic1eq + a2 * v3;
        let v2 = self.ic2eq + a2 * self.ic1eq + a3 * v3;
        self.ic1eq = 2.0 * v1 - self.ic1eq;
        self.ic2eq = 2.0 * v2 - self.ic2eq;
        let lp = v2;

        // Buffer nonlinearity (Phase 2 wraps this in ADAA + oversampling).
        let buffered = nonlinear::saturate(lp, drive);

        buffered * Self::dc_gain(rf, mode, comp)
    }
}
