//! Parameters, physical component values, and the serializable instance state.
//!
//! Component defaults follow Table 1 of Parker & D'Angelo, "A Digital Model of
//! the Buchla Lowpass-Gate" (DAFx-13), and the VTL5C3/2 datasheet figures cited
//! in the design report. They are also the targets that the Phase 3 imperfection
//! layer perturbs per instance (component tolerance), so they live in one struct.

use serde::{Deserialize, Serialize};

/// The three musically distinct routings of the 292 cell.
///
/// * `Both` couples amplitude and brightness: as the gate closes the signal gets
///   quieter *and* duller (the classic "pluck"/"bongo" transient).
/// * `Vca` is dominated by the potential-divider, behaving as an amplitude gate
///   with little filtering.
/// * `Lowpass` engages the Sallen-Key path (C3) and acts mostly as a filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Mode {
    #[default]
    Both,
    Vca,
    Lowpass,
}

impl Mode {
    /// Stable integer encoding for the C ABI.
    pub fn from_u32(v: u32) -> Self {
        match v {
            1 => Mode::Vca,
            2 => Mode::Lowpass,
            _ => Mode::Both,
        }
    }
}

/// Runtime, user-facing parameters. Cheap to copy; set per block.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Params {
    pub mode: Mode,
    /// Resonance / feedback amount, normalized `0.0..=1.0`. `1.0` approaches the
    /// `amax` self-oscillation boundary (Eq. 11) but stays bounded.
    pub resonance: f32,
    /// CV offset in volts, added to the control input before the LED map.
    pub cv_offset: f32,
    /// Drive into the tanh buffer/resonance nonlinearity. `0.0` is linear.
    pub drive: f32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            mode: Mode::Both,
            resonance: 0.2,
            cv_offset: 0.0,
            drive: 1.0,
        }
    }
}

/// Physical component values and vactrol constants.
///
/// SI units throughout: farads, ohms, seconds, amps.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Components {
    /// Audio-path capacitors (Table 1).
    pub c1: f32,
    pub c2: f32,
    pub c3: f32,
    /// Potential-divider resistance: 5 MΩ in Both/Lowpass, 5 kΩ in VCA.
    pub r_alpha_both: f32,
    pub r_alpha_vca: f32,
    /// Vactrol current-to-resistance power law `Rf = A / If^1.4 + B`.
    pub rf_law_a: f32,
    pub rf_law_b: f32,
    /// Clamp bounds for the vactrol resistance (on-resistance floor, dark off-resistance).
    pub r_on_min: f32,
    pub r_off: f32,
    /// Vactrol envelope time constants: fast attack, slow decay.
    pub tau_attack_s: f32,
    pub tau_decay_s: f32,
}

impl Default for Components {
    fn default() -> Self {
        Self {
            c1: 1.0e-9,
            c2: 220.0e-12,
            c3: 4.7e-9,
            r_alpha_both: 5.0e6,
            r_alpha_vca: 5.0e3,
            // Fit to the VTL5C3 datasheet in Parker & D'Angelo.
            rf_law_a: 3.464,
            rf_law_b: 1136.212,
            r_on_min: 500.0,
            r_off: 10.0e6,
            // Between the datasheet extremes (~2.5 ms / ~35 ms) and the
            // VTL5C3/2 figures the paper quotes (~12 ms / ~250 ms).
            tau_attack_s: 0.005,
            tau_decay_s: 0.120,
        }
    }
}

/// Persisted per-instance state: the fingerprint seed plus the parameter snapshot.
///
/// The Phase 3 imperfection layer derives per-component tolerances deterministically
/// from `seed`, so storing the seed is enough to reproduce a saved module exactly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedState {
    pub seed: u64,
    pub params: Params,
    pub components: Components,
}
