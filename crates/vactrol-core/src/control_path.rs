//! Control path: CV in -> shelving/lowpass control filter -> CV-to-LED-current map.
//!
//! The output is the LED current `If` (amps) that drives the vactrol. The exact
//! 292 control circuit is a zener-limited log-amp; here we use a smooth saturating
//! curve fit (documented upgrade path: the Lambert-W exact form from Parker &
//! D'Angelo). Monotonic and bounded is what matters for the vertical slice.

use crate::params::Components;

/// LED current at full drive (datasheet ON region tops out near 40 mA).
const I_MAX_A: f32 = 0.040;
/// Soft-knee scale (volts) of the CV-to-current curve.
const V_SCALE: f32 = 2.5;
/// Time constant of the control smoothing filter (seconds).
const CTRL_TAU_S: f32 = 0.0015;

#[derive(Debug, Clone)]
pub struct ControlPath {
    sample_rate: f32,
    /// One-pole control-filter state (smoothed CV in volts).
    cv_state: f32,
    /// Per-sample smoothing coefficient derived from `CTRL_TAU_S`.
    smooth: f32,
}

impl ControlPath {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            sample_rate,
            cv_state: 0.0,
            smooth: 0.0,
        };
        s.set_sample_rate(sample_rate);
        s
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.smooth = (-1.0 / (CTRL_TAU_S * sample_rate)).exp();
    }

    pub fn reset(&mut self) {
        self.cv_state = 0.0;
    }

    /// Smooth the CV and map it to LED current (amps).
    #[inline]
    pub fn process(&mut self, cv: f32, offset: f32, _comp: &Components) -> f32 {
        let target = cv + offset;
        self.cv_state = target + (self.cv_state - target) * self.smooth;
        Self::cv_to_current(self.cv_state)
    }

    /// Smooth, saturating, monotonic CV-to-current curve.
    #[inline]
    fn cv_to_current(v: f32) -> f32 {
        let vv = v.max(0.0);
        I_MAX_A * (1.0 - (-vv / V_SCALE).exp())
    }
}
