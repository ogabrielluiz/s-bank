//! Buffer / resonance nonlinearity.
//!
//! Phase 1 ships a plain `tanh` saturator. Phase 2 wraps it in first-order
//! antiderivative antialiasing (ADAA): `F1 = ln(cosh(x))` (the antiderivative of
//! `tanh`), discretely differentiated, with an epsilon fallback when
//! `|x[n] - x[n-1]|` is tiny. The antiderivative is provided here already so the
//! ADAA stage can be dropped in without touching call sites.

/// `tanh` saturator with a drive control. `drive <= 0` is the linear identity.
///
/// Unity-gain normalized (`tanh(d*x)/d`) so that increasing drive adds harmonics
/// rather than simply attenuating.
#[inline]
pub fn saturate(x: f32, drive: f32) -> f32 {
    if drive > 0.0 {
        (x * drive).tanh() / drive
    } else {
        x
    }
}

/// Antiderivative of `tanh`: `F1(x) = ln(cosh(x))`. Used by Phase 2 ADAA.
#[inline]
pub fn tanh_antiderivative(x: f32) -> f32 {
    x.cosh().ln()
}
