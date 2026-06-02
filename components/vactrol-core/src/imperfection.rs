//! Analogue imperfection layer -- the headline realism feature.
//!
//! Four independently toggleable subsystems, all fed by explicit seedable RNG
//! streams so tests can pin seeds:
//!   1. Per-instance component tolerance ("fingerprinting"): seeded, bounded,
//!      fixed for the instance's life, and reproduced from the serialized seed.
//!   2. Per-block parameter drift (random-walk + pink/1f at minute amplitude).
//!   3. Sub-Hz thermal wander biasing the control offset.
//!   4. An aggregate (pink) output noise floor.
//!
//! The whole layer is gated by `config.enabled`; when off, `Lpg` skips it
//! entirely, so it is zero-cost and bit-identical to the deterministic core.
//! Component tolerance uses `ChaCha8Rng` (drawn once, at (re)build). The per-block
//! hot path uses a tiny xorshift generator to avoid pulling a heavy RNG onto the
//! audio thread; all streams are seeded deterministically from the instance seed.

use std::f32::consts::TAU;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};

use crate::params::{Components, Params};

/// Default fingerprint seed. Mirrors Instruō's behaviour: the first instance is
/// always the same; the host assigns fresh seeds to later instances.
pub const DEFAULT_SEED: u64 = 0x5641_4354_524F_4C00; // "VACTROL\0"

/// Parameter-update cadence (samples). Drift advances once per block, not per
/// sample, to stay cheap.
const DRIFT_BLOCK: u32 = 64;

/// Per-layer toggles and amplitudes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ImperfectionConfig {
    pub enabled: bool,
    pub tolerance: bool,
    pub drift: bool,
    pub thermal: bool,
    pub noise_floor: bool,
    /// Scales the parameter-drift excursions.
    pub drift_amount: f32,
    /// Output noise-floor amplitude (linear).
    pub noise_amp: f32,
}

impl Default for ImperfectionConfig {
    fn default() -> Self {
        // Disabled by default so the core stays deterministic until asked.
        Self {
            enabled: false,
            tolerance: true,
            drift: true,
            thermal: true,
            noise_floor: true,
            drift_amount: 1.0,
            noise_amp: 1.0e-4,
        }
    }
}

/// Tiny, fast, deterministic xorshift generator for the per-block hot path.
#[derive(Debug, Clone)]
struct XorShift {
    state: u64,
}

impl XorShift {
    fn new(seed: u64) -> Self {
        // Avoid the all-zero state.
        Self {
            state: seed ^ 0x9E37_79B9_7F4A_7C15 | 1,
        }
    }

