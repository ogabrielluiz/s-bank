//! Vactrol model: an LED illuminating a CdS photoresistor.
//!
//! Resistance falls quickly when illumination rises (fast attack) and rises
//! slowly when it drops (slow decay), because conduction-band electrons decay
//! back slowly. Following Parker & D'Angelo, this is modelled heuristically as a
//! one-pole lowpass on the resistance whose time constant switches on the sign of
//! the input derivative, modulated further by the current state (faster when more
//! open). The instantaneous target resistance comes from the datasheet power law
//! `Rf = A / If^1.4 + B`.

use crate::params::Components;

/// Floor on LED current so the power law cannot divide by zero (amps).
pub(crate) const I_FLOOR_A: f32 = 1.0e-7;

#[derive(Debug, Clone)]
pub struct Vactrol {
    sample_rate: f32,
    /// Current photoresistor resistance (ohms). Starts dark/closed.
    rf: f32,
}

impl Vactrol {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            rf: Components::default().r_off,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
    }

    pub fn reset(&mut self, comp: &Components) {
        self.rf = comp.r_off;
    }

    /// Current resistance (ohms).
    #[inline]
    pub fn resistance(&self) -> f32 {
        self.rf
    }

    /// Advance one sample given the LED current (amps); returns resistance (ohms).
    #[inline]
    pub fn process(&mut self, if_current: f32, comp: &Components) -> f32 {
        let i_eff = if_current.max(I_FLOOR_A);
        let target =
            (comp.rf_law_a / i_eff.powf(1.4) + comp.rf_law_b).clamp(comp.r_on_min, comp.r_off);

        // Asymmetric: resistance dropping == gate opening == fast attack.
        let opening = target < self.rf;
        let mut tau = if opening {
            comp.tau_attack_s
        } else {
            comp.tau_decay_s
        };

        // State dependence: respond quicker when more open (small rf).
        // `openness` ~ 1 when fully open, ~ 0 when dark.
        let span = (comp.r_off / comp.r_on_min).ln();
        let openness = ((comp.r_off / self.rf).ln() / span).clamp(0.0, 1.0);
        tau *= 0.5 + 0.5 * (1.0 - openness);

        let alpha = (-1.0 / (tau * self.sample_rate)).exp();
        self.rf = target + (self.rf - target) * alpha;
        self.rf
    }
}
