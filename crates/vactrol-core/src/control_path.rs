//! Control path: CV in -> smoothing -> the Parker & D'Angelo LED-driver control
//! circuit -> LED current `If` (amps).
//!
//! The CV-to-current map is the authors' exact control-circuit model (ported from
//! their reference `lpg.cpp`): a bias stage feeding a transistor/op-amp LED driver
//! whose forward solve is a Lambert-W, approximated by the authors' cubic
//! `w = k0 + k1 x + k2 x^2 + k3 x^3` inside the central operating branch, with
//! saturating branches either side and a piecewise clamp of `If` to
//! `[Ifmin, Ifmax]`. The front-panel `offset`/`scale` controls are fixed here
//! (offset ~ 0 so the gate fully darkens at CV = 0; scale = 1). See
//! `docs/REFERENCES.md`.
//!
//! The resulting `If` is handed to the vactrol, which applies the datasheet power
//! law and the asymmetric attack/decay dynamics. A short symmetric one-pole
//! pre-smooths the CV before the (static) circuit.

use crate::params::Components;

/// Time constant of the control smoothing filter (seconds).
pub(crate) const CTRL_TAU_S: f32 = 0.0015;

/// Fixed front-panel controls. `OFFSET = 0` -> circuit `offset = 0.0001`, so the
/// bias current vanishes and CV = 0 yields `Ifmin` (a fully dark gate).
const OFFSET: f32 = 0.0;
const SCALE: f32 = 1.0;

/// Precomputed coefficients of the authors' control circuit for fixed
/// offset/scale. Shared by the scalar and SIMD control paths so they cannot drift.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ControlCoeffs {
    pub ifmin: f32,
    pub ifmax: f32,
    pub bias: f32,
    pub inv_r5: f32,
    pub bound1: f32,
    pub x_coeff: f32,
    pub v3_w: f32,
    pub v3_ia: f32,
    pub v3_sat_const: f32,
    pub r6r7: f32,
    pub alpha: f32,
    pub beta: f32,
    pub inv_alpha: f32,
    pub ifbound2: f32,
    pub ifbound3: f32,
    pub ifb3_slope: f32,
    pub ifb3_const: f32,
    pub k0: f32,
    pub k1: f32,
    pub k2: f32,
    pub k3: f32,
}

impl ControlCoeffs {
    pub(crate) fn new() -> Self {
        // Authors' constants (lpg.cpp).
        let ifmin = 10.1e-6f64;
        let ifmax = 40e-3f64;
        let r2max = 10e3f64;
        let r6max = 20e3f64;
        let r7 = 33e3f64;
        let r3 = 150e3f64;
        let r5 = 100e3f64;
        let r8 = 4.7e3f64;
        let r9 = 470.0f64;
        let vb_ = 3.9f64; // the constant VB (distinct from the input Vb)
        let vt = 26e-3f64;
        let n = 3.9696f64;
        let kl = 6.3862f64;
        let g = 2e5f64;
        let vs = 15.0f64;
        let gamma = 0.0001f64;

        let offset = 0.9999 * OFFSET as f64 + 0.0001;
        let scale = (SCALE as f64).clamp(0.0, 1.0);
        let r6 = scale * r6max;
        let r1 = (1.0 - offset) * r2max;
        let r2 = offset * r2max;
        let r6r7 = r6 + r7;

        let alpha = 1.0 + r6r7 * (1.0 / r3 + 1.0 / r5);
        let beta = ((1.0 / alpha) - 1.0) / r6r7 - 1.0 / r8;
        let bound1 = 600.0 * alpha * n * vt / (g * (r6r7 - 1.0 / (alpha * beta)));
        let bias = vs / (r3 * (1.0 + r1 / r2));
        let x_coeff = g * (r6r7 - 1.0 / (alpha * beta)) / (alpha * n * vt);
        let v3_w = -(alpha / g) * n * vt;
        let v3_ia = -1.0 / (alpha * beta);
        let v3_sat_const = kl * alpha / g * n * vt;
        let inv_alpha = 1.0 / alpha;
        let ifbound2 = vb_ / r6r7;
        let ifbound3 =
            (gamma * g * vb_ + alpha * r9 * (vb_ * beta + ifmax)) / (gamma * g * r6r7 + r9);
        let ifb3_slope = gamma * g * r6r7 / (alpha * r9) + inv_alpha;
        let ifb3_const = -gamma * g * vb_ / (alpha * r9) - vb_ * beta;

        Self {
            ifmin: ifmin as f32,
            ifmax: ifmax as f32,
            bias: bias as f32,
            inv_r5: (1.0 / r5) as f32,
            bound1: bound1 as f32,
            x_coeff: x_coeff as f32,
            v3_w: v3_w as f32,
            v3_ia: v3_ia as f32,
            v3_sat_const: v3_sat_const as f32,
            r6r7: r6r7 as f32,
            alpha: alpha as f32,
            beta: beta as f32,
            inv_alpha: inv_alpha as f32,
            ifbound2: ifbound2 as f32,
            ifbound3: ifbound3 as f32,
            ifb3_slope: ifb3_slope as f32,
            ifb3_const: ifb3_const as f32,
            k0: 146.8,
            k1: 0.49202,
            k2: 4.1667e-4,
            k3: 7.3915e-9,
        }
    }