    #[inline]
    fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    /// Uniform white noise in `[-1, 1)`.
    #[inline]
    fn next_f32(&mut self) -> f32 {
        (self.next_u32() as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// Pink (1/f) noise via Paul Kellet's economy filter (~ -3 dB/octave).
#[derive(Debug, Clone, Default)]
pub struct PinkNoise {
    b: [f32; 7],
}

impl PinkNoise {
    pub fn reset(&mut self) {
        self.b = [0.0; 7];
    }

    /// Filter one white sample into pink. Output is roughly unit-scaled.
    #[inline]
    pub fn next(&mut self, white: f32) -> f32 {
        let b = &mut self.b;
        b[0] = 0.99886 * b[0] + white * 0.0555179;
        b[1] = 0.99332 * b[1] + white * 0.0750759;
        b[2] = 0.96900 * b[2] + white * 0.153852;
        b[3] = 0.86650 * b[3] + white * 0.3104856;
        b[4] = 0.55000 * b[4] + white * 0.5329522;
        b[5] = -0.7616 * b[5] - white * 0.016898;
        let pink = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + white * 0.5362;
        b[6] = white * 0.115926;
        pink * 0.11
    }
}

/// The imperfection layer for one `Lpg` instance.
#[derive(Debug, Clone)]
pub struct Imperfection {
    pub seed: u64,
    pub config: ImperfectionConfig,

    // Hot-path RNG streams (seeded deterministically from `seed`).
    drift_rng: XorShift,
    noise_rng: XorShift,
    pink_drift: PinkNoise,
    pink_floor: PinkNoise,

    // Per-block drift state.
    counter: u32,
    drift_res: f32,
    drift_gain: f32,
    drift_cv: f32,
    thermal: f32,
    thermal_phase: f32,
}

impl Imperfection {
    /// Build from a fingerprint seed with default (disabled) config.
    pub fn from_seed(seed: u64) -> Self {
        Self::new(seed, ImperfectionConfig::default())
    }

    pub fn new(seed: u64, config: ImperfectionConfig) -> Self {
        Self {
            seed,
            config,
            drift_rng: XorShift::new(seed ^ 0x1111_1111_1111_1111),
            noise_rng: XorShift::new(seed ^ 0x2222_2222_2222_2222),
            pink_drift: PinkNoise::default(),
            pink_floor: PinkNoise::default(),
            counter: 0,
            drift_res: 0.0,
            drift_gain: 0.0,
            drift_cv: 0.0,
            thermal: 0.0,
            thermal_phase: 0.0,
        }
    }

    /// Reset transient drift/noise state (tolerance is a function of the seed and
    /// is unaffected). Re-seeds the hot-path streams so a reset instance is
    /// reproducible.
    pub fn reset(&mut self) {
        self.drift_rng = XorShift::new(self.seed ^ 0x1111_1111_1111_1111);
        self.noise_rng = XorShift::new(self.seed ^ 0x2222_2222_2222_2222);
        self.pink_drift.reset();
        self.pink_floor.reset();
        self.counter = 0;
        self.drift_res = 0.0;
        self.drift_gain = 0.0;
        self.drift_cv = 0.0;
        self.thermal = 0.0;
        self.thermal_phase = 0.0;
    }

    /// One bounded multiplicative deviation `1 ± pct` from a ChaCha stream.
    #[inline]
    fn dev(rng: &mut ChaCha8Rng, pct: f32) -> f32 {
        1.0 + (rng.gen::<f32>() * 2.0 - 1.0) * pct
    }

    /// Per-instance component tolerance ("fingerprint"). Deterministic from the
    /// seed; bounded deviations modelled on the analogue originals.
    pub fn tolerance_components(&self, base: &Components) -> Components {
        let mut c = *base;
        if !(self.config.enabled && self.config.tolerance) {
            return c;
        }
        let mut rng = ChaCha8Rng::seed_from_u64(self.seed);
        // Capacitors ~5%, resistors ~2%, vactrol law/timing 10-15%.
        c.c1 *= Self::dev(&mut rng, 0.05);
        c.c2 *= Self::dev(&mut rng, 0.05);
        c.c3 *= Self::dev(&mut rng, 0.05);
        c.r_alpha_both *= Self::dev(&mut rng, 0.02);
        c.r_alpha_vca *= Self::dev(&mut rng, 0.02);
        c.rf_law_a *= Self::dev(&mut rng, 0.10);
        c.rf_law_b *= Self::dev(&mut rng, 0.05);
        c.r_on_min *= Self::dev(&mut rng, 0.10);
        c.r_off *= Self::dev(&mut rng, 0.10);
        c.tau_attack_s *= Self::dev(&mut rng, 0.15);
        c.tau_decay_s *= Self::dev(&mut rng, 0.15);
        c
    }

    /// Advance per-block drift/thermal state. Cheap; most samples just increment.
    #[inline]
    pub fn update(&mut self, sample_rate: f32) {
        if !self.config.enabled {
            return;
        }
        self.counter += 1;
        if self.counter < DRIFT_BLOCK {
            return;
        }
        self.counter = 0;

        if self.config.drift {
            let amt = self.config.drift_amount;
            let p = self.pink_drift.next(self.drift_rng.next_f32());
            // Leaky random walk with pink injection, tightly bounded.
            self.drift_res = (self.drift_res * 0.99 + p * 0.0008 * amt).clamp(-0.03, 0.03);
            self.drift_gain = (self.drift_gain * 0.99 + self.drift_rng.next_f32() * 0.0004 * amt)
                .clamp(-0.01, 0.01);
            self.drift_cv = (self.drift_cv * 0.999 + p * 0.0015 * amt).clamp(-0.06, 0.06);
        }

        if self.config.thermal {
            // ~0.05 Hz wander.
            self.thermal_phase += TAU * 0.05 * (DRIFT_BLOCK as f32 / sample_rate);
            if self.thermal_phase > TAU {
                self.thermal_phase -= TAU;
            }
            self.thermal = self.thermal_phase.sin();
        }
    }

    /// Apply parameter drift/thermal to a copy of the runtime params.
    #[inline]
    pub fn apply_params(&self, p: &Params) -> Params {
        let mut q = *p;
        if !self.config.enabled {
            return q;
        }
        if self.config.drift {
            q.resonance = (q.resonance + self.drift_res).clamp(0.0, 1.0);
            q.cv_offset += self.drift_cv;
        }
        if self.config.thermal {
            q.cv_offset += self.thermal * 0.05;
        }
        q
    }

    /// Apply the output gain drift and add the noise floor.
    #[inline]
    pub fn apply_output(&mut self, y: f32) -> f32 {
        if !self.config.enabled {
            return y;
        }
        let mut out = if self.config.drift {
            y * (1.0 + self.drift_gain)
        } else {
            y
        };
        if self.config.noise_floor {
            let p = self.pink_floor.next(self.noise_rng.next_f32());
            out += p * self.config.noise_amp;
        }
        out
    }
}
