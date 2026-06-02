//! The shaped envelope generator — the heart of the gate's character.
//!
//! Design (independent / clean-room, achieving the documented behaviours):
//! - **Dual-mode decay**: two parallel one-pole decays, a fixed *fast* component
//!   (the sharp initial transient) plus a *slow* component whose time constant the
//!   DECAY control stretches from a few tens of ms (percussive click) to multiple
//!   seconds (long ring). At low DECAY the slow part collapses → pure click; at high
//!   DECAY it sustains → ring. The shape (sharp front + long tail) is emergent, not a
//!   single stretched curve.
//! - **Memory effect**: a HIT *adds* energy to both components without resetting, so
//!   rapid strikes accumulate and open the gate progressively wider.
//! - **Frequency-dependent decay**: the slow time constant is scaled by a
//!   pitch factor (higher input pitch ⇒ shorter ring).
//! - **CTRL is pre-EG** (summed into the pre-attack target, so it is attack-shaped),
//!   **OPEN is a post-EG floor** the control never falls below.
//! - **Attack** is a one-pole smoother whose time constant comes from MATERIAL.

/// HIT detection threshold (volts) — matches the reference's +0.25 V.
pub const HIT_THRESHOLD: f32 = 0.25;

/// Fast-component decay time constant (s): the initial transient.
const TAU_FAST: f32 = 0.012;
/// Slow-component decay bounds (s): DECAY morphs between these.
const TAU_SLOW_MIN: f32 = 0.030;
const TAU_SLOW_MAX: f32 = 3.5;
/// Energy injected into each component per HIT. Tuned so a single strike peaks well
/// below 1.0, leaving headroom for the memory effect to push toward full open.
const HIT_ENERGY: f32 = 0.35;

#[derive(Debug, Clone)]
pub struct Envelope {
    sr: f32,
    e_fast: f32,
    e_slow: f32,
    /// Attack-smoothed control output.
    env: f32,
    prev_hit_high: bool,
}

impl Envelope {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sr: sample_rate,
            e_fast: 0.0,
            e_slow: 0.0,
            env: 0.0,
            prev_hit_high: false,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sr = sample_rate;
    }

    pub fn reset(&mut self) {
        self.e_fast = 0.0;
        self.e_slow = 0.0;
        self.env = 0.0;
        self.prev_hit_high = false;
    }

    /// Map DECAY `0..1` (and a pitch factor) to the slow time constant (s).
    #[inline]
    fn tau_slow(decay01: f32, pitch_factor: f32) -> f32 {
        let d = decay01.clamp(0.0, 1.0);
        TAU_SLOW_MIN * (TAU_SLOW_MAX / TAU_SLOW_MIN).powf(d) * pitch_factor
    }

    /// Advance one sample. Returns the gate control in `0..=1`.
    ///
    /// * `decay01` – effective DECAY (slider + CV), already clamped by the caller.
    /// * `pitch_factor` – `<1` shortens (high pitch), `>1` lengthens (low pitch).
    /// * `attack_tau` – onset smoothing from MATERIAL (s).
    /// * `ctrl01` – pre-EG opening in `0..1` (CTRL, normalled/attenuverted upstream).
    /// * `open01` – post-EG floor in `0..1` (OPEN).
    /// * `hit_v` – HIT input (volts); a rising edge past the threshold fires.
    #[inline]
    pub fn process(
        &mut self,
        decay01: f32,
        pitch_factor: f32,
        attack_tau: f32,
        ctrl01: f32,
        open01: f32,
        hit_v: f32,
    ) -> f32 {
        // Rising-edge HIT detection with memory (no reset — energy accumulates).
        let hit_high = hit_v >= HIT_THRESHOLD;
        if hit_high && !self.prev_hit_high {
            self.e_fast += HIT_ENERGY;
            self.e_slow += HIT_ENERGY;
        }
        self.prev_hit_high = hit_high;

        // Parallel one-pole decays.
        let d_fast = (-1.0 / (TAU_FAST * self.sr)).exp();
        let tau_slow = Self::tau_slow(decay01, pitch_factor);
        let d_slow = (-1.0 / (tau_slow * self.sr)).exp();
        self.e_fast *= d_fast;
        self.e_slow *= d_slow;

        // Pre-EG target: the decaying sum plus the continuous CTRL opening.
        let target = self.e_fast + self.e_slow + ctrl01.max(0.0);

        // Attack smoothing on the way up; follow instantly on the way down (the decay
        // shape is already in the exponential sum).
        let a = 1.0 - (-1.0 / (attack_tau.max(1.0e-5) * self.sr)).exp();
        if target > self.env {
            self.env += (target - self.env) * a;
        } else {
            self.env = target;
        }

        // OPEN is a post-EG floor; clamp to the gate's control range.
        self.env.max(open01).clamp(0.0, 1.0)
    }
}