    /// LED current (amps) for a control voltage `vb`.
    #[inline]
    pub(crate) fn current(&self, vb: f32) -> f32 {
        let vb = vb.clamp(-10.0, 50.0);
        let ia = vb * self.inv_r5 + self.bias;

        let v3 = if ia <= -self.bound1 {
            self.v3_ia * ia
        } else if ia < self.bound1 {
            let x = self.x_coeff * ia;
            let w = self.k0 + x * (self.k1 + x * (self.k2 + x * self.k3));
            self.v3_w * w + self.v3_ia * ia
        } else {
            self.v3_sat_const - ia * self.r6r7
        };

        let ifbound1 = self.alpha * (self.ifmin - self.beta * v3);
        let if_current = if ia <= ifbound1 {
            self.ifmin
        } else if ia <= self.ifbound2 {
            self.beta * v3 + ia * self.inv_alpha
        } else if ia <= self.ifbound3 {
            self.ifb3_slope * ia + self.ifb3_const
        } else {
            self.ifmax
        };
        if_current.clamp(self.ifmin, self.ifmax)
    }
}

#[derive(Debug, Clone)]
pub struct ControlPath {
    sample_rate: f32,
    /// One-pole control-filter state (smoothed CV in volts).
    cv_state: f32,
    /// Per-sample smoothing coefficient derived from `CTRL_TAU_S`.
    smooth: f32,
    coeffs: ControlCoeffs,
}

impl ControlPath {
    pub fn new(sample_rate: f32) -> Self {
        let mut s = Self {
            sample_rate,
            cv_state: 0.0,
            smooth: 0.0,
            coeffs: ControlCoeffs::new(),
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
        self.coeffs.current(self.cv_state)
    }
}

#[cfg(test)]
mod tests {
    use super::ControlCoeffs;

    /// The control circuit reproduces the authors' reference values (computed from
    /// the verbatim `lpg.cpp` model) across the operating range.
    #[test]
    fn matches_reference_control_circuit() {
        let c = ControlCoeffs::new();
        // (control voltage, expected LED current in mA).
        let cases = [
            (0.0f32, 0.0101f32),
            (2.0, 0.2457),
            (7.0, 0.8595),
            (8.0, 8.6009),
            (10.0, 32.5618),
        ];
        for (cv, expected_ma) in cases {
            let got_ma = c.current(cv) * 1e3;
            assert!(
                (got_ma - expected_ma).abs() < 1e-3,
                "CV={cv}: got {got_ma:.4} mA, expected {expected_ma:.4} mA"
            );
        }
        // Monotonic and clamped to [Ifmin, Ifmax].
        let mut prev = 0.0;
        for i in 0..=120 {
            let cv = i as f32 * 0.1;
            let v = c.current(cv);
            assert!(v >= c.ifmin - 1e-9 && v <= c.ifmax + 1e-9);
            assert!(v >= prev - 1e-6, "control current must be monotonic in CV");
            prev = v;
        }
    }
}
