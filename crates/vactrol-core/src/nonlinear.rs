//! Buffer / resonance nonlinearity, with first-order antiderivative antialiasing.
//!
//! The buffer is a `tanh` saturator. In this slice it sits at the audio-path
//! *output* (memoryless, outside the resonance feedback), so first-order ADAA is
//! exact rather than an approximation. Moving it inside the feedback loop is a
//! documented future option; ADAA would then be stateful and oversampling the
//! robust backstop.
//!
//! First-order ADAA replaces `f(x)` with the discrete derivative of its
//! antiderivative `F1`:
//!
//! ```text
//! y[n] = (F1(x[n]) - F1(x[n-1])) / (x[n] - x[n-1])
//! ```
//!
//! with a midpoint fallback when `|x[n] - x[n-1]|` is below `ADAA_EPS` (the
//! division is ill-conditioned there). For `f(x) = tanh(d·x)/d` the antiderivative
//! is `F1(x) = lncosh(d·x) / d²`.

/// Ill-conditioning guard for the ADAA divided difference (in input units).
pub const ADAA_EPS: f32 = 1.0e-5;

/// Numerically stable `ln(cosh(z))`, avoiding `cosh` overflow for large `|z|`.
#[inline]
fn lncosh(z: f32) -> f32 {
    let a = z.abs();
    a + (-2.0 * a).exp().ln_1p() - core::f32::consts::LN_2
}

/// `tanh` saturator with a drive control. `drive <= 0` is the linear identity.
///
/// Unity-gain normalized (`tanh(d·x)/d`) so increasing drive adds harmonics
/// rather than simply attenuating.
#[inline]
pub fn saturate(x: f32, drive: f32) -> f32 {
    if drive > 0.0 {
        (x * drive).tanh() / drive
    } else {
        x
    }
}

/// Antiderivative of `tanh`: `F1(x) = ln(cosh(x))`.
#[inline]
pub fn tanh_antiderivative(x: f32) -> f32 {
    lncosh(x)
}

/// Stateful first-order ADAA wrapper around the `tanh` saturator.
///
/// Holds the previous input and its antiderivative so each call is one `lncosh`
/// plus a divide. `drive` may vary per sample.
#[derive(Debug, Clone, Default)]
pub struct TanhAdaa {
    /// Previous input sample.
    x1: f32,
    /// `F1(x1)` cached.
    f1: f32,
}

impl TanhAdaa {
    pub fn reset(&mut self) {
        self.x1 = 0.0;
        self.f1 = 0.0;
    }

    /// Antialiased `saturate(x, drive)`.
    #[inline]
    pub fn process(&mut self, x: f32, drive: f32) -> f32 {
        if drive <= 0.0 {
            // Linear: ADAA of the identity is the identity. Keep state coherent.
            self.x1 = x;
            self.f1 = 0.0;
            return x;
        }
        let d = drive;
        let f1x = lncosh(d * x) / (d * d);
        let diff = x - self.x1;
        let y = if diff.abs() > ADAA_EPS {
            (f1x - self.f1) / diff
        } else {
            // Midpoint instantaneous value (the analytic limit of the difference).
            let xbar = 0.5 * (x + self.x1);
            (d * xbar).tanh() / d
        };
        self.x1 = x;
        self.f1 = f1x;
        y
    }
}
