//! Continuous MATERIAL morph (our improvement over the reference's 3-position
//! hard/med/soft switch).
//!
//! One axis `material ∈ [0,1]`: `0` = hardest (fast attack, brightest, loudest),
//! `1` = softest (slow attack, most high-frequency muffling, quieter). The panel can
//! mark hard/medium/soft as reference points, but the control is smooth and CV-able.

/// The decoded material character for one `material` setting.
#[derive(Debug, Clone, Copy)]
pub struct Material {
    /// Onset smoothing time constant (s). Hard = fast/clicky, soft = rounded.
    pub attack_tau: f32,
    /// Low-pass cutoff ceiling (Hz) at a fully-open gate. Hard = bright, soft = dull.
    pub cutoff_ceiling: f32,
    /// Output level. Soft materials are quieter, mirroring the reference.
    pub level: f32,
}

impl Material {
    /// Decode `material ∈ [0,1]` into its character. Exponential interpolation for
    /// the time/frequency axes (perceptually even), linear for level.
    pub fn from01(material: f32) -> Self {
        let m = material.clamp(0.0, 1.0);
        Self {
            // 0.5 ms (hard) … 25 ms (soft)
            attack_tau: 0.0005 * (25.0f32 / 0.5).powf(m),
            // 18 kHz (hard) … 2.5 kHz (soft)
            cutoff_ceiling: 18_000.0 * (2_500.0f32 / 18_000.0).powf(m),
            // 1.0 (hard) … 0.6 (soft)
            level: 1.0 - 0.4 * m,
        }
    }
}
